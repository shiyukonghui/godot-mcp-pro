//! 批量操作命令模块

use std::collections::HashMap;

use godot::builtin::{StringName, Variant};
use godot::classes::file_access::ModeFlags;
use godot::classes::{
    ClassDb, DirAccess, EditorInterface, Expression, FileAccess, Node, PackedScene,
    ResourceLoader, ResourceSaver,
};
use godot::global::Error;
use godot::obj::Gd;
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;
use crate::utils::serialize;

/// 收集本模块的工具定义
pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        // 已有的工具
        ToolDefinition::new("find_nodes_by_type", "按类型查找所有节点", serde_json::json!({
            "type": "object", "properties": { "type": { "type": "string" } }, "required": ["type"]
        })),
        ToolDefinition::new("batch_set_property", "批量设置同类型节点的属性", serde_json::json!({
            "type": "object", "properties": {
                "node_type": { "type": "string" }, "property": { "type": "string" }, "value": {}
            }, "required": ["node_type", "property", "value"]
        })),
        // 1. find_signal_connections: 递归查找场景中所有信号连接
        ToolDefinition::new("find_signal_connections", "递归查找场景中所有信号连接", serde_json::json!({
            "type": "object", "properties": {
                "signal_name": { "type": "string", "description": "信号名过滤（可选，包含匹配）" },
                "node_path": { "type": "string", "description": "节点路径过滤（可选，包含匹配）" }
            }, "required": []
        })),
        // 2. batch_add_nodes: 批量添加节点到场景
        ToolDefinition::new("batch_add_nodes", "批量添加节点到场景", serde_json::json!({
            "type": "object", "properties": {
                "nodes": {
                    "type": "array",
                    "description": "节点数组，每个元素包含 type（必填）、parent_path、name、properties",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "description": "节点类型" },
                            "parent_path": { "type": "string", "description": "父节点路径，默认 \".\"" },
                            "name": { "type": "string", "description": "节点名称" },
                            "properties": { "type": "object", "description": "要设置的属性字典" }
                        },
                        "required": ["type"]
                    }
                }
            }, "required": ["nodes"]
        })),
        // 3. find_node_references: 在项目文件中搜索指定模式的引用
        ToolDefinition::new("find_node_references", "在项目文件中搜索指定模式的引用", serde_json::json!({
            "type": "object", "properties": {
                "pattern": { "type": "string", "description": "要搜索的模式" }
            }, "required": ["pattern"]
        })),
        // 4. get_scene_dependencies: 获取场景文件的依赖资源列表
        ToolDefinition::new("get_scene_dependencies", "获取场景文件的依赖资源列表", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "场景文件路径 (res://)" }
            }, "required": ["path"]
        })),
        // 5. cross_scene_set_property: 跨场景批量设置属性
        ToolDefinition::new("cross_scene_set_property", "跨场景批量设置节点属性", serde_json::json!({
            "type": "object", "properties": {
                "type": { "type": "string", "description": "节点类型" },
                "property": { "type": "string", "description": "属性名" },
                "value": { "description": "属性值" },
                "path_filter": { "type": "string", "description": "路径过滤，默认 res://" },
                "exclude_addons": { "type": "boolean", "description": "是否排除 addons 目录", "default": true },
                "force": { "type": "boolean", "description": "是否强制修改正在编辑的场景", "default": false },
                "dry_run": { "type": "boolean", "description": "仅预览，不实际修改" }
            }, "required": ["type", "property", "value"]
        })),
    ]
}

/// 注册本模块的命令处理函数
pub fn register(
    registry: &mut HashMap<
        String,
        fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>,
    >,
) {
    registry.insert("find_nodes_by_type".into(), cmd_find_nodes_by_type);
    registry.insert("batch_set_property".into(), cmd_batch_set_property);
    registry.insert("find_signal_connections".into(), cmd_find_signal_connections);
    registry.insert("batch_add_nodes".into(), cmd_batch_add_nodes);
    registry.insert("find_node_references".into(), cmd_find_node_references);
    registry.insert("get_scene_dependencies".into(), cmd_get_scene_dependencies);
    registry.insert("cross_scene_set_property".into(), cmd_cross_scene_set_property);
}

