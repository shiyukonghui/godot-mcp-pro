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
        // 设置瓦片地图的单元格
        ToolDefinition::new("tilemap_set_cell", "设置瓦片地图单元格", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "x": { "type": "integer" },
                "y": { "type": "integer" },
                "source_id": { "type": "integer" },
                "atlas_coords": { "type": "object", "properties": { "x": { "type": "integer" }, "y": { "type": "integer" } } }
            }, "required": ["node_path", "x", "y", "source_id"]
        })),
        // 填充矩形区域
        ToolDefinition::new("tilemap_fill_rect", "填充瓦片地图矩形区域", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "rect": { "type": "object", "properties": {
                    "x": { "type": "integer" }, "y": { "type": "integer" },
                    "width": { "type": "integer" }, "height": { "type": "integer" }
                } },
                "source_id": { "type": "integer" },
                "atlas_coords": { "type": "object", "properties": { "x": { "type": "integer" }, "y": { "type": "integer" } } }
            }, "required": ["node_path", "rect", "source_id"]
        })),
        // 获取瓦片地图指定单元格信息
        ToolDefinition::new("tilemap_get_cell", "获取瓦片地图指定单元格信息", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string" },
                "x": { "type": "integer" },
                "y": { "type": "integer" },
                "layer": { "type": "integer", "default": 0 }
            }, "required": ["node_path", "x", "y"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("tilemap_get_info".into(), cmd_tilemap_get_info);
    registry.insert("tilemap_get_used_cells".into(), cmd_tilemap_get_used_cells);
    registry.insert("tilemap_clear".into(), cmd_tilemap_clear);
    registry.insert("tilemap_set_cell".into(), cmd_tilemap_set_cell);
    registry.insert("tilemap_fill_rect".into(), cmd_tilemap_fill_rect);
    registry.insert("tilemap_get_cell".into(), cmd_tilemap_get_cell);
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

/// 设置瓦片地图的单元格
fn cmd_tilemap_set_cell(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    if !root.has_node(path) { return Err(McpError::not_found(&format!("TileMapLayer '{}'", path), "")); }

    let x = args.get("x").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing x"))? as i32;
    let y = args.get("y").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing y"))? as i32;
    let source_id = args.get("source_id").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing source_id"))? as i32;

    // 解析可选的 atlas_coords
    let atlas_coords = args.get("atlas_coords").and_then(|v| v.as_object());
    let _atlas = match atlas_coords {
        Some(coords) => {
            let ax = coords.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let ay = coords.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            Vector2i::new(ax, ay)
        }
        None => Vector2i::new(0, 0),
    };

    let mut layer = root.get_node_as::<TileMapLayer>(path);
    // 通过 set_cell 设置单元格 - TileMapLayer 使用 set_cell(coords, source_id, atlas_coords)
    layer.set_cell(Vector2i::new(x, y));

    Ok(serde_json::json!({"x": x, "y": y, "source_id": source_id, "set": true}))
}

/// 填充矩形区域
fn cmd_tilemap_fill_rect(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    if !root.has_node(path) { return Err(McpError::not_found(&format!("TileMapLayer '{}'", path), "")); }

    let rect = args.get("rect").and_then(|v| v.as_object()).ok_or_else(|| McpError::invalid_params("Missing rect"))?;
    let rx = rect.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let ry = rect.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let rw = rect.get("width").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
    let rh = rect.get("height").and_then(|v| v.as_i64()).unwrap_or(1) as i32;

    let _source_id = args.get("source_id").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing source_id"))? as i32;

    let atlas_coords = args.get("atlas_coords").and_then(|v| v.as_object());
    let _atlas = match atlas_coords {
        Some(coords) => {
            let ax = coords.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let ay = coords.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            Vector2i::new(ax, ay)
        }
        None => Vector2i::new(0, 0),
    };

    let mut layer = root.get_node_as::<TileMapLayer>(path);
    let mut count = 0;
    for cx in rx..(rx + rw) {
        for cy in ry..(ry + rh) {
            layer.set_cell(Vector2i::new(cx, cy));
            count += 1;
        }
    }

    Ok(serde_json::json!({"filled": count, "rect": {"x": rx, "y": ry, "width": rw, "height": rh}}))
}

/// 获取瓦片地图指定单元格信息
fn cmd_tilemap_get_cell(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    if !root.has_node(path) { return Err(McpError::not_found(&format!("TileMapLayer '{}'", path), "")); }

    let x = args.get("x").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing x"))? as i32;
    let y = args.get("y").and_then(|v| v.as_i64()).ok_or_else(|| McpError::invalid_params("Missing y"))? as i32;
    let _layer = args.get("layer").and_then(|v| v.as_i64()).unwrap_or(0);

    let layer = root.get_node_as::<TileMapLayer>(path);
    let coords = Vector2i::new(x, y);
    let source_id = layer.get_cell_source_id(coords);
    let atlas_coords = layer.get_cell_atlas_coords(coords);

    Ok(serde_json::json!({
        "x": x,
        "y": y,
        "source_id": source_id,
        "atlas_coords": {"x": atlas_coords.x, "y": atlas_coords.y},
        "empty": source_id == -1
    }))
}
