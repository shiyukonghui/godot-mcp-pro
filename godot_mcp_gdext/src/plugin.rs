//! EditorPlugin 主实现

use std::sync::Arc;

use godot::classes::{EditorPlugin, ProjectSettings};
use godot::prelude::*;

use crate::commands;
use crate::mcp::transport_http::McpHttpTransport;
use crate::mcp::protocol::{JsonRpcMessage, ToolDefinition};

/// 跨线程共享的插件状态
pub struct PluginState {
    pub pending_requests: crossbeam::channel::Sender<JsonRpcMessage>,
    pub pending_responses: crossbeam::channel::Receiver<String>,
    /// SSE 会话响应通道（HTTP SSE 模式）：主线程发送 (session_id, 响应JSON)
    pub sse_response_rx: crossbeam::channel::Receiver<(String, String)>,
    /// 上一次捕获的编辑器 Output 面板内容（用于计算输出增量）
    pub last_console_content: std::sync::Mutex<String>,
}

/// 运行时 Autoload 定义: (设置键, GDScript 路径)
const MCP_AUTOLOADS: &[(&str, &str)] = &[
    ("autoload/MCPRuntimeAgent", "res://addons/godot_mcp_rs/mcp_runtime_agent.gd"),
];

/// Rust MCP EditorPlugin
#[derive(GodotClass)]
#[class(tool, init, base=EditorPlugin)]
pub struct RustMcpPlugin {
    base: Base<EditorPlugin>,
    state: Option<Arc<PluginState>>,
    request_rx: Option<crossbeam::channel::Receiver<JsonRpcMessage>>,
    response_tx: Option<crossbeam::channel::Sender<String>>,
    /// SSE 响应发送端：主线程向 HTTP 线程发送带 session_id 的响应
    sse_response_tx: Option<crossbeam::channel::Sender<(String, String)>>,
    tool_definitions: Vec<ToolDefinition>,
    port: u16,
    /// 本次会话注入的 Autoload 键列表 (退出时只清理自己注入的)
    session_autoloads: Vec<String>,
}

#[godot_api]
impl RustMcpPlugin {
    /// 插件加载时由 Godot 自动调用
    #[func]
    fn _enter_tree(&mut self) {
        godot_print!("[MCP-RS] === Godot MCP RS 启动中... ===");

        self.port = self.load_port_config();
        self.tool_definitions = commands::collect_all_tools();

        let (req_tx, req_rx) = crossbeam::channel::unbounded();
        let (rsp_tx, rsp_rx) = crossbeam::channel::unbounded();
        // SSE 响应通道：主线程 → HTTP 线程（带 session_id 的响应走此通道）
        let (sse_tx, sse_rx) = crossbeam::channel::unbounded();
        self.request_rx = Some(req_rx);
        self.response_tx = Some(rsp_tx);
        self.sse_response_tx = Some(sse_tx);

        let state = Arc::new(PluginState {
            pending_requests: req_tx,
            pending_responses: rsp_rx,
            sse_response_rx: sse_rx,
            last_console_content: std::sync::Mutex::new(String::new()),
        });
        self.state = Some(state.clone());

        let port = self.port;
        let state_for_http = state.clone();

        // HTTP 服务器（AI 助手可直接连接，无需 mcp_bridge）
        // 端口 = MCP 端口 + 1（默认 9877）
        let http_port = port + 1;
        std::thread::Builder::new()
            .name("mcp-http".into())
            .spawn(move || {
                let transport = McpHttpTransport::new(http_port, state_for_http);
                let rt = tokio::runtime::Runtime::new().expect("[MCP-RS] HTTP tokio runtime 创建失败");
                rt.block_on(async { transport.run().await });
            })
            .expect("[MCP-RS] HTTP 线程启动失败");

        godot_print!("[MCP-RS] HTTP 端口 {}, 已注册 {} 个工具", http_port, self.tool_definitions.len());
        // 注入运行时 Autoload
        self.inject_autoloads();

        godot_print!("[MCP-RS] === Godot MCP RS 启动完成 ===");
    }

    fn inject_autoloads(&mut self) {
        let mut changed = false;
        for &(key, script) in MCP_AUTOLOADS {
            let mut settings = ProjectSettings::singleton();
            if !settings.has_setting(key) {
                let prefixed = format!("*{}", script); // "*" 表示启用
                settings.set_setting(key, &Variant::from(prefixed));
                self.session_autoloads.push(key.to_string());
                changed = true;
            }
        }
        if changed {
            ProjectSettings::singleton().save();
        }
    }

    fn remove_autoloads(&mut self) {
        let mut changed = false;
        let mut settings = ProjectSettings::singleton();
        for key in &self.session_autoloads {
            if settings.has_setting(key) {
                settings.set_setting(key, &Variant::nil());
                changed = true;
            }
        }
        self.session_autoloads.clear();
        if changed {
            ProjectSettings::singleton().save();
        }
    }

    /// 插件卸载时由 Godot 自动调用
    #[func]
    fn _exit_tree(&mut self) {
        godot_print!("[MCP-RS] 关闭中...");

        // 清理运行时 Autoload
        self.remove_autoloads();

        self.state = None;
        self.request_rx = None;
        self.response_tx = None;
        godot_print!("[MCP-RS] 已关闭");
    }

