//! TileMap 命令模块

use std::collections::HashMap;
use godot::classes::{EditorInterface, TileMapLayer};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("tilemap_get_info", "获取 TileMapLayer 信息", serde_json::json!({
            "type": "object", "properties": { "node_path": { "type": "string" } }, "required": ["node_path"]
        })),
        ToolDefinition::new("tilemap_get_used_cells", "获取已使用格子", serde_json::json!({
            "type": "object", "properties": { "node_path": { "type": "string" } }, "required": ["node_path"]
        })),
        ToolDefinition::new("tilemap_clear", "清除所有格子", serde_json::json!({
            "type": "object", "properties": { "node_path": { "type": "string" } }, "required": ["node_path"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("tilemap_get_info".into(), cmd_tilemap_get_info);
    registry.insert("tilemap_get_used_cells".into(), cmd_tilemap_get_used_cells);
    registry.insert("tilemap_clear".into(), cmd_tilemap_clear);
}

fn cmd_tilemap_get_info(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    if !root.has_node(path) { return Err(McpError::not_found(&format!("TileMapLayer '{}'", path), "")); }
    let layer = root.get_node_as::<TileMapLayer>(path);
    let cells = layer.get_used_cells();
    Ok(serde_json::json!({"node_path": path, "cell_count": cells.len(), "enabled": layer.is_enabled()}))
}

fn cmd_tilemap_get_used_cells(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    if !root.has_node(path) { return Err(McpError::not_found(&format!("TileMapLayer '{}'", path), "")); }
    let layer = root.get_node_as::<TileMapLayer>(path);
    let cells = layer.get_used_cells();
    let mut list: Vec<serde_json::Value> = Vec::new();
    for i in 0..cells.len() {
        if let Some(v2) = cells.get(i) {
            list.push(serde_json::json!({"x": v2.x, "y": v2.y}));
        }
    }
    Ok(serde_json::json!({"cells": list, "count": list.len()}))
}

fn cmd_tilemap_clear(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    if !root.has_node(path) { return Err(McpError::not_found(&format!("TileMapLayer '{}'", path), "")); }
    let mut layer = root.get_node_as::<TileMapLayer>(path);
    layer.clear();
    Ok(serde_json::json!({"cleared": true}))
}
