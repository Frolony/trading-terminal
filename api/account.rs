// N2.7 — email accounts (optional sign-in, paper stays open signed-out),
// implemented 2026-10-24. PART 1: full client + server plumbing; the live
// magic-link leg is gated on the provider Carlos sets up (see below).
//
// One function, action-dispatched (same shape as api/sync.rs):
//   POST /api/account {action:"request-link", email}   -> {ok:true}
//   POST /api/account {action:"auth", providerToken}  -> {token, userId, email, expiresAt}
//   GET  /api/account?action=pull (Authorization: Bearer <token>)
//                                                     -> {email, namespaces}
//   POST /api/account {action:"push", namespaces} (Bearer)
//                                                     -> {namespaces: merged}
//
// AUTH (magic link, NO passwords anywhere — nothing to store or leak):
// the client asks THIS endpoint for a link; the server asks the provider
// (a free-tier Supabase-class service, AUTH-ONLY) to email it. The provider
// redirects back to the app with its session token in the URL hash; the
// client posts that token here; the server verifies it against the
// provider's /auth/v1/user endpoint and issues OUR session token. All app
// data stays in OUR Upstash store — the provider never sees it.
//
// PROVIDER SETUP (the one Carlos step, at the delivery leg): he creates
// the free provider project and sets three env vars on the Vercel project:
//   ACCOUNT_PROVIDER_URL  (e.g. https://<project>.supabase.co)
//   ACCOUNT_PROVIDER_KEY  (the project's publishable/anon key)
//   ACCOUNT_APP_URL       (optional; defaults to the production URL —
//                          this URL must also be on the provider's
//                          redirect allowlist)
// Until those vars exist, request-link/auth answer ACCOUNT_NOT_CONFIGURED
// and the client says so honestly — the banner never collects an email
// into nowhere, and no session is ever issued without provider verification.
// A stubbed/unsigned token is NEVER accepted: auth refuses when the
// provider is unconfigured instead of issuing a local session.
//
// SESSIONS: our token is "acct:<userId>.<expiry>.<hmac>" over the same
// SYNC_HMAC_SECRET the wallet sync uses (no new secret to provision; the
// "acct:" prefix keeps the two token families disjoint). Tokens live in
// memory on the client only, never in localStorage — a reload signs out,
// exactly like wallet sync. 24 h TTL.
//
// STORAGE: Upstash Redis via the Marketplace-injected REST env vars
// (KV_REST_API_URL / KV_REST_API_TOKEN). One key per account:
//   acct:v1:<userId> -> {"v":1,"email":...,
//                        "namespaces":{ns:{updatedAt,data}}}
// Namespaces: the five wallet-sync namespaces (settings, slippage,
// layout, paperlive, btConfigs) PLUS "paper" (cash, holdings, paper
// orders, open limit orders). Paper state is the one thing wallet sync
// deliberately never carried — an email account carries it only by the
// user's explicit opt-in (signing in). Signed-out paper stays
// device-local, byte-for-byte today's experience.
//
// MERGE: per-namespace last-writer-wins on updatedAt (ms epoch, client
// clock); ties go to the client (first-sign-in seeding). Unknown
// namespaces and malformed entries are dropped, never stored. Doc cap
// 512 KB (same as sync); body cap 600 KB.
//
// PRECEDENCE (client-enforced, §5 N2.7): while a wallet is connected and
// synced, the wallet's sync doc wins for the five shared namespaces —
// the client neither pulls nor pushes them for the account. The paper
// namespace always rides the account (wallet sync never carries it).
// On sign-out the client restores the exact device-local snapshot taken
// at sign-in, so account data never leaks into the signed-out session.
//
// Error shape follows the house pattern (common::api_error):
// {ok:false, error:<CODE>, message:<...>}, HTTP 200 envelope like the
// other api routes. request-link never reveals whether an address is
// registered: provider errors are reported generically as unavailable,
// never as "unknown email".

#[path = "../src/raydium_common.rs"]
mod common;

use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};
use vercel_runtime::{run, service_fn, Error, Request};

type HmacSha256 = Hmac<Sha256>;

