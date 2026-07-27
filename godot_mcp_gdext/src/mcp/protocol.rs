//! MCP (Model Context Protocol) 协议数据类型
//!
//! 定义 JSON-RPC 2.0 + MCP 扩展的消息结构。
//! 对应原有 GDScript 插件的 WebSocket JSON-RPC 通信格式。

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════
// JSON-RPC 2.0 消息
// ═══════════════════════════════════════════════

/// 通用的 JSON-RPC 请求消息
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: Option<String>,
    pub id: Option<serde_json::Value>,
    pub method: Option<String>,
    pub params: Option<serde_json::Value>,
    /// SSE 会话 ID（仅 HTTP SSE 模式使用）
    #[serde(skip)]
    pub session_id: Option<String>,
}

// ═══════════════════════════════════════════════
// MCP 工具定义
// ═══════════════════════════════════════════════

/// MCP Tool 定义（对应 MCP ListToolsResult 中的 tool）
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    pub fn new(name: &str, description: &str, input_schema: serde_json::Value) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
        }
    }
}

// ═══════════════════════════════════════════════
// 命令执行上下文
// ═══════════════════════════════════════════════

/// 命令执行结果
#[derive(Debug)]
pub struct CommandResult {
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

impl CommandResult {
    pub fn ok(data: serde_json::Value) -> Self {
        Self { success: true, data, error: None }
    }

    pub fn err(msg: &str) -> Self {
        Self {
            success: false,
            data: serde_json::Value::Null,
            error: Some(msg.to_string()),
        }
    }
}