// ============================================================================
// 辅助函数: 递归按类型收集节点
// ============================================================================

/// 递归遍历节点树，收集指定类型的所有节点
fn collect_by_type(
    node: &Gd<Node>,
    type_name: &str,
    results: &mut Vec<serde_json::Value>,
    root: &Gd<Node>,
) {
    if node.get_class().to_string() == type_name || node.is_class(type_name) {
        results.push(serde_json::json!({
            "name": node.get_name().to_string(),
            "path": root.get_path_to(node).to_string(),
            "type": node.get_class().to_string(),
        }));
    }
    for i in 0..node.get_child_count() {
        if let Some(child) = node.get_child(i) {
            collect_by_type(&child, type_name, results, root);
        }
    }
}

// ============================================================================
// 已有工具
// ============================================================================

/// find_nodes_by_type: 按类型查找节点
fn cmd_find_nodes_by_type(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor
        .get_edited_scene_root()
        .ok_or_else(|| McpError::no_scene())?;
    let type_name = args
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing type"))?;
    let mut results = Vec::new();
    collect_by_type(&root, type_name, &mut results, &root);
    Ok(serde_json::json!({"nodes": results, "count": results.len()}))
}

/// batch_set_property: 批量设置同类型节点的属性
fn cmd_batch_set_property(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor
        .get_edited_scene_root()
        .ok_or_else(|| McpError::no_scene())?;
    let node_type = args
        .get("node_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing node_type"))?;
    let property = args
        .get("property")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing property"))?;
    let value = args
        .get("value")
        .ok_or_else(|| McpError::invalid_params("Missing value"))?;

    let mut nodes = Vec::new();
    collect_by_type(&root, node_type, &mut nodes, &root);
    let variant = serialize::parse_value_for_property(value);
    let mut updated = 0i64;
    for entry in &nodes {
        if let Some(path) = entry.get("path").and_then(|v| v.as_str()) {
            if root.has_node(path) {
                let mut n = root.get_node_as::<Node>(path);
                n.set(property, &variant);
                updated += 1;
            }
        }
    }
    Ok(serde_json::json!({"updated": updated, "property": property, "node_type": node_type}))
}

// ============================================================================
// 1. find_signal_connections: 递归查找场景中所有信号连接
// ============================================================================

/// 通过 GDScript Expression 执行脚本并返回字符串结果
/// (因为 get_signal_list / get_signal_connection_list 返回 Dictionary，
///  在 Rust 绑定中处理较为复杂)
fn execute_gdscript(code: &str) -> Result<String, McpError> {
    let mut expr = Expression::new_gd();
    let parse_err = expr.parse(code);
    if parse_err != Error::OK {
        return Err(McpError::invalid_params(&format!(
            "脚本解析失败: {:?}",
            parse_err
        )));
    }
    let result = expr.execute();
    if result.is_nil() {
        Ok("null".to_string())
    } else {
        Ok(result.to::<String>())
    }
}

