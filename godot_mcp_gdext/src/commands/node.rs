//! 节点 CRUD 命令模块

use std::collections::HashMap;

use godot::builtin::{Callable, NodePath, Variant};
use godot::classes::{ClassDb, EditorInterface, Node};
use godot::obj::Gd;
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;
use crate::utils::serialize;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("add_node", "向场景添加新节点", serde_json::json!({
            "type": "object", "properties": {
                "type": { "type": "string" },
                "name": { "type": "string" },
                "parent_path": { "type": "string", "default": "." },
                "properties": { "type": "object", "default": {} }
            }, "required": ["type"]
        })),
        ToolDefinition::new("delete_node", "删除节点", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" }
            }, "required": ["path"]
        })),
        ToolDefinition::new("rename_node", "重命名节点", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" }, "name": { "type": "string" }
            }, "required": ["path", "name"]
        })),
        ToolDefinition::new("update_property", "修改属性", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" }, "property": { "type": "string" }, "value": {}
            }, "required": ["path", "property", "value"]
        })),
        ToolDefinition::new("get_node_properties", "获取节点属性", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" },
                "properties": { "type": "array", "items": { "type": "string" } }
            }, "required": ["path"]
        })),
        ToolDefinition::new("duplicate_node", "复制指定节点及其子节点", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "源节点路径" },
                "new_name": { "type": "string", "description": "新节点名称 (可选)" }
            }, "required": ["path"]
        })),
        ToolDefinition::new("connect_signal", "连接节点的信号到目标方法", serde_json::json!({
            "type": "object", "properties": {
                "source_path": { "type": "string", "description": "源节点路径" },
                "signal": { "type": "string", "description": "信号名称" },
                "target_path": { "type": "string", "description": "目标节点路径" },
                "method": { "type": "string", "description": "目标方法名" }
            }, "required": ["source_path", "signal", "method"]
        })),
        ToolDefinition::new("disconnect_signal", "断开节点的信号连接", serde_json::json!({
            "type": "object", "properties": {
                "source_path": { "type": "string", "description": "源节点路径" },
                "signal": { "type": "string", "description": "信号名称" },
                "target_path": { "type": "string", "description": "目标节点路径" },
                "method": { "type": "string", "description": "目标方法名" }
            }, "required": ["source_path", "signal", "method"]
        })),
        ToolDefinition::new("move_node", "将节点移动到新的父节点下", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "要移动的节点路径" },
                "new_parent": { "type": "string", "description": "目标父节点路径" },
                "new_name": { "type": "string", "description": "移动后的新名称 (可选)" }
            }, "required": ["path", "new_parent"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("add_node".into(), cmd_add_node);
    registry.insert("delete_node".into(), cmd_delete_node);
    registry.insert("rename_node".into(), cmd_rename_node);
    registry.insert("update_property".into(), cmd_update_property);
    registry.insert("get_node_properties".into(), cmd_get_node_properties);
    registry.insert("duplicate_node".into(), cmd_duplicate_node);
    registry.insert("connect_signal".into(), cmd_connect_signal);
    registry.insert("disconnect_signal".into(), cmd_disconnect_signal);
    registry.insert("move_node".into(), cmd_move_node);
}

fn find_node(root: &Gd<Node>, path: &str) -> Option<Gd<Node>> {
    if path == "." || path == root.get_name().to_string() {
        return Some(root.clone());
    }
    // 简化: 使用 has_node + get_node (通过 NodePath 的 to_string 方式)
    let np = NodePath::from(path);
    if root.has_node(&np.to_string()) {
        return Some(root.get_node_as::<Node>(&np.to_string()));
    }
    let root_name = root.get_name().to_string();
    if let Some(rest) = path.strip_prefix(&(root_name + "/")) {
        let np2 = NodePath::from(rest);
        if root.has_node(&np2.to_string()) {
            return Some(root.get_node_as::<Node>(&np2.to_string()));
        }
    }
    None
}

fn cmd_add_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let node_type = args.get("type").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing type"))?;
    let node_name = args.get("name").and_then(|v| v.as_str()).unwrap_or(node_type);
    let parent_path = args.get("parent_path").and_then(|v| v.as_str()).unwrap_or(".");

    let mut parent = find_node(&root, parent_path)
        .ok_or_else(|| McpError::not_found(&format!("Parent '{}'", parent_path), ""))?;

    let node_var = ClassDb::singleton().instantiate(node_type);
    let mut new_node: Gd<Node> = node_var.to();

    new_node.set_name(node_name);

    if let Some(props) = args.get("properties").and_then(|v| v.as_object()) {
        for (key, val) in props {
            new_node.set(key.as_str(), &serialize::parse_value_for_property(val));
        }
    }

    parent.add_child(&new_node);
    // 通过 Variant 方式设置 owner，绕开 AsArg 的类型约束
    new_node.set("owner", &Variant::from(root.clone()));

    let path_str = root.get_path_to(&new_node).to_string();
    Ok(serde_json::json!({"node_path": path_str, "name": node_name, "type": node_type}))
}

