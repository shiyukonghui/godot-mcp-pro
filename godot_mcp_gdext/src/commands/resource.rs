//! 资源命令模块
//! 对应原 GDScript 插件 resource_commands.gd

use std::collections::HashMap;

use godot::classes::{
    ClassDb, DirAccess, EditorInterface, FileAccess, Image, ProjectSettings,
    ResourceLoader, ResourceSaver, Texture2D,
};
use godot::global::Error;
use godot::obj::Gd;
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;
use crate::utils::serialize;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("read_resource", "读取资源文件", serde_json::json!({
            "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"]
        })),
        ToolDefinition::new("add_autoload", "注册自动加载", serde_json::json!({
            "type": "object", "properties": { "name": { "type": "string" }, "path": { "type": "string" } }, "required": ["name", "path"]
        })),
        ToolDefinition::new("remove_autoload", "移除自动加载", serde_json::json!({
            "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"]
        })),
        // 编辑资源文件属性
        ToolDefinition::new("edit_resource", "编辑资源文件属性并保存", serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "资源文件路径 (res://)" },
                "properties": { "type": "object", "description": "要修改的属性字典" }
            },
            "required": ["path", "properties"]
        })),
        // 创建新资源文件
        ToolDefinition::new("create_resource", "创建新资源文件", serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "保存路径 (res://)" },
                "type": { "type": "string", "description": "资源类型" },
                "properties": { "type": "object", "description": "初始属性字典" },
                "overwrite": { "type": "boolean", "description": "是否覆盖已存在的文件", "default": false }
            },
            "required": ["path", "type"]
        })),
        // 获取资源预览图片 (base64)
        ToolDefinition::new("get_resource_preview", "获取资源预览图片 (base64编码PNG)", serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "资源文件路径 (res://)" },
                "max_size": { "type": "integer", "description": "最大尺寸 (默认 256)", "default": 256 }
            },
            "required": ["path"]
        })),
    ]
}

pub fn register(
    registry: &mut HashMap<
        String,
        fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>,
    >,
) {
    registry.insert("read_resource".into(), cmd_read_resource);
    registry.insert("add_autoload".into(), cmd_add_autoload);
    registry.insert("remove_autoload".into(), cmd_remove_autoload);
    registry.insert("edit_resource".into(), cmd_edit_resource);
    registry.insert("create_resource".into(), cmd_create_resource);
    registry.insert("get_resource_preview".into(), cmd_get_resource_preview);
}

fn cmd_read_resource(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let mut rl = ResourceLoader::singleton();
    let res = rl.load(path);
    match res {
        Some(r) => Ok(serde_json::json!({
            "path": path,
            "type": r.get_class().to_string(),
            "loaded": true
        })),
        None => Err(McpError::not_found(&format!("Resource '{}'", path), "")),
    }
}

fn cmd_add_autoload(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing name"))?;
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let mut ps = ProjectSettings::singleton();
    ps.set_setting(
        &format!("autoload/{}", name),
        &Variant::from(format!("*{}", path)),
    );
    ps.save();
    Ok(serde_json::json!({"autoload": name, "path": path, "added": true}))
}

fn cmd_remove_autoload(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing name"))?;
    let mut ps = ProjectSettings::singleton();
    let key = format!("autoload/{}", name);
    if ps.has_setting(&key) {
        ps.set_setting(&key, &Variant::nil());
        ps.save();
    }
    Ok(serde_json::json!({"autoload": name, "removed": true}))
}

/// 编辑资源文件属性: 加载 .tres/.res 文件, 修改属性后保存
fn cmd_edit_resource(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: path"))?;
    let properties = args
        .get("properties")
        .and_then(|v| v.as_object())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: properties"))?;

    // 加载资源
    let mut rl = ResourceLoader::singleton();
    let mut resource = rl
        .load(path)
        .ok_or_else(|| McpError::internal(&format!("Failed to load resource: {}", path)))?;

    // 遍历属性, 记录变更并设置新值
    let mut changed = serde_json::Map::new();
    for (prop_name, new_val) in properties {
        let old_var = resource.get(prop_name);
        // 跳过不存在的属性 (get 返回 nil)
        if old_var.is_nil() {
            continue;
        }
        let old_serialized = serialize::serialize_variant(&old_var);
        let parsed = serialize::parse_value_for_property(new_val);
        resource.set(prop_name, &parsed);

        let new_var = resource.get(prop_name);
        changed.insert(
            prop_name.clone(),
            serde_json::json!({
                "old": old_serialized,
                "new": serialize::serialize_variant(&new_var),
            }),
        );
    }

    if changed.is_empty() {
        return Ok(serde_json::json!({
            "path": path,
            "changed": {},
            "message": "No properties were changed"
        }));
    }

    // 保存资源
    let err = ResourceSaver::singleton()
        .save_ex(&resource)
        .path(path)
        .done();
    if err != Error::OK {
        return Err(McpError::internal(&format!(
            "Failed to save resource: {:?}",
            err
        )));
    }

    Ok(serde_json::json!({
        "path": path,
        "type": resource.get_class().to_string(),
        "changed": changed,
    }))
}

