//! DSH 状态引擎：五态状态机 + 会话存储 + 2s 轮询 / SSE 编排。
//!
//! 直译 main.js 的 pollSessions / connectMux / connectHost / buildSnapshot / emit。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tokio::time::MissedTickBehavior;

use crate::discover;
use crate::rpc;
use crate::sse;

/// 轮询间隔（main.js 的 POLL_MS）。
const POLL_MS: u64 = 2000;
/// 会话完成后"完成待查看"的保留时长（main.js 的 DONE_WINDOW_MS）。
const DONE_WINDOW_MS: i64 = 120_000;
/// 待决项超时（main.js 的 PENDING_TTL）。
const PENDING_TTL_MS: i64 = 30 * 60 * 1000;

/// 单条会话（main.js pet.sessions 的值）。
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub title: String,
    pub running: bool,
}

/// 待决审批（main.js pet.pendingApprovals 的值）。
#[derive(Debug, Clone)]
pub struct PendingApproval {
    pub session_id: String,
    pub tool_name: String,
    #[allow(dead_code)]
    pub reason: Value,
    pub requested_at: i64,
}

/// 待决提问（main.js pet.pendingQuestions 的值）。
#[derive(Debug, Clone)]
pub struct PendingQuestion {
    pub session_id: String,
    pub text: String,
    pub requested_at: i64,
}

/// 完成待查看项（main.js pet.done 的值，sessionId 在 Map key 里）。
#[derive(Debug, Clone)]
pub struct DoneItem {
    pub title: String,
    pub at: i64,
}

/// 引擎共享状态（main.js 的 pet 对象）。
#[derive(Debug, Clone)]
pub struct EngineState {
    pub last_emitted_mode: Option<String>,
    pub connected: bool,
    pub last_error: Option<String>,
    pub sessions: HashMap<String, SessionInfo>,
    pub pending_approvals: HashMap<String, PendingApproval>,
    pub pending_questions: HashMap<String, PendingQuestion>,
    pub done: HashMap<String, DoneItem>,
    #[allow(dead_code)]
    pub queued_count: usize,
}

impl Default for EngineState {
    fn default() -> Self {
        Self {
            last_emitted_mode: None,
            connected: false,
            last_error: None,
            sessions: HashMap::new(),
            pending_approvals: HashMap::new(),
            pending_questions: HashMap::new(),
            done: HashMap::new(),
            queued_count: 0,
        }
    }
}

/// 状态快照（发给前端的 JSON）。
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub state: String,
    pub attention: Vec<String>,
    pub running: Vec<String>,
    pub done: Vec<String>,
}

/// 对外发送回调。
pub type SnapshotSink = Arc<dyn Fn(Snapshot) + Send + Sync + 'static>;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn session_title(state: &EngineState, id: &str) -> String {
    state
        .sessions
        .get(id)
        .map(|s| s.title.clone())
        .unwrap_or_else(|| "某会话".to_string())
}

/// 五态推导（offline > attention > working > done > idle）。
pub fn build_snapshot(state: &EngineState) -> Snapshot {
    let running: Vec<String> = state
        .sessions
        .values()
        .filter(|s| s.running)
        .map(|s| s.title.clone())
        .collect();

    let mut attention: Vec<String> = Vec::new();
    for a in state.pending_approvals.values() {
        attention.push(format!(
            "「{}」请求使用 {}",
            session_title(state, &a.session_id),
            a.tool_name
        ));
    }
    for q in state.pending_questions.values() {
        attention.push(format!(
            "「{}」：{}",
            session_title(state, &q.session_id),
            q.text
        ));
    }

    let done: Vec<String> = state.done.values().map(|d| d.title.clone()).collect();

    let state_name = if !state.connected {
        "offline"
    } else if state.last_error.is_some() {
        "error"
    } else if !attention.is_empty() {
        "attention"
    } else if !running.is_empty() {
        "working"
    } else if !done.is_empty() {
        "done"
    } else if !state.sessions.is_empty() {
        // 有会话但都未运行：回合中空闲（等待下一轮）
        "turn-idle"
    } else {
        "idle"
    };

    Snapshot {
        state: state_name.to_string(),
        attention,
        running,
        done,
    }
}

