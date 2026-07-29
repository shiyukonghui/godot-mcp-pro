//! 脚本工具命令模块

use std::collections::HashMap;
use godot::classes::file_access::ModeFlags;
use godot::classes::{DirAccess, EditorInterface, FileAccess, ResourceLoader};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("list_scripts", "列出所有脚本文件", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        ToolDefinition::new("read_script", "读取脚本文件", serde_json::json!({
            "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"]
        })),
        ToolDefinition::new("create_script", "创建脚本文件", serde_json::json!({
            "type": "object", "properties": { "path": { "type": "string" }, "content": { "type": "string" }, "template": { "type": "string", "default": "Node" } }, "required": ["path"]
        })),
        ToolDefinition::new("edit_script", "编辑脚本文件", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" }, "search": { "type": "string" }, "replace": { "type": "string" },
                "content": { "type": "string", "description": "直接替换整个内容" }
            }, "required": ["path"]
        })),
        ToolDefinition::new("attach_script", "为节点附加脚本", serde_json::json!({
            "type": "object", "properties": { "node_path": { "type": "string" }, "script_path": { "type": "string" } }, "required": ["node_path", "script_path"]
        })),
        // 获取编辑器中打开的脚本列表
        ToolDefinition::new("get_open_scripts", "获取编辑器中打开的所有脚本", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        // 验证脚本语法
        ToolDefinition::new("validate_script", "验证脚本语法", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" }
            }, "required": ["path"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("list_scripts".into(), cmd_list_scripts);
    registry.insert("read_script".into(), cmd_read_script);
    registry.insert("create_script".into(), cmd_create_script);
    registry.insert("edit_script".into(), cmd_edit_script);
    registry.insert("attach_script".into(), cmd_attach_script);
    registry.insert("get_open_scripts".into(), cmd_get_open_scripts);
    registry.insert("validate_script".into(), cmd_validate_script);
}

fn collect_gd_files(path: &str, files: &mut Vec<String>) {
    let mut dir = match DirAccess::open(path) { Some(d) => d, None => return };
    dir.list_dir_begin();
    loop {
        let f = dir.get_next().to_string();
        if f.is_empty() { break; }
        if f == "." || f == ".." { continue; }
        let full = if path == "res://" { format!("res://{}", f) } else { format!("{}/{}", path, f) };
        if dir.current_is_dir() { collect_gd_files(&full, files); }
        else if f.ends_with(".gd") || f.ends_with(".gdshader") { files.push(full); }
    }
    dir.list_dir_end();
}

fn cmd_list_scripts(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let mut files = Vec::new(); collect_gd_files("res://", &mut files);
    Ok(serde_json::json!({"scripts": files, "count": files.len()}))
}

fn cmd_read_script(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let mut file = FileAccess::open(path, ModeFlags::READ).ok_or_else(|| McpError::not_found(&format!("File '{}'", path), ""))?;
    let content = file.get_as_text().to_string(); file.close();
    Ok(serde_json::json!({"path": path, "content": content, "size": content.len()}))
}

fn cmd_create_script(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let template = args.get("template").and_then(|v| v.as_str()).unwrap_or("Node");
    if let Some(p) = std::path::Path::new(path).parent() { let _ = DirAccess::make_dir_recursive_absolute(&p.to_string_lossy().to_string()); }
    let code = if content.is_empty() { format!("extends {}\n\n# TODO\n", template) } else { content.to_string() };
    let mut file = FileAccess::open(path, ModeFlags::WRITE).ok_or_else(|| McpError::internal("Cannot create file"))?;
    file.store_string(&code); file.close();
    if let Some(mut fs) = EditorInterface::singleton().get_resource_filesystem() { fs.scan(); }
    Ok(serde_json::json!({"path": path, "created": true, "size": code.len()}))
}

fn cmd_edit_script(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
        let mut file = FileAccess::open(path, ModeFlags::WRITE).ok_or_else(|| McpError::internal("Cannot open file"))?;
        file.store_string(content); file.close();
        return Ok(serde_json::json!({"path": path, "replaced": true, "size": content.len()}));
    }
    let search = args.get("search").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing search"))?;
    let replace = args.get("replace").and_then(|v| v.as_str()).unwrap_or("");
    let mut file = FileAccess::open(path, ModeFlags::READ).ok_or_else(|| McpError::internal("Cannot read file"))?;
    let content = file.get_as_text().to_string(); file.close();
    let new_content = content.replace(search, replace);
    let mut wf = FileAccess::open(path, ModeFlags::WRITE).ok_or_else(|| McpError::internal("Cannot write file"))?;
    wf.store_string(&new_content); wf.close();
    Ok(serde_json::json!({"path": path, "replacements": content.matches(search).count(), "size": new_content.len()}))
}

fn cmd_attach_script(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let np = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let sp = args.get("script_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing script_path"))?;
    let mut rl = ResourceLoader::singleton();
    let script_gd = rl.load(sp);
    match script_gd {
        Some(script) => {
            // 验证加载的资源确实是 Script 类型
            let class_name = script.get_class().to_string();
            if !script.is_class("Script") {
                return Err(McpError::invalid_params(&format!(
                    "Resource at '{}' is not a Script (loaded as: {})",
                    sp, class_name
                )));
            }
            if !root.has_node(np) { return Err(McpError::not_found(&format!("Node '{}'", np), "")); }
            let mut node = root.get_node_as::<godot::classes::Node>(np);
            // 通过 Variant 方式设置 script，绕开类型约束
            node.set("script", &Variant::from(script));
            Ok(serde_json::json!({"attached": true, "node": np, "script": sp}))
        }
        None => Err(McpError::not_found(&format!("Script '{}'", sp), "")),
    }
}

/// 获取编辑器中打开的所有脚本
fn cmd_get_open_scripts(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let script_editor = editor.get_script_editor().ok_or_else(|| McpError::internal("Script editor not available"))?;
    let open_scripts = script_editor.get_open_scripts();

    let mut scripts: Vec<serde_json::Value> = Vec::new();
    for i in 0..open_scripts.len() {
        if let Some(s) = open_scripts.get(i) {
            let path = s.get("resource_path").to::<String>();
            let class_name = s.get_class().to_string();
            scripts.push(serde_json::json!({
                "path": path,
                "type": class_name,
            }));
        }
    }

    Ok(serde_json::json!({"scripts": scripts, "count": scripts.len()}))
}

/// 验证脚本语法
fn cmd_validate_script(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;

    use godot::classes::file_access::ModeFlags;
    if !FileAccess::file_exists(path) {
        return Err(McpError::not_found(&format!("Script '{}'", path), ""));
    }

    let mut file = FileAccess::open(path, ModeFlags::READ).ok_or_else(|| McpError::internal("Cannot read script"))?;
    let source_code = file.get_as_text().to_string();
    file.close();

    // 创建 GDScript 实例来验证语法
    let mut script = godot::classes::GDScript::new_gd();
    script.set_source_code(&source_code);
    let err_code = script.reload();

    if err_code == godot::global::Error::OK {
        Ok(serde_json::json!({"path": path, "valid": true, "message": "Script compiles successfully"}))
    } else {
        // 返回完整错误信息
        let err_str = format!("{:?}", err_code);
        Ok(serde_json::json!({
            "path": path,
            "valid": false,
            "error_text": err_str,
            "message": "Compilation failed. Check the script for errors."
        }))
    }
}
