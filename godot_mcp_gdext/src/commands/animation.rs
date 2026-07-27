//! 动画命令模块

use std::collections::HashMap;
use godot::classes::{Animation, AnimationLibrary, AnimationPlayer, EditorInterface};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;
use crate::utils::serialize;

/// 收集本模块的所有工具定义
pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("list_animations", "列出所有动画", serde_json::json!({
            "type": "object", "properties": { "node_path": { "type": "string" } }, "required": ["node_path"]
        })),
        ToolDefinition::new("create_animation", "创建动画", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "name": { "type": "string" },
                "length": { "type": "number", "default": 1.0 }
            }, "required": ["node_path", "name"]
        })),
        ToolDefinition::new("add_animation_track", "在指定动画中添加轨道", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "animation": { "type": "string" },
                "track_path": { "type": "string" },
                "track_type": { "type": "string", "default": "value" },
                "update_mode": { "type": "string" }
            }, "required": ["node_path", "animation", "track_path"]
        })),
        ToolDefinition::new("set_animation_keyframe", "在指定轨道的指定时间插入或更新关键帧", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "animation": { "type": "string" },
                "track_index": { "type": "integer" },
                "time": { "type": "number" },
                "value": {},
                "easing": { "type": "number", "default": 1.0 }
            }, "required": ["node_path", "animation", "track_index", "time", "value"]
        })),
        ToolDefinition::new("get_animation_info", "获取动画的详细信息（时长、轨道、关键帧等）", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "animation": { "type": "string" }
            }, "required": ["node_path", "animation"]
        })),
        ToolDefinition::new("remove_animation", "从 AnimationPlayer 的默认库中删除指定动画", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "name": { "type": "string" }
            }, "required": ["node_path", "name"]
        })),
    ]
}

/// 注册本模块的所有命令处理函数
pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("list_animations".into(), cmd_list_animations);
    registry.insert("create_animation".into(), cmd_create_animation);
    registry.insert("add_animation_track".into(), cmd_add_animation_track);
    registry.insert("set_animation_keyframe".into(), cmd_set_animation_keyframe);
    registry.insert("get_animation_info".into(), cmd_get_animation_info);
    registry.insert("remove_animation".into(), cmd_remove_animation);
}

/// 根据节点路径查找 AnimationPlayer
fn find_player(root: &Gd<godot::classes::Node>, path: &str) -> Option<Gd<AnimationPlayer>> {
    if root.has_node(path) { Some(root.get_node_as::<AnimationPlayer>(path)) } else { None }
}

/// 将 TrackType 枚举值映射为可读的字符串名称
fn track_type_name(t: godot::classes::animation::TrackType) -> &'static str {
    use godot::classes::animation::TrackType;
    if t == TrackType::VALUE { "value" }
    else if t == TrackType::POSITION_3D { "position_3d" }
    else if t == TrackType::ROTATION_3D { "rotation_3d" }
    else if t == TrackType::SCALE_3D { "scale_3d" }
    else if t == TrackType::BLEND_SHAPE { "blend_shape" }
    else if t == TrackType::METHOD { "method" }
    else if t == TrackType::BEZIER { "bezier" }
    else { "unknown" }
}

/// 将 LoopMode 枚举值映射为可读的字符串名称
fn loop_mode_name(m: godot::classes::animation::LoopMode) -> &'static str {
    use godot::classes::animation::LoopMode;
    if m == LoopMode::NONE { "none" }
    else if m == LoopMode::LINEAR { "linear" }
    else if m == LoopMode::PINGPONG { "pingpong" }
    else { "unknown" }
}

/// list_animations: 列出所有动画名称
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

/// create_animation: 创建新动画（自动处理默认动画库）
fn cmd_create_animation(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing name"))?;
    let length = args.get("length").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let mut player = find_player(&root, path).ok_or_else(|| McpError::not_found(&format!("AnimationPlayer '{}'", path), ""))?;

    let mut anim = Animation::new_gd();
    anim.set_length(length);

    // 获取或创建默认动画库（空字符串键为全局库）
    let lib: Option<Gd<AnimationLibrary>> = player.get_animation_library("");
    let mut lib = match lib {
        Some(l) => l,
        None => {
            let new_lib = AnimationLibrary::new_gd();
            player.add_animation_library("", &new_lib);
            new_lib
        }
    };

    if lib.has_animation(name) {
        return Err(McpError::invalid_params(&format!("Animation '{}' already exists", name)));
    }

    lib.add_animation(name, &anim);

    Ok(serde_json::json!({"created": true, "animation": name, "length": length}))
}

