//! DSH JSON-RPC 客户端：直译 main.js 的 rpc(method, payload)。

use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

/// DSH 默认地址（与 main.js 的 DSH_PET_URL 默认值一致）。
pub const DEFAULT_DSH_URL: &str = "http://127.0.0.1:3080";

/// 自动发现结果缓存：discover.rs 找到 DSH 后写入，rpc/sse 统一从这里取。
static DISCOVERED_URL: RwLock<Option<String>> = RwLock::new(None);

/// 缓存自动发现的 DSH 地址（去除末尾斜杠）。
pub fn set_dsh_url(url: String) {
    let url = url.trim().trim_end_matches('/').to_string();
    if let Ok(mut guard) = DISCOVERED_URL.write() {
        *guard = Some(url);
    }
}

/// 取当前 DSH 地址：优先自动发现的缓存 URL（discover 会把校验通过的环境变量写入缓存，
/// 从而保持 DSH_PET_URL 的最高优先级）；未发现时回退环境变量原始值，再回退默认 3080。
pub fn dsh_url() -> String {
    if let Ok(guard) = DISCOVERED_URL.read() {
        if let Some(url) = guard.as_deref() {
            if !url.is_empty() {
                return url.to_string();
            }
        }
    }
    if let Ok(url) = std::env::var("DSH_PET_URL") {
        let url = url.trim().trim_end_matches('/').to_string();
        if !url.is_empty() {
            return url;
        }
    }
    DEFAULT_DSH_URL.to_string()
}

static RPC_SEQ: AtomicU64 = AtomicU64::new(0);

/// 生成形如 pet-{ms}-{seq:x} 的 rpcId（对齐 main.js 的 pet-{Date.now()}-{random}）。
fn next_rpc_id() -> String {
    let seq = RPC_SEQ.fetch_add(1, Ordering::Relaxed);
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("pet-{ms}-{seq:x}")
}

/// POST {DSH_URL}/api/{method}，body 为 {type:"client-request", rpcId, method, payload}；
/// 校验 result.ok 后返回 result.value。
pub async fn rpc(client: &reqwest::Client, method: &str, payload: Value) -> Result<Value, String> {
    let url = format!("{}/api/{}", dsh_url(), method);
    let body = json!({
        "type": "client-request",
        "rpcId": next_rpc_id(),
        "method": method,
        "payload": payload,
    });

    let res = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    if !res.status().is_success() {
        return Err(format!("HTTP {}", res.status()));
    }

    let body: Value = res.json().await.map_err(|e| format!("invalid JSON: {e}"))?;

    let result = body.get("result");
    let ok = result
        .and_then(|r| r.get("ok"))
        .and_then(|o| o.as_bool())
        .unwrap_or(false);
    if !ok {
        let msg = result
            .and_then(|r| r.get("error"))
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("rpc error");
        return Err(msg.to_string());
    }

    Ok(result
        .and_then(|r| r.get("value"))
        .cloned()
        .unwrap_or(Value::Null))
}
