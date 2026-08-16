//! DSH 端口自动发现：无需用户配置环境变量，启动时自动定位本机 DSH。
//!
//! 优先级：环境变量 DSH_PET_URL → 默认 3080 → 枚举 127.0.0.1 的 TCP 监听端口。

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::stream::{self, StreamExt};
use serde_json::{json, Value};

/// 并发探测上限，避免一次打满所有端口。
const PROBE_CONCURRENCY: usize = 16;

static DISCOVER_SEQ: AtomicU64 = AtomicU64::new(0);

fn next_discover_id() -> String {
    let seq = DISCOVER_SEQ.fetch_add(1, Ordering::Relaxed);
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("discover-{ms}-{seq:x}")
}

/// 探测单个 base URL：POST {base}/api/session.list，校验 result.ok == true。
/// body 格式与 rpc.rs 的 rpc() 保持一致。
async fn probe_dsh(client: &reqwest::Client, base: &str) -> bool {
    let url = format!("{}/api/session.list", base.trim_end_matches('/'));
    let body = json!({
        "type": "client-request",
        "rpcId": next_discover_id(),
        "method": "session.list",
        "payload": {},
    });
    let res = match client.post(&url).json(&body).send().await {
        Ok(res) => res,
        Err(_) => return false,
    };
    if !res.status().is_success() {
        return false;
    }
    let Ok(value) = res.json::<Value>().await else {
        return false;
    };
    value
        .get("result")
        .and_then(|r| r.get("ok"))
        .and_then(|o| o.as_bool())
        .unwrap_or(false)
}

/// 枚举 127.0.0.1 上的 TCP LISTENING 端口（解析 Windows netstat 输出）。
fn listening_ports() -> Vec<u16> {
    let output = match std::process::Command::new("netstat")
        .args(["-ano"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut seen: HashSet<u16> = HashSet::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // 形如: TCP  127.0.0.1:59040  0.0.0.0:0  LISTENING  19592
        // 按空白分割后: [0]=TCP [1]=本地地址 [2]=远端地址 [3]=状态 [4]=PID
        if parts.len() < 4
            || !parts[0].eq_ignore_ascii_case("TCP")
            || !parts[3].eq_ignore_ascii_case("LISTENING")
        {
            continue;
        }
        if let Some(port_str) = parts[1].strip_prefix("127.0.0.1:") {
            if let Ok(port) = port_str.parse::<u16>() {
                if port != 0 {
                    seen.insert(port);
                }
            }
        }
    }
    let mut ports: Vec<u16> = seen.into_iter().collect();
    ports.sort_unstable();
    ports
}

/// 自动发现 DSH URL。按优先级：环境变量 → 默认 3080 → 本机端口枚举；找不到返回 None。
pub async fn discover_dsh_url(client: &reqwest::Client) -> Option<String> {
    // a. 环境变量 DSH_PET_URL（用户手动指定时最高优先级，需探测通过）
    if let Ok(env) = std::env::var("DSH_PET_URL") {
        let env = env.trim().trim_end_matches('/').to_string();
        if !env.is_empty() {
            if probe_dsh(client, &env).await {
                log::info!("[discover] 使用环境变量 DSH_PET_URL: {}", env);
                return Some(env);
            }
            log::warn!("[discover] 环境变量 DSH_PET_URL 指向的地址不可用，继续自动发现");
        }
    }

    // b. 默认端口
    let default = "http://127.0.0.1:3080";
    if probe_dsh(client, default).await {
        return Some(default.to_string());
    }

    // c. 枚举本机 127.0.0.1 监听端口，并发探测，命中即返回
    let ports = listening_ports();
    log::info!(
        "[discover] 枚举到 {} 个 127.0.0.1 监听端口，开始并发探测",
        ports.len()
    );
    let probes = ports.into_iter().map(|port| {
        let url = format!("http://127.0.0.1:{}", port);
        let client = client.clone();
        async move {
            if probe_dsh(&client, &url).await {
                Some(url)
            } else {
                None
            }
        }
    });
    let mut stream = stream::iter(probes).buffer_unordered(PROBE_CONCURRENCY);
    while let Some(found) = stream.next().await {
        if let Some(url) = found {
            return Some(url);
        }
    }

    None
}

