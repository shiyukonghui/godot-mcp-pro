//! HTTP MCP 传输层（基于 axum 框架）
//!
//! 提供两种模式：
//! - **POST /mcp** — 直接 JSON-RPC（无需 SSE）
//! - **GET  /mcp** — SSE 流（MCP HTTP SSE 传输）
//! - **POST /mcp?sessionId=xxx** — SSE 模式的消息提交
//!
//! SSE 流程：
//! 1. 客户端 GET /mcp → 建立 SSE 连接
//! 2. 服务端发送 `endpoint` 事件，告知 POST URL（含 session_id）
//! 3. 客户端 POST /mcp?sessionId=xxx 发送 JSON-RPC
//! 4. 服务端返回 202 Accepted，响应通过 SSE 流推送

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::MethodRouter;
use axum::Router;
use axum::http::StatusCode;
use futures::future;
use futures::stream::{self, Stream, StreamExt};
use tokio_stream::wrappers::UnboundedReceiverStream;

use godot::prelude::godot_print;

use crate::mcp::protocol::JsonRpcMessage;
use crate::plugin::PluginState;

/// SSE 会话映射: session_id → 响应发送端
type SseSessions = Arc<Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<String>>>>;

/// HTTP MCP 传输层
pub struct McpHttpTransport {
    port: u16,
    state: Arc<PluginState>,
    sse_sessions: SseSessions,
}

