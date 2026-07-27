//! 性能监控命令模块

use std::collections::HashMap;

use godot::classes::{performance::Monitor, Performance};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("get_performance_monitors", "获取编辑器性能指标", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        // 获取编辑器性能摘要
        ToolDefinition::new("get_editor_performance", "获取编辑器性能摘要", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("get_performance_monitors".into(), cmd_get_performance_monitors);
    registry.insert("get_editor_performance".into(), cmd_get_editor_performance);
}

fn perf(m: Monitor) -> f64 {
    Performance::singleton().get_monitor(m)
}

fn cmd_get_performance_monitors(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    Ok(serde_json::json!({
        "time": {
            "fps": perf(Monitor::TIME_FPS),
            "process_msec": perf(Monitor::TIME_PROCESS) * 1000.0,
            "physics_msec": perf(Monitor::TIME_PHYSICS_PROCESS) * 1000.0,
        },
        "memory": {
            "static_mb": perf(Monitor::MEMORY_STATIC) / (1024.0 * 1024.0),
            "static_max_mb": perf(Monitor::MEMORY_STATIC_MAX) / (1024.0 * 1024.0),
        },
        "objects": {
            "node_count": perf(Monitor::OBJECT_NODE_COUNT) as i64,
            "orphan_nodes": perf(Monitor::OBJECT_ORPHAN_NODE_COUNT) as i64,
            "resource_count": perf(Monitor::OBJECT_RESOURCE_COUNT) as i64,
        },
        "render": {
            "objects_in_frame": perf(Monitor::RENDER_TOTAL_OBJECTS_IN_FRAME) as i64,
            "draw_calls": perf(Monitor::RENDER_TOTAL_DRAW_CALLS_IN_FRAME) as i64,
            "video_mem_mb": perf(Monitor::RENDER_VIDEO_MEM_USED) / (1024.0 * 1024.0),
        },
        "physics_2d": {
            "active_objects": perf(Monitor::PHYSICS_2D_ACTIVE_OBJECTS) as i64,
        },
        "physics_3d": {
            "active_objects": perf(Monitor::PHYSICS_3D_ACTIVE_OBJECTS) as i64,
        },
    }))
}

/// 获取编辑器性能摘要 - FPS、内存、节点数等关键指标
fn cmd_get_editor_performance(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    Ok(serde_json::json!({
        "fps": perf(Monitor::TIME_FPS),
        "frame_time_msec": perf(Monitor::TIME_PROCESS) * 1000.0,
        "draw_calls": perf(Monitor::RENDER_TOTAL_DRAW_CALLS_IN_FRAME) as i64,
        "objects_in_frame": perf(Monitor::RENDER_TOTAL_OBJECTS_IN_FRAME) as i64,
        "node_count": perf(Monitor::OBJECT_NODE_COUNT) as i64,
        "orphan_nodes": perf(Monitor::OBJECT_ORPHAN_NODE_COUNT) as i64,
        "memory_static_mb": perf(Monitor::MEMORY_STATIC) / (1024.0 * 1024.0),
        "video_mem_mb": perf(Monitor::RENDER_VIDEO_MEM_USED) / (1024.0 * 1024.0),
    }))
}
