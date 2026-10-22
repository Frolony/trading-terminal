#[path = "../../src/raydium_common.rs"] mod common;
use serde_json::{json,Value};
use vercel_runtime::{run,service_fn,Error,Request};
#[tokio::main] async fn main()->Result<(),Error>{run(service_fn(handler)).await}
async fn handler(_req:Request)->Result<Value,Error>{Ok(json!({"ok":true,"tokens":common::TOKENS,"pairs":common::PAIRS}))}
