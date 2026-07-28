//! 项目信息命令模块

use std::collections::HashMap;

use godot::classes::file_access::ModeFlags;
use godot::classes::{DirAccess, EditorInterface, FileAccess, ProjectSettings, ResourceUid};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("get_project_info", "获取项目信息", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        // 获取文件系统树
        ToolDefinition::new("get_filesystem_tree", "获取文件系统树状结构", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "default": "res://" },
                "max_depth": { "type": "integer", "default": -1 }
            }, "required": []
        })),
        // 搜索文件
        ToolDefinition::new("search_files", "搜索文件", serde_json::json!({
            "type": "object", "properties": {
                "pattern": { "type": "string" },
                "path": { "type": "string", "default": "res://" }
            }, "required": ["pattern"]
        })),
        // 在文件内容中搜索
        ToolDefinition::new("search_in_files", "在文件内容中搜索文本", serde_json::json!({
            "type": "object", "properties": {
                "pattern": { "type": "string" },
                "file_pattern": { "type": "string", "default": "*" },
                "path": { "type": "string", "default": "res://" }
            }, "required": ["pattern"]
        })),
        // 获取项目设置
        ToolDefinition::new("get_project_settings", "获取项目设置", serde_json::json!({
            "type": "object", "properties": {
                "prefix": { "type": "string" },
                "include_default": { "type": "boolean", "default": false }
            }, "required": []
        })),
        // 设置项目设置
        ToolDefinition::new("set_project_setting", "设置项目设置", serde_json::json!({
            "type": "object", "properties": {
                "key": { "type": "string" },
                "value": { "description": "设置值" },
                "type": { "type": "string" }
            }, "required": ["key", "value"]
        })),
        // UID 转项目路径
        ToolDefinition::new("uid_to_project_path", "将 UID 转换为项目路径", serde_json::json!({
            "type": "object", "properties": {
                "uid": { "type": "string" }
            }, "required": ["uid"]
        })),
        // 项目路径转 UID
        ToolDefinition::new("project_path_to_uid", "将项目路径转换为 UID", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" }
            }, "required": ["path"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("get_project_info".into(), cmd_get_project_info);
    registry.insert("get_filesystem_tree".into(), cmd_get_filesystem_tree);
    registry.insert("search_files".into(), cmd_search_files);
    registry.insert("search_in_files".into(), cmd_search_in_files);
    registry.insert("get_project_settings".into(), cmd_get_project_settings);
    registry.insert("set_project_setting".into(), cmd_set_project_setting);
    registry.insert("uid_to_project_path".into(), cmd_uid_to_project_path);
    registry.insert("project_path_to_uid".into(), cmd_project_path_to_uid);
}

fn cmd_get_project_info(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let project = ProjectSettings::singleton();
    let editor = EditorInterface::singleton();

    let project_name = get_setting(&project, "application/config/name");
    let version = get_setting(&project, "application/config/version");

    let screen_size = editor.get_base_control()
        .map(|ctrl| ctrl.get_size())
        .map(|s| serde_json::json!({"width": s.x, "height": s.y}))
        .unwrap_or(serde_json::json!({"width": 0, "height": 0}));

    Ok(serde_json::json!({
        "project_name": project_name,
        "version": version,
        "editor_screen_size": screen_size,
    }))
}

fn get_setting(project: &ProjectSettings, key: &str) -> String {
    if project.has_setting(key) {
        project.get_setting(key).to::<String>()
    } else {
        String::new()
    }
}

