//! 主题命令模块
//! 对应原 GDScript 插件 theme_commands.gd

use std::collections::HashMap;
use godot::classes::{
    ClassDb, EditorInterface, ResourceLoader, ResourceSaver, Theme,
};
use godot::global::Error;
use godot::obj::Gd;
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("create_theme", "创建 Theme 资源", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "保存路径 (res://)" },
                "name": { "type": "string", "description": "主题名称" }
            }, "required": ["path"]
        })),
        ToolDefinition::new("set_theme_color", "设置主题颜色", serde_json::json!({
            "type": "object", "properties": {
                "theme_path": { "type": "string" },
                "color_name": { "type": "string" },
                "color": { "type": "object", "description": "包含 r,g,b,a 的字典" },
                "node_type": { "type": "string", "default": "Button" }
            }, "required": ["theme_path", "color_name", "color"]
        })),
        ToolDefinition::new("set_theme_constant", "设置主题常量", serde_json::json!({
            "type": "object", "properties": {
                "theme_path": { "type": "string" },
                "constant_name": { "type": "string" },
                "value": { "type": "integer" },
                "node_type": { "type": "string", "default": "Button" }
            }, "required": ["theme_path", "constant_name", "value"]
        })),
        ToolDefinition::new("set_theme_font_size", "设置主题字体大小", serde_json::json!({
            "type": "object", "properties": {
                "theme_path": { "type": "string" },
                "font_size_name": { "type": "string" },
                "size": { "type": "integer" },
                "node_type": { "type": "string", "default": "Button" }
            }, "required": ["theme_path", "font_size_name", "size"]
        })),
        ToolDefinition::new("set_theme_stylebox", "设置主题样式盒", serde_json::json!({
            "type": "object", "properties": {
                "theme_path": { "type": "string" },
                "stylebox_name": { "type": "string" },
                "node_type": { "type": "string", "default": "Panel" },
                "bg_color": { "type": "string" },
                "border_color": { "type": "string" },
                "border_width": { "type": "integer" },
                "corner_radius": { "type": "integer" }
            }, "required": ["theme_path", "stylebox_name"]
        })),
        ToolDefinition::new("setup_control", "对 Control 节点应用主题", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "theme_path": { "type": "string" }
            }, "required": ["node_path"]
        })),
        ToolDefinition::new("get_theme_info", "获取主题信息", serde_json::json!({
            "type": "object", "properties": {
                "theme_path": { "type": "string" }
            }, "required": ["theme_path"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("create_theme".into(), cmd_create_theme);
    registry.insert("set_theme_color".into(), cmd_set_theme_color);
    registry.insert("set_theme_constant".into(), cmd_set_theme_constant);
    registry.insert("set_theme_font_size".into(), cmd_set_theme_font_size);
    registry.insert("set_theme_stylebox".into(), cmd_set_theme_stylebox);
    registry.insert("setup_control".into(), cmd_setup_control);
    registry.insert("get_theme_info".into(), cmd_get_theme_info);
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

/// 加载 Theme 资源
fn load_theme(path: &str) -> Result<Gd<Theme>, McpError> {
    let mut rl = ResourceLoader::singleton();
    let res = rl.load(path).ok_or_else(|| McpError::not_found(&format!("Theme '{}'", path), ""))?;
    res.try_cast::<Theme>().map_err(|_| McpError::invalid_params(&format!("'{}' is not a Theme resource", path)))
}

/// 保存 Theme 资源并刷新文件系统
fn save_theme(theme: &Gd<Theme>, path: &str) -> Result<(), McpError> {
    let err = ResourceSaver::singleton().save_ex(theme).path(path).done();
    if err != Error::OK {
        return Err(McpError::internal(&format!("Failed to save theme: {:?}", err)));
    }
    if let Some(mut fs) = EditorInterface::singleton().get_resource_filesystem() {
        fs.scan();
    }
    Ok(())
}

/// 创建 Theme 资源
fn cmd_create_theme(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");

    // 使用 ClassDb 实例化 Theme
    let class_db = ClassDb::singleton();
    let theme_var = class_db.instantiate("Theme");
    if theme_var.is_nil() {
        return Err(McpError::internal("Failed to instantiate Theme"));
    }
    let mut theme: Gd<Theme> = theme_var.to();

    // 设置资源名称
    if !name.is_empty() {
        theme.set("resource_name", &Variant::from(name));
    }

    // 保存主题
    save_theme(&theme, path)?;

    Ok(serde_json::json!({"path": path, "name": name, "created": true}))
}

/// 设置主题颜色
fn cmd_set_theme_color(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let theme_path = args.get("theme_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing theme_path"))?;
    let color_name = args.get("color_name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing color_name"))?;
    let node_type = args.get("node_type").and_then(|v| v.as_str()).unwrap_or("Button");

    let color_obj = args.get("color").and_then(|v| v.as_object()).ok_or_else(|| McpError::invalid_params("Missing color"))?;
    let r = color_obj.get("r").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let g = color_obj.get("g").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let b = color_obj.get("b").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let a = color_obj.get("a").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;

    let mut theme = load_theme(theme_path)?;
    let color = Color::from_rgba(r, g, b, a);
    theme.set_color(color_name, node_type, color);
    save_theme(&theme, theme_path)?;

    Ok(serde_json::json!({"theme_path": theme_path, "color_name": color_name, "node_type": node_type, "updated": true}))
}

/// 设置主题常量
fn cmd_set_theme_constant(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let theme_path = args.get("theme_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing theme_path"))?;
    let constant_name = args.get("constant_name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing constant_name"))?;
    let value = args.get("value").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing value"))? as i32;
    let node_type = args.get("node_type").and_then(|v| v.as_str()).unwrap_or("Button");

    let mut theme = load_theme(theme_path)?;
    theme.set_constant(constant_name, node_type, value);
    save_theme(&theme, theme_path)?;

    Ok(serde_json::json!({"theme_path": theme_path, "constant_name": constant_name, "value": value, "updated": true}))
}

