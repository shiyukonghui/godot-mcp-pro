//! 3D 场景命令模块

use std::collections::HashMap;
use godot::classes::{Camera3D, DirectionalLight3D, EditorInterface, MeshInstance3D, Node, OmniLight3D};
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
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("add_mesh_instance".into(), cmd_add_mesh_instance);
    registry.insert("setup_camera_3d".into(), cmd_setup_camera_3d);
    registry.insert("setup_lighting".into(), cmd_setup_lighting);
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
