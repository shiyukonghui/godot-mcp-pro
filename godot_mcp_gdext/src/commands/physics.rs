//! 物理命令模块

use std::collections::HashMap;
use godot::classes::{CollisionShape2D, CollisionShape3D, EditorInterface, Node, RayCast2D, RayCast3D};
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
        // 设置碰撞形状
        ToolDefinition::new("setup_collision", "为物理体添加碰撞形状", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "shape_type": { "type": "string", "default": "RectangleShape2D" },
                "shape_params": { "type": "object", "description": "形状参数, 如 {\"size\": {\"x\": 32, \"y\": 32}}" }
            }, "required": ["node_path"]
        })),
        // 设置物理层
        ToolDefinition::new("set_physics_layers", "设置物理层", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "layers": { "type": "integer" },
                "layer_type": { "type": "string", "default": "collision" }
            }, "required": ["node_path", "layers"]
        })),
        // 获取物理层信息
        ToolDefinition::new("get_physics_layers", "获取物理层信息", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" }
            }, "required": ["node_path"]
        })),
        // 创建物理体
        ToolDefinition::new("setup_physics_body", "创建物理体节点", serde_json::json!({
            "type": "object", "properties": {
                "parent_path": { "type": "string", "default": "." },
                "body_type": { "type": "string", "default": "RigidBody2D" },
                "name": { "type": "string" }
            }, "required": []
        })),
        // 获取碰撞信息
        ToolDefinition::new("get_collision_info", "获取碰撞信息", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" }
            }, "required": []
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("add_raycast".into(), cmd_add_raycast);
    registry.insert("setup_collision".into(), cmd_setup_collision);
    registry.insert("set_physics_layers".into(), cmd_set_physics_layers);
    registry.insert("get_physics_layers".into(), cmd_get_physics_layers);
    registry.insert("setup_physics_body".into(), cmd_setup_physics_body);
    registry.insert("get_collision_info".into(), cmd_get_collision_info);
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

/// 设置碰撞形状 - 为物理体节点添加 CollisionShape2D/3D 子节点
fn cmd_setup_collision(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let shape_type = args.get("shape_type").and_then(|v| v.as_str()).unwrap_or("RectangleShape2D");
    let _shape_params = args.get("shape_params");

    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    if !root.has_node(node_path) {
        return Err(McpError::not_found(&format!("Node '{}'", node_path), ""));
    }
    let mut node = root.get_node_as::<Node>(node_path);

    // 根据形状类型创建对应的碰撞节点
    match shape_type {
        "RectangleShape2D" => {
            let shape = godot::classes::RectangleShape2D::new_gd();
            let mut col = CollisionShape2D::new_alloc();
            col.set_shape(&shape);
            col.set_name("CollisionShape2D");
            node.add_child(&col);
            col.set("owner", &Variant::from(root.clone()));
        }
        "CircleShape2D" => {
            let shape = godot::classes::CircleShape2D::new_gd();
            let mut col = CollisionShape2D::new_alloc();
            col.set_shape(&shape);
            col.set_name("CollisionShape2D");
            node.add_child(&col);
            col.set("owner", &Variant::from(root.clone()));
        }
        "BoxShape3D" => {
            let shape = godot::classes::BoxShape3D::new_gd();
            let mut col = CollisionShape3D::new_alloc();
            col.set_shape(&shape);
            col.set_name("CollisionShape3D");
            node.add_child(&col);
            col.set("owner", &Variant::from(root.clone()));
        }
        "SphereShape3D" => {
            let shape = godot::classes::SphereShape3D::new_gd();
            let mut col = CollisionShape3D::new_alloc();
            col.set_shape(&shape);
            col.set_name("CollisionShape3D");
            node.add_child(&col);
            col.set("owner", &Variant::from(root.clone()));
        }
        _ => {
            // 默认为2D矩形
            let shape = godot::classes::RectangleShape2D::new_gd();
            let mut col = CollisionShape2D::new_alloc();
            col.set_shape(&shape);
            col.set_name("CollisionShape2D");
            node.add_child(&col);
            col.set("owner", &Variant::from(root.clone()));
        }
    }

    Ok(serde_json::json!({"node_path": node_path, "shape_type": shape_type, "added": true}))
}

