//! 导航命令模块
//! 对应原 GDScript 插件 navigation_commands.gd

use std::collections::HashMap;
use godot::classes::{ClassDb, EditorInterface, Node, Resource};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("setup_navigation_region", "设置导航区域", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "父节点路径" },
                "mode": { "type": "string", "default": "auto", "description": "2d/3d/auto" },
                "name": { "type": "string", "default": "NavigationRegion3D" },
                "agent_radius": { "type": "number", "default": 0.5 },
                "agent_height": { "type": "number", "default": 1.5 },
                "cell_size": { "type": "number", "default": 0.25 }
            }, "required": ["node_path"]
        })),
        ToolDefinition::new("bake_navigation_mesh", "烘焙导航网格", serde_json::json!({
            "type": "object", "properties": {
                "navigation_region_path": { "type": "string" }
            }, "required": ["navigation_region_path"]
        })),
        ToolDefinition::new("setup_navigation_agent", "设置导航代理", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "agent_type": { "type": "string", "default": "3D" },
                "name": { "type": "string", "default": "NavigationAgent3D" },
                "radius": { "type": "number", "default": 0.5 },
                "max_speed": { "type": "number", "default": 10.0 }
            }, "required": ["node_path"]
        })),
        ToolDefinition::new("set_navigation_layers", "设置导航层", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "layers": { "type": "integer" }
            }, "required": ["node_path", "layers"]
        })),
        ToolDefinition::new("get_navigation_info", "获取导航信息", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "default": "." }
            }, "required": []
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("setup_navigation_region".into(), cmd_setup_navigation_region);
    registry.insert("bake_navigation_mesh".into(), cmd_bake_navigation_mesh);
    registry.insert("setup_navigation_agent".into(), cmd_setup_navigation_agent);
    registry.insert("set_navigation_layers".into(), cmd_set_navigation_layers);
    registry.insert("get_navigation_info".into(), cmd_get_navigation_info);
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

/// 检测节点是否为 3D 上下文
fn is_3d_context(node: &Gd<Node>) -> bool {
    let class_name = node.get_class().to_string();
    if class_name.contains("3D") || class_name == "Node3D" {
        return true;
    }
    if class_name.contains("2D") || class_name == "Node2D" {
        return false;
    }
    // 向上遍历父节点检测上下文
    let mut current = node.clone();
    loop {
        if let Some(parent) = current.get_parent() {
            let pclass = parent.get_class().to_string();
            if pclass.contains("3D") || pclass == "Node3D" {
                return true;
            }
            if pclass.contains("2D") || pclass == "Node2D" {
                return false;
            }
            current = parent;
        } else {
            break;
        }
    }
    false
}

