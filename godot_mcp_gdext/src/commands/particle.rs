//! 粒子系统命令模块
//! 对应原 GDScript 插件 particle_commands.gd

use std::collections::HashMap;
use godot::classes::{
    ClassDb, EditorInterface, Gradient, GradientTexture1D, Node,
    ParticleProcessMaterial, Resource,
};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("create_particles", "创建粒子系统节点", serde_json::json!({
            "type": "object", "properties": {
                "parent_path": { "type": "string", "default": ".", "description": "父节点路径" },
                "particle_type": { "type": "string", "default": "GPUParticles2D", "description": "粒子类型" },
                "name": { "type": "string", "default": "Particles" }
            }, "required": []
        })),
        ToolDefinition::new("set_particle_material", "设置粒子材质", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "material_params": { "type": "object", "description": "材质参数字典" }
            }, "required": ["node_path", "material_params"]
        })),
        ToolDefinition::new("set_particle_color_gradient", "设置粒子颜色渐变", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "colors": { "type": "array", "description": "颜色停止点数组 [{offset, color}]" }
            }, "required": ["node_path", "colors"]
        })),
        ToolDefinition::new("apply_particle_preset", "应用粒子预设", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "preset": { "type": "string", "description": "fire/smoke/magic/explosion/rain/snow" }
            }, "required": ["node_path", "preset"]
        })),
        ToolDefinition::new("get_particle_info", "获取粒子系统信息", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" }
            }, "required": ["node_path"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("create_particles".into(), cmd_create_particles);
    registry.insert("set_particle_material".into(), cmd_set_particle_material);
    registry.insert("set_particle_color_gradient".into(), cmd_set_particle_color_gradient);
    registry.insert("apply_particle_preset".into(), cmd_apply_particle_preset);
    registry.insert("get_particle_info".into(), cmd_get_particle_info);
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

/// 创建粒子系统节点
fn cmd_create_particles(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let parent_path = args.get("parent_path").and_then(|v| v.as_str()).unwrap_or(".");
    let mut parent = find_node_by_path(&root, parent_path)?;

    let particle_type = args.get("particle_type").and_then(|v| v.as_str()).unwrap_or("GPUParticles2D");
    let node_name = args.get("name").and_then(|v| v.as_str()).unwrap_or("Particles");

    // 使用 ClassDb 实例化粒子节点
    let class_db = ClassDb::singleton();
    let particle_var = class_db.instantiate(particle_type);
    if particle_var.is_nil() {
        return Err(McpError::internal(&format!("Failed to instantiate: {}", particle_type)));
    }
    // 安全转换：验证类型继承 Node 再转换
    let mut particles: Gd<Node> = particle_var.try_to().map_err(|_| {
        McpError::invalid_params(&format!("类型 '{}' 不是 Node 的子类", particle_type))
    })?;
    particles.set_name(node_name);

    // 创建默认的 ParticleProcessMaterial
    let mat = ParticleProcessMaterial::new_gd();
    particles.set("process_material", &Variant::from(mat));

    // 添加到场景
    parent.add_child(&particles);
    particles.set("owner", &Variant::from(root.clone()));

    Ok(serde_json::json!({
        "name": node_name,
        "parent": parent_path,
        "type": particle_type,
        "created": true,
    }))
}

/// 设置粒子材质
fn cmd_set_particle_material(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let material_params = args.get("material_params").and_then(|v| v.as_object()).ok_or_else(|| McpError::invalid_params("Missing material_params"))?;

    let mut node = find_node_by_path(&root, node_path)?;

    // 获取或创建 ParticleProcessMaterial
    let current_mat = node.get("process_material");
    let mat: Gd<ParticleProcessMaterial> = if current_mat.is_nil() {
        ParticleProcessMaterial::new_gd()
    } else {
        // 克隆现有材质: Variant.call("duplicate") 返回 Variant, 使用 try_to 安全转换
        let dup = current_mat.call("duplicate", &[]);
        let dup_gd: Gd<ParticleProcessMaterial> = dup.try_to().unwrap_or_else(|_| ParticleProcessMaterial::new_gd());
        dup_gd
    };

    // 设置材质属性
    let mut mat = mat;
    let mut changes: Vec<String> = Vec::new();

    // direction
    if let Some(dir) = material_params.get("direction").and_then(|v| v.as_object()) {
        let x = dir.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let y = dir.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let z = dir.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        mat.set("direction", &Variant::from(Vector3::new(x, y, z)));
        changes.push("direction".into());
    }

    // spread
    if let Some(spread) = material_params.get("spread").and_then(|v| v.as_f64()) {
        mat.set("spread", &Variant::from(spread as f32));
        changes.push("spread".into());
    }

    // initial_velocity
    if let Some(v) = material_params.get("initial_velocity_min").and_then(|v| v.as_f64()) {
        mat.set("initial_velocity_min", &Variant::from(v as f32));
        changes.push("initial_velocity_min".into());
    }
    if let Some(v) = material_params.get("initial_velocity_max").and_then(|v| v.as_f64()) {
        mat.set("initial_velocity_max", &Variant::from(v as f32));
        changes.push("initial_velocity_max".into());
    }

    // gravity
    if let Some(grav) = material_params.get("gravity").and_then(|v| v.as_object()) {
        let x = grav.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let y = grav.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let z = grav.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        mat.set("gravity", &Variant::from(Vector3::new(x, y, z)));
        changes.push("gravity".into());
    }

    // scale
    if let Some(v) = material_params.get("scale_min").and_then(|v| v.as_f64()) {
        mat.set("scale_min", &Variant::from(v as f32));
        changes.push("scale_min".into());
    }
    if let Some(v) = material_params.get("scale_max").and_then(|v| v.as_f64()) {
        mat.set("scale_max", &Variant::from(v as f32));
        changes.push("scale_max".into());
    }

    // color
    if let Some(color) = material_params.get("color").and_then(|v| v.as_str()) {
        mat.set("color", &Variant::from(color));
        changes.push("color".into());
    }

    // angular_velocity
    if let Some(v) = material_params.get("angular_velocity_min").and_then(|v| v.as_f64()) {
        mat.set("angular_velocity_min", &Variant::from(v as f32));
        changes.push("angular_velocity_min".into());
    }
    if let Some(v) = material_params.get("angular_velocity_max").and_then(|v| v.as_f64()) {
        mat.set("angular_velocity_max", &Variant::from(v as f32));
        changes.push("angular_velocity_max".into());
    }

    // damping
    if let Some(v) = material_params.get("damping_min").and_then(|v| v.as_f64()) {
        mat.set("damping_min", &Variant::from(v as f32));
        changes.push("damping_min".into());
    }
    if let Some(v) = material_params.get("damping_max").and_then(|v| v.as_f64()) {
        mat.set("damping_max", &Variant::from(v as f32));
        changes.push("damping_max".into());
    }

    // emission_shape
    if let Some(shape) = material_params.get("emission_shape").and_then(|v| v.as_str()) {
        let shape_val: i32 = match shape {
            "point" => 0,
            "sphere" => 1,
            "sphere_surface" => 2,
            "box" => 3,
            "ring" => 4,
            _ => 0,
        };
        mat.set("emission_shape", &Variant::from(shape_val));
        changes.push("emission_shape".into());
    }

    node.set("process_material", &Variant::from(mat));

    Ok(serde_json::json!({
        "node_path": node_path,
        "changes": changes,
    }))
}

/// 设置粒子颜色渐变
fn cmd_set_particle_color_gradient(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let colors = args.get("colors").and_then(|v| v.as_array()).ok_or_else(|| McpError::invalid_params("Missing colors array"))?;

    let mut node = find_node_by_path(&root, node_path)?;

    // 获取或创建 ParticleProcessMaterial
    let current_mat = node.get("process_material");
    let mat: Gd<ParticleProcessMaterial> = if current_mat.is_nil() {
        ParticleProcessMaterial::new_gd()
    } else {
        // 克隆现有材质
        let dup = current_mat.call("duplicate", &[]);
        let dup_gd: Gd<ParticleProcessMaterial> = dup.try_to().unwrap_or_else(|_| ParticleProcessMaterial::new_gd());
        dup_gd
    };
    let mut mat = mat;

    // 构建渐变色
    let mut offsets: Vec<f32> = Vec::new();
    let mut color_values: Vec<String> = Vec::new();

    for stop in colors {
        if let Some(obj) = stop.as_object() {
            let offset = obj.get("offset").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let color_str = obj.get("color").and_then(|v| v.as_str()).unwrap_or("#ffffff");
            offsets.push(offset);
            color_values.push(color_str.to_string());
        }
    }

    if offsets.is_empty() {
        return Err(McpError::invalid_params("colors array must contain {offset, color} objects"));
    }

    // 创建 Gradient 资源
    let mut gradient = Gradient::new_gd();
    // 使用动态属性设置
    let offset_arr: PackedFloat32Array = offsets.iter().map(|&v| v).collect();
    gradient.set("offsets", &Variant::from(offset_arr));

    // 创建颜色数组 (PackedColorArray)
    let mut color_arr = PackedColorArray::new();
    for c in &color_values {
        if let Some(color) = Color::from_string(c) {
            color_arr.push(color);
        } else {
            color_arr.push(Color::from_rgba(1.0, 1.0, 1.0, 1.0));
        }
    }
    gradient.set("colors", &Variant::from(color_arr));

    // 创建 GradientTexture1D
    let mut grad_tex = GradientTexture1D::new_gd();
    grad_tex.set("gradient", &Variant::from(gradient));

    // 设置到材质
    mat.set("color_ramp", &Variant::from(grad_tex));
    node.set("process_material", &Variant::from(mat));

    Ok(serde_json::json!({
        "node_path": node_path,
        "stops_count": offsets.len(),
    }))
}

/// 应用粒子预设
fn cmd_apply_particle_preset(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let preset = args.get("preset").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing preset"))?;

    let mut node = find_node_by_path(&root, node_path)?;

    // 检测是 2D 还是 3D
    let class_name = node.get_class().to_string();
    let is_2d = class_name.contains("2D");

    // 创建材质
    let mut mat = ParticleProcessMaterial::new_gd();
    let gravity_down = if is_2d { 98.0 } else { 9.8 };

    // 根据预设设置参数
    match preset {
        "fire" => {
            node.set("amount", &Variant::from(24i32));
            node.set("lifetime", &Variant::from(1.2f32));
            mat.set("direction", &Variant::from(Vector3::new(0.0, -1.0, 0.0)));
            mat.set("spread", &Variant::from(15.0f32));
            mat.set("initial_velocity_min", &Variant::from(if is_2d { 30.0f32 } else { 1.5 }));
            mat.set("initial_velocity_max", &Variant::from(if is_2d { 60.0f32 } else { 3.0 }));
            mat.set("scale_min", &Variant::from(0.8f32));
            mat.set("scale_max", &Variant::from(1.5f32));
            mat.set("color", &Variant::from(Color::from_rgba(1.0, 0.6, 0.0, 1.0)));
        }
        "smoke" => {
            node.set("amount", &Variant::from(16i32));
            node.set("lifetime", &Variant::from(3.0f32));
            mat.set("direction", &Variant::from(Vector3::new(0.0, -1.0, 0.0)));
            mat.set("spread", &Variant::from(25.0f32));
            mat.set("initial_velocity_min", &Variant::from(if is_2d { 10.0f32 } else { 0.5 }));
            mat.set("initial_velocity_max", &Variant::from(if is_2d { 25.0f32 } else { 1.2 }));
            mat.set("scale_min", &Variant::from(1.5f32));
            mat.set("scale_max", &Variant::from(3.0f32));
            mat.set("damping_min", &Variant::from(1.0f32));
            mat.set("damping_max", &Variant::from(2.0f32));
            mat.set("color", &Variant::from(Color::from_rgba(0.5, 0.5, 0.5, 0.6)));
        }
        "magic" => {
            node.set("amount", &Variant::from(24i32));
            node.set("lifetime", &Variant::from(2.0f32));
            mat.set("spread", &Variant::from(180.0f32));
            mat.set("initial_velocity_min", &Variant::from(if is_2d { 20.0f32 } else { 1.0 }));
            mat.set("initial_velocity_max", &Variant::from(if is_2d { 50.0f32 } else { 2.5 }));
            mat.set("orbit_velocity_min", &Variant::from(0.5f32));
            mat.set("orbit_velocity_max", &Variant::from(1.5f32));
            mat.set("scale_min", &Variant::from(0.3f32));
            mat.set("scale_max", &Variant::from(0.8f32));
            mat.set("color", &Variant::from(Color::from_rgba(0.3, 0.5, 1.0, 1.0)));
        }
        "explosion" => {
            node.set("amount", &Variant::from(32i32));
            node.set("lifetime", &Variant::from(0.6f32));
            node.set("one_shot", &Variant::from(true));
            node.set("explosiveness", &Variant::from(1.0f32));
            mat.set("spread", &Variant::from(180.0f32));
            mat.set("initial_velocity_min", &Variant::from(if is_2d { 100.0f32 } else { 5.0 }));
            mat.set("initial_velocity_max", &Variant::from(if is_2d { 200.0f32 } else { 10.0 }));
            mat.set("gravity", &Variant::from(Vector3::new(0.0, gravity_down * 0.5, 0.0)));
            mat.set("damping_min", &Variant::from(2.0f32));
            mat.set("damping_max", &Variant::from(4.0f32));
            mat.set("scale_min", &Variant::from(0.5f32));
            mat.set("scale_max", &Variant::from(1.5f32));
            mat.set("color", &Variant::from(Color::from_rgba(1.0, 0.6, 0.1, 1.0)));
        }
        "rain" => {
            node.set("amount", &Variant::from(64i32));
            node.set("lifetime", &Variant::from(0.8f32));
            mat.set("spread", &Variant::from(5.0f32));
            mat.set("initial_velocity_min", &Variant::from(if is_2d { 300.0f32 } else { 12.0 }));
            mat.set("initial_velocity_max", &Variant::from(if is_2d { 400.0f32 } else { 16.0 }));
            mat.set("gravity", &Variant::from(Vector3::new(0.0, gravity_down, 0.0)));
            mat.set("scale_min", &Variant::from(0.1f32));
            mat.set("scale_max", &Variant::from(0.2f32));
            mat.set("color", &Variant::from(Color::from_rgba(0.6, 0.7, 1.0, 0.7)));
        }
        "snow" => {
            node.set("amount", &Variant::from(48i32));
            node.set("lifetime", &Variant::from(4.0f32));
            mat.set("spread", &Variant::from(20.0f32));
            mat.set("initial_velocity_min", &Variant::from(if is_2d { 20.0f32 } else { 0.8 }));
            mat.set("initial_velocity_max", &Variant::from(if is_2d { 40.0f32 } else { 1.5 }));
            mat.set("gravity", &Variant::from(Vector3::new(0.0, 20.0, 0.0)));
            mat.set("scale_min", &Variant::from(0.3f32));
            mat.set("scale_max", &Variant::from(0.8f32));
            mat.set("damping_min", &Variant::from(0.5f32));
            mat.set("damping_max", &Variant::from(1.5f32));
            mat.set("color", &Variant::from(Color::from_rgba(1.0, 1.0, 1.0, 0.9)));
        }
        _ => return Err(McpError::invalid_params(&format!(
            "Unknown preset: '{}'. Valid presets: fire, smoke, magic, explosion, rain, snow", preset
        ))),
    }

    node.set("process_material", &Variant::from(mat));

    Ok(serde_json::json!({
        "node_path": node_path,
        "preset": preset,
        "applied": true,
    }))
}

/// 获取粒子系统信息
fn cmd_get_particle_info(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let node = find_node_by_path(&root, node_path)?;

    let class_name = node.get_class().to_string();

    // 读取基本属性
    let amount: i32 = node.get("amount").try_to().unwrap_or(0);
    let lifetime: f32 = node.get("lifetime").try_to().unwrap_or(0.0);
    let one_shot: bool = node.get("one_shot").try_to().unwrap_or(false);
    let emitting: bool = node.get("emitting").try_to().unwrap_or(false);
    let speed_scale: f32 = node.get("speed_scale").try_to().unwrap_or(1.0);
    let explosiveness: f32 = node.get("explosiveness").try_to().unwrap_or(0.0);
    let randomness: f32 = node.get("randomness").try_to().unwrap_or(0.0);

    // 读取材质信息: 通过 Gd<Resource> 动态访问属性
    let mat_var = node.get("process_material");
    let material_info = if !mat_var.is_nil() {
        let mat: Gd<Resource> = mat_var.to();
        let direction = mat.get("direction").to_string();
        let spread: f32 = mat.get("spread").try_to().unwrap_or(0.0);
        let initial_velocity_min: f32 = mat.get("initial_velocity_min").try_to().unwrap_or(0.0);
        let initial_velocity_max: f32 = mat.get("initial_velocity_max").try_to().unwrap_or(0.0);
        let gravity = mat.get("gravity").to_string();
        let scale_min: f32 = mat.get("scale_min").try_to().unwrap_or(0.0);
        let scale_max: f32 = mat.get("scale_max").try_to().unwrap_or(0.0);
        let color = mat.get("color").to_string();

        serde_json::json!({
            "direction": direction,
            "spread": spread,
            "initial_velocity_min": initial_velocity_min,
            "initial_velocity_max": initial_velocity_max,
            "gravity": gravity,
            "scale_min": scale_min,
            "scale_max": scale_max,
            "color": color,
        })
    } else {
        serde_json::Value::Null
    };

    Ok(serde_json::json!({
        "node_path": node_path,
        "type": class_name,
        "amount": amount,
        "lifetime": lifetime,
        "one_shot": one_shot,
        "emitting": emitting,
        "speed_scale": speed_scale,
        "explosiveness": explosiveness,
        "randomness": randomness,
        "material": material_info,
    }))
}