/// 设置物理层
fn cmd_set_physics_layers(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let layers = args.get("layers").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing layers"))?;
    let layer_type = args.get("layer_type").and_then(|v| v.as_str()).unwrap_or("collision");

    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    if !root.has_node(node_path) {
        return Err(McpError::not_found(&format!("Node '{}'", node_path), ""));
    }
    let mut node = root.get_node_as::<Node>(node_path);

    match layer_type {
        "collision" => node.set("collision_layer", &Variant::from(layers as i32)),
        "mask" => node.set("collision_mask", &Variant::from(layers as i32)),
        _ => node.set("collision_layer", &Variant::from(layers as i32)),
    }

    Ok(serde_json::json!({"node_path": node_path, "layers": layers, "layer_type": layer_type, "set": true}))
}

/// 获取物理层信息
fn cmd_get_physics_layers(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;

    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    if !root.has_node(node_path) {
        return Err(McpError::not_found(&format!("Node '{}'", node_path), ""));
    }
    let node = root.get_node_as::<Node>(node_path);

    let collision_layer = node.get("collision_layer").to::<i32>();
    let collision_mask = node.get("collision_mask").to::<i32>();

    Ok(serde_json::json!({
        "node_path": node_path,
        "collision_layer": collision_layer,
        "collision_mask": collision_mask
    }))
}

/// 创建物理体节点
fn cmd_setup_physics_body(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let parent_path = args.get("parent_path").and_then(|v| v.as_str()).unwrap_or(".");
    let body_type = args.get("body_type").and_then(|v| v.as_str()).unwrap_or("RigidBody2D");
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("PhysicsBody");

    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let mut parent = if parent_path == "." || parent_path == root.get_name().to_string() { root.clone() }
                     else { root.get_node_as::<Node>(parent_path) };

    // 通过 ClassDb 实例化物理体
    let body = godot::classes::ClassDb::singleton().instantiate(body_type);
    if body.is_nil() {
        return Err(McpError::invalid_params(&format!("Unknown body type: {}", body_type)));
    }

    // 将 Variant 转换为 Gd<Node>
    let mut body_node = body.to::<Gd<godot::classes::Node>>();
    body_node.set_name(name);
    parent.add_child(&body_node);
    body_node.set("owner", &Variant::from(root.clone()));

    Ok(serde_json::json!({"parent_path": parent_path, "body_type": body_type, "name": name, "created": true}))
}

/// 获取碰撞信息 - 遍历节点及其子节点中的碰撞形状
fn cmd_get_collision_info(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = args.get("node_path").and_then(|v| v.as_str());

    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let node: Gd<Node> = match node_path {
        Some(path) if root.has_node(path) => root.get_node_as::<Node>(path),
        _ => root.clone(),
    };

    // 收集碰撞形状信息
    let mut shapes: Vec<serde_json::Value> = Vec::new();
    let mut check_nodes: Vec<Gd<Node>> = vec![node.clone()];
    while let Some(current) = check_nodes.pop() {
        // 检查当前节点是否是碰撞形状
        let class_name = current.get_class().to_string();
        if class_name == "CollisionShape2D" || class_name == "CollisionShape3D" {
            let shape_info = current.get("shape");
            let disabled = current.get("disabled").to::<bool>();
            shapes.push(serde_json::json!({
                "node_path": format!("{}/{}", node_path.unwrap_or("."), current.get_name()),
                "type": class_name,
                "disabled": disabled,
                "has_shape": !shape_info.is_nil(),
            }));
        }
        // 添加子节点
        for i in 0..current.get_child_count() {
            if let Some(child) = current.get_child(i) {
                check_nodes.push(child);
            }
        }
    }

    Ok(serde_json::json!({
        "node_path": node_path.unwrap_or("."),
        "node_type": node.get_class().to_string(),
        "collision_shapes": shapes,
        "shape_count": shapes.len()
    }))
}