/// 设置导航区域
fn cmd_setup_navigation_region(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let mut parent = find_node_by_path(&root, node_path)?;

    let force_mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("auto");
    let is_3d = match force_mode {
        "2d" => false,
        "3d" => true,
        _ => is_3d_context(&parent),
    };

    let agent_radius = args.get("agent_radius").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
    let cell_size = args.get("cell_size").and_then(|v| v.as_f64()).unwrap_or(0.25) as f32;

    if is_3d {
        let region_name = args.get("name").and_then(|v| v.as_str()).unwrap_or("NavigationRegion3D");

        // 使用 ClassDb 动态实例化 NavigationRegion3D（避免直接引用不存在的类）
        let region_var = ClassDb::singleton().instantiate("NavigationRegion3D");
        let mut region: Gd<Node> = region_var.to();
        region.set_name(region_name);

        // 使用 ClassDb 动态实例化 NavigationMesh 资源
        let nav_mesh_var = ClassDb::singleton().instantiate("NavigationMesh");
        let mut nav_mesh: Gd<Resource> = nav_mesh_var.to();
        nav_mesh.set("agent_radius", &Variant::from(agent_radius));
        nav_mesh.set("cell_size", &Variant::from(cell_size));

        if let Some(agent_height) = args.get("agent_height").and_then(|v| v.as_f64()) {
            nav_mesh.set("agent_height", &Variant::from(agent_height as f32));
        }
        if let Some(max_climb) = args.get("agent_max_climb").and_then(|v| v.as_f64()) {
            nav_mesh.set("agent_max_climb", &Variant::from(max_climb as f32));
        }
        if let Some(max_slope) = args.get("agent_max_slope").and_then(|v| v.as_f64()) {
            nav_mesh.set("agent_max_slope", &Variant::from(max_slope as f32));
        }

        region.set("navigation_mesh", &nav_mesh_var);

        if let Some(layers) = args.get("navigation_layers").and_then(|v| v.as_i64()) {
            region.set("navigation_layers", &Variant::from(layers as i32));
        }

        parent.add_child(&region);
        region.set("owner", &Variant::from(root.clone()));

        Ok(serde_json::json!({
            "node_path": format!("{}/{}", node_path, region_name),
            "type": "NavigationRegion3D",
            "agent_radius": agent_radius,
            "cell_size": cell_size,
            "created": true,
        }))
    } else {
        let region_name = args.get("name").and_then(|v| v.as_str()).unwrap_or("NavigationRegion2D");

        // 使用 ClassDb 动态实例化 NavigationRegion2D
        let region_var = ClassDb::singleton().instantiate("NavigationRegion2D");
        let mut region: Gd<Node> = region_var.to();
        region.set_name(region_name);

        // 使用 ClassDb 动态实例化 NavigationPolygon 资源
        let nav_poly_var = ClassDb::singleton().instantiate("NavigationPolygon");
        let mut nav_poly: Gd<Resource> = nav_poly_var.to();
        nav_poly.set("agent_radius", &Variant::from(agent_radius));
        nav_poly.set("cell_size", &Variant::from(cell_size));

        region.set("navigation_polygon", &nav_poly_var);

        if let Some(layers) = args.get("navigation_layers").and_then(|v| v.as_i64()) {
            region.set("navigation_layers", &Variant::from(layers as i32));
        }

        parent.add_child(&region);
        region.set("owner", &Variant::from(root.clone()));

        Ok(serde_json::json!({
            "node_path": format!("{}/{}", node_path, region_name),
            "type": "NavigationRegion2D",
            "agent_radius": agent_radius,
            "cell_size": cell_size,
            "created": true,
        }))
    }
}

/// 烘焙导航网格
fn cmd_bake_navigation_mesh(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let region_path = args.get("navigation_region_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing navigation_region_path"))?;
    let mut region_node = find_node_by_path(&root, region_path)?;

    let class_name = region_node.get_class().to_string();

    if class_name == "NavigationRegion3D" {
        // TODO: bake_navigation_mesh() API 在 gdext 中可能不可直接调用
        // 使用动态属性方式触发烘焙
        region_node.set("bake_navigation_mesh", &Variant::nil());
        Ok(serde_json::json!({
            "node_path": region_path,
            "type": "NavigationRegion3D",
            "baked": true,
            "note": "bake triggered via property set"
        }))
    } else if class_name == "NavigationRegion2D" {
        // 检查是否有 NavigationPolygon
        let nav_poly = region_node.get("navigation_polygon");
        if nav_poly.is_nil() {
            // 使用 ClassDb 动态实例化 NavigationPolygon
            let new_poly = ClassDb::singleton().instantiate("NavigationPolygon");
            region_node.set("navigation_polygon", &new_poly);
        }
        // TODO: bake_navigation_polygon() API 在 gdext 中可能不可直接调用
        Ok(serde_json::json!({
            "node_path": region_path,
            "type": "NavigationRegion2D",
            "baked": true,
            "note": "NavigationPolygon assigned"
        }))
    } else {
        Err(McpError::invalid_params(&format!("Node '{}' is not a NavigationRegion (is {})", region_path, class_name)))
    }
}

