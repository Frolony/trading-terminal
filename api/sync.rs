// N2.2 — server-side sync (wallet-as-identity), implemented 2026-10-24.
//
// One function, action-dispatched: Vercel Rust functions are one binary per
// file, so the prep pack's four routes (challenge/auth/pull/push) collapse
// into POST/GET /api/sync with an `action` field — the protocol itself is
// unchanged: challenge -> wallet signMessage -> auth token -> pull/push.
//
//   POST /api/sync {action:"challenge", pubkey}            -> {nonce, issuedAt, expiresAt}
//   POST /api/sync {action:"auth", pubkey, nonce, signature}
//                                                          -> {token, expiresAt}
//   GET  /api/sync?action=pull  (Authorization: Bearer <token>)
//                                                          -> {namespaces}
//   POST /api/sync {action:"push", namespaces} (Bearer)    -> {namespaces: merged}
//
// Auth: the client asks the wallet to signMessage exactly the
// plain-language message from challenge_message (N2.3) — site name +
// purpose, an explicit "cannot move funds" line, nonce/issued-at labeled
// as security codes. The client builder (syncChallengeMessage in
// src/index.html) must stay byte-identical to it.
// (signMessage for the login challenge ONLY — never signTransaction, never a
// chain transaction). The server verifies the ed25519 signature against the
// pubkey, then issues a 24 h HMAC token (SYNC_HMAC_SECRET). The token is the
// identity for pull/push; the client keeps it in memory only, never in
// localStorage — a reload requires a fresh challenge signature, exactly like
// the wallet itself requires a fresh connect (autoConnect stays off).
//
// Storage: Upstash Redis via the Marketplace-injected REST env vars
// (KV_REST_API_URL / KV_REST_API_TOKEN). One key per wallet:
//   sync:v1:<pubkey> -> {"v":1,"namespaces":{ns:{updatedAt,data}}}
// Challenges live 5 min under challenge:<pubkey> and are deleted on use.
// Server never interprets `data` — opaque JSON, validated as JSON only.
// Paper-mode state is never stored: the client only ever sends the five
// namespaces in SYNC_NAMESPACES (settings/slippage/layout/paperlive/btConfigs);
// paper cash/holdings/orders/history never leave the device.
//
// Merge: per-namespace last-writer-wins on `updatedAt` (ms epoch, client
// clock); ties go to the client (first-sync seeding). Unknown namespaces and
// malformed entries are dropped, never stored. Doc cap 512 KB (pack §7);
// the body cap is 600 KB to leave room for the envelope.
//
// Error shape follows the house pattern (common::api_error): {ok:false,
// error:<CODE>, message:<...>}, HTTP 200 envelope like the other api routes.

#[path = "../src/raydium_common.rs"]
mod common;

use base64::Engine;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};
use vercel_runtime::{run, service_fn, Error, Request};

type HmacSha256 = Hmac<Sha256>;

const CHALLENGE_TTL_SECS: u64 = 300;
const TOKEN_TTL_SECS: u64 = 86_400;
const MAX_DOC_BYTES: usize = 524_288;
const BODY_LIMIT: usize = 600_000;
const SYNC_NAMESPACES: [&str; 5] = ["settings", "slippage", "layout", "paperlive", "btConfigs"];

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// The exact string the wallet signs for the login challenge. The client
/// builds the identical string (syncChallengeMessage); any drift here
/// breaks auth by design. Plain language (N2.3): purpose first, an explicit
/// cannot-move-funds line, nonce/issued-at labeled as security codes.
fn challenge_message(nonce: &str, issued_at: u64) -> String {
    format!(
        "Sign in to Turnpool sync\nThis signs you in so your settings follow this wallet across devices.\nThis signature cannot move funds, approve transactions, or spend anything.\nSecurity code: {}\nIssued at: {}",
        nonce, issued_at
    )
}

fn random_nonce() -> String {
    // Read EXACTLY 24 bytes: std::fs::read() loops to EOF, and /dev/urandom
    // never EOFs — the naive form hangs the function forever (caught live
    // 2026-10-24: the production challenge endpoint timed out on every call
    // while the validation-only paths answered instantly). read_exact
    // returns after 24 bytes. Fallback is time-derived and still unique per
    // (pubkey, second) in practice; challenges also expire in 5 minutes and
    // are single-use, so a weak fallback cannot be replayed across windows.
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let mut buf = [0u8; 24];
        if f.read_exact(&mut buf).is_ok() {
            return hex_encode(&buf);
        }
    }
    let t = now_ms();
    let pid = std::process::id() as u64;
    format!("{:016x}{:016x}{:016x}", t, t ^ (pid << 32), t.wrapping_mul(0x9e37_79b9_7f4a_7c15))
}