/// emit：待决项 TTL 清理 + 快照推导 + 每次都对外发送（main.js 的 emit()）。
fn emit(state: &Arc<Mutex<EngineState>>, sink: &SnapshotSink) {
    let now = now_ms();
    let snapshot = {
        let mut st = state.lock().unwrap();
        // 待决项超时（30 分钟）自动过期，防止残留
        st.pending_approvals
            .retain(|_, a| now - a.requested_at <= PENDING_TTL_MS);
        st.pending_questions
            .retain(|_, q| now - q.requested_at <= PENDING_TTL_MS);

        let snapshot = build_snapshot(&st);
        // 仅在模式变化时打印日志，避免刷屏（仅用于日志去重）
        if st.last_emitted_mode.as_deref() != Some(snapshot.state.as_str()) {
            st.last_emitted_mode = Some(snapshot.state.clone());
            log::info!(
                "[pet] state={} running={} approvals={} questions={} done={}",
                snapshot.state,
                snapshot.running.len(),
                st.pending_approvals.len(),
                st.pending_questions.len(),
                snapshot.done.len()
            );
        }
        snapshot
    };
    sink(snapshot);
}

/// 轮询 session.list（main.js 的 pollSessions()）。
async fn poll_sessions(
    client: reqwest::Client,
    state: Arc<Mutex<EngineState>>,
    sink: SnapshotSink,
) -> bool {
    let items: Vec<Value> = match rpc::rpc(&client, "session.list", json!({})).await {
        Ok(v) => v
            .get("items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default(),
        Err(e) => {
            {
                let mut st = state.lock().unwrap();
                st.connected = false;
                st.last_error = Some(e);
            }
            emit(&state, &sink);
            return false;
        }
    };

    let now = now_ms();
    {
        let mut st = state.lock().unwrap();
        st.connected = true;
        st.last_error = None;

        let mut seen: HashSet<String> = HashSet::new();
        for s in &items {
            let session_id = s
                .get("sessionId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            seen.insert(session_id.clone());

            let title = s
                .get("projections")
                .and_then(|p| p.get("values"))
                .and_then(|v| v.get("title"))
                .and_then(|t| t.as_str())
                .unwrap_or("未命名会话")
                .to_string();
            let running = s.get("running").and_then(|r| r.as_bool()).unwrap_or(false);

            let prev_running = st.sessions.get(&session_id).map(|p| p.running);
            st.sessions.insert(
                session_id.clone(),
                SessionInfo {
                    title: title.clone(),
                    running,
                },
            );

            // running true -> false：会话刚结束，标记"完成待查看"
            if prev_running == Some(true) && !running {
                st.done.insert(session_id, DoneItem { title, at: now });
            }
        }

        // 删除消失的会话
        st.sessions.retain(|id, _| seen.contains(id));
        // 清理超时 done
        st.done.retain(|_, d| now - d.at <= DONE_WINDOW_MS);
    }
    emit(&state, &sink);
    true
}

/// 处理 /api/events.mux 帧（main.js 的 handleFrame()）。
fn handle_frame(state: &Arc<Mutex<EngineState>>, sink: &SnapshotSink, p: Value) {
    let frame_type = p.get("type").and_then(|v| v.as_str()).unwrap_or_default();
    match frame_type {
        "approval/requested" => {
            let approval_id = p
                .get("approvalId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let session_id = p
                .get("sessionId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let tool_name = p
                .get("toolName")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let reason = p.get("reason").cloned().unwrap_or(Value::Null);
            {
                let mut st = state.lock().unwrap();
                st.pending_approvals.insert(
                    approval_id,
                    PendingApproval {
                        session_id,
                        tool_name,
                        reason,
                        requested_at: now_ms(),
                    },
                );
            }
            emit(state, sink);
        }
        "approval/resolved" => {
            let approval_id = p
                .get("approvalId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            {
                let mut st = state.lock().unwrap();
                st.pending_approvals.remove(&approval_id);
            }
            emit(state, sink);
        }
        "question/requested" => {
            let question_rpc_id = p
                .get("questionRpcId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let session_id = p
                .get("sessionId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let text = p
                .get("questions")
                .and_then(|q| q.as_array())
                .and_then(|arr| arr.first())
                .and_then(|q| q.get("question"))
                .and_then(|q| q.as_str())
                .unwrap_or("")
                .to_string();
            {
                let mut st = state.lock().unwrap();
                st.pending_questions.insert(
                    question_rpc_id,
                    PendingQuestion {
                        session_id,
                        text,
                        requested_at: now_ms(),
                    },
                );
            }
            emit(state, sink);
        }
        "question/resolved" => {
            let question_rpc_id = p
                .get("questionRpcId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            {
                let mut st = state.lock().unwrap();
                st.pending_questions.remove(&question_rpc_id);
            }
            emit(state, sink);
        }
        "session/queue" => {
            let count = p
                .get("items")
                .and_then(|i| i.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            {
                let mut st = state.lock().unwrap();
                st.queued_count = count;
            }
            emit(state, sink);
        }
        "session/event" => {
            // 原版此处 playSoundFile('done')（turn/end completed 语音播报）；
            // 音频属于桌宠 UI 层，留待后续阶段处理。
        }
        _ => {}
    }
}

/// 处理 /api/events.host 帧（main.js 的 handleHostFrame()）。
fn handle_host_frame(
    state: &Arc<Mutex<EngineState>>,
    sink: &SnapshotSink,
    client: &reqwest::Client,
    p: Value,
) {
    if p.get("type").and_then(|v| v.as_str()) != Some("host/session-status") {
        return;
    }
    let session_id = p
        .get("sessionId")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let running = p.get("running").and_then(|v| v.as_bool()).unwrap_or(false);

    let unknown = {
        let mut st = state.lock().unwrap();
        let mut unknown = false;
        if let Some(cur) = st.sessions.get_mut(&session_id) {
            let was_running = cur.running;
            cur.running = running;
            if was_running && !running {
                let title = cur.title.clone();
                st.done.insert(
                    session_id.clone(),
                    DoneItem {
                        title,
                        at: now_ms(),
                    },
                );
            }
        } else {
            unknown = true;
        }
        // 会话结束：清除该会话残留的待决审批/提问
        if !running {
            st.pending_approvals
                .retain(|_, a| a.session_id != session_id);
            st.pending_questions
                .retain(|_, q| q.session_id != session_id);
        }
        unknown
    };

    if unknown {
        // 未知会话（如新建）：立即刷新拿标题等信息
        let c = client.clone();
        let st = state.clone();
        let sk = sink.clone();
        tokio::spawn(async move {
            poll_sessions(c, st, sk).await;
        });
    }

    emit(state, sink);
}

/// 启动引擎：先自动发现 DSH，再启动 SSE（mux + host）+ 2s 轮询，永不返回。
pub async fn run(
    state: Arc<Mutex<EngineState>>,
    client: reqwest::Client,
    sink: SnapshotSink,
) {
    // 发现专用短超时 client：逐端口探测要快，不能拖慢启动。
    let discovery_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap_or_else(|_| client.clone());

    // 外层循环：DSH 可能重启导致端口变化，失联后重新发现并重连。
    loop {
        // 自动发现 DSH：失败进入离线态并每 10s 重试，直到找到。
        loop {
            match discover::discover_dsh_url(&discovery_client).await {
                Some(url) => {
                    rpc::set_dsh_url(url.clone());
                    log::info!("[pet] 已发现 DSH: {}", url);
                    break;
                }
                None => {
                    {
                        let mut st = state.lock().unwrap();
                        st.connected = false;
                        st.last_error = Some("未发现 DSH，10 秒后重试".to_string());
                    }
                    emit(&state, &sink);
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }

        // mux 连接前清空待决（main.js connectMux 的语义：服务端会重放）
        {
            let mut st = state.lock().unwrap();
            st.pending_approvals.clear();
            st.pending_questions.clear();
        }

        // host：running 翻转即时推送
        let host_frame_state = state.clone();
        let host_frame_sink = sink.clone();
        let host_frame_client = client.clone();
        let host_on_frame = move |p: Value| {
            handle_host_frame(&host_frame_state, &host_frame_sink, &host_frame_client, p);
        };
        let host_open_state = state.clone();
        let host_open_sink = sink.clone();
        let host_open_client = client.clone();
        let host_on_open = move || {
            let c = host_open_client.clone();
            let st = host_open_state.clone();
            let sk = host_open_sink.clone();
            tokio::spawn(async move {
                let _ = poll_sessions(c, st, sk).await;
            });
        };
        let _host_handle = sse::connect_sse(
            client.clone(),
            "/api/events.host".to_string(),
            host_on_frame,
            host_on_open,
        );

        // mux：审批/提问/队列推送
        let mux_frame_state = state.clone();
        let mux_frame_sink = sink.clone();
        let mux_on_frame = move |p: Value| {
            handle_frame(&mux_frame_state, &mux_frame_sink, p);
        };
        let _mux_handle = sse::connect_sse(
            client.clone(),
            "/api/events.mux".to_string(),
            mux_on_frame,
            || {},
        );

        // 2s 轮询 session.list；连续失败若干次视为 DSH 失联，跳出并重新发现
        let mut interval = tokio::time::interval(Duration::from_millis(POLL_MS));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut fail_count: u32 = 0;
        loop {
            interval.tick().await;
            if poll_sessions(client.clone(), state.clone(), sink.clone()).await {
                fail_count = 0;
            } else {
                fail_count += 1;
                if fail_count >= 3 {
                    log::warn!("[pet] DSH 连接失败 {} 次，重新发现", fail_count);
                    break;
                }
            }
        }
        // 离开作用域时 SSE 句柄 drop，外层循环重新发现并重建连接
    }
}