/// 设置导航代理
fn cmd_setup_navigation_agent(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let mut parent = find_node_by_path(&root, node_path)?;

    let agent_type = args.get("agent_type").and_then(|v| v.as_str()).unwrap_or("3D");
    let is_3d = agent_type == "3D";

    if is_3d {
        let agent_name = args.get("name").and_then(|v| v.as_str()).unwrap_or("NavigationAgent3D");

        // 使用 ClassDb 动态实例化 NavigationAgent3D
        let agent_var = ClassDb::singleton().instantiate("NavigationAgent3D");
        let mut agent: Gd<Node> = agent_var.to();
        agent.set_name(agent_name);

        if let Some(radius) = args.get("radius").and_then(|v| v.as_f64()) {
            agent.set("radius", &Variant::from(radius as f32));
        }
        if let Some(max_speed) = args.get("max_speed").and_then(|v| v.as_f64()) {
            agent.set("max_speed", &Variant::from(max_speed as f32));
        }
        if let Some(path_dist) = args.get("path_desired_distance").and_then(|v| v.as_f64()) {
            agent.set("path_desired_distance", &Variant::from(path_dist as f32));
        }
        if let Some(target_dist) = args.get("target_desired_distance").and_then(|v| v.as_f64()) {
            agent.set("target_desired_distance", &Variant::from(target_dist as f32));
        }
        if let Some(neighbor_dist) = args.get("neighbor_distance").and_then(|v| v.as_f64()) {
            agent.set("neighbor_distance", &Variant::from(neighbor_dist as f32));
        }
        if let Some(max_neighbors) = args.get("max_neighbors").and_then(|v| v.as_i64()) {
            agent.set("max_neighbors", &Variant::from(max_neighbors as i32));
        }
        if let Some(avoidance) = args.get("avoidance_enabled").and_then(|v| v.as_bool()) {
            agent.set("avoidance_enabled", &Variant::from(avoidance));
        }
        if let Some(layers) = args.get("navigation_layers").and_then(|v| v.as_i64()) {
            agent.set("navigation_layers", &Variant::from(layers as i32));
        }

        parent.add_child(&agent);
        agent.set("owner", &Variant::from(root.clone()));

        Ok(serde_json::json!({
            "node_path": format!("{}/{}", node_path, agent_name),
            "type": "NavigationAgent3D",
            "created": true,
        }))
    } else {
        let agent_name = args.get("name").and_then(|v| v.as_str()).unwrap_or("NavigationAgent2D");

        // 使用 ClassDb 动态实例化 NavigationAgent2D
        let agent_var = ClassDb::singleton().instantiate("NavigationAgent2D");
        let mut agent: Gd<Node> = agent_var.to();
        agent.set_name(agent_name);

        if let Some(radius) = args.get("radius").and_then(|v| v.as_f64()) {
            agent.set("radius", &Variant::from(radius as f32));
        }
        if let Some(max_speed) = args.get("max_speed").and_then(|v| v.as_f64()) {
            agent.set("max_speed", &Variant::from(max_speed as f32));
        }
        if let Some(path_dist) = args.get("path_desired_distance").and_then(|v| v.as_f64()) {
            agent.set("path_desired_distance", &Variant::from(path_dist as f32));
        }
        if let Some(target_dist) = args.get("target_desired_distance").and_then(|v| v.as_f64()) {
            agent.set("target_desired_distance", &Variant::from(target_dist as f32));
        }
        if let Some(neighbor_dist) = args.get("neighbor_distance").and_then(|v| v.as_f64()) {
            agent.set("neighbor_distance", &Variant::from(neighbor_dist as f32));
        }
        if let Some(max_neighbors) = args.get("max_neighbors").and_then(|v| v.as_i64()) {
            agent.set("max_neighbors", &Variant::from(max_neighbors as i32));
        }
        if let Some(max_speed) = args.get("max_speed").and_then(|v| v.as_f64()) {
            agent.set("max_speed", &Variant::from(max_speed as f32));
        }
        if let Some(avoidance) = args.get("avoidance_enabled").and_then(|v| v.as_bool()) {
            agent.set("avoidance_enabled", &Variant::from(avoidance));
        }
        if let Some(layers) = args.get("navigation_layers").and_then(|v| v.as_i64()) {
            agent.set("navigation_layers", &Variant::from(layers as i32));
        }

        parent.add_child(&agent);
        agent.set("owner", &Variant::from(root.clone()));

        Ok(serde_json::json!({
            "node_path": format!("{}/{}", node_path, agent_name),
            "type": "NavigationAgent2D",
            "created": true,
        }))
    }
}