fn cmd_delete_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let mut node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;
    node.queue_free();
    Ok(serde_json::json!({"deleted": true, "path": path}))
}

fn cmd_rename_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing name"))?;
    let mut node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;
    node.set_name(name);
    Ok(serde_json::json!({"renamed": true, "new_name": name}))
}

fn cmd_update_property(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let property = args.get("property").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing property"))?;
    let value = args.get("value").ok_or_else(|| McpError::invalid_params("Missing value"))?;
    let mut node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;
    node.set(property, &serialize::parse_value_for_property(value));
    Ok(serde_json::json!({"node_path": path, "property": property, "updated": true}))
}

fn cmd_get_node_properties(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;

    let prop_list = node.get_property_list();
    let mut props = serde_json::Map::new();

    let filter: Option<Vec<String>> = args.get("properties")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

    let nil_var = Variant::nil();
    for i in 0..prop_list.len() {
        let entry = match prop_list.get(i) { Some(e) => e, None => continue, };
        let name = entry.get(&Variant::from("name")).unwrap_or(nil_var.clone()).to::<String>();
        if name.starts_with('_') || name == "script" { continue; }
        if let Some(ref f) = filter { if !f.contains(&name) { continue; } }

        let val = node.get(&name);
        props.insert(name, serialize::serialize_variant(&val));
    }

    Ok(serde_json::json!({
        "node_path": path, "type": node.get_class().to_string(), "properties": props
    }))
}

/// duplicate_node: 复制节点
fn cmd_duplicate_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let new_name = args.get("new_name").and_then(|v| v.as_str()).unwrap_or(path);

    let node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;
    let mut parent = node.get_parent().ok_or_else(|| McpError::internal("Node has no parent"))?;

    // duplicate() 返回 Option<Gd<Node>>，需要 unwrap
    let mut dup = node.duplicate().ok_or_else(|| McpError::internal("复制节点失败"))?;
    dup.set_name(new_name);
    parent.add_child(&dup);
    // 通过 Variant 方式设置 owner
    dup.set("owner", &Variant::from(root.clone()));

    let new_path = root.get_path_to(&dup).to_string();
    Ok(serde_json::json!({"node_path": new_path, "name": new_name, "duplicated": true}))
}

/// move_node: 移动节点到新父节点 (reparent)
fn cmd_move_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let new_parent_path = args.get("new_parent").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing new_parent"))?;
    let new_name = args.get("new_name").and_then(|v| v.as_str());

    let mut node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;
    let mut new_parent = find_node(&root, new_parent_path)
        .ok_or_else(|| McpError::not_found(&format!("Parent '{}'", new_parent_path), ""))?;

    // 从当前父节点移除
    if let Some(mut old_parent) = node.get_parent() {
        old_parent.remove_child(&node);
    }
    // 添加到新父节点
    new_parent.add_child(&node);
    node.set("owner", &Variant::from(root.clone()));

    if let Some(name) = new_name {
        node.set_name(name);
    }

    let final_path = root.get_path_to(&node).to_string();
    Ok(serde_json::json!({"node_path": final_path, "moved": true}))
}

/// connect_signal: 连接节点信号
fn cmd_connect_signal(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let src_path = args.get("source_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing source_path"))?;
    let signal = args.get("signal").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing signal"))?;
    let method = args.get("method").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing method"))?;
    let tgt_path = args.get("target_path").and_then(|v| v.as_str());

    let mut source = find_node(&root, src_path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", src_path), ""))?;

    if let Some(tp) = tgt_path {
        let target = find_node(&root, tp).ok_or_else(|| McpError::not_found(&format!("Node '{}'", tp), ""))?;
        let callable = Callable::from_object_method(&target, method);
        source.connect(signal, &callable);
        Ok(serde_json::json!({"connected": true, "signal": signal, "source": src_path, "target": tp}))
    } else {
        // 无目标节点时连接到场景根节点
        let callable = Callable::from_object_method(&root, method);
        source.connect(signal, &callable);
        Ok(serde_json::json!({"connected": true, "signal": signal, "source": src_path}))
    }
}

/// disconnect_signal: 断开信号连接
fn cmd_disconnect_signal(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let src_path = args.get("source_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing source_path"))?;
    let signal = args.get("signal").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing signal"))?;
    let method = args.get("method").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing method"))?;

    let mut source = find_node(&root, src_path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", src_path), ""))?;

    // disconnect 需要一个 Callable 来匹配要断开的连接
    let callable = Callable::from_object_method(&root, method);
    source.disconnect(signal, &callable);
    Ok(serde_json::json!({"disconnected": true, "signal": signal, "source": src_path}))
}
