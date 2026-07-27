//! TCP 服务器传输层
//!
//! 在后台 tokio 线程中运行，负责：
//! 1. 监听 TCP 端口，接收 MCP 桥接器的连接
//! 2. 解析 JSON-RPC 消息，投递到主线程处理队列
//! 3. 从响应队列中取出结果，发送回 TCP 客户端
//!
//! 消息格式：每行一个 JSON-RPC 消息 (newline-delimited JSON)

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use godot::prelude::godot_print;

use crate::plugin::PluginState;
use crate::mcp::protocol::JsonRpcMessage;

/// MCP TCP 传输层
pub struct McpTransport {
    port: u16,
    state: Arc<PluginState>,
    tools_json: serde_json::Value,
}

impl McpTransport {
    pub fn new(port: u16, state: Arc<PluginState>, tools_json: serde_json::Value) -> Self {
        Self { port, state, tools_json }
    }

    /// 运行 TCP 服务器 (tokio 异步上下文)
    pub async fn run(&mut self) {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                godot_print!("[MCP-RS] ❌ TCP 绑定失败 {}: {}", addr, e);
                return;
            }
        };

        godot_print!("[MCP-RS] ✅ TCP 服务器监听 {} (等待 mcp_bridge 连接)", addr);

        // 主循环：接受连接
        while let Ok((stream, peer)) = listener.accept().await {
            godot_print!("[MCP-RS] 🔗 收到连接: {:?}", peer);
            let state = self.state.clone();
            let (reader, mut writer) = stream.into_split();
            let mut buf_reader = BufReader::new(reader);
            let mut line = String::new();

            // 此连接的消息循环
            loop {
                line.clear();

                // 1. 从 TCP 读取一行 (非阻塞)
                let bytes_read = tokio::select! {
                    result = buf_reader.read_line(&mut line) => {
                        match result {
                            Ok(0) => break,  // 连接关闭
                            Ok(n) => n,
                            Err(e) => {
                                godot_print!("[MCP-RS] ⚠️ 读取错误: {:?}", e);
                                break;
                            }
                        }
                    }
                };

                if bytes_read == 0 {
                    break; // EOF
                }

                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                // 2. 解析 JSON-RPC 消息
                let msg: JsonRpcMessage = match serde_json::from_str(trimmed) {
                    Ok(m) => m,
                    Err(e) => {
                        // 发送解析错误响应
                        let err_rsp = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": null,
                            "error": {"code": -32700, "message": format!("Parse error: {}", e)}
                        });
                        let _ = writer.write_all(
                            format!("{}\n", serde_json::to_string(&err_rsp).unwrap()).as_bytes()
                        ).await;
                        continue;
                    }
                };

                // 3. 检查是否是心跳 ping (直接在 TCP 线程处理)
                if msg.method.as_deref() == Some("ping") {
                    let pong = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": msg.id,
                        "result": {}
                    });
                    let _ = writer.write_all(
                        format!("{}\n", serde_json::to_string(&pong).unwrap()).as_bytes()
                    ).await;
                    continue;
                }

                // 4. 将请求投递到主线程处理队列
                if let Err(e) = state.pending_requests.send(msg) {
                    godot_print!("[MCP-RS] ⚠️ 投递请求失败 (插件已退出?): {:?}", e);
                    break;
                }

                // 5. 等待并从响应队列取出结果
                //    非阻塞方式：最多尝试 500 次 * 10ms = 5 秒超时
                let response = loop {
                    match state.pending_responses.try_recv() {
                        Ok(rsp) => break rsp,
                        Err(crossbeam::channel::TryRecvError::Empty) => {
                            // 短暂等待后重试
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                            // 不无限等待，防止 TCP 线程阻塞
                            // (实际应该在 timeout 后继续而不是 break)
                            continue;
                        }
                        Err(crossbeam::channel::TryRecvError::Disconnected) => {
                            // 插件已卸载，退出连接循环
                            break String::new();
                        }
                    }
                };

                // 如果响应为空 (如 notifications)，不发送
                if response.is_empty() {
                    continue;
                }

                // 6. 将结果写回 TCP
                if let Err(e) = writer.write_all(
                    format!("{}\n", response).as_bytes()
                ).await {
                    godot_print!("[MCP-RS] ⚠️ 发送响应失败: {:?}", e);
                    break;
                }
            }

            godot_print!("[MCP-RS] 🔌 连接已断开: {:?}", peer);
        }
    }
}
