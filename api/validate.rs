use http_body_util::BodyExt;
use serde::Deserialize;
use serde_json::{json, Value};
use vercel_runtime::{run, service_fn, Error, Request};

#[derive(Deserialize)]
struct OrderCheck { amount_usd: f64, balance_usd: f64 }

#[tokio::main]
async fn main() -> Result<(), Error> { run(service_fn(handler)).await }

async fn handler(req: Request) -> Result<Value, Error> {
    let collected = match req.into_body().collect().await {
        Ok(value) => value,
        Err(_) => return Ok(json!({"ok": false, "error": "invalid_json"})),
    };
    let bytes = collected.to_bytes();
    if bytes.len() > 65536 { return Ok(json!({"ok": false, "error": "request_too_large"})); }
    let value: OrderCheck = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => return Ok(json!({"ok": false, "error": "invalid_json"})),
    };
    if !value.amount_usd.is_finite() || !value.balance_usd.is_finite()
        || value.amount_usd <= 0.0 || value.balance_usd < 0.0 {
        return Ok(json!({"ok": false, "error": "invalid_amount_or_balance"}));
    }
    if value.amount_usd > value.balance_usd {
        return Ok(json!({"ok": false, "error": "insufficient_balance"}));
    }
    Ok(json!({"ok": true, "validated_amount_usd": value.amount_usd}))
}
