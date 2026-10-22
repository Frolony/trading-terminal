use serde_json::{json, Value};
use vercel_runtime::{run, service_fn, Error, Request};

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}

async fn handler(_req: Request) -> Result<Value, Error> {
    Ok(json!({
        "service": "gecko-terminal-rust",
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "signing_keys_stored": false
    }))
}