impl McpHttpTransport {
    pub fn new(port: u16, state: Arc<PluginState>) -> Self {
        Self {
            port,
            state,
            sse_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(&self) {
        let app_state = Arc::new(AppState {
            state: self.state.clone(),
            sse_sessions: self.sse_sessions.clone(),
        });

        // SSE 响应转发后台任务：读取主线程发来的带 session_id 的响应，转发给对应 SSE 连接
        let fwd_sessions = self.sse_sessions.clone();
        let sse_rx = self.state.sse_response_rx.clone();
        tokio::spawn(async move {
            loop {
                match sse_rx.recv() {
                    Ok((session_id, response)) => {
                        let sessions = fwd_sessions.lock().unwrap();
                        if let Some(sender) = sessions.get(&session_id) {
                            let _ = sender.send(response);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // 定期清理已断开的 SSE 会话（60s 间隔）
        let cleanup_sessions = self.sse_sessions.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                let mut sessions = cleanup_sessions.lock().unwrap();
                sessions.retain(|_, sender| !sender.is_closed());
            }
        });

        // 构建 axum Router
    let app = Router::new()
        .route(
            "/mcp",
            MethodRouter::new()
                .get(get_handler)       // GET /mcp → JSON 状态（连通性检查）
                .post(post_handler)     // POST /mcp → JSON-RPC
                .options(options_handler),
        )
        .route(
            "/sse",
            MethodRouter::new()
                .get(sse_handler)       // GET /sse → SSE 流
                .options(options_handler),
        )
        .with_state(app_state);

        let addr = format!("127.0.0.1:{}", self.port);
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                godot_print!("[MCP-RS] ❌ HTTP 绑定失败 {}: {}", addr, e);
                return;
            }
        };

        godot_print!("[MCP-RS] 🌐 HTTP 服务器监听 {} (axum)", addr);

        if let Err(e) = axum::serve(listener, app).await {
            godot_print!("[MCP-RS] ⚠️ HTTP 服务器退出: {:?}", e);
        }
    }
}

/// 共享应用状态
struct AppState {
    state: Arc<PluginState>,
    sse_sessions: SseSessions,
}

// ═══════════════════════════════════════════════════════════
// SSE 端点 (GET /sse)
// ═══════════════════════════════════════════════════════════

/// SSE 流：先发送 endpoint 事件，后续转发工具调用响应
async fn sse_handler(
    State(app): State<Arc<AppState>>,
) -> impl IntoResponse {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let session_id = generate_session_id();

    app.sse_sessions.lock().unwrap().insert(session_id.clone(), tx);
    godot_print!("[MCP-RS] 🔗 SSE 连接建立: {}", session_id);

    // endpoint 事件告知客户端 POST 地址
    let endpoint_msg = format!("/mcp?sessionId={}", session_id);
    let endpoint = Event::default().event("endpoint").data(endpoint_msg);

    // 将 endpoint 事件与后续响应事件流合并
    let stream: Box<dyn Stream<Item = Result<Event, axum::Error>> + Unpin + Send> = Box::new(
        stream::once(future::ready(Ok(endpoint))).chain(
            UnboundedReceiverStream::new(rx)
                .map(|msg| Ok(Event::default().event("message").data(msg))),
        ),
    );

    // 带 CORS 头的 SSE 响应
    let headers = [("Access-Control-Allow-Origin", "*")];
    (
        headers,
        Sse::new(stream).keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        ),
    )
}

// ═══════════════════════════════════════════════════════════
// 连通性检查 (GET /mcp)
// ═══════════════════════════════════════════════════════════

/// GET /mcp — 返回 JSON 状态，供客户端检查连通性
async fn get_handler() -> Response {
    let status = serde_json::json!({
        "status": "ok",
        "server": "godot-mcp-rs",
        "transport": "streamable-http"
    });
    cors_json_response(
        StatusCode::OK,
        serde_json::to_string(&status).unwrap_or_default(),
    )
}

// ═══════════════════════════════════════════════════════════
// POST 端点
// ═══════════════════════════════════════════════════════════

/// POST /mcp 处理函数
async fn post_handler(
    State(app): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    body: String,
) -> Response {
    // 判断是否带 sessionId（SSE 模式 vs 直接模式）
    let session_id = params.get("sessionId").cloned();

    // 解析 JSON-RPC 消息
    let mut msg: JsonRpcMessage = match serde_json::from_str(&body) {
        Ok(m) => m,
        Err(e) => {
            let err_rsp = serde_json::json!({
                "jsonrpc": "2.0", "id": null,
                "error": {"code": -32700, "message": format!("Parse error: {}", e)}
            });
            return cors_response(
                StatusCode::BAD_REQUEST,
                serde_json::to_string(&err_rsp).unwrap_or_default(),
            );
        }
    };

    // 心跳 ping 直接处理
    if msg.method.as_deref() == Some("ping") {
        let pong = serde_json::json!({"jsonrpc": "2.0", "id": msg.id, "result": {}});
        return cors_response(
            StatusCode::OK,
            serde_json::to_string(&pong).unwrap_or_default(),
        );
    }

    if let Some(sid) = session_id {
        // === SSE 模式 ===
        msg.session_id = Some(sid);

        if let Err(e) = app.state.pending_requests.send(msg) {
            godot_print!("[MCP-RS] ⚠️ SSE POST 投递失败: {:?}", e);
            return cors_response(StatusCode::INTERNAL_SERVER_ERROR, String::new());
        }
        // 返回 202 Accepted — 响应通过 SSE 推送
        cors_response(StatusCode::ACCEPTED, String::new())
    } else {
        // === 直接模式（同步等待响应） ===
        if let Err(e) = app.state.pending_requests.send(msg) {
            godot_print!("[MCP-RS] ⚠️ 直接 POST 投递失败: {:?}", e);
            return cors_response(StatusCode::INTERNAL_SERVER_ERROR, String::new());
        }

        // 轮询等待主线程响应
        let response = loop {
            match app.state.pending_responses.try_recv() {
                Ok(rsp) => break rsp,
                Err(crossbeam::channel::TryRecvError::Empty) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    continue;
                }
                Err(_) => break String::new(),
            }
        };

        if response.is_empty() {
            cors_response(StatusCode::NO_CONTENT, String::new())
        } else {
            cors_json_response(StatusCode::OK, response)
        }
    }
}

// ═══════════════════════════════════════════════════════════
// CORS 预检
// ═══════════════════════════════════════════════════════════

/// OPTIONS 预检请求
async fn options_handler() -> impl IntoResponse {
    (
        [
            ("Access-Control-Allow-Origin", "*"),
            ("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
            ("Access-Control-Allow-Headers", "Content-Type"),
        ],
        axum::http::StatusCode::NO_CONTENT,
    )
}

// ═══════════════════════════════════════════════════════════
// 工具函数
// ═══════════════════════════════════════════════════════════

/// 构建带 CORS 头的 HTTP 响应
fn cors_response(status: StatusCode, body: String) -> Response {
    let mut resp = (status, body).into_response();
    resp.headers_mut().insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        axum::http::HeaderValue::from_static("*"),
    );
    resp
}

/// 构建带 CORS + JSON Content-Type 头的 HTTP 响应
fn cors_json_response(status: StatusCode, body: String) -> Response {
    let mut resp = (status, body).into_response();
    resp.headers_mut().insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        axum::http::HeaderValue::from_static("*"),
    );
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    resp
}

/// 生成唯一的 SSE session_id
fn generate_session_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let rand: u64 = rand_simple();
    format!("sse-{:x}-{:x}", ts, rand)
}

/// 简单随机数生成（避免引入 uuid 依赖）
fn rand_simple() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    hasher.finish()
}