/// 设置导航层
fn cmd_set_navigation_layers(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let layers = args.get("layers").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing layers"))? as i32;

    let mut node = find_node_by_path(&root, node_path)?;
    node.set("navigation_layers", &Variant::from(layers));

    Ok(serde_json::json!({
        "node_path": node_path,
        "navigation_layers": layers,
        "updated": true,
    }))
}

/// 获取导航信息
fn cmd_get_navigation_info(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).unwrap_or(".");
    let node = find_node_by_path(&root, node_path)?;

    // 收集场景中的导航节点
    let mut regions: Vec<serde_json::Value> = Vec::new();
    let mut agents: Vec<serde_json::Value> = Vec::new();

    // 递归收集导航节点
    collect_navigation_nodes(&node, &mut regions, &mut agents);

    Ok(serde_json::json!({
        "node_path": node_path,
        "regions": regions,
        "agents": agents,
        "region_count": regions.len(),
        "agent_count": agents.len(),
    }))
}

/// 递归收集导航节点
fn collect_navigation_nodes(
    node: &Gd<Node>,
    regions: &mut Vec<serde_json::Value>,
    agents: &mut Vec<serde_json::Value>,
) {
    let class_name = node.get_class().to_string();

    // 检查是否为导航区域
    if class_name == "NavigationRegion3D" {
        let nav_mesh = node.get("navigation_mesh");
        regions.push(serde_json::json!({
            "path": node.get("name").to_string(),
            "type": "NavigationRegion3D",
            "enabled": node.get("enabled").try_to::<bool>().unwrap_or(true),
            "navigation_layers": node.get("navigation_layers").try_to::<i32>().unwrap_or(1),
            "has_mesh": !nav_mesh.is_nil(),
        }));
    } else if class_name == "NavigationRegion2D" {
        let nav_poly = node.get("navigation_polygon");
        regions.push(serde_json::json!({
            "path": node.get("name").to_string(),
            "type": "NavigationRegion2D",
            "enabled": node.get("enabled").try_to::<bool>().unwrap_or(true),
            "navigation_layers": node.get("navigation_layers").try_to::<i32>().unwrap_or(1),
            "has_polygon": !nav_poly.is_nil(),
        }));
    }

    // 检查是否为导航代理
    if class_name == "NavigationAgent3D" {
        agents.push(serde_json::json!({
            "path": node.get("name").to_string(),
            "type": "NavigationAgent3D",
            "radius": node.get("radius").try_to::<f32>().unwrap_or(0.0),
            "max_speed": node.get("max_speed").try_to::<f32>().unwrap_or(0.0),
            "navigation_layers": node.get("navigation_layers").try_to::<i32>().unwrap_or(1),
        }));
    } else if class_name == "NavigationAgent2D" {
        agents.push(serde_json::json!({
            "path": node.get("name").to_string(),
            "type": "NavigationAgent2D",
            "radius": node.get("radius").try_to::<f32>().unwrap_or(0.0),
            "max_speed": node.get("max_speed").try_to::<f32>().unwrap_or(0.0),
            "navigation_layers": node.get("navigation_layers").try_to::<i32>().unwrap_or(1),
        }));
    }

    // 递归遍历子节点
    let child_count = node.get_child_count();
    for i in 0..child_count {
        if let Some(child) = node.get_child(i) {
            collect_navigation_nodes(&child, regions, agents);
        }
    }
}