/// find_signal_connections: 递归遍历场景节点，收集所有信号连接
fn cmd_find_signal_connections(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let _editor = EditorInterface::singleton();
    let _root = _editor
        .get_edited_scene_root()
        .ok_or_else(|| McpError::no_scene())?;

    let signal_filter = args
        .get("signal_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let node_filter = args
        .get("node_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // 使用 GDScript Expression 遍历信号连接
    // 使用栈（数组）模拟递归，避免 GDScript 中定义函数的复杂性
    let code = format!(
        r#"var root = EditorInterface.get_edited_scene_root()
var signal_filter = "{}"
var node_filter = "{}"
var connections = []
var stack = [root]
while stack.size() > 0:
    var node = stack[-1]
    stack.resize(stack.size() - 1)
    var np = root.get_path_to(node).to_string()
    if node_filter.length() == 0 or np.find(node_filter) >= 0:
        for sig in node.get_signal_list():
            var sn = sig["name"]
            if signal_filter.length() == 0 or sn.find(signal_filter) >= 0:
                for conn in node.get_signal_connection_list(sn):
                    var conn_obj = conn["callable"].get_object()
                    var tp = ""
                    if conn_obj:
                        tp = root.get_path_to(conn_obj).to_string()
                    connections.append({{
                        "source": np,
                        "signal": sn,
                        "target": tp,
                        "method": conn["callable"].get_method()
                    }})
    for ci in node.get_child_count():
        stack.push_back(node.get_child(ci))
return JSON.stringify({{"connections": connections, "count": connections.size()}})"#,
        signal_filter.replace('"', "\\\""),
        node_filter.replace('"', "\\\"")
    );

    let json_str = execute_gdscript(&code)?;
    let result: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| McpError::internal(&format!("解析信号数据失败: {}", e)))?;

    Ok(result)
}

// ============================================================================
// 2. batch_add_nodes: 批量添加节点到场景
// ============================================================================

/// batch_add_nodes: 批量创建并添加节点到场景树
fn cmd_batch_add_nodes(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor
        .get_edited_scene_root()
        .ok_or_else(|| McpError::no_scene())?;

    let nodes_arr = args
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: nodes (Array)"))?;

    if nodes_arr.is_empty() {
        return Err(McpError::invalid_params("nodes array is empty"));
    }

    let class_db = ClassDb::singleton();
    let mut created: Vec<serde_json::Value> = Vec::new();
    let mut errors: Vec<serde_json::Value> = Vec::new();

    for (i, entry_val) in nodes_arr.iter().enumerate() {
        let entry = match entry_val.as_object() {
            Some(obj) => obj,
            None => {
                errors.push(serde_json::json!({"index": i, "error": "Entry is not an object"}));
                continue;
            }
        };

        // 获取节点类型（必填）
        let node_type = match entry.get("type").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                errors.push(serde_json::json!({"index": i, "error": "Missing or invalid 'type'"}));
                continue;
            }
        };

        // 验证类型是否存在
        let type_sn = StringName::from(node_type);
        if !class_db.class_exists(&type_sn) {
            errors.push(serde_json::json!({"index": i, "error": format!("Unknown node type: {}", node_type)}));
            continue;
        }

        // 获取父节点路径（可选，默认为 ".")
        let parent_path = entry
            .get("parent_path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        // 获取节点名称（可选）
        let node_name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("");
        // 获取属性字典（可选）
        let properties = entry
            .get("properties")
            .and_then(|v| v.as_object())
            .map(|m| m.clone())
            .unwrap_or_default();

        // 查找父节点
        let mut parent = if root.has_node(parent_path) {
            root.get_node_as::<Node>(parent_path)
        } else {
            errors.push(serde_json::json!({"index": i, "error": format!("Parent node '{}' not found", parent_path)}));
            continue;
        };

        // 实例化节点
        let node_var = class_db.instantiate(node_type);
        if node_var.is_nil() {
            errors.push(serde_json::json!({"index": i, "error": format!("Failed to instantiate type: {}", node_type)}));
            continue;
        }
        let mut new_node: Gd<Node> = match node_var.try_to() {
            Ok(n) => n,
            Err(_) => {
                errors.push(serde_json::json!({"index": i, "error": format!("'{}' is not a Node subclass", node_type)}));
                continue;
            }
        };

        // 设置节点名称
        if !node_name.is_empty() {
            new_node.set_name(node_name);
        }

        // 设置属性
        for (prop_name, prop_val) in &properties {
            // 检查属性是否存在
            let prop_list = new_node.get_property_list();
            let mut exists = false;
            for j in 0..prop_list.len() {
                if let Some(entry_dict) = prop_list.get(j) {
                    let name = entry_dict
                        .get(&Variant::from("name"))
                        .unwrap_or(Variant::nil());
                    // 兼容 StringName / String 两种属性名类型（Godot 4.7 为 StringName）
                    let name_str: String = match name.get_type() {
                        godot::builtin::VariantType::STRING_NAME => {
                            name.to::<godot::builtin::StringName>().to_string()
                        }
                        godot::builtin::VariantType::STRING => {
                            name.to::<godot::prelude::GString>().to_string()
                        }
                        _ => continue,
                    };
                    if name_str == *prop_name {
                        exists = true;
                        break;
                    }
                }
            }
            if exists {
                let variant = serialize::parse_value_for_property(prop_val);
                new_node.set(prop_name.as_str(), &variant);
            }
        }

        // 添加到场景树
        parent.add_child(&new_node);
        // 设置 owner 为场景根，确保场景保存时节点不被遗漏
        new_node.set("owner", &Variant::from(root.clone()));

        let node_path_str = root.get_path_to(&new_node).to_string();
        created.push(serde_json::json!({
            "index": i,
            "type": node_type,
            "name": new_node.get_name().to_string(),
            "parent": parent_path,
            "node_path": node_path_str,
        }));
    }

    let mut result = serde_json::json!({
        "created": created,
        "count": created.len(),
    });
    if !errors.is_empty() {
        result["errors"] = serde_json::json!(errors);
    }
    Ok(result)
}

