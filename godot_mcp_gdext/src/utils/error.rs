//! JSON-RPC 错误码体系
//! 与原有 GDScript 插件的错误码保持一致

use serde::Serialize;

/// JSON-RPC 错误对象
#[derive(Debug, Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[allow(dead_code)]
impl McpError {
    /// -32700: JSON 解析错误
    pub fn parse_error() -> Self {
        Self { code: -32700, message: "Parse error".into(), data: None }
    }

    /// -32600: 无效请求
    pub fn invalid_request(detail: &str) -> Self {
        Self { code: -32600, message: format!("Invalid request: {}", detail), data: None }
    }

    /// -32601: 方法未找到
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {}", method),
            data: None,
        }
    }

    /// -32602: 无效参数
    pub fn invalid_params(msg: &str) -> Self {
        Self { code: -32602, message: msg.into(), data: None }
    }

    /// -32603: 内部错误
    pub fn internal(msg: &str) -> Self {
        Self { code: -32603, message: format!("Internal error: {}", msg), data: None }
    }

    /// -32000: 编辑器状态错误 (无场景等)
    pub fn no_scene() -> Self {
        Self {
            code: -32000,
            message: "No scene is currently open".into(),
            data: Some(serde_json::json!({
                "suggestion": "Use open_scene to open a scene first"
            })),
        }
    }

    /// -32001: 资源未找到
    pub fn not_found(what: &str, suggestion: &str) -> Self {
        let mut data = None;
        if !suggestion.is_empty() {
            data = Some(serde_json::json!({"suggestion": suggestion}));
        }
        Self { code: -32001, message: format!("{} not found", what), data }
    }

    /// 转换为 JSON-RPC 响应中的 error 字段
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::json!({
            "code": -32603,
            "message": "Failed to serialize error"
        }))
    }
}
