//! 动画树命令模块
//! 对应原 GDScript 插件 animation_tree_commands.gd

use std::collections::HashMap;
use godot::classes::{
    AnimationNode, AnimationNodeBlendTree, AnimationNodeStateMachine,
    AnimationNodeStateMachineTransition, EditorInterface, Node,
};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("create_animation_tree", "创建 AnimationTree 节点", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "父节点路径" },
                "animation_player_path": { "type": "string", "description": "AnimationPlayer 节点路径" },
                "name": { "type": "string", "default": "AnimationTree" }
            }, "required": ["node_path"]
        })),
        ToolDefinition::new("get_animation_tree_structure", "获取动画树结构", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" }
            }, "required": ["node_path"]
        })),
        ToolDefinition::new("add_state_machine_state", "添加状态机状态", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "state_name": { "type": "string" },
                "state_machine_path": { "type": "string", "default": "" },
                "state_type": { "type": "string", "default": "animation" },
                "position_x": { "type": "number", "default": 0 },
                "position_y": { "type": "number", "default": 0 }
            }, "required": ["node_path", "state_name"]
        })),
        ToolDefinition::new("remove_state_machine_state", "删除状态机状态", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "state_name": { "type": "string" },
                "state_machine_path": { "type": "string", "default": "" }
            }, "required": ["node_path", "state_name"]
        })),
        ToolDefinition::new("add_state_machine_transition", "添加状态机过渡", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "from_state": { "type": "string" },
                "to_state": { "type": "string" },
                "state_machine_path": { "type": "string", "default": "" },
                "switch_mode": { "type": "string", "default": "immediate" },
                "advance_mode": { "type": "string", "default": "enabled" }
            }, "required": ["node_path", "from_state", "to_state"]
        })),
        ToolDefinition::new("remove_state_machine_transition", "删除状态机过渡", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "from_state": { "type": "string" },
                "to_state": { "type": "string" },
                "state_machine_path": { "type": "string", "default": "" }
            }, "required": ["node_path", "from_state", "to_state"]
        })),
        ToolDefinition::new("set_blend_tree_node", "设置混合树节点", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "blend_tree_state": { "type": "string", "description": "BlendTree 所在的状态名称" },
                "bt_node_name": { "type": "string" },
                "bt_node_type": { "type": "string" },
                "state_machine_path": { "type": "string", "default": "" },
                "position_x": { "type": "number", "default": 0 },
                "position_y": { "type": "number", "default": 0 }
            }, "required": ["node_path", "blend_tree_state", "bt_node_name", "bt_node_type"]
        })),
        ToolDefinition::new("set_tree_parameter", "设置动画树参数", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "parameter": { "type": "string" },
                "value": {}
            }, "required": ["node_path", "parameter", "value"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("create_animation_tree".into(), cmd_create_animation_tree);
    registry.insert("get_animation_tree_structure".into(), cmd_get_animation_tree_structure);
    registry.insert("add_state_machine_state".into(), cmd_add_state_machine_state);
    registry.insert("remove_state_machine_state".into(), cmd_remove_state_machine_state);
    registry.insert("add_state_machine_transition".into(), cmd_add_state_machine_transition);
    registry.insert("remove_state_machine_transition".into(), cmd_remove_state_machine_transition);
    registry.insert("set_blend_tree_node".into(), cmd_set_blend_tree_node);
    registry.insert("set_tree_parameter".into(), cmd_set_tree_parameter);
}

/// 获取被编辑场景的根节点
fn find_root() -> Result<Gd<Node>, McpError> {
    EditorInterface::singleton().get_edited_scene_root().ok_or_else(|| McpError::no_scene())
}

/// 按路径查找节点
fn find_node_by_path(root: &Gd<Node>, path: &str) -> Result<Gd<Node>, McpError> {
    if path == "." || path == root.get_name().to_string() {
        Ok(root.clone())
    } else if root.has_node(path) {
        Ok(root.get_node_as::<Node>(path))
    } else {
        Err(McpError::not_found(&format!("Node '{}'", path), ""))
    }
}

