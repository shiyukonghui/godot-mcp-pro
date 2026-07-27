//! EditorPlugin 主实现

use std::sync::Arc;

use godot::classes::{EditorPlugin, ProjectSettings};
use godot::prelude::*;

use crate::commands;
use crate::mcp::transport::McpTransport;
use crate::mcp::protocol::{JsonRpcMessage, ToolDefinition};

/// 跨线程共享的插件状态
pub struct PluginState {
    pub pending_requests: crossbeam::channel::Sender<JsonRpcMessage>,
    pub pending_responses: crossbeam::channel::Receiver<String>,
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
        self.request_rx = Some(req_rx);
        self.response_tx = Some(rsp_tx);

        let state = Arc::new(PluginState {
            pending_requests: req_tx,
            pending_responses: rsp_rx,
        });
        self.state = Some(state.clone());

        let port = self.port;
        let tools_json = serde_json::to_value(&self.tool_definitions).unwrap_or_default();

        std::thread::Builder::new()
            .name("mcp-tcp".into())
            .spawn(move || {
                let mut transport = McpTransport::new(port, state, tools_json);
                let rt = tokio::runtime::Runtime::new().expect("[MCP-RS] tokio runtime 创建失败");
                rt.block_on(async { transport.run().await });
            })
            .expect("[MCP-RS] TCP 线程启动失败");

        godot_print!("[MCP-RS] TCP 服务器端口 {}, 已注册 {} 个工具", self.port, self.tool_definitions.len());
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

    /// 每帧处理: 消费 TCP 线程投递的命令请求
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

            let response = self.handle_mcp_message(msg);
            if let Some(tx) = &self.response_tx {
                let _ = tx.send(response);
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

        match commands::execute_tool(&tool_name, &args) {
            Ok(result) => {
                let content = serde_json::json!([{
                    "type": "text",
                    "text": serde_json::to_string(&result).unwrap_or_else(|_| "{}".into())
                }]);
                self.build_response(id, serde_json::json!({"content": content}))
            }
            Err(err) => self.build_error_response(id, err),
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