// ============================================================================
// 3. find_node_references: 在项目文件中搜索指定模式的引用
// ============================================================================

/// 递归遍历目录，在匹配的文件中搜索模式
fn search_files_for_pattern(
    path: &str,
    pattern: &str,
    matches: &mut Vec<serde_json::Value>,
    max_results: usize,
) {
    if matches.len() >= max_results {
        return;
    }

    let mut dir = match DirAccess::open(path) {
        Some(d) => d,
        None => return,
    };

    dir.list_dir_begin();
    loop {
        let file_name = dir.get_next().to_string();
        if file_name.is_empty() {
            break;
        }
        if matches.len() >= max_results {
            break;
        }
        // 跳过隐藏文件和目录
        if file_name.starts_with('.') {
            continue;
        }

        // 构造完整路径
        let full_path = if path == "res://" {
            format!("res://{}", file_name)
        } else {
            format!("{}/{}", path, file_name)
        };

        if dir.current_is_dir() {
            // 跳过 addons 目录
            if file_name == "addons" {
                continue;
            }
            search_files_for_pattern(&full_path, pattern, matches, max_results);
        } else if file_name.ends_with(".tscn")
            || file_name.ends_with(".gd")
            || file_name.ends_with(".tres")
            || file_name.ends_with(".gdshader")
        {
            // 打开文件并搜索模式
            let mut file = match FileAccess::open(&full_path, ModeFlags::READ) {
                Some(f) => f,
                None => continue,
            };
            let content = file.get_as_text().to_string();
            file.close();

            if content.contains(pattern) {
                // 查找行号（最多记录 5 行）
                let mut line_matches: Vec<i64> = Vec::new();
                for (line_idx, line) in content.lines().enumerate() {
                    if line.contains(pattern) {
                        line_matches.push((line_idx + 1) as i64);
                        if line_matches.len() >= 5 {
                            break;
                        }
                    }
                }
                matches.push(serde_json::json!({
                    "file": full_path,
                    "lines": line_matches,
                }));
            }
        }
    }
    dir.list_dir_end();
}

/// find_node_references: 在项目文件中搜索指定模式的引用
fn cmd_find_node_references(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: pattern"))?;

    let mut matches: Vec<serde_json::Value> = Vec::new();
    search_files_for_pattern("res://", pattern, &mut matches, 100);

    Ok(serde_json::json!({
        "pattern": pattern,
        "matches": matches,
        "count": matches.len(),
    }))
}

// ============================================================================
// 4. get_scene_dependencies: 获取场景文件的依赖资源列表
// ============================================================================

/// get_scene_dependencies: 获取场景文件的依赖资源列表
fn cmd_get_scene_dependencies(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: path"))?;

    if !FileAccess::file_exists(path) {
        return Err(McpError::not_found(
            &format!("File '{}'", path),
            "检查文件路径是否正确",
        ));
    }

    let rl = ResourceLoader::singleton();
    // get_dependencies 返回 PackedStringArray，格式为 "path::type"
    let deps = rl.get_dependencies(path);
    let mut dependencies: Vec<serde_json::Value> = Vec::new();

    for i in 0..deps.len() {
        let dep: String = deps.get(i).unwrap_or_default().to_string();
        let parts: Vec<&str> = dep.split("::").collect();
        dependencies.push(serde_json::json!({
            "path": parts.first().copied().unwrap_or(&dep),
            "type": if parts.len() > 2 { parts[2] } else { "" },
        }));
    }

    Ok(serde_json::json!({
        "path": path,
        "dependencies": dependencies,
        "count": dependencies.len(),
    }))
}

