//! 批量操作命令模块

use std::collections::HashMap;
use godot::classes::{EditorInterface, Node};
use godot::obj::Gd;
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;
use crate::utils::serialize;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("find_nodes_by_type", "按类型查找所有节点", serde_json::json!({
            "type": "object", "properties": { "type": { "type": "string" } }, "required": ["type"]
        })),
        ToolDefinition::new("batch_set_property", "批量设置同类型节点的属性", serde_json::json!({
            "type": "object", "properties": {
                "node_type": { "type": "string" }, "property": { "type": "string" }, "value": {}
            }, "required": ["node_type", "property", "value"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("find_nodes_by_type".into(), cmd_find_nodes_by_type);
    registry.insert("batch_set_property".into(), cmd_batch_set_property);
}

fn collect_by_type(node: &Gd<Node>, type_name: &str, results: &mut Vec<serde_json::Value>, root: &Gd<Node>) {
    if node.get_class().to_string() == type_name || node.is_class(type_name) {
        results.push(serde_json::json!({
            "name": node.get_name().to_string(),
            "path": root.get_path_to(node).to_string(),
            "type": node.get_class().to_string(),
        }));
    }
    for i in 0..node.get_child_count() {
        if let Some(child) = node.get_child(i) { collect_by_type(&child, type_name, results, root); }
    }
}

fn cmd_find_nodes_by_type(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let type_name = args.get("type").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing type"))?;
    let mut results = Vec::new();
    collect_by_type(&root, type_name, &mut results, &root);
    Ok(serde_json::json!({"nodes": results, "count": results.len()}))
}

fn cmd_batch_set_property(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let node_type = args.get("node_type").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_type"))?;
    let property = args.get("property").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing property"))?;
    let value = args.get("value").ok_or_else(|| McpError::invalid_params("Missing value"))?;

    let mut nodes = Vec::new();
    collect_by_type(&root, node_type, &mut nodes, &root);
    let variant = serialize::parse_value_for_property(value);
    let mut updated = 0i64;
    for entry in &nodes {
        if let Some(path) = entry.get("path").and_then(|v| v.as_str()) {
            if root.has_node(path) { let mut n = root.get_node_as::<Node>(path); n.set(property, &variant); updated += 1; }
        }
    }
    Ok(serde_json::json!({"updated": updated, "property": property, "node_type": node_type}))
}