/// 查找 AnimationTree 节点
fn find_animation_tree(root: &Gd<Node>, path: &str) -> Result<Gd<godot::classes::AnimationTree>, McpError> {
    let node = find_node_by_path(root, path)?;
    node.try_cast::<godot::classes::AnimationTree>()
        .map_err(|_| McpError::invalid_params(&format!("Node '{}' is not an AnimationTree", path)))
}

/// 创建 AnimationTree 节点并关联 AnimationPlayer
fn cmd_create_animation_tree(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let mut parent = find_node_by_path(&root, node_path)?;

    let anim_player_path = args.get("animation_player_path").and_then(|v| v.as_str()).unwrap_or("");
    let tree_name = args.get("name").and_then(|v| v.as_str()).unwrap_or("AnimationTree");

    // 创建 AnimationTree 节点
    let mut tree = godot::classes::AnimationTree::new_alloc();
    tree.set_name(tree_name);

    // 创建根状态机
    let state_machine = godot::classes::AnimationNodeStateMachine::new_gd();
    tree.set_tree_root(&state_machine);

    // 关联 AnimationPlayer
    if !anim_player_path.is_empty() {
        tree.set("anim_player", &Variant::from(NodePath::from(anim_player_path)));
    }

    // 添加到场景
    parent.add_child(&tree);
    tree.set("owner", &Variant::from(root.clone()));

    Ok(serde_json::json!({
        "name": tree_name,
        "node_path": format!("{}/{}", node_path, tree_name),
        "anim_player": anim_player_path,
        "created": true,
    }))
}

/// 获取动画树结构
fn cmd_get_animation_tree_structure(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let tree = find_animation_tree(&root, node_path)?;

    let active: bool = tree.get("active").try_to().unwrap_or(false);
    let anim_player = tree.get("anim_player").to_string();

    // 获取树根节点信息
    let root_node = tree.get_tree_root();
    let root_type = root_node.as_ref().map(|n| n.get_class().to_string()).unwrap_or_else(|| "null".into());

    Ok(serde_json::json!({
        "node_path": node_path,
        "active": active,
        "anim_player": anim_player,
        "root_type": root_type,
    }))
}

/// 添加状态机状态
fn cmd_add_state_machine_state(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let state_name = args.get("state_name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing state_name"))?;
    let sm_path = args.get("state_machine_path").and_then(|v| v.as_str()).unwrap_or("");

    let tree = find_animation_tree(&root, node_path)?;
    let mut sm = get_state_machine(&tree, sm_path)?;
    if sm.has_node(state_name) {
        return Err(McpError::invalid_params(&format!("State '{}' already exists", state_name)));
    }

    // 创建状态节点
    let state_type = args.get("state_type").and_then(|v| v.as_str()).unwrap_or("animation");
    let pos_x = args.get("position_x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let pos_y = args.get("position_y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

    let node: Gd<AnimationNode> = match state_type {
        "animation" => {
            // 创建 AnimationNodeAnimation
            let mut anim_node = godot::classes::AnimationNodeAnimation::new_gd();
            if let Some(anim_name) = args.get("animation").and_then(|v| v.as_str()) {
                if !anim_name.is_empty() {
                    anim_node.set("animation", &Variant::from(StringName::from(anim_name)));
                }
            }
            anim_node.upcast()
        }
        "blend_tree" => {
            godot::classes::AnimationNodeBlendTree::new_gd().upcast()
        }
        "state_machine" => {
            godot::classes::AnimationNodeStateMachine::new_gd().upcast()
        }
        _ => return Err(McpError::invalid_params(&format!("Unknown state_type: '{}'", state_type))),
    };

    sm.add_node(state_name, &node);

    Ok(serde_json::json!({
        "state_name": state_name,
        "state_type": state_type,
        "position": {"x": pos_x, "y": pos_y},
        "added": true,
    }))
}

