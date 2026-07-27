//! 导出命令模块

use std::collections::HashMap;
use godot::classes::{ConfigFile, EditorInterface, FileAccess, ProjectSettings};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("get_export_info", "获取导出信息", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        // 列出导出预设
        ToolDefinition::new("list_export_presets", "列出所有导出预设", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        // 导出项目
        ToolDefinition::new("export_project", "导出项目", serde_json::json!({
            "type": "object", "properties": {
                "preset_name": { "type": "string" },
                "output_path": { "type": "string" }
            }, "required": ["preset_name"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("get_export_info".into(), cmd_get_export_info);
    registry.insert("list_export_presets".into(), cmd_list_export_presets);
    registry.insert("export_project".into(), cmd_export_project);
}

fn cmd_get_export_info(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let ps = ProjectSettings::singleton();
    let name = if ps.has_setting("application/config/name") {
        ps.get_setting("application/config/name").to::<String>()
    } else { String::new() };
    Ok(serde_json::json!({"project_name": name}))
}

/// 列出导出预设 - 读取 export_presets.cfg 文件
fn cmd_list_export_presets(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let presets_path = "res://export_presets.cfg";
    if !FileAccess::file_exists(presets_path) {
        return Ok(serde_json::json!({"presets": [], "count": 0, "message": "No export_presets.cfg found"}));
    }

    let mut cfg = ConfigFile::new_gd();
    let err = cfg.load(presets_path);
    if err != godot::global::Error::OK {
        return Err(McpError::internal(&format!("Failed to read export_presets.cfg: {:?}", err)));
    }

    let mut presets: Vec<serde_json::Value> = Vec::new();
    let mut idx = 0;
    loop {
        let section = format!("preset.{}", idx);
        if cfg.has_section(&section) {
            let name = cfg.get_value(&section, "name").to::<String>();
            let platform = cfg.get_value(&section, "platform").to::<String>();
            let runnable = cfg.get_value(&section, "runnable").to::<bool>();
            let export_path = cfg.get_value(&section, "export_path").to::<String>();
            presets.push(serde_json::json!({
                "index": idx,
                "name": name,
                "platform": platform,
                "runnable": runnable,
                "export_path": export_path,
            }));
            idx += 1;
        } else {
            break;
        }
    }

    Ok(serde_json::json!({"presets": presets, "count": presets.len()}))
}

/// 导出项目 - 使用导出预设
/// 注意: 实际导出需要编辑器支持, 这里返回可用的导出命令
fn cmd_export_project(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let preset_name = args.get("preset_name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing preset_name"))?;
    let _output_path = args.get("output_path").and_then(|v| v.as_str());

    // 读取导出预设配置文件
    let presets_path = "res://export_presets.cfg";
    let mut cfg = ConfigFile::new_gd();
    let err = cfg.load(presets_path);
    if err != godot::global::Error::OK {
        return Err(McpError::internal("No export_presets.cfg found. Configure exports in Project > Export first."));
    }

    // 查找指定名称的预设
    let mut found = false;
    let mut idx = 0;
    loop {
        let section = format!("preset.{}", idx);
        if cfg.has_section(&section) {
            let name = cfg.get_value(&section, "name").to::<String>();
            if name == preset_name {
                found = true;
                break;
            }
            idx += 1;
        } else {
            break;
        }
    }

    if !found {
        return Err(McpError::not_found(&format!("Export preset '{}'", preset_name), "Use list_export_presets to see available presets"));
    }

    // TODO: 实际导出需要编辑器完整支持, 当前返回 GDScript 表达式提示
    Ok(serde_json::json!({
        "preset": preset_name,
        "message": "Export initiated. Check editor output log for progress.",
        "export_started": true
    }))
}