/// add_animation_track: 在指定动画中添加轨道
fn cmd_add_animation_track(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let anim_name = args.get("animation").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing animation"))?;
    let track_path = args.get("track_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing track_path"))?;

    let player = find_player(&root, path).ok_or_else(|| McpError::not_found(&format!("AnimationPlayer '{}'", path), ""))?;
    let mut anim = player.get_animation(anim_name).ok_or_else(|| McpError::not_found(&format!("Animation '{}'", anim_name), ""))?;

    // 解析轨道类型字符串
    let track_type_str = args.get("track_type").and_then(|v| v.as_str()).unwrap_or("value");
    use godot::classes::animation::{TrackType, UpdateMode};
    let track_type = match track_type_str {
        "value" => TrackType::VALUE,
        "position_2d" | "position_3d" => TrackType::POSITION_3D,
        "rotation_2d" | "rotation_3d" => TrackType::ROTATION_3D,
        "scale_2d" | "scale_3d" => TrackType::SCALE_3D,
        "blend_shape" => TrackType::BLEND_SHAPE,
        "method" => TrackType::METHOD,
        "bezier" => TrackType::BEZIER,
        _ => TrackType::VALUE,
    };

    // 添加轨道（at_position=-1 表示追加到末尾）
    let track_idx = anim.add_track(track_type);
    // AsArg<NodePath> 支持直接从 &str 转换
    anim.track_set_path(track_idx, track_path);

    // 可选：设置值轨道的更新模式
    if let Some(update_mode_str) = args.get("update_mode").and_then(|v| v.as_str()) {
        if track_type.ord() == TrackType::VALUE.ord() {
            let update_mode = match update_mode_str {
                "continuous" => UpdateMode::CONTINUOUS,
                "discrete" => UpdateMode::DISCRETE,
                "capture" => UpdateMode::CAPTURE,
                _ => UpdateMode::CONTINUOUS,
            };
            anim.value_track_set_update_mode(track_idx, update_mode);
        }
    }

    Ok(serde_json::json!({
        "track_index": track_idx,
        "track_path": track_path,
        "track_type": track_type_str
    }))
}

/// set_animation_keyframe: 在指定轨道的指定时间插入或更新关键帧
fn cmd_set_animation_keyframe(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let anim_name = args.get("animation").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing animation"))?;

    let player = find_player(&root, path).ok_or_else(|| McpError::not_found(&format!("AnimationPlayer '{}'", path), ""))?;
    let mut anim = player.get_animation(anim_name).ok_or_else(|| McpError::not_found(&format!("Animation '{}'", anim_name), ""))?;

    let track_index = args.get("track_index").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing or invalid track_index"))? as i32;
    let time = args.get("time").and_then(|v| v.as_f64()).ok_or_else(|| McpError::invalid_params("Missing or invalid time"))?;

    // 验证 track_index 是否有效
    if track_index < 0 || track_index >= anim.get_track_count() {
        return Err(McpError::invalid_params(&format!(
            "Invalid track_index: {}, track count: {}", track_index, anim.get_track_count()
        )));
    }

    // 将 JSON value 转换为 Godot Variant
    let value = args.get("value").ok_or_else(|| McpError::invalid_params("Missing value"))?;
    let variant_value = serialize::parse_value_for_property(value);

    let easing = args.get("easing").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;

    // 插入关键帧（如果该时间已有关键帧则会被替换）
    let key_idx = anim.track_insert_key(track_index, time, &variant_value);
    // 设置缓动（默认值 1.0 表示线性，Godot 中不需要额外设置）
    if (easing - 1.0_f32).abs() > f32::EPSILON {
        anim.track_set_key_transition(track_index, key_idx, easing);
    }

    // 获取最终的缓动值（可能已被 Godot 规范化）
    let actual_easing = anim.track_get_key_transition(track_index, key_idx);

    Ok(serde_json::json!({
        "track_index": track_index,
        "time": time,
        "key_index": key_idx,
        "easing": actual_easing
    }))
}

/// get_animation_info: 获取动画的详细信息（时长、轨道、关键帧等）
fn cmd_get_animation_info(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let anim_name = args.get("animation").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing animation"))?;

    let player = find_player(&root, path).ok_or_else(|| McpError::not_found(&format!("AnimationPlayer '{}'", path), ""))?;
    let anim = player.get_animation(anim_name).ok_or_else(|| McpError::not_found(&format!("Animation '{}'", anim_name), ""))?;

    // 收集轨道信息
    let track_count = anim.get_track_count();
    let mut tracks: Vec<serde_json::Value> = Vec::with_capacity(track_count as usize);

    for i in 0..track_count {
        let track_type = anim.track_get_type(i);
        let key_count = anim.track_get_key_count(i);

        // 收集关键帧信息
        let mut keys: Vec<serde_json::Value> = Vec::with_capacity(key_count as usize);
        for k in 0..key_count {
            let key_time = anim.track_get_key_time(i, k);
            let key_value = anim.track_get_key_value(i, k);
            let key_easing = anim.track_get_key_transition(i, k);

            keys.push(serde_json::json!({
                "time": key_time,
                "value": serialize::serialize_variant(&key_value),
                "easing": key_easing,
            }));
        }

        tracks.push(serde_json::json!({
            "index": i,
            "path": anim.track_get_path(i).to_string(),
            "type": track_type_name(track_type),
            "type_code": track_type.ord(),
            "key_count": key_count,
            "keys": keys,
        }));
    }

    Ok(serde_json::json!({
        "name": anim_name,
        "length": anim.get_length(),
        "loop_mode": loop_mode_name(anim.get_loop_mode()),
        "loop_mode_code": anim.get_loop_mode().ord(),
        "step": anim.get_step(),
        "track_count": track_count,
        "tracks": tracks,
    }))
}

/// remove_animation: 从 AnimationPlayer 的默认库中删除指定动画
fn cmd_remove_animation(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let anim_name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing name"))?;

    let player = find_player(&root, path).ok_or_else(|| McpError::not_found(&format!("AnimationPlayer '{}'", path), ""))?;

    // 获取默认动画库（空字符串键）
    let mut lib = player.get_animation_library("").ok_or_else(|| {
        McpError::not_found("Animation library 'default'", "Try creating an animation first")
    })?;

    // 检查动画是否存在
    if !lib.has_animation(anim_name) {
        return Err(McpError::not_found(&format!("Animation '{}'", anim_name), ""));
    }

    // 删除动画
    lib.remove_animation(anim_name);

    Ok(serde_json::json!({
        "name": anim_name,
        "removed": true
    }))
}