    /// 每帧处理: 消费 HTTP 线程投递的命令请求
    #[func]
    fn _process(&mut self, _delta: f64) {
        let rx = match &self.request_rx {
            Some(rx) => rx,
            None => return,
        };

        loop {
            let msg = match rx.try_recv() {
                Ok(m) => m,
                Err(crossbeam::channel::TryRecvError::Empty) => break,
                Err(crossbeam::channel::TryRecvError::Disconnected) => {
                    self.request_rx = None;
                    return;
                }
            };

            // 判断是否为 SSE 会话请求（带 session_id）
            let is_sse = msg.session_id.is_some();
            let session_id = msg.session_id.clone();

            let response = self.handle_mcp_message(msg);

            if is_sse {
                // SSE 模式的响应：通过 SSE 通道发回 HTTP 线程
                if let (Some(sid), Some(tx)) = (session_id, &self.sse_response_tx) {
                    let _ = tx.send((sid, response));
                }
            } else {
                // 直接 HTTP 模式的响应：通过原通道发回
                if let Some(tx) = &self.response_tx {
                    let _ = tx.send(response);
                }
            }
        }
    }

    fn handle_mcp_message(&self, msg: JsonRpcMessage) -> String {
        let method = msg.method.as_deref().unwrap_or("").to_string();
        let id = msg.id;

        match method.as_str() {
            "initialize" => self.build_response(id, serde_json::json!({
                "protocolVersion": "2025-03-26",
                "capabilities": { "tools": { "listChanged": false }, "logging": {} },
                "serverInfo": { "name": "godot-mcp-rs", "version": "0.1.0" }
            })),
            "notifications/initialized" => String::new(),
            "tools/list" => self.build_response(id, serde_json::json!({"tools": self.tool_definitions})),
            "tools/call" => self.handle_call_tool(id, msg.params),
            "ping" => self.build_response(id, serde_json::json!({})),
            _ => self.build_error_response(id, crate::utils::error::McpError::method_not_found(&method)),
        }
    }

    fn handle_call_tool(&self, id: Option<serde_json::Value>, params: Option<serde_json::Value>) -> String {
        let tool_name = params.as_ref().and_then(|p| p.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
        let arguments = params.as_ref().and_then(|p| p.get("arguments")).cloned().unwrap_or_default();
        let args = match arguments { serde_json::Value::Object(m) => m, _ => serde_json::Map::new() };

        // 获取上一次捕获的控制台内容
        let last_content: String = self.state.as_ref()
            .and_then(|s| s.last_console_content.lock().ok())
            .map(|guard| guard.clone())
            .unwrap_or_default();

        // 执行工具
        let result = commands::execute_tool(&tool_name, &args);

        // 捕获执行后的控制台内容并计算增量
        let console_output = match crate::commands::console_capture::capture_output_panel() {
            Ok(current) => {
                // 更新存储的上一次内容
                if let Some(state) = &self.state {
                    if let Ok(mut guard) = state.last_console_content.lock() {
                        *guard = current.clone();
                    }
                }
                // 计算新增的输出行
                let delta = crate::commands::console_capture::compute_output_delta(&current, &last_content);
                if delta.is_empty() { None } else { Some(delta) }
            }
            Err(_) => None, // 捕获失败时不阻塞工具结果
        };

        match result {
            Ok(mut result_value) => {
                // 如果有控制台输出增量，附加到结果中
                if let Some(output_lines) = console_output {
                    if let Some(obj) = result_value.as_object_mut() {
                        obj.insert("console_output".into(), serde_json::json!(output_lines));
                    }
                }
                let content = serde_json::json!([{
                    "type": "text",
                    "text": serde_json::to_string(&result_value).unwrap_or_else(|_| "{}".into())
                }]);
                self.build_response(id, serde_json::json!({"content": content}))
            }
            Err(err) => {
                // 即使工具执行失败，也尝试附加控制台输出
                if let Some(output_lines) = console_output {
                    let err_data = serde_json::json!({
                        "error": err.message,
                        "code": err.code,
                        "console_output": output_lines,
                    });
                    self.build_response(id, serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string(&err_data).unwrap_or_default()
                        }]
                    }))
                } else {
                    self.build_error_response(id, err)
                }
            }
        }
    }

    fn build_response(&self, id: Option<serde_json::Value>, result: serde_json::Value) -> String {
        serde_json::to_string(&serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}))
            .unwrap_or_default()
    }

    fn build_error_response(&self, id: Option<serde_json::Value>, error: crate::utils::error::McpError) -> String {
        serde_json::to_string(&serde_json::json!({"jsonrpc": "2.0", "id": id, "error": error}))
            .unwrap_or_default()
    }

    fn load_port_config(&self) -> u16 {
        let settings = ProjectSettings::singleton();
        if settings.has_setting("godot_mcp/port") {
            settings.get_setting("godot_mcp/port").to::<i64>() as u16
        } else {
            9876
        }
    }
}
