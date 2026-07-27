//! 3D 场景命令模块

use std::collections::HashMap;
use godot::classes::{Camera3D, DirectionalLight3D, EditorInterface, MeshInstance3D, Node, OmniLight3D, ResourceLoader};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("add_mesh_instance", "添加 MeshInstance3D", serde_json::json!({
            "type": "object", "properties": {
                "parent_path": { "type": "string", "default": "." },
                "name": { "type": "string", "default": "Mesh" }
            }, "required": []
        })),
        ToolDefinition::new("setup_camera_3d", "配置 Camera3D", serde_json::json!({
            "type": "object", "properties": { "node_path": { "type": "string" } }, "required": ["node_path"]
        })),
        ToolDefinition::new("setup_lighting", "添加光照", serde_json::json!({
            "type": "object", "properties": {
                "parent_path": { "type": "string", "default": "." },
                "light_type": { "type": "string", "default": "directional" }
            }, "required": []
        })),
        // 设置 3D 材质
        ToolDefinition::new("set_material_3d", "设置 3D 材质到 MeshInstance3D", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "material_path": { "type": "string" },
                "material_slot": { "type": "string" }
            }, "required": ["node_path", "material_path"]
        })),
        // 设置 3D 环境
        ToolDefinition::new("setup_environment", "设置 3D 环境", serde_json::json!({
            "type": "object", "properties": {
                "bg_color": { "type": "object", "properties": { "r": { "type": "number" }, "g": { "type": "number" }, "b": { "type": "number" } } },
                "ambient_color": { "type": "object", "properties": { "r": { "type": "number" }, "g": { "type": "number" }, "b": { "type": "number" } } },
                "world_env_path": { "type": "string" }
            }, "required": []
        })),
        // 添加 GridMap
        ToolDefinition::new("add_gridmap", "添加 GridMap 节点", serde_json::json!({
            "type": "object", "properties": {
                "parent_path": { "type": "string", "default": "." },
                "mesh_library_path": { "type": "string" },
                "name": { "type": "string" }
            }, "required": ["mesh_library_path"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("add_mesh_instance".into(), cmd_add_mesh_instance);
    registry.insert("setup_camera_3d".into(), cmd_setup_camera_3d);
    registry.insert("setup_lighting".into(), cmd_setup_lighting);
    registry.insert("set_material_3d".into(), cmd_set_material_3d);
    registry.insert("setup_environment".into(), cmd_setup_environment);
    registry.insert("add_gridmap".into(), cmd_add_gridmap);
}

fn find_root() -> Result<Gd<Node>, McpError> {
    EditorInterface::singleton().get_edited_scene_root().ok_or_else(|| McpError::no_scene())
}

fn find_parent(root: &Gd<Node>, path: &str) -> Result<Gd<Node>, McpError> {
    if path == "." || path == root.get_name().to_string() { Ok(root.clone()) }
    else if root.has_node(path) { Ok(root.get_node_as::<Node>(path)) }
    else { Err(McpError::not_found(&format!("Node '{}'", path), "")) }
}

fn cmd_add_mesh_instance(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let parent_path = args.get("parent_path").and_then(|v| v.as_str()).unwrap_or(".");
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("Mesh");
    let mut parent = find_parent(&root, parent_path)?;
    let mut mesh = MeshInstance3D::new_alloc();
    mesh.set_name(name);
    parent.add_child(&mesh);
    mesh.set("owner", &Variant::from(root.clone()));
    Ok(serde_json::json!({"added": true, "name": name}))
}

fn cmd_setup_camera_3d(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    if root.has_node(path) {
        let mut cam = root.get_node_as::<Camera3D>(path);
        cam.set_current(true);
        Ok(serde_json::json!({"configured": true, "node_path": path}))
    } else {
        // Path is a parent path, create camera under it
        let mut parent = find_parent(&root, path)?;
        let mut cam = Camera3D::new_alloc();
        cam.set_name("Camera3D");
        parent.add_child(&cam);
        cam.set("owner", &Variant::from(root.clone()));
        cam.set_current(true);
        Ok(serde_json::json!({"created": true, "node_path": format!("{}/Camera3D", path)}))
    }
}

fn cmd_setup_lighting(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let parent_path = args.get("parent_path").and_then(|v| v.as_str()).unwrap_or(".");
    let light_type = args.get("light_type").and_then(|v| v.as_str()).unwrap_or("directional");
    let mut parent = find_parent(&root, parent_path)?;

    match light_type {
        "directional" => {
            let mut light = DirectionalLight3D::new_alloc();
            light.set_name("DirectionalLight");
            parent.add_child(&light);
            light.set("owner", &Variant::from(root.clone()));
        }
        _ => {
            let mut light = OmniLight3D::new_alloc();
            light.set_name("OmniLight");
            parent.add_child(&light);
            light.set("owner", &Variant::from(root.clone()));
        }
    }
    Ok(serde_json::json!({"added": true, "light_type": light_type}))
}

/// 设置 3D 材质到 MeshInstance3D
fn cmd_set_material_3d(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let material_path = args.get("material_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing material_path"))?;
    let _material_slot = args.get("material_slot").and_then(|v| v.as_str());

    let root = find_root()?;
    if !root.has_node(node_path) {
        return Err(McpError::not_found(&format!("Node '{}'", node_path), ""));
    }

    // 加载材质资源并通过 GDScript 表达式设置
    let code = format!(
        "var mesh = EditorInterface.get_edited_scene_root().get_node('{}'); \
         var mat = load('{}'); \
         mesh.set_surface_override_material(0, mat); \
         true",
        node_path.replace('\'', "\\'"),
        material_path.replace('\'', "\\'")
    );
    let mut expr = godot::classes::Expression::new_gd();
    if expr.parse(&code) == godot::global::Error::OK {
        expr.execute();
    }

    Ok(serde_json::json!({"node_path": node_path, "material_path": material_path, "set": true}))
}

/// 设置 3D 环境 - 创建或配置 WorldEnvironment
fn cmd_setup_environment(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let mut root = find_root()?;
    let world_env_path = args.get("world_env_path").and_then(|v| v.as_str());

    // 查找或创建 WorldEnvironment 节点
    let mut world_env = if let Some(path) = world_env_path {
        if root.has_node(path) {
            root.get_node_as::<godot::classes::WorldEnvironment>(path)
        } else {
            let mut env = godot::classes::WorldEnvironment::new_alloc();
            env.set_name("WorldEnvironment");
            root.add_child(&env);
            env.set("owner", &Variant::from(root.clone()));
            env
        }
    } else {
        // 查找 WorldEnvironment 子节点
    let mut found = false;
    let mut result_env = godot::classes::WorldEnvironment::new_alloc();
    for i in 0..root.get_child_count() {
        if let Some(child) = root.get_child(i) {
            if child.get_class().to_string() == "WorldEnvironment" {
                // 使用 Gd::try_from 或 Variant 方式转换
                let gd_child = unsafe { std::mem::transmute::<Gd<Node>, Gd<godot::classes::WorldEnvironment>>(child) };
                result_env = gd_child;
                found = true;
                break;
            }
        }
    }
        if !found {
            result_env = godot::classes::WorldEnvironment::new_alloc();
            result_env.set_name("WorldEnvironment");
            root.add_child(&result_env);
            result_env.set("owner", &Variant::from(root.clone()));
        }
        result_env
    };

    // 创建或获取 Environment 资源
    let env = world_env.get_environment();
    let env_clone = env.clone();
    if env_clone.is_none() {
        let new_env = godot::classes::Environment::new_gd();
        world_env.set_environment(&new_env);
    }

    // 设置背景颜色 - 使用 GDScript Expression 避免类型转换问题
    if let Some(bg) = args.get("bg_color").and_then(|v| v.as_object()) {
        let r = bg.get("r").and_then(|v| v.as_f64()).unwrap_or(0.3);
        let g = bg.get("g").and_then(|v| v.as_f64()).unwrap_or(0.3);
        let b = bg.get("b").and_then(|v| v.as_f64()).unwrap_or(0.3);
        let bg_code = format!(
            "var we = EditorInterface.get_edited_scene_root().get_node('{}'); \
             var e = we.get_environment(); \
             if e != null: e.set('background_color_mode', 1); e.set('background_color', Color({}, {}, {}))",
            world_env.get_name().to_string().replace('\'', "\\'"), r, g, b
        );
        let mut bg_expr = godot::classes::Expression::new_gd();
        if bg_expr.parse(&bg_code) == godot::global::Error::OK {
            bg_expr.execute();
        }
    }

    // 设置环境光颜色
    if let Some(ambient) = args.get("ambient_color").and_then(|v| v.as_object()) {
        let r = ambient.get("r").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let g = ambient.get("g").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let b = ambient.get("b").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let amb_code = format!(
            "var we = EditorInterface.get_edited_scene_root().get_node('{}'); \
             var e = we.get_environment(); \
             if e != null: e.set('ambient_light_color', Color({}, {}, {}))",
            world_env.get_name().to_string().replace('\'', "\\'"), r, g, b
        );
        let mut amb_expr = godot::classes::Expression::new_gd();
        if amb_expr.parse(&amb_code) == godot::global::Error::OK {
            amb_expr.execute();
        }
    }

    Ok(serde_json::json!({"setup": true, "world_environment": world_env.get_name().to_string()}))
}

/// 添加 GridMap 节点
fn cmd_add_gridmap(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let parent_path = args.get("parent_path").and_then(|v| v.as_str()).unwrap_or(".");
    let mesh_library_path = args.get("mesh_library_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing mesh_library_path"))?;
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("GridMap");

    let root = find_root()?;
    let mut parent = find_parent(&root, parent_path)?;

    // 通过 ClassDb 实例化 GridMap
    let gridmap = godot::classes::ClassDb::singleton().instantiate("GridMap");
    // 将 Variant 转换为 Gd<Node>
    let mut gridmap_node = gridmap.to::<Gd<godot::classes::Node>>();

    gridmap_node.set_name(name);

    // 加载 MeshLibrary
    let mut rl = ResourceLoader::singleton();
    if let Some(lib) = rl.load(mesh_library_path) {
        gridmap_node.set("mesh_library", &Variant::from(lib));
    }

    parent.add_child(&gridmap_node);
    gridmap_node.set("owner", &Variant::from(root.clone()));

    Ok(serde_json::json!({"name": name, "parent_path": parent_path, "mesh_library_path": mesh_library_path, "created": true}))
}