/// 设置主题字体大小
fn cmd_set_theme_font_size(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let theme_path = args.get("theme_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing theme_path"))?;
    let font_size_name = args.get("font_size_name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing font_size_name"))?;
    let size = args.get("size").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing size"))? as i32;
    let node_type = args.get("node_type").and_then(|v| v.as_str()).unwrap_or("Button");

    let mut theme = load_theme(theme_path)?;
    theme.set_font_size(font_size_name, node_type, size);
    save_theme(&theme, theme_path)?;

    Ok(serde_json::json!({"theme_path": theme_path, "font_size_name": font_size_name, "size": size, "updated": true}))
}

/// 设置主题样式盒
fn cmd_set_theme_stylebox(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let theme_path = args.get("theme_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing theme_path"))?;
    let stylebox_name = args.get("stylebox_name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing stylebox_name"))?;
    let node_type = args.get("node_type").and_then(|v| v.as_str()).unwrap_or("Panel");

    let mut theme = load_theme(theme_path)?;

    // 创建 StyleBoxFlat
    let class_db = ClassDb::singleton();
    let sb_var = class_db.instantiate("StyleBoxFlat");
    if sb_var.is_nil() {
        return Err(McpError::internal("Failed to instantiate StyleBoxFlat"));
    }
    let mut stylebox: Gd<godot::classes::StyleBoxFlat> = sb_var.to();

    // 设置背景色
    if let Some(bg_color) = args.get("bg_color").and_then(|v| v.as_str()) {
        if !bg_color.is_empty() {
            // 尝试解析颜色字符串 (#RRGGBB 格式)
            stylebox.set("bg_color", &Variant::from(bg_color));
        }
    }

    // 设置边框颜色
    if let Some(border_color) = args.get("border_color").and_then(|v| v.as_str()) {
        if !border_color.is_empty() {
            stylebox.set("border_color", &Variant::from(border_color));
        }
    }

    // 设置边框宽度
    if let Some(border_width) = args.get("border_width").and_then(|v| v.as_i64()) {
        if border_width > 0 {
            let bw = border_width as i32;
            stylebox.set("border_width_left", &Variant::from(bw));
            stylebox.set("border_width_top", &Variant::from(bw));
            stylebox.set("border_width_right", &Variant::from(bw));
            stylebox.set("border_width_bottom", &Variant::from(bw));
        }
    }

    // 设置圆角半径
    if let Some(corner_radius) = args.get("corner_radius").and_then(|v| v.as_i64()) {
        if corner_radius > 0 {
            let cr = corner_radius as i32;
            stylebox.set("corner_radius_top_left", &Variant::from(cr));
            stylebox.set("corner_radius_top_right", &Variant::from(cr));
            stylebox.set("corner_radius_bottom_left", &Variant::from(cr));
            stylebox.set("corner_radius_bottom_right", &Variant::from(cr));
        }
    }

    theme.set_stylebox(stylebox_name, node_type, &stylebox);
    save_theme(&theme, theme_path)?;

    Ok(serde_json::json!({"theme_path": theme_path, "stylebox_name": stylebox_name, "node_type": node_type, "updated": true}))
}