fn pubkey_bytes(s: &str) -> Option<[u8; 32]> {
    let v = bs58::decode(s).into_vec().ok()?;
    if v.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    Some(out)
}

fn token_for(secret: &str, pubkey: &str, expiry: u64) -> String {
    let payload = format!("{}.{}", pubkey, expiry);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    format!("{}.{}.{}", pubkey, expiry, hex_encode(&mac.finalize().into_bytes()))
}

/// Returns (pubkey, expiry) iff the token is well-formed, unexpired, and its
/// HMAC matches the server secret. The pubkey inside the token IS the
/// identity — pull/push never trust a client-supplied pubkey alongside it.
fn verify_token(secret: &str, token: &str) -> Option<(String, u64)> {
    let mut it = token.splitn(3, '.');
    let pubkey = it.next()?;
    let expiry_s = it.next()?;
    let mac_hex = it.next()?;
    if it.next().is_some() {
        return None;
    }
    let expiry: u64 = expiry_s.parse().ok()?;
    if expiry <= now_secs() || !common::valid_pubkey(pubkey) {
        return None;
    }
    let expected = token_for(secret, pubkey, expiry);
    if expected == token {
        // token_for is deterministic, so equality here is exactly the HMAC
        // check (the third segment is the MAC over pubkey.expiry).
        let _ = mac_hex;
        Some((pubkey.to_string(), expiry))
    } else {
        None
    }
}