// ============================================================================
// 5. cross_scene_set_property: 跨场景批量设置属性
// ============================================================================

/// 递归收集 .tscn 场景文件路径
fn collect_scene_files(path: &str, files: &mut Vec<String>, exclude_addons: bool) {
    let mut dir = match DirAccess::open(path) {
        Some(d) => d,
        None => return,
    };

    dir.list_dir_begin();
    loop {
        let file_name = dir.get_next().to_string();
        if file_name.is_empty() {
            break;
        }
        if file_name.starts_with('.') {
            continue;
        }

        let full_path = if path == "res://" {
            format!("res://{}", file_name)
        } else {
            format!("{}/{}", path, file_name)
        };

        if dir.current_is_dir() {
            if exclude_addons && file_name == "addons" {
                continue;
            }
            collect_scene_files(&full_path, files, exclude_addons);
        } else if file_name.ends_with(".tscn") {
            files.push(full_path);
        }
    }
    dir.list_dir_end();
}

/// 递归遍历节点，收集匹配类型的节点路径
fn cross_scene_collect_changes(
    node: &Gd<Node>,
    root: &Gd<Node>,
    type_name: &str,
    property: &str,
    _value: &Variant,
    affected: &mut Vec<String>,
) {
    if node.get_class().to_string() == type_name || node.is_class(type_name) {
        // 检查属性是否存在（通过 get_property_list 检查）
        let prop_list = node.get_property_list();
        for i in 0..prop_list.len() {
            if let Some(entry) = prop_list.get(i) {
                let name = entry
                    .get(&Variant::from("name"))
                    .unwrap_or(Variant::nil());
                // 兼容 StringName / String 两种属性名类型（Godot 4.7 为 StringName）
                let name_str: String = match name.get_type() {
                    godot::builtin::VariantType::STRING_NAME => {
                        name.to::<godot::builtin::StringName>().to_string()
                    }
                    godot::builtin::VariantType::STRING => {
                        name.to::<godot::prelude::GString>().to_string()
                    }
                    _ => continue,
                };
                if name_str == property {
                    affected.push(root.get_path_to(node).to_string());
                    break;
                }
            }
        }
    }
    for i in 0..node.get_child_count() {
        if let Some(child) = node.get_child(i) {
            cross_scene_collect_changes(&child, root, type_name, property, _value, affected);
        }
    }
}