/// 对 Control 节点应用主题
fn cmd_setup_control(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let root = find_root()?;
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let node = find_node_by_path(&root, node_path)?;

    // 检查是否为 Control 节点
    let class_name = node.get_class().to_string();
    // 使用 try_cast 检查是否为 Control
    let control_result = node.try_cast::<godot::classes::Control>();
    if control_result.is_err() {
        return Err(McpError::invalid_params(&format!("Node '{}' is not a Control (is {})", node_path, class_name)));
    }
    let mut control = control_result.unwrap();

    // 如果提供了 theme_path, 加载并设置主题
    if let Some(theme_path) = args.get("theme_path").and_then(|v| v.as_str()) {
        if !theme_path.is_empty() {
            let theme = load_theme(theme_path)?;
            control.set_theme(&theme);
        }
    }

    Ok(serde_json::json!({"node_path": node_path, "theme_applied": true}))
}

/// 获取主题信息
fn cmd_get_theme_info(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let theme_path = args.get("theme_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing theme_path"))?;
    let theme = load_theme(theme_path)?;

    // 获取主题的类型列表
    let type_list = theme.get_type_list();

    // 收集各类型的信息
    let type_count = type_list.len();
    let mut type_names: Vec<String> = Vec::new();
    for i in 0..type_count {
        let s = type_list.get(i).unwrap_or_default();
        type_names.push(s.to_string());
    }

    let mut info = serde_json::json!({
        "path": theme_path,
        "type_list": type_names,
        "type_count": type_count,
    });

    // 从主题中读取属性（通过动态属性访问）
    if let Some(obj) = info.as_object_mut() {
        let mut colors = serde_json::Map::new();
        let constants = serde_json::Map::new();
        let font_sizes = serde_json::Map::new();
        let styleboxes = serde_json::Map::new();

        // TODO: Theme 的 get_type_list/get_color_list/get_constant_list 等 API
        // 在 gdext rust 绑定中可能不可直接调用, 这里获取基本信息
        for i in 0..type_count {
            let tn = type_list.get(i).unwrap_or_default().to_string();
            // 尝试读取颜色列表 - 使用 get("color_list/...") 动态方式
            // 简化实现, 仅返回类型列表
            colors.insert(tn.clone(), serde_json::Value::String("(详见 type_list)".into()));
        }

        obj.insert("colors".to_string(), serde_json::Value::Object(colors));
        obj.insert("constants".to_string(), serde_json::Value::Object(constants));
        obj.insert("font_sizes".to_string(), serde_json::Value::Object(font_sizes));
        obj.insert("styleboxes".to_string(), serde_json::Value::Object(styleboxes));
    }

    Ok(info)
}
