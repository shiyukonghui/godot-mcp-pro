//! 音频命令模块

use std::collections::HashMap;
use godot::classes::{AudioServer, EditorInterface, Node};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("add_audio_player", "添加音频播放器", serde_json::json!({
            "type": "object", "properties": {
                "parent_path": { "type": "string", "default": "." },
                "name": { "type": "string", "default": "AudioPlayer" }
            }, "required": []
        })),
        ToolDefinition::new("get_audio_info", "获取音频信息", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("add_audio_player".into(), cmd_add_audio_player);
    registry.insert("get_audio_info".into(), cmd_get_audio_info);
}

fn find_root() -> Result<Gd<Node>, McpError> {
    EditorInterface::singleton().get_edited_scene_root().ok_or_else(|| McpError::no_scene())
}

fn find_parent(root: &Gd<Node>, path: &str) -> Result<Gd<Node>, McpError> {
    if path == "." || path == root.get_name().to_string() { Ok(root.clone()) }
    else if root.has_node(path) { Ok(root.get_node_as::<Node>(path)) }
    else { Err(McpError::not_found(&format!("Node '{}'", path), "")) }
}

fn cmd_add_audio_player(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let parent_path = args.get("parent_path").and_then(|v| v.as_str()).unwrap_or(".");
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("AudioPlayer");
    let mut parent = find_parent(&root, parent_path)?;
    let mut player = godot::classes::AudioStreamPlayer2D::new_alloc();
    player.set_name(name);
    parent.add_child(&player);
    player.set("owner", &Variant::from(root.clone()));
    Ok(serde_json::json!({"added": true, "name": name}))
}

fn cmd_get_audio_info(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let audio = AudioServer::singleton();
    Ok(serde_json::json!({
        "bus_count": audio.get_bus_count(),
        "output_latency": audio.get_output_latency(),
    }))
}