/// cross_scene_set_property: 跨场景批量设置节点属性
fn cmd_cross_scene_set_property(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let type_name = args
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: type"))?;
    let property = args
        .get("property")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: property"))?;
    let value = args
        .get("value")
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: value"))?;

    let path_filter = args
        .get("path_filter")
        .and_then(|v| v.as_str())
        .unwrap_or("res://");
    let exclude_addons = args
        .get("exclude_addons")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    let dry_run = args
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(!force);

    // 如果 dry_run=false 且 force=false，则报错
    if !dry_run && !force {
        return Err(McpError::invalid_params(
            "cross_scene_set_property requires force=true when dry_run=false",
        ));
    }

    let editor = EditorInterface::singleton();
    let variant = serialize::parse_value_for_property(value);

    // 收集场景文件
    let mut scene_files: Vec<String> = Vec::new();
    collect_scene_files(path_filter, &mut scene_files, exclude_addons);

    let mut scenes_affected: Vec<serde_json::Value> = Vec::new();
    let mut skipped_open_scenes: Vec<serde_json::Value> = Vec::new();
    let mut total_nodes: i64 = 0;

    // 获取当前编辑的场景路径（如果有）
    let active_scene_path = editor
        .get_edited_scene_root()
        .map(|r| r.get_scene_file_path().to_string());

    for scene_path in &scene_files {
        // 检查是否当前正在编辑的活动场景
        let is_active = active_scene_path.as_deref() == Some(scene_path.as_str());

        if is_active {
            if force && !dry_run {
                // 当前活动场景，直接修改
                if let Some(active_root) = editor.get_edited_scene_root() {
                    let mut affected_nodes: Vec<String> = Vec::new();
                    cross_scene_collect_changes(
                        &active_root,
                        &active_root,
                        type_name,
                        property,
                        &variant,
                        &mut affected_nodes,
                    );
                    for node_path in &affected_nodes {
                        if active_root.has_node(node_path) {
                            let mut n = active_root.get_node_as::<Node>(node_path);
                            n.set(property, &variant);
                        }
                    }
                    total_nodes += affected_nodes.len() as i64;
                    scenes_affected.push(serde_json::json!({
                        "scene": scene_path,
                        "nodes": affected_nodes,
                        "count": affected_nodes.len(),
                        "mode": "live_open_scene",
                    }));
                }
            } else {
                let reason = if dry_run {
                    "active scene skipped during dry_run"
                } else {
                    "force=false, skipping active scene"
                };
                skipped_open_scenes.push(serde_json::json!({
                    "scene": scene_path,
                    "reason": reason,
                }));
            }
            continue;
        }

        // 未打开的场景：加载、修改、保存
        let mut rl = ResourceLoader::singleton();
        let resource = rl.load(scene_path);
        let packed = match resource {
            Some(res) => match res.try_cast::<PackedScene>() {
                Ok(ps) => ps,
                Err(_) => continue,
            },
            None => continue,
        };

        let instance = match packed.instantiate() {
            Some(inst) => inst,
            None => continue,
        };

        // 收集匹配的节点路径
        let mut affected_nodes: Vec<String> = Vec::new();
        cross_scene_collect_changes(
            &instance,
            &instance,
            type_name,
            property,
            &variant,
            &mut affected_nodes,
        );

        if !affected_nodes.is_empty() {
            if !dry_run {
                // 设置属性值
                for node_path in &affected_nodes {
                    if instance.has_node(node_path) {
                        let mut n = instance.get_node_as::<Node>(node_path);
                        n.set(property, &variant);
                    }
                }

                // 重新打包并保存场景
                let mut new_packed = PackedScene::new_gd();
                let pack_err = new_packed.pack(&instance);
                if pack_err != Error::OK {
                    instance.free();
                    return Err(McpError::internal(&format!(
                        "Failed to pack scene '{}': {:?}",
                        scene_path, pack_err
                    )));
                }

                let save_err = ResourceSaver::singleton()
                    .save_ex(&new_packed)
                    .path(scene_path.as_str())
                    .done();
                if save_err != Error::OK {
                    instance.free();
                    return Err(McpError::internal(&format!(
                        "Failed to save scene '{}': {:?}",
                        scene_path, save_err
                    )));
                }
            }

            total_nodes += affected_nodes.len() as i64;
            scenes_affected.push(serde_json::json!({
                "scene": scene_path,
                "nodes": affected_nodes,
                "count": affected_nodes.len(),
                "mode": if dry_run { "dry_run" } else { "offline_saved" },
            }));
        }

        instance.free();
    }

    // 刷新文件系统，让编辑器感知到变化
    if !scenes_affected.is_empty() && !dry_run {
        if let Some(mut fs) = editor.get_resource_filesystem() {
            fs.scan();
        }
    }

    let message = if dry_run {
        "Dry run only. Re-run with force=true and dry_run=false to write closed scenes and live-edit the active open scene."
    } else {
        "Changes applied."
    };

    Ok(serde_json::json!({
        "type": type_name,
        "property": property,
        "dry_run": dry_run,
        "force": force,
        "scenes_affected": scenes_affected,
        "skipped_open_scenes": skipped_open_scenes,
        "total_scenes": scenes_affected.len(),
        "total_nodes": total_nodes,
        "message": message,
    }))
}