fn hmac_secret() -> Option<String> {
    match std::env::var("SYNC_HMAC_SECRET") {
        Ok(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

/// Per-namespace last-writer-wins. `server` wins iff its updatedAt is strictly
/// newer; ties and client-newer go to the client (first-sync seeding). Only
/// known namespaces with a {updatedAt: u64, data: any} shape survive.
fn merge_namespaces(server: &Map<String, Value>, client: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    for ns in SYNC_NAMESPACES {
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
                    (Some(_), None) => s.clone().into(),
                    (None, Some(_)) => c.clone().into(),
                    (None, None) => None,
                }
            }
            (Some(s), None) => {
                if s.get("updatedAt").and_then(Value::as_u64).is_some() && s.get("data").is_some() {
                    Some(s.clone())
                } else {
                    None
                }
            }
            (None, Some(c)) => {
                if c.get("updatedAt").and_then(Value::as_u64).is_some() && c.get("data").is_some() {
                    Some(c.clone())
                } else {
                    None
                }
            }
            (None, None) => None,
        };
        // Normalize: keep only {updatedAt, data} so a client cannot smuggle
        // extra fields into the stored doc through a namespace entry.
        if let Some(entry) = pick {
            if let (Some(ts), Some(data)) = (
                entry.get("updatedAt").and_then(Value::as_u64),
                entry.get("data"),
            ) {
                out.insert(
                    ns.to_string(),
                    json!({"updatedAt": ts, "data": data}),
                );
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
    for ns in SYNC_NAMESPACES {
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

// ---- Upstash REST (command-body form: POST {base} with ["CMD", args...]) ----

async fn kv_cmd(cmd: Value) -> Result<Value, String> {
    let base = std::env::var("KV_REST_API_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "sync_unavailable".to_string())?;
    let token = std::env::var("KV_REST_API_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "sync_unavailable".to_string())?;
    let client = common::client().map_err(|_| "sync_unavailable".to_string())?;
    let resp = client
        .post(base.trim_end_matches('/'))
        .header("Authorization", format!("Bearer {}", token))
        .json(&cmd)
        .send()
        .await
        .map_err(|_| "sync_unavailable".to_string())?;
    if !resp.status().is_success() {
        return Err("sync_unavailable".to_string());
    }
    resp.json::<Value>()
        .await
        .map_err(|_| "sync_unavailable".to_string())
}

async fn kv_get(key: &str) -> Result<Option<String>, String> {
    let v = kv_cmd(json!(["GET", key])).await?;
    match v.get("result") {
        Some(Value::String(s)) => Ok(Some(s.clone())),
        _ => Ok(None),
    }
}

async fn kv_set_ex(key: &str, value: &str, secs: u64) -> Result<(), String> {
    kv_cmd(json!(["SET", key, value, "EX", secs])).await?;
    Ok(())
}

async fn kv_set(key: &str, value: &str) -> Result<(), String> {
    kv_cmd(json!(["SET", key, value])).await?;
    Ok(())
}

async fn kv_del(key: &str) -> Result<(), String> {
    kv_cmd(json!(["DEL", key])).await?;
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
            "Request body exceeds the sync limit.",
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
struct SyncBody {
    action: Option<String>,
    pubkey: Option<String>,
    nonce: Option<String>,
    signature: Option<String>,
    namespaces: Option<Value>,
}

async fn handler(req: Request) -> Result<Value, Error> {
    // Headers, method, and query are read BEFORE the body is consumed —
    // pull/push authenticate from the Bearer token in the headers.
    let method = req.method().to_string();
    let q_action = query_param(&req, "action");
    let q_pubkey = query_param(&req, "pubkey");
    let bearer = bearer_token(&req);
    let body: Value = if method == "GET" {
        json!({})
    } else {
        match read_body(req).await {
            Ok(v) => v,
            Err(e) => return Ok(e),
        }
    };
    let parsed: SyncBody = serde_json::from_value(body.clone()).unwrap_or_default();
    let action = parsed
        .action
        .or(q_action)
        .unwrap_or_default();
    let action = action.as_str();

    match action {
        "challenge" => {
            let pubkey = parsed.pubkey.or(q_pubkey).unwrap_or_default();
            if !common::valid_pubkey(&pubkey) {
                return Ok(common::api_error(
                    "invalid_pubkey",
                    "pubkey must be a valid base58 Solana address.",
                ));
            }
            let nonce = random_nonce();
            let issued_at = now_secs();
            let doc = json!({"nonce": nonce, "issuedAt": issued_at});
            let doc_str = match serde_json::to_string(&doc) {
                Ok(s) => s,
                Err(_) => return Ok(common::api_error("sync_unavailable", "Could not issue a challenge.")),
            };
            if kv_set_ex(&format!("challenge:{}", pubkey), &doc_str, CHALLENGE_TTL_SECS)
                .await
                .is_err()
            {
                return Ok(common::api_error("sync_unavailable", "Sync storage is unavailable."));
            }
            Ok(json!({"ok": true, "nonce": nonce, "issuedAt": issued_at, "expiresAt": issued_at + CHALLENGE_TTL_SECS}))
        }
        "auth" => {
            let pubkey = parsed.pubkey.unwrap_or_default();
            let nonce = parsed.nonce.unwrap_or_default();
            let sig_b64 = parsed.signature.unwrap_or_default();
            if !common::valid_pubkey(&pubkey) {
                return Ok(common::api_error(
                    "invalid_pubkey",
                    "pubkey must be a valid base58 Solana address.",
                ));
            }
            if nonce.is_empty() || nonce.len() > 128 || sig_b64.is_empty() || sig_b64.len() > 128 {
                return Ok(common::api_error("invalid_signature", "Challenge or signature is missing."));
            }
            let secret = match hmac_secret() {
                Some(s) => s,
                None => {
                    return Ok(common::api_error(
                        "sync_unavailable",
                        "Sync is not configured on the server.",
                    ))
                }
            };
            let stored = match kv_get(&format!("challenge:{}", pubkey)).await {
                Ok(v) => v,
                Err(_) => {
                    return Ok(common::api_error("sync_unavailable", "Sync storage is unavailable."))
                }
            };
            let stored = match stored {
                Some(s) => s,
                None => {
                    return Ok(common::api_error(
                        "challenge_expired",
                        "No live challenge for this wallet — request a new one.",
                    ))
                }
            };
            let stored_json: Value = match serde_json::from_str(&stored) {
                Ok(v) => v,
                Err(_) => {
                    return Ok(common::api_error(
                        "challenge_expired",
                        "No live challenge for this wallet — request a new one.",
                    ))
                }
            };
            let stored_nonce = stored_json.get("nonce").and_then(Value::as_str).unwrap_or("");
            let issued_at = stored_json.get("issuedAt").and_then(Value::as_u64).unwrap_or(0);
            if stored_nonce != nonce || issued_at == 0 || issued_at + CHALLENGE_TTL_SECS < now_secs() {
                return Ok(common::api_error(
                    "challenge_expired",
                    "This challenge expired — request a new one.",
                ));
            }
            // Single-use: delete regardless of whether verification succeeds.
            let _ = kv_del(&format!("challenge:{}", pubkey)).await;
            let sig_bytes = base64::engine::general_purpose::STANDARD
                .decode(sig_b64.as_bytes())
                .or_else(|_| {
                    base64::engine::general_purpose::URL_SAFE.decode(sig_b64.as_bytes())
                });
            let sig_bytes = match sig_bytes {
                Ok(b) if b.len() == 64 => b,
                _ => {
                    return Ok(common::api_error(
                        "invalid_signature",
                        "Signature must be 64 bytes, base64-encoded.",
                    ))
                }
            };
            let pk = match pubkey_bytes(&pubkey) {
                Some(b) => b,
                None => {
                    return Ok(common::api_error(
                        "invalid_pubkey",
                        "pubkey must be a valid base58 Solana address.",
                    ))
                }
            };
            let ok_sig = (|| {
                use ed25519_dalek::{Signature, Verifier, VerifyingKey};
                let mut sig_arr = [0u8; 64];
                sig_arr.copy_from_slice(&sig_bytes);
                let vk = VerifyingKey::from_bytes(&pk).ok()?;
                let sig = Signature::from_bytes(&sig_arr);
                vk.verify(challenge_message(&nonce, issued_at).as_bytes(), &sig).ok()
            })();
            if ok_sig.is_none() {
                return Ok(common::api_error(
                    "invalid_signature",
                    "Signature does not match this wallet and challenge.",
                ));
            }
            let expiry = now_secs() + TOKEN_TTL_SECS;
            let token = token_for(&secret, &pubkey, expiry);
            Ok(json!({"ok": true, "token": token, "expiresAt": expiry}))
        }
        "pull" => {
            let secret = match hmac_secret() {
                Some(s) => s,
                None => {
                    return Ok(common::api_error(
                        "sync_unavailable",
                        "Sync is not configured on the server.",
                    ))
                }
            };
            let token = match bearer {
                Some(t) => t,
                None => {
                    return Ok(common::api_error(
                        "invalid_token",
                        "A sync token is required — sign the login challenge first.",
                    ))
                }
            };
            let (pubkey, _) = match verify_token(&secret, &token) {
                Some(v) => v,
                None => {
                    return Ok(common::api_error(
                        "invalid_token",
                        "This sync token is expired or invalid — reconnect to sign in again.",
                    ))
                }
            };
            let stored = match kv_get(&format!("sync:v1:{}", pubkey)).await {
                Ok(v) => v,
                Err(_) => {
                    return Ok(common::api_error("sync_unavailable", "Sync storage is unavailable."))
                }
            };
            let stored = match stored {
                Some(s) => s,
                None => return Ok(json!({"ok": true, "namespaces": {}})),
            };
            let doc: Value = match serde_json::from_str(&stored) {
                Ok(v) => v,
                Err(_) => {
                    // Corrupt server doc: the client treats it as empty and
                    // re-seeds from local (pack §7). Never crash on it.
                    return Ok(json!({"ok": true, "namespaces": {}, "corrupt": true}));
                }
            };
            let namespaces = doc
                .get("namespaces")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let namespaces = match namespaces.as_object() {
                Some(_) => namespaces,
                None => json!({}),
            };
            Ok(json!({"ok": true, "namespaces": namespaces}))
        }
        "push" => {
            let secret = match hmac_secret() {
                Some(s) => s,
                None => {
                    return Ok(common::api_error(
                        "sync_unavailable",
                        "Sync is not configured on the server.",
                    ))
                }
            };
            let token = match bearer {
                Some(t) => t,
                None => {
                    return Ok(common::api_error(
                        "invalid_token",
                        "A sync token is required — sign the login challenge first.",
                    ))
                }
            };
            let (pubkey, _) = match verify_token(&secret, &token) {
                Some(v) => v,
                None => {
                    return Ok(common::api_error(
                        "invalid_token",
                        "This sync token is expired or invalid — reconnect to sign in again.",
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
            let key = format!("sync:v1:{}", pubkey);
            let stored = match kv_get(&key).await {
                Ok(v) => v,
                Err(_) => {
                    return Ok(common::api_error("sync_unavailable", "Sync storage is unavailable."))
                }
            };
            let server_ns: Map<String, Value> = match stored {
                Some(s) => match serde_json::from_str::<Value>(&s) {
                    Ok(doc) => match doc.get("namespaces") {
                        Some(Value::Object(o)) => sanitize_namespaces(&Value::Object(o.clone())),
                        _ => Map::new(),
                    },
                    Err(_) => Map::new(),
                },
                None => Map::new(),
            };
            let merged = merge_namespaces(&server_ns, &client_ns);
            let doc = json!({"v": 1, "namespaces": merged});
            let doc_str = match serde_json::to_string(&doc) {
                Ok(s) => s,
                Err(_) => return Ok(common::api_error("sync_unavailable", "Could not store the sync doc.")),
            };
            if doc_str.len() > MAX_DOC_BYTES {
                return Ok(common::api_error(
                    "doc_too_large",
                    "The sync document exceeds 512 KB — trim saved configurations first.",
                ));
            }
            if kv_set(&key, &doc_str).await.is_err() {
                return Ok(common::api_error("sync_unavailable", "Sync storage is unavailable."));
            }
            Ok(json!({"ok": true, "namespaces": merged}))
        }
        _ => Ok(common::api_error(
            "invalid_action",
            "action must be challenge, auth, pull, or push.",
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
    fn challenge_message_is_stable() {
        assert_eq!(
            challenge_message("abc123", 1700000000),
            "Sign in to Turnpool sync\nThis signs you in so your settings follow this wallet across devices.\nThis signature cannot move funds, approve transactions, or spend anything.\nSecurity code: abc123\nIssued at: 1700000000"
        );
    }

    #[test]
    fn token_round_trips_and_rejects_tampering() {
        let secret = "test-secret";
        let pk = "So11111111111111111111111111111111111111112";
        let expiry = now_secs() + 3600;
        let token = token_for(secret, pk, expiry);
        let (got_pk, got_exp) = verify_token(secret, &token).expect("token verifies");
        assert_eq!(got_pk, pk);
        assert_eq!(got_exp, expiry);
        // Tampered MAC fails.
        let bad = format!("{}x", token);
        assert!(verify_token(secret, &bad).is_none());
        // Wrong secret fails.
        assert!(verify_token("other-secret", &token).is_none());
        // Expired fails.
        let old = token_for(secret, pk, now_secs().saturating_sub(10));
        assert!(verify_token(secret, &old).is_none());
    }

    #[test]
    fn merge_prefers_newer_and_tie_goes_to_client() {
        let server = serde_json::from_value(json!({
            "settings": {"updatedAt": 100, "data": {"theme": "dark"}},
            "layout": {"updatedAt": 50, "data": {"v": 1}},
            "bogus": {"updatedAt": 999, "data": {}}
        }))
        .unwrap();
        let client = serde_json::from_value(json!({
            "settings": {"updatedAt": 100, "data": {"theme": "light"}},
            "slippage": {"updatedAt": 70, "data": {"bps": 50}}
        }))
        .unwrap();
        let merged = merge_namespaces(&server, &client);
        // Tie -> client (light).
        assert_eq!(merged["settings"]["data"]["theme"], "light");
        // Server-only survives.
        assert_eq!(merged["layout"]["data"]["v"], 1);
        // Client-only survives.
        assert_eq!(merged["slippage"]["data"]["bps"], 50);
        // Unknown namespaces never survive.
        assert!(merged.get("bogus").is_none());
    }

    #[test]
    fn merge_server_newer_wins() {
        let server = serde_json::from_value(json!({
            "settings": {"updatedAt": 200, "data": {"theme": "dark"}}
        }))
        .unwrap();
        let client = serde_json::from_value(json!({
            "settings": {"updatedAt": 100, "data": {"theme": "light"}}
        }))
        .unwrap();
        let merged = merge_namespaces(&server, &client);
        assert_eq!(merged["settings"]["data"]["theme"], "dark");
    }

    #[test]
    fn sanitize_drops_malformed_entries() {
        let raw = json!({
            "settings": {"updatedAt": "not-a-number", "data": {}},
            "layout": {"updatedAt": 5},
            "slippage": {"updatedAt": 7, "data": {"bps": 50}, "extra": "dropped"}
        });
        let clean = sanitize_namespaces(&raw);
        assert!(clean.get("settings").is_none());
        assert!(clean.get("layout").is_none());
        assert_eq!(clean["slippage"]["data"]["bps"], 50);
        assert!(clean["slippage"].get("extra").is_none());
    }

    #[test]
    fn pubkey_bytes_validates_length() {
        assert!(pubkey_bytes("So11111111111111111111111111111111111111112").is_some());
        assert!(pubkey_bytes("not-a-pubkey").is_none());
        assert!(pubkey_bytes("So1111111111111111111111111111111111111111").is_none());
    }

    #[test]
    fn random_nonce_returns_promptly_and_is_hex() {
        // Regression test for the live hang: the naive fs::read(/dev/urandom)
        // form never returns (urandom has no EOF). This must complete.
        let a = random_nonce();
        let b = random_nonce();
        assert_eq!(a.len(), 48, "24 bytes hex-encoded");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b, "two nonces differ");
    }
}