const TOKEN_TTL_SECS: u64 = 86_400;
const MAX_DOC_BYTES: usize = 524_288;
const BODY_LIMIT: usize = 600_000;
const ACCT_NAMESPACES: [&str; 6] = [
    "settings",
    "slippage",
    "layout",
    "paperlive",
    "btConfigs",
    "paper",
];
const PRODUCTION_APP_URL: &str = "https://gecko-solana-terminal.vercel.app/";

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn valid_email(s: &str) -> bool {
    // Same shape the client enforces (acctValidEmail): one @, a dot in the
    // domain half, no whitespace, 254 chars max. This is a shape check,
    // not a deliverability proof — the provider owns that.
    if s.len() > 254 || s.len() < 3 {
        return false;
    }
    if s.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    let mut parts = s.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    if local.is_empty() {
        return false;
    }
    domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

fn valid_user_id(s: &str) -> bool {
    // Provider user ids (Supabase UUIDs) are hex + hyphens. Reject dots
    // and colons so a user id can never smuggle structure into the token
    // or the storage key.
    !s.is_empty()
        && s.len() <= 128
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn hmac_secret() -> Option<String> {
    match std::env::var("SYNC_HMAC_SECRET") {
        Ok(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

fn provider_config() -> Option<(String, String)> {
    let url = std::env::var("ACCOUNT_PROVIDER_URL")
        .ok()
        .filter(|s| !s.is_empty())?;
    let key = std::env::var("ACCOUNT_PROVIDER_KEY")
        .ok()
        .filter(|s| !s.is_empty())?;
    Some((url.trim_end_matches('/').to_string(), key))
}

fn app_url() -> String {
    match std::env::var("ACCOUNT_APP_URL") {
        Ok(s) if !s.is_empty() => s,
        _ => PRODUCTION_APP_URL.to_string(),
    }
}

fn acct_token_for(secret: &str, user_id: &str, expiry: u64) -> String {
    let payload = format!("acct:{}.{}", user_id, expiry);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    format!(
        "acct:{}.{}.{}",
        user_id,
        expiry,
        hex_encode(&mac.finalize().into_bytes())
    )
}

/// Returns (user_id, expiry) iff the token is well-formed, unexpired, and
/// its HMAC matches the server secret. The user id inside the token IS the
/// identity — pull/push never trust a client-supplied user id alongside it.
fn verify_acct_token(secret: &str, token: &str) -> Option<(String, u64)> {
    let rest = token.strip_prefix("acct:")?;
    let mut it = rest.splitn(3, '.');
    let user_id = it.next()?;
    let expiry_s = it.next()?;
    let _mac_hex = it.next()?;
    if it.next().is_some() {
        return None;
    }
    if !valid_user_id(user_id) {
        return None;
    }
    let expiry: u64 = expiry_s.parse().ok()?;
    if expiry <= now_secs() {
        return None;
    }
    let expected = acct_token_for(secret, user_id, expiry);
    if expected == token {
        Some((user_id.to_string(), expiry))
    } else {
        None
    }
}

/// Per-namespace last-writer-wins. `server` wins iff its updatedAt is
/// strictly newer; ties and client-newer go to the client (first-sign-in
/// seeding). Only known namespaces with a {updatedAt: u64, data: any}
/// shape survive, normalized to exactly {updatedAt, data}.
fn merge_namespaces(server: &Map<String, Value>, client: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    for ns in ACCT_NAMESPACES {
        let s_entry = server.get(ns);
        let c_entry = client.get(ns);
        let pick = match (s_entry, c_entry) {
            (Some(s), Some(c)) => {
                let s_ts = s.get("updatedAt").and_then(Value::as_u64);
                let c_ts = c.get("updatedAt").and_then(Value::as_u64);
                match (s_ts, c_ts) {
                    (Some(st), Some(ct)) => {
                        if st > ct && s.get("data").is_some() {
                            Some(s.clone())
                        } else if c.get("data").is_some() {
                            Some(c.clone())
                        } else {
                            None
                        }
                    }
                    (Some(_), None) => Some(s.clone()),
                    (None, Some(_)) => Some(c.clone()),
                    (None, None) => None,
                }
            }
            (Some(s), None) => {
                if s.get("updatedAt").and_then(Value::as_u64).is_some() && s.get("data").is_some()
                {
                    Some(s.clone())
                } else {
                    None
                }
            }
            (None, Some(c)) => {
                if c.get("updatedAt").and_then(Value::as_u64).is_some() && c.get("data").is_some()
                {
                    Some(c.clone())
                } else {
                    None
                }
            }
            (None, None) => None,
        };
        if let Some(entry) = pick {
            if let (Some(ts), Some(data)) = (
                entry.get("updatedAt").and_then(Value::as_u64),
                entry.get("data"),
            ) {
                out.insert(ns.to_string(), json!({"updatedAt": ts, "data": data}));
            }
        }
    }
    out
}

fn sanitize_namespaces(raw: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    let obj = match raw.as_object() {
        Some(o) => o,
        None => return out,
    };
    for ns in ACCT_NAMESPACES {
        if let Some(entry) = obj.get(ns) {
            if let (Some(ts), Some(data)) = (
                entry.get("updatedAt").and_then(Value::as_u64),
                entry.get("data"),
            ) {
                out.insert(ns.to_string(), json!({"updatedAt": ts, "data": data}));
            }
        }
    }
    out
}

// ---- Upstash REST (command-body form, same as api/sync.rs) ----

async fn kv_cmd(cmd: Value) -> Result<Value, String> {
    let base = std::env::var("KV_REST_API_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "account_unavailable".to_string())?;
    let token = std::env::var("KV_REST_API_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "account_unavailable".to_string())?;
    let client = common::client().map_err(|_| "account_unavailable".to_string())?;
    let resp = client
        .post(base.trim_end_matches('/'))
        .header("Authorization", format!("Bearer {}", token))
        .json(&cmd)
        .send()
        .await
        .map_err(|_| "account_unavailable".to_string())?;
    if !resp.status().is_success() {
        return Err("account_unavailable".to_string());
    }
    resp.json::<Value>()
        .await
        .map_err(|_| "account_unavailable".to_string())
}

async fn kv_get(key: &str) -> Result<Option<String>, String> {
    let v = kv_cmd(json!(["GET", key])).await?;
    match v.get("result") {
        Some(Value::String(s)) => Ok(Some(s.clone())),
        _ => Ok(None),
    }
}

async fn kv_set(key: &str, value: &str) -> Result<(), String> {
    kv_cmd(json!(["SET", key, value])).await?;
    Ok(())
}

// ---- request plumbing ----

fn bearer_token(req: &Request) -> Option<String> {
    let h = req.headers().get("authorization")?.to_str().ok()?;
    let token = h.strip_prefix("Bearer ")?;
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
                out.push((hex(bytes[i + 1]) << 4 | hex(bytes[i + 2])) as char);
                i += 3;
            }
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}

fn query_param(req: &Request, key: &str) -> Option<String> {
    let qs = req.uri().query()?;
    for kv in qs.split('&') {
        let mut it = kv.splitn(2, '=');
        if let (Some(k), Some(v)) = (it.next(), it.next()) {
            if k == key {
                return Some(url_decode(v));
            }
        }
    }
    None
}

async fn read_body(req: Request) -> Result<Value, Value> {
    use http_body_util::BodyExt;
    let collected = match req.into_body().collect().await {
        Ok(v) => v,
        Err(_) => return Err(common::api_error("invalid_json", "Could not read the request body.")),
    };
    let bytes = collected.to_bytes();
    if bytes.len() > BODY_LIMIT {
        return Err(common::api_error(
            "request_too_large",
            "Request body exceeds the account limit.",
        ));
    }
    if bytes.is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_slice(&bytes) {
        Ok(v) => Ok(v),
        Err(_) => Err(common::api_error("invalid_json", "Request body is not valid JSON.")),
    }
}

#[derive(Deserialize, Default)]
struct AcctBody {
    action: Option<String>,
    email: Option<String>,
    #[serde(rename = "providerToken")]
    provider_token: Option<String>,
    namespaces: Option<Value>,
}

async fn handler(req: Request) -> Result<Value, Error> {
    let method = req.method().to_string();
    let q_action = query_param(&req, "action");
    let bearer = bearer_token(&req);
    let body: Value = if method == "GET" {
        json!({})
    } else {
        match read_body(req).await {
            Ok(v) => v,
            Err(e) => return Ok(e),
        }
    };
    let parsed: AcctBody = serde_json::from_value(body.clone()).unwrap_or_default();
    let action = parsed.action.or(q_action).unwrap_or_default();
    let action = action.as_str();

    match action {
        "request-link" => {
            let email = parsed.email.unwrap_or_default();
            if !valid_email(&email) {
                return Ok(common::api_error(
                    "invalid_email",
                    "That email address does not look valid.",
                ));
            }
            let (provider_url, provider_key) = match provider_config() {
                Some(c) => c,
                None => {
                    return Ok(common::api_error(
                        "account_not_configured",
                        "Email sign-in is not set up yet.",
                    ))
                }
            };
            let client = match common::client() {
                Ok(c) => c,
                Err(_) => {
                    return Ok(common::api_error(
                        "account_unavailable",
                        "Email sign-in is unavailable right now.",
                    ))
                }
            };
            // Supabase-class magic-link contract: POST /auth/v1/magiclink
            // with the publishable key; the provider emails the link. A
            // failure here is reported generically — never as "unknown
            // email" — so the endpoint cannot be used to probe addresses.
            let resp = client
                .post(format!("{}/auth/v1/magiclink", provider_url))
                .header("apikey", provider_key)
                .header("Content-Type", "application/json")
                .json(&json!({"email": email, "redirect_to": app_url()}))
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => Ok(json!({"ok": true})),
                _ => Ok(common::api_error(
                    "account_unavailable",
                    "Email sign-in is unavailable right now.",
                )),
            }
        }
        "auth" => {
            let provider_token = parsed.provider_token.unwrap_or_default();
            if provider_token.is_empty() || provider_token.len() > 4096 {
                return Ok(common::api_error(
                    "invalid_token",
                    "The sign-in link did not return a usable session.",
                ));
            }
            let (provider_url, provider_key) = match provider_config() {
                Some(c) => c,
                None => {
                    return Ok(common::api_error(
                        "account_not_configured",
                        "Email sign-in is not set up yet.",
                    ))
                }
            };
            let secret = match hmac_secret() {
                Some(s) => s,
                None => {
                    return Ok(common::api_error(
                        "account_unavailable",
                        "Email sign-in is not configured on the server.",
                    ))
                }
            };
            let client = match common::client() {
                Ok(c) => c,
                Err(_) => {
                    return Ok(common::api_error(
                        "account_unavailable",
                        "Email sign-in is unavailable right now.",
                    ))
                }
            };
            // Verify the provider session server-side: GET /auth/v1/user
            // with the token. The token itself is never trusted — only
            // the provider's answer (user id + email) mints our session.
            let resp = client
                .get(format!("{}/auth/v1/user", provider_url))
                .header("apikey", provider_key)
                .header("Authorization", format!("Bearer {}", provider_token))
                .send()
                .await;
            let user: Value = match resp {
                Ok(r) if r.status().is_success() => match r.json::<Value>().await {
                    Ok(v) => v,
                    Err(_) => {
                        return Ok(common::api_error(
                            "invalid_token",
                            "That sign-in link is expired or invalid.",
                        ))
                    }
                },
                _ => {
                    return Ok(common::api_error(
                        "invalid_token",
                        "That sign-in link is expired or invalid.",
                    ))
                }
            };
            let user_id = user.get("id").and_then(Value::as_str).unwrap_or("");
            let email = user.get("email").and_then(Value::as_str).unwrap_or("");
            if !valid_user_id(user_id) || !valid_email(email) {
                return Ok(common::api_error(
                    "invalid_token",
                    "That sign-in link is expired or invalid.",
                ));
            }
            let expiry = now_secs() + TOKEN_TTL_SECS;
            let token = acct_token_for(&secret, user_id, expiry);
            // Best-effort: record the email on the account doc at sign-in
            // (preserving any existing namespaces). A storage failure here
            // must not fail the sign-in — push fills the email later.
            {
                let key = format!("acct:v1:{}", user_id);
                let existing_ns = match kv_get(&key).await {
                    Ok(Some(s)) => serde_json::from_str::<Value>(&s)
                        .ok()
                        .and_then(|d| d.get("namespaces").cloned())
                        .unwrap_or_else(|| json!({})),
                    _ => json!({}),
                };
                if let Ok(doc_str) =
                    serde_json::to_string(&json!({"v": 1, "email": email, "namespaces": existing_ns}))
                {
                    let _ = kv_set(&key, &doc_str).await;
                }
            }
            Ok(json!({"ok": true, "token": token, "userId": user_id, "email": email, "expiresAt": expiry}))
        }
        "pull" => {
            let secret = match hmac_secret() {
                Some(s) => s,
                None => {
                    return Ok(common::api_error(
                        "account_unavailable",
                        "Email sign-in is not configured on the server.",
                    ))
                }
            };
            let token = match bearer {
                Some(t) => t,
                None => {
                    return Ok(common::api_error(
                        "invalid_token",
                        "An account session is required — sign in first.",
                    ))
                }
            };
            let (user_id, _) = match verify_acct_token(&secret, &token) {
                Some(v) => v,
                None => {
                    return Ok(common::api_error(
                        "invalid_token",
                        "This account session is expired or invalid — sign in again.",
                    ))
                }
            };
            let stored = match kv_get(&format!("acct:v1:{}", user_id)).await {
                Ok(v) => v,
                Err(_) => {
                    return Ok(common::api_error(
                        "account_unavailable",
                        "Account storage is unavailable.",
                    ))
                }
            };
            let stored = match stored {
                Some(s) => s,
                None => return Ok(json!({"ok": true, "email": "", "namespaces": {}})),
            };
            let doc: Value = match serde_json::from_str(&stored) {
                Ok(v) => v,
                Err(_) => {
                    // Corrupt server doc: the client treats it as empty
                    // and re-seeds from local (sync pack §7 pattern).
                    // Never crash on it.
                    return Ok(json!({"ok": true, "email": "", "namespaces": {}, "corrupt": true}));
                }
            };
            let email = doc
                .get("email")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let namespaces = doc.get("namespaces").cloned().unwrap_or_else(|| json!({}));
            let namespaces = match namespaces.as_object() {
                Some(_) => namespaces,
                None => json!({}),
            };
            Ok(json!({"ok": true, "email": email, "namespaces": namespaces}))
        }
        "push" => {
            let secret = match hmac_secret() {
                Some(s) => s,
                None => {
                    return Ok(common::api_error(
                        "account_unavailable",
                        "Email sign-in is not configured on the server.",
                    ))
                }
            };
            let token = match bearer {
                Some(t) => t,
                None => {
                    return Ok(common::api_error(
                        "invalid_token",
                        "An account session is required — sign in first.",
                    ))
                }
            };
            let (user_id, _) = match verify_acct_token(&secret, &token) {
                Some(v) => v,
                None => {
                    return Ok(common::api_error(
                        "invalid_token",
                        "This account session is expired or invalid — sign in again.",
                    ))
                }
            };
            let client_ns = match parsed.namespaces {
                Some(Value::Object(_)) => parsed.namespaces.unwrap(),
                _ => {
                    return Ok(common::api_error(
                        "invalid_json",
                        "push requires a namespaces object.",
                    ))
                }
            };
            let client_ns = sanitize_namespaces(&client_ns);
            let key = format!("acct:v1:{}", user_id);
            let stored = match kv_get(&key).await {
                Ok(v) => v,
                Err(_) => {
                    return Ok(common::api_error(
                        "account_unavailable",
                        "Account storage is unavailable.",
                    ))
                }
            };
            let server_ns: Map<String, Value> = match &stored {
                Some(s) => match serde_json::from_str::<Value>(s) {
                    Ok(doc) => match doc.get("namespaces") {
                        Some(Value::Object(o)) => o.clone(),
                        _ => Map::new(),
                    },
                    Err(_) => Map::new(),
                },
                None => Map::new(),
            };
            let merged = merge_namespaces(&server_ns, &client_ns);
            let mut email = match &stored {
                Some(s) => match serde_json::from_str::<Value>(s) {
                    Ok(doc) => doc
                        .get("email")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    Err(_) => String::new(),
                },
                None => String::new(),
            };
            // A client that knows its email (from the auth response) may
            // fill an empty stored email; it never overwrites a stored one.
            if email.is_empty() {
                if let Some(incoming) = parsed.email {
                    if valid_email(&incoming) {
                        email = incoming;
                    }
                }
            }
            let doc = json!({"v": 1, "email": email, "namespaces": merged});
            let doc_str = match serde_json::to_string(&doc) {
                Ok(s) => s,
                Err(_) => {
                    return Ok(common::api_error(
                        "account_unavailable",
                        "Could not save the account document.",
                    ))
                }
            };
            if doc_str.len() > MAX_DOC_BYTES {
                return Ok(common::api_error(
                    "request_too_large",
                    "Account document exceeds the account limit.",
                ));
            }
            if kv_set(&key, &doc_str).await.is_err() {
                return Ok(common::api_error(
                    "account_unavailable",
                    "Account storage is unavailable.",
                ));
            }
            Ok(json!({"ok": true, "namespaces": merged}))
        }
        _ => Ok(common::api_error(
            "invalid_action",
            "Unknown account action.",
        )),
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acct_token_round_trips_and_rejects_tampering() {
        let secret = "test-secret";
        let token = acct_token_for(secret, "user-123", now_secs() + 3600);
        assert!(token.starts_with("acct:user-123."));
        let (uid, _) = verify_acct_token(secret, &token).expect("valid token verifies");
        assert_eq!(uid, "user-123");
        // Tampered MAC fails.
        let bad = format!("{}x", token);
        assert!(verify_acct_token(secret, &bad).is_none());
        // Wrong secret fails.
        assert!(verify_acct_token("other-secret", &token).is_none());
        // Wallet-style token (no acct: prefix) is not an account token.
        assert!(verify_acct_token(secret, "user-123.9999999999.abcdef").is_none());
    }

    #[test]
    fn acct_token_rejects_expired_and_bad_user_ids() {
        let secret = "test-secret";
        let expired = acct_token_for(secret, "user-123", now_secs() - 10);
        assert!(verify_acct_token(secret, &expired).is_none());
        assert!(!valid_user_id("user.id"));
        assert!(!valid_user_id("user:id"));
        assert!(valid_user_id("550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn acct_email_shape_matches_client_rule() {
        assert!(valid_email("a@b.co"));
        assert!(!valid_email("plainaddress"));
        assert!(!valid_email("a@b"));
        assert!(!valid_email("a b@c.co"));
        assert!(!valid_email("@b.co"));
    }

    #[test]
    fn acct_merge_prefers_newer_server_and_client_ties() {
        let server = json!({"paper": {"updatedAt": 10, "data": {"cash": 9000}}})
            .as_object()
            .unwrap()
            .clone();
        let client = json!({"paper": {"updatedAt": 20, "data": {"cash": 10000}}})
            .as_object()
            .unwrap()
            .clone();
        let merged = merge_namespaces(&server, &client);
        assert_eq!(merged["paper"]["data"]["cash"], 10000);
        let server2 = json!({"paper": {"updatedAt": 30, "data": {"cash": 9000}}})
            .as_object()
            .unwrap()
            .clone();
        let merged2 = merge_namespaces(&server2, &client);
        assert_eq!(merged2["paper"]["data"]["cash"], 9000);
        // Unknown namespaces never survive sanitize.
        let raw = json!({"paper": {"updatedAt": 5, "data": {}}, "evil": {"updatedAt": 9, "data": {}}});
        let clean = sanitize_namespaces(&raw);
        assert!(clean.get("paper").is_some());
        assert!(clean.get("evil").is_none());
    }
}
