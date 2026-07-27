//! Shader 命令模块

use std::collections::HashMap;
use godot::classes::file_access::ModeFlags;
use godot::classes::{DirAccess, FileAccess};
use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("read_shader", "读取着色器文件", serde_json::json!({
            "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"]
        })),
        ToolDefinition::new("create_shader", "创建着色器文件", serde_json::json!({
            "type": "object", "properties": { "path": { "type": "string" }, "shader_type": { "type": "string", "default": "shader_type spatial;" } }, "required": ["path"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("read_shader".into(), cmd_read_shader);
    registry.insert("create_shader".into(), cmd_create_shader);
}

fn cmd_read_shader(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let mut file = FileAccess::open(path, ModeFlags::READ).ok_or_else(|| McpError::not_found(&format!("File '{}'", path), ""))?;
    let content = file.get_as_text().to_string(); file.close();
    Ok(serde_json::json!({"path": path, "content": content}))
}

fn cmd_create_shader(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let stype = args.get("shader_type").and_then(|v| v.as_str()).unwrap_or("shader_type spatial;");
    let code = format!("{}\n\nvoid fragment() {{\n}}\n", stype);
    if let Some(p) = std::path::Path::new(path).parent() { let _ = DirAccess::make_dir_recursive_absolute(&p.to_string_lossy().to_string()); }
    let mut f = FileAccess::open(path, ModeFlags::WRITE).ok_or_else(|| McpError::internal("Cannot create file"))?;
    f.store_string(&code); f.close();
    Ok(serde_json::json!({"path": path, "created": true}))
}
