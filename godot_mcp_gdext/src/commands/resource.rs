//! 资源命令模块

use std::collections::HashMap;
use godot::classes::{ProjectSettings, ResourceLoader};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("read_resource", "读取资源文件", serde_json::json!({
            "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"]
        })),
        ToolDefinition::new("add_autoload", "注册自动加载", serde_json::json!({
            "type": "object", "properties": { "name": { "type": "string" }, "path": { "type": "string" } }, "required": ["name", "path"]
        })),
        ToolDefinition::new("remove_autoload", "移除自动加载", serde_json::json!({
            "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("read_resource".into(), cmd_read_resource);
    registry.insert("add_autoload".into(), cmd_add_autoload);
    registry.insert("remove_autoload".into(), cmd_remove_autoload);
}

fn cmd_read_resource(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let mut rl = ResourceLoader::singleton();
    let res = rl.load(path);
    match res {
        Some(r) => Ok(serde_json::json!({"path": path, "type": r.get_class().to_string(), "loaded": true})),
        None => Err(McpError::not_found(&format!("Resource '{}'", path), "")),
    }
}

fn cmd_add_autoload(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing name"))?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let mut ps = ProjectSettings::singleton();
    ps.set_setting(&format!("autoload/{}", name), &Variant::from(format!("*{}", path)));
    ps.save();
    Ok(serde_json::json!({"autoload": name, "path": path, "added": true}))
}

fn cmd_remove_autoload(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing name"))?;
    let mut ps = ProjectSettings::singleton();
    let key = format!("autoload/{}", name);
    if ps.has_setting(&key) { ps.set_setting(&key, &Variant::nil()); ps.save(); }
    Ok(serde_json::json!({"autoload": name, "removed": true}))
}