/// 删除状态机状态
fn cmd_remove_state_machine_state(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let state_name = args.get("state_name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing state_name"))?;
    let sm_path = args.get("state_machine_path").and_then(|v| v.as_str()).unwrap_or("");

    let tree = find_animation_tree(&root, node_path)?;
    let mut sm = get_state_machine(&tree, sm_path)?;
    if !sm.has_node(state_name) {
        return Err(McpError::not_found(&format!("State '{}'", state_name), ""));
    }

    sm.remove_node(state_name);

    Ok(serde_json::json!({"state_name": state_name, "removed": true}))
}

/// 添加状态机过渡
fn cmd_add_state_machine_transition(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let from_state = args.get("from_state").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing from_state"))?;
    let to_state = args.get("to_state").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing to_state"))?;
    let sm_path = args.get("state_machine_path").and_then(|v| v.as_str()).unwrap_or("");

    let tree = find_animation_tree(&root, node_path)?;
    let mut sm = get_state_machine(&tree, sm_path)?;
    let mut transition = AnimationNodeStateMachineTransition::new_gd();

    // switch_mode
    let switch_mode = args.get("switch_mode").and_then(|v| v.as_str()).unwrap_or("immediate");
    match switch_mode {
        "at_end" => { transition.set("switch_mode", &Variant::from(0i32)); }
        "sync" => { transition.set("switch_mode", &Variant::from(2i32)); }
        _ => { transition.set("switch_mode", &Variant::from(1i32)); } // immediate
    }

    // advance_mode
    let advance_mode = args.get("advance_mode").and_then(|v| v.as_str()).unwrap_or("enabled");
    match advance_mode {
        "disabled" => { transition.set("advance_mode", &Variant::from(0i32)); }
        "auto" => { transition.set("advance_mode", &Variant::from(2i32)); }
        _ => { transition.set("advance_mode", &Variant::from(1i32)); } // enabled
    }

    // advance_expression
    if let Some(expr) = args.get("advance_expression").and_then(|v| v.as_str()) {
        if !expr.is_empty() {
            transition.set("advance_expression", &Variant::from(expr));
        }
    }

    // xfade_time
    if let Some(xfade) = args.get("xfade_time").and_then(|v| v.as_f64()) {
        transition.set("xfade_time", &Variant::from(xfade as f32));
    }

    sm.add_transition(from_state, to_state, &transition);

    Ok(serde_json::json!({
        "from": from_state,
        "to": to_state,
        "switch_mode": switch_mode,
        "advance_mode": advance_mode,
        "added": true,
    }))
}

/// 删除状态机过渡
fn cmd_remove_state_machine_transition(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let from_state = args.get("from_state").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing from_state"))?;
    let to_state = args.get("to_state").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing to_state"))?;
    let sm_path = args.get("state_machine_path").and_then(|v| v.as_str()).unwrap_or("");

    let tree = find_animation_tree(&root, node_path)?;
    let mut sm = get_state_machine(&tree, sm_path)?;

    sm.remove_transition(from_state, to_state);

    Ok(serde_json::json!({"from": from_state, "to": to_state, "removed": true}))
}

