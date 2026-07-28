//! Shader 命令模块

use std::collections::HashMap;
use godot::classes::file_access::ModeFlags;
use godot::classes::{DirAccess, EditorInterface, FileAccess, ResourceLoader};
use godot::prelude::*;
use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("read_shader", "读取着色器文件", serde_json::json!({
            "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"]
        })),
        ToolDefinition::new("create_shader", "创建着色器文件", serde_json::json!({
            "type": "object", "properties": { "path": { "type": "string" }, "shader_type": { "type": "string", "default": "shader_type spatial;" } }, "required": ["path"]
        })),
        // 编辑着色器代码
        ToolDefinition::new("edit_shader", "编辑着色器代码", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" },
                "code": { "type": "string" }
            }, "required": ["path", "code"]
        })),
        // 分配着色器材质到节点
        ToolDefinition::new("assign_shader_material", "为节点分配 ShaderMaterial", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "shader_path": { "type": "string" },
                "material_slot": { "type": "string", "default": "material" }
            }, "required": ["node_path", "shader_path"]
        })),
        // 设置着色器参数
        ToolDefinition::new("set_shader_param", "设置着色器参数", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "param": { "type": "string" },
                "value": { "description": "参数值" }
            }, "required": ["node_path", "param", "value"]
        })),
        // 获取着色器参数列表
        ToolDefinition::new("get_shader_params", "获取着色器参数列表", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" }
            }, "required": ["path"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("read_shader".into(), cmd_read_shader);
    registry.insert("create_shader".into(), cmd_create_shader);
    registry.insert("edit_shader".into(), cmd_edit_shader);
    registry.insert("assign_shader_material".into(), cmd_assign_shader_material);
    registry.insert("set_shader_param".into(), cmd_set_shader_param);
    registry.insert("get_shader_params".into(), cmd_get_shader_params);
}

fn cmd_read_shader(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let mut file = FileAccess::open(path, ModeFlags::READ).ok_or_else(|| McpError::not_found(&format!("File '{}'", path), ""))?;
    let content = file.get_as_text().to_string(); file.close();
    Ok(serde_json::json!({"path": path, "content": content}))
}

fn cmd_create_shader(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let stype = args.get("shader_type").and_then(|v| v.as_str()).unwrap_or("shader_type spatial;");
    let code = format!("{}\n\nvoid fragment() {{\n}}\n", stype);
    if let Some(p) = std::path::Path::new(path).parent() { let _ = DirAccess::make_dir_recursive_absolute(&p.to_string_lossy().to_string()); }
    let mut f = FileAccess::open(path, ModeFlags::WRITE).ok_or_else(|| McpError::internal("Cannot create file"))?;
    f.store_string(&code); f.close();
    Ok(serde_json::json!({"path": path, "created": true}))
}

/// 编辑着色器代码
fn cmd_edit_shader(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let code = args.get("code").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing code"))?;
    let mut file = FileAccess::open(path, ModeFlags::WRITE).ok_or_else(|| McpError::internal("Cannot open file for writing"))?;
    file.store_string(code);
    file.close();
    // 刷新编辑器中的着色器缓存
    EditorInterface::singleton().get_resource_filesystem().map(|mut fs| fs.scan());
    Ok(serde_json::json!({"path": path, "edited": true, "size": code.len()}))
}

/// 为节点分配 ShaderMaterial
fn cmd_assign_shader_material(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let shader_path = args.get("shader_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing shader_path"))?;
    let _material_slot = args.get("material_slot").and_then(|v| v.as_str()).unwrap_or("material");

    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    if !root.has_node(node_path) {
        return Err(McpError::not_found(&format!("Node '{}'", node_path), ""));
    }

    // 加载着色器资源
    let mut rl = ResourceLoader::singleton();
    let shader = rl.load(shader_path).ok_or_else(|| McpError::not_found(&format!("Shader '{}'", shader_path), ""))?;

    // 创建 ShaderMaterial 并设置着色器
    let mut material = godot::classes::ShaderMaterial::new_gd();
    material.set("shader", &Variant::from(shader));

    // 设置材质到节点
    let mut node = root.get_node_as::<godot::classes::Node>(node_path);
    node.set("material", &Variant::from(material));

    Ok(serde_json::json!({"node_path": node_path, "shader_path": shader_path, "assigned": true}))
}

/// 设置着色器参数
fn cmd_set_shader_param(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let param = args.get("param").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing param"))?;
    let value = args.get("value").ok_or_else(|| McpError::invalid_params("Missing value"))?;

    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    if !root.has_node(node_path) {
        return Err(McpError::not_found(&format!("Node '{}'", node_path), ""));
    }
    let mut node = root.get_node_as::<godot::classes::Node>(node_path);

    // 获取节点的材质
    let _material = node.get("material");
    // 通过 Variant 设置 shader parameter
    // 使用 serialize 模块解析值
    let variant = crate::utils::serialize::parse_value_for_property(value);
    node.set(&format!("material:shader_parameter/{}", param), &variant);

    Ok(serde_json::json!({"node_path": node_path, "param": param, "value": value, "set": true}))
}

/// 获取着色器参数列表
fn cmd_get_shader_params(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;

    // 加载 Shader 资源
    let mut rl = ResourceLoader::singleton();
    let _shader = rl.load(path).ok_or_else(|| McpError::not_found(&format!("Shader '{}'", path), ""))?;

    // 获取着色器参数列表 - 使用 GDScript Expression
    let mut params = serde_json::Map::new();
    let code = format!(
        "var sh = load('{}'); \
         var props = []; \
         for p in sh.get_property_list(): \
           if p.name.begins_with('shader_parameter/'): \
             props.append({{'name': p.name.trim_prefix('shader_parameter/'), 'type': p.type}}); \
         return props",
        path.replace('\'', "\\'")
    );
    let mut expr = godot::classes::Expression::new_gd();
    if expr.parse(&code) == godot::global::Error::OK {
        let result = expr.execute();
        let arr = result.to::<godot::builtin::VarArray>();
        for i in 0..arr.len() {
            if let Some(entry) = arr.get(i) {
                let dict: godot::builtin::Dictionary<godot::builtin::Variant, godot::builtin::Variant> = entry.to();
                if let Some(dict_entry) = dict.get("name") {
                    let name = dict_entry.to::<String>();
                    if let Some(type_entry) = dict.get("type") {
                        let val = type_entry.to::<String>();
                        params.insert(name, serde_json::Value::String(val));
                    }
                }
            }
        }
    }

    Ok(serde_json::json!({"path": path, "params": params, "count": params.len()}))
}