/// 递归扫描目录，构建文件系统树
fn scan_directory(path: &str, max_depth: i64, current_depth: i64) -> serde_json::Value {
    // 如果达到最大深度则停止递归
    if max_depth >= 0 && current_depth > max_depth {
        return serde_json::json!({"name": path.split('/').last().unwrap_or(path), "path": path, "type": "directory"});
    }

    let mut dir = match DirAccess::open(path) {
        Some(d) => d,
        None => return serde_json::json!({"name": path.split('/').last().unwrap_or(path), "path": path, "type": "directory"}),
    };

    let mut result = serde_json::json!({
        "name": if path == "res://" { "res://" } else { path.split('/').last().unwrap_or(path) },
        "path": path,
        "type": "directory"
    });

    let mut children: Vec<serde_json::Value> = Vec::new();
    dir.list_dir_begin();
    loop {
        let f = dir.get_next().to_string();
        if f.is_empty() { break; }
        if f == "." || f == ".." { continue; }
        let full = if path == "res://" { format!("res://{}", f) } else { format!("{}/{}", path, f) };
        if dir.current_is_dir() {
            children.push(scan_directory(&full, max_depth, current_depth + 1));
        } else {
            children.push(serde_json::json!({
                "name": f,
                "path": full,
                "type": "file"
            }));
        }
    }
    dir.list_dir_end();

    if !children.is_empty() {
        result.as_object_mut().unwrap().insert("children".into(), serde_json::Value::Array(children));
    }
    result
}

/// 获取文件系统树
fn cmd_get_filesystem_tree(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("res://");
    let max_depth = args.get("max_depth").and_then(|v| v.as_i64()).unwrap_or(-1);
    let tree = scan_directory(path, max_depth, 0);
    Ok(serde_json::json!({"tree": tree}))
}

/// 递归搜索文件
fn search_files_recursive(path: &str, query: &str, matches: &mut Vec<String>, max_results: usize) {
    if matches.len() >= max_results { return; }
    let mut dir = match DirAccess::open(path) {
        Some(d) => d,
        None => return,
    };
    dir.list_dir_begin();
    loop {
        let f = dir.get_next().to_string();
        if f.is_empty() { break; }
        if f == "." || f == ".." { continue; }
        let full = if path == "res://" { format!("res://{}", f) } else { format!("{}/{}", path, f) };
        if dir.current_is_dir() {
            search_files_recursive(&full, query, matches, max_results);
        } else {
            // 匹配文件名（不区分大小写）
            if f.to_lowercase().contains(&query.to_lowercase()) {
                matches.push(full);
            }
        }
        if matches.len() >= max_results { break; }
    }
    dir.list_dir_end();
}

/// 搜索文件
fn cmd_search_files(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing pattern"))?;
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("res://");
    let mut matches = Vec::new();
    search_files_recursive(path, pattern, &mut matches, 200);
    Ok(serde_json::json!({"matches": matches, "count": matches.len()}))
}

/// 在文件内容中递归搜索
fn search_in_files_recursive(path: &str, query: &str, file_pattern: &str, matches: &mut Vec<serde_json::Value>, max_results: usize) {
    if matches.len() >= max_results { return; }
    let mut dir = match DirAccess::open(path) {
        Some(d) => d,
        None => return,
    };
    dir.list_dir_begin();
    loop {
        let f = dir.get_next().to_string();
        if f.is_empty() { break; }
        if f == "." || f == ".." { continue; }
        let full = if path == "res://" { format!("res://{}", f) } else { format!("{}/{}", path, f) };
        if dir.current_is_dir() {
            // 跳过 addons 和 .godot 目录
            if f != "addons" && f != ".godot" {
                search_in_files_recursive(&full, query, file_pattern, matches, max_results);
            }
        } else {
            // 检查文件模式
            if file_pattern != "*" && !f.contains(file_pattern.trim_matches('*')) { continue; }
            // 只搜索文本文件
            let ext = f.rsplit('.').last().unwrap_or("");
            if !["gd", "tscn", "tres", "cfg", "godot", "gdshader", "md", "txt", "json", "yaml", "yml", "xml", "csv", "ini"].contains(&ext) { continue; }
            // 读取文件内容
            if let Some(mut file) = FileAccess::open(&full, ModeFlags::READ) {
                let content = file.get_as_text().to_string();
                file.close();
                for (i, line) in content.lines().enumerate() {
                    if matches.len() >= max_results { break; }
                    if line.to_lowercase().contains(&query.to_lowercase()) {
                        matches.push(serde_json::json!({
                            "file": full,
                            "line": i + 1,
                            "text": line.trim()
                        }));
                    }
                }
            }
        }
        if matches.len() >= max_results { break; }
    }
    dir.list_dir_end();
}

