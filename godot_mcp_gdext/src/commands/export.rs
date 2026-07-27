//! 导出命令模块

use std::collections::HashMap;
use godot::classes::ProjectSettings;
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("get_export_info", "获取导出信息", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("get_export_info".into(), cmd_get_export_info);
}

fn cmd_get_export_info(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let ps = ProjectSettings::singleton();
    let name = if ps.has_setting("application/config/name") {
        ps.get_setting("application/config/name").to::<String>()
    } else { String::new() };
    Ok(serde_json::json!({"project_name": name}))
}
