//! 项目信息命令模块

use std::collections::HashMap;

use godot::classes::{EditorInterface, ProjectSettings};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("get_project_info", "获取项目信息", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("get_project_info".into(), cmd_get_project_info);
}

fn cmd_get_project_info(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let project = ProjectSettings::singleton();
    let editor = EditorInterface::singleton();

    let project_name = get_setting(&project, "application/config/name");
    let version = get_setting(&project, "application/config/version");

    let screen_size = editor.get_base_control()
        .map(|ctrl| ctrl.get_size())
        .map(|s| serde_json::json!({"width": s.x, "height": s.y}))
        .unwrap_or(serde_json::json!({"width": 0, "height": 0}));

    Ok(serde_json::json!({
        "project_name": project_name,
        "version": version,
        "editor_screen_size": screen_size,
    }))
}

fn get_setting(project: &ProjectSettings, key: &str) -> String {
    if project.has_setting(key) {
        project.get_setting(key).to::<String>()
    } else {
        String::new()
    }
}
