//! SSE 客户端：直译 main.js 的 connectSSE(path, onFrame, onOpen)。
//!
//! 用 reqwest 流式响应（bytes_stream）按空行切块，解析 data: 行为 JSON 帧逐帧回调；
//! 断线 3s 自动重连，句柄 drop / cancel() 等价于 AbortController 取消。

use futures_util::StreamExt;
use serde_json::Value;
use tokio::sync::watch;
use tokio::time::{sleep, Duration};

use crate::rpc::dsh_url;

/// 断线重连间隔（main.js 固定 3s）。
const RECONNECT_MS: u64 = 3000;
/// SSE 接收缓冲上限，超限丢弃并重置，防止内存无限增长。
const MAX_BUF_BYTES: usize = 1024 * 1024; // 1 MiB

/// SSE 连接的取消句柄：持有它连接保持存活；drop 或 cancel() 会停止连接。
pub struct SseHandle {
    shutdown: watch::Sender<bool>,
}

impl SseHandle {
    /// 主动停止连接（等价于 main.js 的 AbortController.abort()）。
    #[allow(dead_code)]
    pub fn cancel(&self) {
        let _ = self.shutdown.send(true);
    }
}

impl Drop for SseHandle {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

/// 连接 {DSH_URL}{path} 的 SSE 流，逐帧回调 on_frame；连接建立时回调 on_open。
/// 断线后 3s 自动重连。返回取消句柄。
pub fn connect_sse<F, O>(
    client: reqwest::Client,
    path: String,
    on_frame: F,
    on_open: O,
) -> SseHandle
where
    F: Fn(Value) + Send + Sync + 'static,
    O: Fn() + Send + Sync + 'static,
{
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(run_loop(client, path, on_frame, on_open, shutdown_rx));
    SseHandle {
        shutdown: shutdown_tx,
    }
}

async fn run_loop<F, O>(
    client: reqwest::Client,
    path: String,
    on_frame: F,
    on_open: O,
    mut shutdown_rx: watch::Receiver<bool>,
) where
    F: Fn(Value) + Send + Sync + 'static,
    O: Fn() + Send + Sync + 'static,
{
    loop {
        if *shutdown_rx.borrow() {
            break;
        }
        let err = run_once(&client, &path, &on_frame, &on_open, &mut shutdown_rx)
            .await
            .err();
        if *shutdown_rx.borrow() {
            break;
        }
        log::warn!(
            "[sse] {} 断开（{}），3s 后重连",
            path,
            err.unwrap_or_else(|| "stream ended".to_string())
        );
        tokio::select! {
            _ = shutdown_rx.changed() => break,
            _ = sleep(Duration::from_millis(RECONNECT_MS)) => {}
        }
    }
}

async fn run_once<F, O>(
    client: &reqwest::Client,
    path: &str,
    on_frame: &F,
    on_open: &O,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> Result<(), String>
where
    F: Fn(Value) + Send + Sync + 'static,
    O: Fn() + Send + Sync + 'static,
{
    let url = format!("{}{}", dsh_url(), path);
    let res = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("HTTP {}", res.status()));
    }

    on_open();

    let mut stream = Box::pin(res.bytes_stream());
    let mut buf: Vec<u8> = Vec::new();

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => return Err("cancelled".to_string()),
            chunk = stream.next() => match chunk {
                None => break,
                Some(Err(e)) => return Err(e.to_string()),
                Some(Ok(bytes)) => {
                    buf.extend_from_slice(&bytes);
                    if buf.len() > MAX_BUF_BYTES {
                        // 缓冲超限：丢弃并重置，防止内存无限增长
                        log::warn!("[sse] {} 缓冲超过 {} 字节，丢弃并重置", path, MAX_BUF_BYTES);
                        buf.clear();
                        continue;
                    }
                    drain_frames(&mut buf, on_frame);
                }
            },
        }
    }

    Err("stream ended".to_string())
}

/// 累积原始字节，按空行（\n\n 或 \r\n\r\n）切出完整块后整体做 UTF-8 解码，
/// 再解析每块中 data: 行的 JSON 帧并逐帧回调。
fn drain_frames<F>(buf: &mut Vec<u8>, on_frame: &F)
where
    F: Fn(Value) + Send + Sync + 'static,
{
    loop {
        let Some((block_end, sep_len)) = find_separator(buf) else {
            break;
        };
        let block_bytes: Vec<u8> = buf.drain(..block_end).collect();
        buf.drain(..sep_len); // 移除分隔块的空行
        let block = match String::from_utf8(block_bytes) {
            Ok(s) => s,
            Err(_) => {
                log::warn!("[sse] 跳过无法按 UTF-8 解码的帧块");
                continue;
            }
        };
        for line in block.split('\n') {
            let line = line.trim_end_matches('\r');
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(frame) = serde_json::from_str::<Value>(data) {
                    let payload = frame
                        .get("payload")
                        .filter(|v| !v.is_null())
                        .unwrap_or(&frame)
                        .clone();
                    on_frame(payload);
                }
            }
        }
    }
}

/// 查找块分隔符，返回（块结束偏移，分隔符字节数）。
/// 优先匹配 CRLF 双换行（\r\n\r\n，4 字节），否则匹配 LF 双换行（\n\n，2 字节）。
fn find_separator(buf: &[u8]) -> Option<(usize, usize)> {
    if let Some(idx) = find_subslice(buf, b"\r\n\r\n") {
        return Some((idx, 4));
    }
    find_subslice(buf, b"\n\n").map(|idx| (idx, 2))
}

/// 字节切片子串查找（str::find 的字节版）。
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}
