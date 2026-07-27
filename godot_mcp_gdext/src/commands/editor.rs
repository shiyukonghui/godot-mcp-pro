//! 编辑器工具命令模块

use std::collections::HashMap;

use godot::classes::Expression;
use godot::global::Error;
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

/// 收集本模块的工具定义
pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("execute_editor_script", "在编辑器上下文中执行 GDScript 代码", serde_json::json!({
            "type": "object", "properties": {
                "code": { "type": "string", "description": "要执行的 GDScript 代码" }
            }, "required": ["code"]
        })),
    ]
}

/// 注册本模块的命令
pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("execute_editor_script".into(), cmd_execute_editor_script);
}

/// execute_editor_script: 使用 Godot Expression 执行 GDScript
///
/// Expression 在 Godot 0.5.x 中的 API:
/// - new_gd() 构造
/// - parse(&mut self, code: impl AsArg<GString>) -> Error
/// - execute(&mut self) -> Variant
fn cmd_execute_editor_script(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let code = args.get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: code"))?;

    let mut expr = Expression::new_gd();
    let parse_err = expr.parse(code);

    if parse_err != Error::OK {
        let err_msg = format!("脚本解析失败 (Error code: {:?})", parse_err);
        return Err(McpError::invalid_params(&err_msg));
    }

    // 执行表达式
    let result = expr.execute();
    let output_str = if result.is_nil() {
        "null".to_string()
    } else {
        result.to::<String>()
    };

    Ok(serde_json::json!({
        "output": [output_str],
        "return_value": output_str,
    }))
}