/// 在文件内容中搜索
fn cmd_search_in_files(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing pattern"))?;
    let file_pattern = args.get("file_pattern").and_then(|v| v.as_str()).unwrap_or("*");
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("res://");
    let mut matches = Vec::new();
    search_in_files_recursive(path, pattern, file_pattern, &mut matches, 50);
    Ok(serde_json::json!({"matches": matches, "count": matches.len(), "query": pattern}))
}

/// 获取项目设置
fn cmd_get_project_settings(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let project = ProjectSettings::singleton();
    let prefix = args.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
    let _include_default = args.get("include_default").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut settings = serde_json::Map::new();
    // 通过 get_property_list 获取所有设置 - 使用 GDScript Expression
    let code = format!(
        "var ps = ProjectSettings.get_singleton(); \
         var list = ps.get_property_list(); \
         var result = []; \
         for p in list: \
           if p.name.begins_with('{}'): \
             result.append(p.name); \
         return result",
        prefix.replace('\'', "\\'")
    );
    let mut expr = godot::classes::Expression::new_gd();
    if expr.parse(&code) != godot::global::Error::OK {
        return Ok(serde_json::json!({"settings": {}, "count": 0, "error": "Script parsing failed"}));
    }
    let result = expr.execute();
    let names_arr = result.to::<godot::builtin::VarArray>();
    for i in 0..names_arr.len() {
        if let Some(name_v) = names_arr.get(i) {
            let name = name_v.to::<String>();
            let val = if project.has_setting(&name) {
                format!("{}", project.get_setting(&name))
            } else { String::new() };
            settings.insert(name, serde_json::Value::String(val));
        }
    }
    Ok(serde_json::json!({"settings": settings, "count": settings.len()}))
}

/// 设置项目设置
fn cmd_set_project_setting(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let key = args.get("key").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing key"))?;
    let value = args.get("value").ok_or_else(|| McpError::invalid_params("Missing value"))?;

    let mut project = ProjectSettings::singleton();
    // 通过 Variant 方式设置值
    // 使用 serialize 模块解析值
    let parsed = crate::utils::serialize::parse_value_for_property(value);
    project.set_setting(key, &parsed);
    project.save();

    Ok(serde_json::json!({"key": key, "value": value, "saved": true}))
}

/// UID 转项目路径
fn cmd_uid_to_project_path(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let uid_str = args.get("uid").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing uid"))?;
    let uid = ResourceUid::singleton();
    // 将文本 UID 转换为 ID
    let id = uid.text_to_id(uid_str);
    if id == -1 {
        return Err(McpError::invalid_params(&format!("Invalid UID format: {}", uid_str)));
    }
    let path = uid.get_id_path(id);
    Ok(serde_json::json!({"uid": uid_str, "path": path.to_string()}))
}

/// 项目路径转 UID
fn cmd_project_path_to_uid(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let _uid = ResourceUid::singleton();
    // 使用 GDScript Expression 获取 UID
    let code = format!("var uid = ResourceUID::get_singleton(); uid.id_to_text(uid.get_id_path('{}'))", path.replace('\'', "\\'"));
    let mut expr = godot::classes::Expression::new_gd();
    if expr.parse(&code) == godot::global::Error::OK {
        let result = expr.execute().to::<String>();
        if !result.is_empty() {
            return Ok(serde_json::json!({"path": path, "uid": result}));
        }
    }
    Ok(serde_json::json!({"path": path, "uid": ""}))
}
