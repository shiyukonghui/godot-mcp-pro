//! HTTP MCP 传输层
//!
//! 提供 HTTP POST /mcp 端点，AI 助手可直接通过 HTTP 连接。
//! 内部通过 crossbeam 通道将请求投递到主线程处理（与 TCP 传输相同机制）。
//!
//! 端点: POST http://127.0.0.1:9877/mcp
//! 请求体: JSON-RPC 2.0 MCP 消息
//! 响应体: JSON-RPC 2.0 MCP 响应

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use godot::prelude::godot_print;

use crate::mcp::protocol::JsonRpcMessage;
use crate::plugin::PluginState;

/// HTTP MCP 传输层
pub struct McpHttpTransport {
    port: u16,
    state: Arc<PluginState>,
}

impl McpHttpTransport {
    pub fn new(port: u16, state: Arc<PluginState>) -> Self {
        Self { port, state }
    }

    pub async fn run(&self) {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                godot_print!("[MCP-RS] ❌ HTTP 绑定失败 {}: {}", addr, e);
                return;
            }
        };

        godot_print!("[MCP-RS] 🌐 HTTP 服务器监听 {} (AI 助手可直接 POST /mcp)", addr);

        while let Ok((stream, peer)) = listener.accept().await {
            let state = self.state.clone();
            tokio::spawn(async move {
                let (reader, mut writer) = stream.into_split();
                let mut buf_reader = BufReader::new(reader);

                // 读取 HTTP 请求行
                let mut request_line = String::new();
                if buf_reader.read_line(&mut request_line).await.is_err() || request_line.is_empty() {
                    return;
                }
                if !request_line.starts_with("POST /mcp ") {
                    let _ = writer.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await;
                    return;
                }

                // 读取头部，找到 Content-Length
                let mut content_length: usize = 0;
                loop {
                    let mut header = String::new();
                    if buf_reader.read_line(&mut header).await.is_err() || header.trim().is_empty() {
                        break;
                    }
                    if let Some(len_str) = header.strip_prefix("Content-Length:") {
                        content_length = len_str.trim().parse().unwrap_or(0);
                    }
                }

                // 读取请求体
                let mut body = vec![0u8; content_length];
                if content_length > 0 {
                    if tokio::io::AsyncReadExt::read_exact(&mut buf_reader, &mut body).await.is_err() {
                        let _ = writer.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n").await;
                        return;
                    }
                }
                let body_str = String::from_utf8_lossy(&body);

                // 解析 JSON-RPC 消息
                let msg: JsonRpcMessage = match serde_json::from_str(&body_str) {
                    Ok(m) => m,
                    Err(e) => {
                        let err_rsp = serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":format!("Parse error: {}", e)}});
                        let rsp = serde_json::to_string(&err_rsp).unwrap_or_default();
                        let _ = write_http_response(&mut writer, &rsp).await;
                        return;
                    }
                };

                // 心跳 ping 直接在 tokio 线程处理
                if msg.method.as_deref() == Some("ping") {
                    let pong = serde_json::json!({"jsonrpc":"2.0","id":msg.id,"result":{}});
                    let rsp = serde_json::to_string(&pong).unwrap_or_default();
                    let _ = write_http_response(&mut writer, &rsp).await;
                    return;
                }

                // 投递到主线程处理队列（与 TCP 传输相同通道）
                if let Err(e) = state.pending_requests.send(msg) {
                    godot_print!("[MCP-RS] ⚠️ HTTP 投递请求失败: {:?}", e);
                    return;
                }

                // 等待响应（带超时）
                let response = loop {
                    match state.pending_responses.try_recv() {
                        Ok(rsp) => break rsp,
                        Err(crossbeam::channel::TryRecvError::Empty) => {
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                            continue;
                        }
                        Err(_) => break String::new(),
                    }
                };

                if response.is_empty() {
                    let _ = writer.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n").await;
                } else {
                    let _ = write_http_response(&mut writer, &response).await;
                }
            });
        }
    }
}

/// 写入 HTTP 200 JSON 响应
async fn write_http_response(writer: &mut (impl AsyncWrite + Unpin), body: &str) -> std::io::Result<()> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
        body.len()
    );
    writer.write_all(headers.as_bytes()).await?;
    writer.write_all(body.as_bytes()).await?;
    writer.flush().await
}
