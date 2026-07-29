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
        // 获取音频总线布局
        ToolDefinition::new("get_audio_bus_layout", "获取音频总线布局", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        // 添加音频总线
        ToolDefinition::new("add_audio_bus", "添加音频总线", serde_json::json!({
            "type": "object", "properties": {
                "name": { "type": "string" },
                "after_bus_index": { "type": "integer", "default": -1 }
            }, "required": ["name"]
        })),
        // 设置音频总线属性
        ToolDefinition::new("set_audio_bus", "设置音频总线属性", serde_json::json!({
            "type": "object", "properties": {
                "bus_index": { "type": "integer" },
                "property": { "type": "string" },
                "value": { "description": "属性值" }
            }, "required": ["bus_index", "property", "value"]
        })),
        // 添加音频总线效果
        ToolDefinition::new("add_audio_bus_effect", "添加音频总线效果", serde_json::json!({
            "type": "object", "properties": {
                "bus_index": { "type": "integer" },
                "effect_type": { "type": "string" },
                "name": { "type": "string" }
            }, "required": ["bus_index", "effect_type"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("add_audio_player".into(), cmd_add_audio_player);
    registry.insert("get_audio_info".into(), cmd_get_audio_info);
    registry.insert("get_audio_bus_layout".into(), cmd_get_audio_bus_layout);
    registry.insert("add_audio_bus".into(), cmd_add_audio_bus);
    registry.insert("set_audio_bus".into(), cmd_set_audio_bus);
    registry.insert("add_audio_bus_effect".into(), cmd_add_audio_bus_effect);
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

/// 获取音频总线布局
fn cmd_get_audio_bus_layout(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let audio = AudioServer::singleton();
    let bus_count = audio.get_bus_count();
    let mut buses: Vec<serde_json::Value> = Vec::new();

    for i in 0..bus_count {
        let name = audio.get_bus_name(i).to_string();
        let volume_db = audio.get_bus_volume_db(i);
        let is_mute = audio.is_bus_mute(i);
        let is_solo = audio.is_bus_solo(i);
        let bypass = audio.is_bus_bypassing_effects(i);
        let send = audio.get_bus_send(i).to_string();

        // 获取总线上的效果
        let effect_count = audio.get_bus_effect_count(i);
        let mut effects: Vec<serde_json::Value> = Vec::new();
        for j in 0..effect_count {
            if let Some(effect) = audio.get_bus_effect(i, j) {
                effects.push(serde_json::json!({
                    "index": j,
                    "type": effect.get_class().to_string(),
                    "enabled": audio.is_bus_effect_enabled(i, j),
                }));
            }
        }

        buses.push(serde_json::json!({
            "index": i,
            "name": name,
            "volume_db": volume_db,
            "mute": is_mute,
            "solo": is_solo,
            "bypass_effects": bypass,
            "send": send,
            "effects": effects,
        }));
    }

    Ok(serde_json::json!({"bus_count": bus_count, "buses": buses}))
}

/// 添加音频总线
fn cmd_add_audio_bus(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing name"))?;
    let after_bus_index = args.get("after_bus_index").and_then(|v| v.as_i64()).unwrap_or(-1);

    let mut audio = AudioServer::singleton();
    audio.add_bus();

    let idx = if after_bus_index < 0 { audio.get_bus_count() - 1 } else { after_bus_index as i32 };
    audio.set_bus_name(idx, name);

    Ok(serde_json::json!({"name": name, "index": idx, "bus_count": audio.get_bus_count()}))
}

/// 设置音频总线属性
fn cmd_set_audio_bus(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let bus_index = args.get("bus_index").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing bus_index"))? as i32;
    let property = args.get("property").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing property"))?;
    let value = args.get("value").ok_or_else(|| McpError::invalid_params("Missing value"))?;

    let mut audio = AudioServer::singleton();
    match property {
        "volume_db" => {
            let vol = value.as_f64().unwrap_or(0.0) as f32;
            audio.set_bus_volume_db(bus_index, vol);
        }
        "mute" => {
            let mute = value.as_bool().unwrap_or(false);
            audio.set_bus_mute(bus_index, mute);
        }
        "solo" => {
            let solo = value.as_bool().unwrap_or(false);
            audio.set_bus_solo(bus_index, solo);
        }
        "bypass_effects" => {
            let bypass = value.as_bool().unwrap_or(false);
            audio.set_bus_bypass_effects(bus_index, bypass);
        }
        "send" => {
            let send = value.as_str().unwrap_or("Master");
            audio.set_bus_send(bus_index, send);
        }
        "name" => {
            let new_name = value.as_str().unwrap_or("Bus");
            audio.set_bus_name(bus_index, new_name);
        }
        _ => return Err(McpError::invalid_params(&format!("Unknown property: {}", property))),
    }

    Ok(serde_json::json!({"bus_index": bus_index, "property": property, "set": true}))
}

/// 添加音频总线效果
fn cmd_add_audio_bus_effect(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let bus_index = args.get("bus_index").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing bus_index"))? as i32;
    let effect_type = args.get("effect_type").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing effect_type"))?;

    let mut audio = AudioServer::singleton();

    // 通过 ClassDb 实例化效果 - instantiate 返回 Variant
    let effect_var = godot::classes::ClassDb::singleton().instantiate(effect_type);
    if effect_var.is_nil() {
        return Err(McpError::invalid_params(&format!("Unknown effect type: {}", effect_type)));
    }

    // add_bus_effect 接收 Gd<AudioEffect>，使用 try_to 安全验证继承关系
    let effect_gd: Gd<godot::classes::AudioEffect> = effect_var.try_to().map_err(|_| {
        McpError::invalid_params(&format!("类型 '{}' 不是 AudioEffect 的子类", effect_type))
    })?;
    audio.add_bus_effect(bus_index, &effect_gd);

    let effect_count = audio.get_bus_effect_count(bus_index);

    Ok(serde_json::json!({
        "bus_index": bus_index,
        "effect_type": effect_type,
        "effect_index": effect_count - 1,
        "added": true
    }))
}