/// 创建新资源: 使用 ClassDb 实例化指定类型, 设置属性后保存
fn cmd_create_resource(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: path"))?;
    let resource_type = args
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: type"))?;
    let overwrite = args
        .get("overwrite")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // 检查文件是否已存在
    if !overwrite && FileAccess::file_exists(path) {
        return Err(McpError {
            code: -32000,
            message: format!("Resource already exists: {}", path),
            data: Some(serde_json::json!({"suggestion": "Set overwrite=true to replace"})),
        });
    }

    // 使用 ClassDb 实例化资源
    let class_db = ClassDb::singleton();
    let resource_var = class_db.instantiate(resource_type);
    if resource_var.is_nil() {
        return Err(McpError::internal(&format!(
            "Failed to instantiate: {}",
            resource_type
        )));
    }
    let mut resource: Gd<Resource> = resource_var.to();

    // 设置初始属性
    let mut properties_set: Vec<String> = Vec::new();
    if let Some(properties) = args.get("properties").and_then(|v| v.as_object()) {
        for (key, val) in properties {
            let current = resource.get(key);
            if !current.is_nil() {
                resource.set(key, &serialize::parse_value_for_property(val));
                properties_set.push(key.clone());
            }
        }
    }

    // 确保父目录存在
    if let Some(parent) = std::path::Path::new(path).parent() {
        let parent_str = parent.to_string_lossy().to_string();
        let _ = DirAccess::make_dir_recursive_absolute(&parent_str);
    }

    // 保存资源
    let err = ResourceSaver::singleton()
        .save_ex(&resource)
        .path(path)
        .done();
    if err != Error::OK {
        return Err(McpError::internal(&format!(
            "Failed to save resource: {:?}",
            err
        )));
    }

    // 刷新文件系统
    if let Some(mut fs) = EditorInterface::singleton().get_resource_filesystem() {
        fs.scan();
    }

    Ok(serde_json::json!({
        "path": path,
        "type": resource_type,
        "properties_set": properties_set,
    }))
}

/// 获取资源预览图片: 返回 base64 编码的 PNG
fn cmd_get_resource_preview(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: path"))?;
    let max_size = args
        .get("max_size")
        .and_then(|v| v.as_i64())
        .unwrap_or(256) as i32;

    let image: Option<Gd<Image>>;

    // 检查扩展名是否为常见图片格式
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "webp" | "svg") {
        // 直接加载图片文件
        let mut img = Image::new_gd();
        let err = img.load(path);
        if err != Error::OK {
            return Err(McpError::internal(&format!(
                "Failed to load image: {:?}",
                err
            )));
        }
        image = Some(img);
    } else {
        // 尝试作为资源加载并提取图片
        let mut rl = ResourceLoader::singleton();
        let resource = rl
            .load(path)
            .ok_or_else(|| McpError::internal(&format!("Failed to load resource: {}", path)))?;

        // 尝试转换为 Texture2D 或 Image
        let class_name = resource.get_class().to_string();
        // 使用 try_cast 链: 先尝试 Texture2D, 失败后再尝试 Image
        match resource.try_cast::<Texture2D>() {
            Ok(tex) => {
                image = tex.get_image();
            }
            Err(res) => match res.try_cast::<Image>() {
                Ok(img) => {
                    image = Some(img);
                }
                Err(_) => {
                    return Err(McpError::invalid_params(&format!(
                        "Resource type '{}' does not have an image preview",
                        class_name
                    )));
                }
            },
        }
    }

    let mut img = image.ok_or_else(|| McpError::internal("Could not extract image from resource"))?;

    let width = img.get_width();
    let height = img.get_height();

    // 按比例缩放图片到 max_size 以内
    if width > max_size || height > max_size {
        let scale_x = max_size as f64 / width as f64;
        let scale_y = max_size as f64 / height as f64;
        let scale = scale_x.min(scale_y);
        let new_w = (width as f64 * scale) as i32;
        let new_h = (height as f64 * scale) as i32;
        img.resize(new_w, new_h);
    }

    // 保存为 PNG 并 base64 编码
    let png_data = img.save_png_to_buffer();
    use base64::Engine as _;
    let b64 =
        base64::engine::general_purpose::STANDARD.encode(png_data.as_slice());

    Ok(serde_json::json!({
        "image_base64": b64,
        "width": img.get_width(),
        "height": img.get_height(),
        "format": "png",
        "path": path,
    }))
}
