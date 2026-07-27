//! 物理命令模块

use std::collections::HashMap;
use godot::classes::{EditorInterface, Node, RayCast2D, RayCast3D};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("add_raycast", "添加射线检测节点", serde_json::json!({
            "type": "object", "properties": {
                "parent_path": { "type": "string", "default": "." },
                "name": { "type": "string", "default": "RayCast" },
                "dimension": { "type": "string", "default": "2d" }
            }, "required": []
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("add_raycast".into(), cmd_add_raycast);
}

fn cmd_add_raycast(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let parent_path = args.get("parent_path").and_then(|v| v.as_str()).unwrap_or(".");
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("RayCast");
    let dim = args.get("dimension").and_then(|v| v.as_str()).unwrap_or("2d");

    let mut parent = if parent_path == "." || parent_path == root.get_name().to_string() { root.clone() }
                     else { root.get_node_as::<Node>(parent_path) };

    match dim {
        "2d" => { let mut r = RayCast2D::new_alloc(); r.set_name(name); parent.add_child(&r); r.set("owner", &Variant::from(root.clone())); }
        _ => { let mut r = RayCast3D::new_alloc(); r.set_name(name); parent.add_child(&r); r.set("owner", &Variant::from(root.clone())); }
    }
    Ok(serde_json::json!({"added": true, "name": name}))
}