/// 设置混合树节点
fn cmd_set_blend_tree_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let bt_state = args.get("blend_tree_state").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing blend_tree_state"))?;
    let bt_node_name = args.get("bt_node_name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing bt_node_name"))?;
    let bt_node_type = args.get("bt_node_type").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing bt_node_type"))?;
    let sm_path = args.get("state_machine_path").and_then(|v| v.as_str()).unwrap_or("");

    let tree = find_animation_tree(&root, node_path)?;
    let sm = get_state_machine(&tree, sm_path)?;

    // 获取 BlendTree 节点
    if !sm.has_node(bt_state) {
        return Err(McpError::not_found(&format!("State '{}' in state machine", bt_state), ""));
    }

    let bt_node = sm.get_node(bt_state).ok_or_else(|| McpError::not_found(&format!("State '{}'", bt_state), ""))?;
    let mut bt = bt_node.try_cast::<AnimationNodeBlendTree>()
        .map_err(|_| McpError::invalid_params(&format!("State '{}' is not an AnimationNodeBlendTree", bt_state)))?;

    let pos_x = args.get("position_x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let pos_y = args.get("position_y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

    // 创建节点
    let node: Gd<AnimationNode> = match bt_node_type {
        "Animation" => {
            let mut anim_node = godot::classes::AnimationNodeAnimation::new_gd();
            if let Some(anim_name) = args.get("animation").and_then(|v| v.as_str()) {
                if !anim_name.is_empty() {
                    anim_node.set("animation", &Variant::from(StringName::from(anim_name)));
                }
            }
            anim_node.upcast()
        }
        "Add2" => godot::classes::AnimationNodeAdd2::new_gd().upcast(),
        "Blend2" => godot::classes::AnimationNodeBlend2::new_gd().upcast(),
        "Add3" => godot::classes::AnimationNodeAdd3::new_gd().upcast(),
        "Blend3" => godot::classes::AnimationNodeBlend3::new_gd().upcast(),
        "TimeScale" => godot::classes::AnimationNodeTimeScale::new_gd().upcast(),
        "TimeSeek" => godot::classes::AnimationNodeTimeSeek::new_gd().upcast(),
        "Transition" => godot::classes::AnimationNodeTransition::new_gd().upcast(),
        "OneShot" => godot::classes::AnimationNodeOneShot::new_gd().upcast(),
        "Sub2" => godot::classes::AnimationNodeSub2::new_gd().upcast(),
        _ => return Err(McpError::invalid_params(&format!("Unknown bt_node_type: '{}'", bt_node_type))),
    };

    // 如果已存在同名节点, 先移除
    if bt.has_node(bt_node_name) {
        bt.remove_node(bt_node_name);
    }

    bt.add_node(bt_node_name, &node);

    Ok(serde_json::json!({
        "blend_tree_state": bt_state,
        "bt_node_name": bt_node_name,
        "bt_node_type": bt_node_type,
        "position": {"x": pos_x, "y": pos_y},
        "added": true,
    }))
}

/// 设置动画树参数
fn cmd_set_tree_parameter(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let parameter = args.get("parameter").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing parameter"))?;
    let value = args.get("value").ok_or_else(|| McpError::invalid_params("Missing value"))?;

    let mut tree = find_animation_tree(&root, node_path)?;
    let full_param = if parameter.starts_with("parameters/") {
        parameter.to_string()
    } else {
        format!("parameters/{}", parameter)
    };

    // 设置参数值
    match value {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                tree.set(&full_param, &Variant::from(f as f32));
            } else if let Some(i) = n.as_i64() {
                tree.set(&full_param, &Variant::from(i as f32));
            }
        }
        serde_json::Value::Bool(b) => {
            tree.set(&full_param, &Variant::from(*b));
        }
        serde_json::Value::String(s) => {
            tree.set(&full_param, &Variant::from(s.as_str()));
        }
        _ => {
            tree.set(&full_param, &Variant::from(value.to_string()));
        }
    }

    Ok(serde_json::json!({
        "parameter": full_param,
        "set": true,
    }))
}

/// 从 AnimationTree 中解析状态机
fn get_state_machine(tree: &Gd<godot::classes::AnimationTree>, sm_path: &str) -> Result<Gd<AnimationNodeStateMachine>, McpError> {
    let root_node = tree.get_tree_root().ok_or_else(|| McpError::internal("AnimationTree has no tree_root"))?;
    let sm = root_node.try_cast::<AnimationNodeStateMachine>()
        .map_err(|_| McpError::invalid_params("AnimationTree root is not an AnimationNodeStateMachine"))?;

    if sm_path.is_empty() || sm_path == "." {
        return Ok(sm);
    }

    // 沿路径向下查找嵌套状态机
    let parts: Vec<&str> = sm_path.split('/').collect();
    let mut current = sm;
    for part in parts {
        if !current.has_node(part) {
            return Err(McpError::not_found(&format!("State machine node '{}' in path '{}'", part, sm_path), ""));
        }
        let child = current.get_node(part).ok_or_else(|| McpError::not_found(&format!("State machine node '{}' in path '{}'", part, sm_path), ""))?;
        current = child.try_cast::<AnimationNodeStateMachine>()
            .map_err(|_| McpError::invalid_params(&format!("Node '{}' is not a StateMachine", part)))?;
    }

    Ok(current)
}
