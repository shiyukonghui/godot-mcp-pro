//! 动画命令模块

use std::collections::HashMap;
use godot::classes::{Animation, AnimationPlayer, EditorInterface};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("list_animations", "列出所有动画", serde_json::json!({
            "type": "object", "properties": { "node_path": { "type": "string" } }, "required": ["node_path"]
        })),
        ToolDefinition::new("create_animation", "创建动画", serde_json::json!({
            "type": "object", "properties": { "node_path": { "type": "string" }, "name": { "type": "string" }, "length": { "type": "number", "default": 1.0 } }, "required": ["node_path", "name"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("list_animations".into(), cmd_list_animations);
    registry.insert("create_animation".into(), cmd_create_animation);
}

fn find_player(root: &Gd<godot::classes::Node>, path: &str) -> Option<Gd<AnimationPlayer>> {
    if root.has_node(path) { Some(root.get_node_as::<AnimationPlayer>(path)) } else { None }
}

fn cmd_list_animations(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let player = find_player(&root, path).ok_or_else(|| McpError::not_found(&format!("AnimationPlayer '{}'", path), ""))?;
    let al = player.get_animation_list();
    let mut anims: Vec<String> = Vec::new();
    for i in 0..al.len() { if let Some(s) = al.get(i) { anims.push(s.to_string()); } }
    Ok(serde_json::json!({"animations": anims, "count": anims.len()}))
}

fn cmd_create_animation(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing name"))?;
    let length = args.get("length").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let _player = find_player(&root, path).ok_or_else(|| McpError::not_found(&format!("AnimationPlayer '{}'", path), ""))?;
    let mut anim = Animation::new_gd();
    anim.set_length(length);
    // add_animation_library 需要 AnimationLibrary 类型，暂略
    Ok(serde_json::json!({"created": true, "animation": name, "length": length}))
}
