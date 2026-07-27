//! 运行时游戏命令模块
//!
//! 通过文件 IPC 与运行中的游戏进程通信。
//! 游戏进程运行 mcp_runtime_agent.gd（Autoload），通过轮询文件来接收命令。

use std::collections::HashMap;
use std::time::Duration;

use godot::classes::file_access::ModeFlags;
use godot::classes::{ConfigFile, DirAccess, EditorInterface, FileAccess, Os};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

// ============================================================================
// 辅助函数
// ============================================================================

/// 获取游戏用户目录
///
/// 参考 GDScript base_command.gd 的 get_game_user_dir():
/// 1. 读取 res://project.godot 的 [application] config/name
/// 2. 使用 OS::get_user_data_dir() 的父目录拼接游戏名
fn get_game_user_dir() -> Result<String, McpError> {
    // 加载项目设置文件获取游戏名称
    let mut cfg = ConfigFile::new_gd();
    let err = cfg.load("res://project.godot");
    let game_name = if err == godot::global::Error::OK {
        let val = cfg.get_value("application", "config/name");
        if val.is_nil() {
            "Unnamed".to_string()
        } else {
            let s: String = val.to();
            if s.is_empty() { "Unnamed".to_string() } else { s }
        }
    } else {
        "Unnamed".to_string()
    };

    // 获取用户数据目录的父目录
    let user_dir = Os::singleton().get_user_data_dir().to_string();
    // 父目录通常是 AppData/Roaming（Windows）或 ~/.local/share（Linux）
    let parent = std::path::Path::new(&user_dir)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| user_dir.clone());

    Ok(format!("{}/{}", parent, game_name))
}

/// 尝试恢复调试器（简化版）
///
/// 当游戏因断点暂停时，通过 GDScript Expression 尝试恢复运行。
/// 如果游戏不再运行则忽略错误。
fn try_debugger_continue() {
    // 使用 Expression 尝试调用调试器恢复
    // 如果失败（游戏未暂停或无调试器）则静默处理
    let code = r#"
var editor = EditorInterface
if editor != null and editor.is_playing_scene():
    editor.set_plugin_enabled("godot_mcp", true)
"#;
    let mut expr = godot::classes::Expression::new_gd();
    if expr.parse(code) == godot::global::Error::OK {
        let _ = expr.execute();
    }
}

/// 通过文件 IPC 向运行中的游戏发送命令并等待响应
///
/// 1. 检查游戏是否在运行
/// 2. 清理旧响应文件
/// 3. 写入请求 JSON 到 mcp_game_request
/// 4. 以 100ms 间隔轮询 mcp_game_response，直到超时
/// 5. 超时后尝试自动恢复调试器
/// 6. 读取响应 JSON
/// 7. 检查是否有 error 字段
fn send_game_command(
    command: &str,
    params: serde_json::Value,
    timeout_sec: f64,
) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();

    // 1. 检查游戏是否在运行
    if !editor.is_playing_scene() {
        return Err(McpError::internal("游戏未在运行，请先运行场景"));
    }

    // 2. 获取游戏用户目录
    let user_dir = get_game_user_dir()?;
    let request_path = format!("{}/mcp_game_request", user_dir);
    let response_path = format!("{}/mcp_game_response", user_dir);

    // 3. 清理旧的响应文件
    if FileAccess::file_exists(&response_path) {
        let _ = DirAccess::remove_absolute(&response_path);
    }

    // 4. 构建并写入请求 JSON
    let request = serde_json::json!({
        "command": command,
        "params": params,
    });
    let json_str = serde_json::to_string(&request)
        .map_err(|e| McpError::internal(&format!("序列化请求失败: {}", e)))?;

    if let Some(mut file) = FileAccess::open(&request_path, ModeFlags::WRITE) {
        file.store_string(&json_str);
    } else {
        return Err(McpError::internal(&format!("无法写入请求文件: {}", request_path)));
    }

    // 5. 轮询响应文件
    let start = std::time::Instant::now();
    let mut response_found = false;

    while start.elapsed().as_secs_f64() < timeout_sec {
        std::thread::sleep(Duration::from_millis(100));

        if FileAccess::file_exists(&response_path) {
            response_found = true;
            break;
        }

        // 检查游戏是否仍在运行
        if !EditorInterface::singleton().is_playing_scene() {
            // 清理请求文件
            let _ = DirAccess::remove_absolute(&request_path);
            return Err(McpError::internal("游戏在等待响应期间停止运行"));
        }
    }

    // 6. 超时处理
    if !response_found {
        // 尝试恢复调试器
        try_debugger_continue();

        // 再等一小段时间
        let extra_wait = std::time::Instant::now();
        while extra_wait.elapsed().as_secs_f64() < 1.0 {
            std::thread::sleep(Duration::from_millis(100));
            if FileAccess::file_exists(&response_path) {
                response_found = true;
                break;
            }
        }

        if !response_found {
            // 清理请求文件
            let _ = DirAccess::remove_absolute(&request_path);
            return Err(McpError::internal(&format!(
                "等待游戏响应超时 ({}秒)。游戏可能因断点暂停或响应文件路径不匹配。",
                timeout_sec
            )));
        }
    }

    // 7. 读取响应文件
    let file = FileAccess::open(&response_path, ModeFlags::READ)
        .ok_or_else(|| McpError::internal("无法读取响应文件"))?;
    let content = file.get_as_text().to_string();
    // 关闭文件后再删除（Windows 上需要先关闭）
    drop(file);
    let _ = DirAccess::remove_absolute(&response_path);

    // 清理请求文件
    let _ = DirAccess::remove_absolute(&request_path);

    // 8. 解析 JSON 响应
    let response: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| McpError::internal(&format!("解析响应 JSON 失败: {}", e)))?;

    // 9. 检查是否有 error 字段
    if let Some(error_msg) = response.get("error").and_then(|v| v.as_str()) {
        return Err(McpError::internal(error_msg));
    }

    Ok(response)
}

/// 从参数中读取可选字符串
fn opt_string(args: &serde_json::Map<String, serde_json::Value>, key: &str, default: &str) -> String {
    args.get(key).and_then(|v| v.as_str()).unwrap_or(default).to_string()
}

/// 从参数中读取可选整数
fn opt_int(args: &serde_json::Map<String, serde_json::Value>, key: &str, default: i64) -> i64 {
    args.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

/// 从参数中读取可选浮点数
fn opt_float(args: &serde_json::Map<String, serde_json::Value>, key: &str, default: f64) -> f64 {
    args.get(key).and_then(|v| v.as_f64()).unwrap_or(default)
}

/// 从参数中读取可选布尔值
fn opt_bool(args: &serde_json::Map<String, serde_json::Value>, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

/// 从参数中读取必填字符串
fn req_string(args: &serde_json::Map<String, serde_json::Value>, key: &str) -> Result<String, McpError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| McpError::invalid_params(&format!("缺少必填参数: {}", key)))
}

// ============================================================================
// 工具命令实现
// ============================================================================

/// 1. get_game_scene_tree: 获取游戏场景树
fn cmd_get_game_scene_tree(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let params = serde_json::json!({
        "max_depth": opt_int(args, "max_depth", -1),
        "script_filter": opt_string(args, "script_filter", ""),
        "type_filter": opt_string(args, "type_filter", ""),
        "named_only": opt_bool(args, "named_only", false),
    });
    send_game_command("get_scene_tree", params, 5.0)
}

/// 2. get_game_node_properties: 获取游戏节点属性
fn cmd_get_game_node_properties(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = req_string(args, "node_path")?;
    let properties = args.get("properties")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let params = serde_json::json!({
        "node_path": node_path,
        "properties": properties,
    });
    send_game_command("get_node_properties", params, 5.0)
}

/// 3. set_game_node_property: 设置游戏节点属性
fn cmd_set_game_node_property(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = req_string(args, "node_path")?;
    let property = req_string(args, "property")?;
    let value = args.get("value")
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: value"))?
        .clone();
    let params = serde_json::json!({
        "node_path": node_path,
        "property": property,
        "value": value,
    });
    send_game_command("set_node_property", params, 5.0)
}

/// 4. capture_frames: 连续截取游戏帧
fn cmd_capture_frames(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let count = opt_int(args, "count", 5);
    let frame_interval = opt_int(args, "frame_interval", 10);
    let half_resolution = opt_bool(args, "half_resolution", true);

    // 动态计算超时: count * frame_interval / 60.0 + 2.0，最多 25 秒
    let timeout = ((count as f64 * frame_interval as f64 / 60.0) + 2.0).min(25.0);

    let params = serde_json::json!({
        "count": count,
        "frame_interval": frame_interval,
        "half_resolution": half_resolution,
    });
    send_game_command("capture_frames", params, timeout)
}

/// 5. monitor_properties: 监控节点属性变化
fn cmd_monitor_properties(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = req_string(args, "node_path")?;
    let properties = args.get("properties")
        .and_then(|v| v.as_array())
        .cloned()
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: properties"))?;
    let frame_count = opt_int(args, "frame_count", 60);
    let frame_interval = opt_int(args, "frame_interval", 1);

    // 动态计算超时
    let timeout = (frame_count as f64 * frame_interval as f64 / 60.0) + 3.0;

    let params = serde_json::json!({
        "node_path": node_path,
        "properties": properties,
        "frame_count": frame_count,
        "frame_interval": frame_interval,
    });
    send_game_command("monitor_properties", params, timeout)
}

/// 6. execute_game_script: 在游戏中执行 GDScript 代码
fn cmd_execute_game_script(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let code = req_string(args, "code")?;
    let params = serde_json::json!({
        "code": code,
    });
    // 超时 10 秒
    send_game_command("execute_script", params, 10.0)
}

/// 7. start_recording: 开始录制输入事件
fn cmd_start_recording(_args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    send_game_command("start_recording", serde_json::json!({}), 5.0)
}

/// 8. stop_recording: 停止录制并获取录制数据
fn cmd_stop_recording(_args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    send_game_command("stop_recording", serde_json::json!({}), 5.0)
}

/// 9. replay_recording: 回放录制的事件
fn cmd_replay_recording(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let events = args.get("events")
        .and_then(|v| v.as_array())
        .cloned()
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: events"))?;
    let speed = opt_float(args, "speed", 1.0);

    // 动态计算超时: 最大事件时间 / speed + 5 秒，最多 120 秒
    let max_time = events.iter()
        .filter_map(|e| e.get("time").and_then(|t| t.as_f64()))
        .fold(0.0, f64::max);
    let timeout = ((max_time / speed) + 5.0).min(120.0);

    let params = serde_json::json!({
        "events": events,
        "speed": speed,
    });
    send_game_command("replay_recording", params, timeout)
}

/// 10. find_nodes_by_script: 按脚本查找节点
fn cmd_find_nodes_by_script(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let script = req_string(args, "script")?;
    let properties = args.get("properties")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let params = serde_json::json!({
        "script": script,
        "properties": properties,
    });
    send_game_command("find_nodes_by_script", params, 5.0)
}

/// 11. get_autoload: 获取自动加载节点信息
fn cmd_get_autoload(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let name = req_string(args, "name")?;
    let properties = args.get("properties")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let params = serde_json::json!({
        "name": name,
        "properties": properties,
    });
    send_game_command("get_autoload", params, 5.0)
}

/// 12. batch_get_properties: 批量获取节点属性
fn cmd_batch_get_properties(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let nodes = args.get("nodes")
        .and_then(|v| v.as_array())
        .cloned()
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: nodes"))?;
    let params = serde_json::json!({
        "nodes": nodes,
    });
    send_game_command("batch_get_properties", params, 5.0)
}

/// 13. find_ui_elements: 查找 UI 元素
fn cmd_find_ui_elements(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let type_filter = opt_string(args, "type_filter", "");
    let params = serde_json::json!({
        "type_filter": type_filter,
    });
    send_game_command("find_ui_elements", params, 5.0)
}

/// 14. click_button_by_text: 通过文本点击按钮
fn cmd_click_button_by_text(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let text = req_string(args, "text")?;
    let partial = opt_bool(args, "partial", true);
    let params = serde_json::json!({
        "text": text,
        "partial": partial,
    });
    send_game_command("click_button_by_text", params, 5.0)
}

/// 15. wait_for_node: 等待节点出现
fn cmd_wait_for_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = req_string(args, "node_path")?;
    let timeout = opt_float(args, "timeout", 5.0);
    let poll_frames = opt_int(args, "poll_frames", 5);

    // 超时 = timeout + 2 秒
    let cmd_timeout = timeout + 2.0;

    let params = serde_json::json!({
        "node_path": node_path,
        "timeout": timeout,
        "poll_frames": poll_frames,
    });
    send_game_command("wait_for_node", params, cmd_timeout)
}

/// 16. find_nearby_nodes: 查找附近的节点
fn cmd_find_nearby_nodes(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let position = args.get("position")
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: position"))?;
    let radius = args.get("radius").and_then(|v| v.as_f64());
    let type_filter = opt_string(args, "type_filter", "");
    let group_filter = opt_string(args, "group_filter", "");
    let max_results = args.get("max_results").and_then(|v| v.as_i64());

    let mut params = serde_json::json!({
        "position": position,
        "type_filter": type_filter,
        "group_filter": group_filter,
    });
    if let Some(r) = radius {
        params["radius"] = serde_json::json!(r);
    }
    if let Some(m) = max_results {
        params["max_results"] = serde_json::json!(m);
    }

    send_game_command("find_nearby_nodes", params, 5.0)
}

/// 17. navigate_to: 导航到目标位置（自动移动）
fn cmd_navigate_to(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let target = args.get("target")
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: target"))?;
    let player_path = opt_string(args, "player_path", "");
    let camera_path = opt_string(args, "camera_path", "");
    let move_speed = args.get("move_speed").and_then(|v| v.as_f64());

    let mut params = serde_json::json!({
        "target": target,
        "player_path": player_path,
        "camera_path": camera_path,
    });
    if let Some(s) = move_speed {
        params["move_speed"] = serde_json::json!(s);
    }

    send_game_command("navigate_to", params, 10.0)
}

/// 18. move_to: 移动到目标位置
fn cmd_move_to(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let target = args.get("target")
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: target"))?;
    let player_path = opt_string(args, "player_path", "");
    let camera_path = opt_string(args, "camera_path", "");
    let arrival_radius = args.get("arrival_radius").and_then(|v| v.as_f64());
    let timeout = opt_float(args, "timeout", 15.0);
    let run = opt_bool(args, "run", false);
    let look_at_target = opt_bool(args, "look_at_target", false);

    // 超时 = timeout + 5 秒
    let cmd_timeout = timeout + 5.0;

    let mut params = serde_json::json!({
        "target": target,
        "player_path": player_path,
        "camera_path": camera_path,
        "timeout": timeout,
        "run": run,
        "look_at_target": look_at_target,
    });
    if let Some(r) = arrival_radius {
        params["arrival_radius"] = serde_json::json!(r);
    }

    send_game_command("move_to", params, cmd_timeout)
}

/// 19. watch_signals: 监听节点信号
fn cmd_watch_signals(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_paths = args.get("node_paths")
        .and_then(|v| v.as_array())
        .cloned()
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: node_paths"))?;
    let signal_filter = args.get("signal_filter")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let duration_ms = opt_int(args, "duration_ms", 5000);

    // 超时 = duration_ms / 1000 + 5 秒
    let cmd_timeout = (duration_ms as f64 / 1000.0) + 5.0;

    let params = serde_json::json!({
        "node_paths": node_paths,
        "signal_filter": signal_filter,
        "duration_ms": duration_ms,
    });
    send_game_command("watch_signals", params, cmd_timeout)
}

// ============================================================================
// 工具注册
// ============================================================================

/// 收集本模块的所有工具定义
pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        // 1. get_game_scene_tree: 获取游戏场景树
        ToolDefinition::new("get_game_scene_tree", "获取运行中游戏的场景树结构", serde_json::json!({
            "type": "object", "properties": {
                "max_depth": { "type": "integer", "description": "最大遍历深度，-1 表示无限", "default": -1 },
                "script_filter": { "type": "string", "description": "按脚本路径过滤节点" },
                "type_filter": { "type": "string", "description": "按节点类型过滤（如 Node2D, Control）" },
                "named_only": { "type": "boolean", "description": "是否只返回有名称的节点", "default": false }
            }, "required": []
        })),

        // 2. get_game_node_properties: 获取游戏节点属性
        ToolDefinition::new("get_game_node_properties", "获取运行中游戏指定节点的属性", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "节点路径（相对于场景根节点）" },
                "properties": { "type": "array", "items": { "type": "string" }, "description": "要获取的属性列表（可选，不传则返回所有属性）" }
            }, "required": ["node_path"]
        })),

        // 3. set_game_node_property: 设置游戏节点属性
        ToolDefinition::new("set_game_node_property", "设置运行中游戏节点的属性值", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "节点路径" },
                "property": { "type": "string", "description": "属性名称" },
                "value": { "description": "属性值（支持数字、字符串、布尔、数组、对象等类型）" }
            }, "required": ["node_path", "property", "value"]
        })),

        // 4. capture_frames: 连续截取游戏帧
        ToolDefinition::new("capture_frames", "连续截取运行中游戏的帧画面", serde_json::json!({
            "type": "object", "properties": {
                "count": { "type": "integer", "description": "截取帧数", "default": 5 },
                "frame_interval": { "type": "integer", "description": "帧间隔", "default": 10 },
                "half_resolution": { "type": "boolean", "description": "是否使用半分辨率", "default": true }
            }, "required": []
        })),

        // 5. monitor_properties: 监控节点属性变化
        ToolDefinition::new("monitor_properties", "监控运行中游戏节点的属性随时间的变化", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "节点路径" },
                "properties": { "type": "array", "items": { "type": "string" }, "description": "要监控的属性列表" },
                "frame_count": { "type": "integer", "description": "采集帧数", "default": 60 },
                "frame_interval": { "type": "integer", "description": "采集间隔帧数", "default": 1 }
            }, "required": ["node_path", "properties"]
        })),

        // 6. execute_game_script: 在游戏中执行 GDScript 代码
        ToolDefinition::new("execute_game_script", "在运行中的游戏内执行 GDScript 代码", serde_json::json!({
            "type": "object", "properties": {
                "code": { "type": "string", "description": "要执行的 GDScript 代码" }
            }, "required": ["code"]
        })),

        // 7. start_recording: 开始录制输入事件
        ToolDefinition::new("start_recording", "开始录制游戏中的输入事件（键盘、鼠标等）", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),

        // 8. stop_recording: 停止录制并获取录制数据
        ToolDefinition::new("stop_recording", "停止录制并返回已录制的输入事件数据", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),

        // 9. replay_recording: 回放录制的事件
        ToolDefinition::new("replay_recording", "回放之前录制的输入事件序列", serde_json::json!({
            "type": "object", "properties": {
                "events": { "type": "array", "description": "要回放的事件数组" },
                "speed": { "type": "number", "description": "回放速度倍率", "default": 1.0 }
            }, "required": ["events"]
        })),

        // 10. find_nodes_by_script: 按脚本查找节点
        ToolDefinition::new("find_nodes_by_script", "在运行中的游戏中按脚本路径查找节点", serde_json::json!({
            "type": "object", "properties": {
                "script": { "type": "string", "description": "脚本路径（如 res://enemy.gd）" },
                "properties": { "type": "array", "items": { "type": "string" }, "description": "需要同时获取的属性列表" }
            }, "required": ["script"]
        })),

        // 11. get_autoload: 获取自动加载节点信息
        ToolDefinition::new("get_autoload", "获取自动加载节点的信息", serde_json::json!({
            "type": "object", "properties": {
                "name": { "type": "string", "description": "自动加载节点名称" },
                "properties": { "type": "array", "items": { "type": "string" }, "description": "需要获取的属性列表" }
            }, "required": ["name"]
        })),

        // 12. batch_get_properties: 批量获取节点属性
        ToolDefinition::new("batch_get_properties", "批量获取多个节点的属性", serde_json::json!({
            "type": "object", "properties": {
                "nodes": { "type": "array", "description": "节点信息数组，每个元素包含 node_path 和可选的 properties", "items": {
                    "type": "object", "properties": {
                        "node_path": { "type": "string", "description": "节点路径" },
                        "properties": { "type": "array", "items": { "type": "string" }, "description": "需要获取的属性" }
                    }, "required": ["node_path"]
                }}
            }, "required": ["nodes"]
        })),

        // 13. find_ui_elements: 查找 UI 元素
        ToolDefinition::new("find_ui_elements", "查找运行中游戏的所有 UI 元素", serde_json::json!({
            "type": "object", "properties": {
                "type_filter": { "type": "string", "description": "按控件类型过滤（如 Button, Label）" }
            }, "required": []
        })),

        // 14. click_button_by_text: 通过文本点击按钮
        ToolDefinition::new("click_button_by_text", "通过按钮文本点击运行中游戏的按钮", serde_json::json!({
            "type": "object", "properties": {
                "text": { "type": "string", "description": "按钮上显示的文本" },
                "partial": { "type": "boolean", "description": "是否使用部分匹配", "default": true }
            }, "required": ["text"]
        })),

        // 15. wait_for_node: 等待节点出现
        ToolDefinition::new("wait_for_node", "等待运行中游戏的指定节点出现", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "要等待的节点路径" },
                "timeout": { "type": "number", "description": "超时时间（秒）", "default": 5.0 },
                "poll_frames": { "type": "integer", "description": "每隔多少帧检查一次", "default": 5 }
            }, "required": ["node_path"]
        })),

        // 16. find_nearby_nodes: 查找附近的节点
        ToolDefinition::new("find_nearby_nodes", "在运行中的游戏内查找指定位置附近的节点", serde_json::json!({
            "type": "object", "properties": {
                "position": { "type": "object", "description": "中心位置，如 {\"x\": 0, \"y\": 0}", "additionalProperties": true },
                "radius": { "type": "number", "description": "搜索半径" },
                "type_filter": { "type": "string", "description": "节点类型过滤" },
                "group_filter": { "type": "string", "description": "节点组过滤" },
                "max_results": { "type": "integer", "description": "最大返回结果数" }
            }, "required": ["position"]
        })),

        // 17. navigate_to: 导航到目标位置
        ToolDefinition::new("navigate_to", "控制游戏角色导航到目标位置（使用导航网格自动寻路）", serde_json::json!({
            "type": "object", "properties": {
                "target": { "description": "目标位置（字符串路径或坐标字典 {\"x\", \"y\", \"z\"}）" },
                "player_path": { "type": "string", "description": "玩家节点路径（可选）" },
                "camera_path": { "type": "string", "description": "相机节点路径（可选）" },
                "move_speed": { "type": "number", "description": "移动速度（可选）" }
            }, "required": ["target"]
        })),

        // 18. move_to: 移动到目标位置
        ToolDefinition::new("move_to", "控制游戏角色直接移动到目标位置", serde_json::json!({
            "type": "object", "properties": {
                "target": { "description": "目标位置（字符串路径或坐标字典）" },
                "player_path": { "type": "string", "description": "玩家节点路径" },
                "camera_path": { "type": "string", "description": "相机节点路径" },
                "arrival_radius": { "type": "number", "description": "到达判定半径" },
                "timeout": { "type": "number", "description": "超时时间（秒）", "default": 15.0 },
                "run": { "type": "boolean", "description": "是否奔跑", "default": false },
                "look_at_target": { "type": "boolean", "description": "是否面向目标", "default": false }
            }, "required": ["target"]
        })),

        // 19. watch_signals: 监听节点信号
        ToolDefinition::new("watch_signals", "监听运行中游戏节点的信号发射", serde_json::json!({
            "type": "object", "properties": {
                "node_paths": { "type": "array", "items": { "type": "string" }, "description": "要监听的节点路径列表" },
                "signal_filter": { "type": "array", "items": { "type": "string" }, "description": "要监听的信号名称过滤列表（可选）" },
                "duration_ms": { "type": "integer", "description": "监听持续时间（毫秒）", "default": 5000 }
            }, "required": ["node_paths"]
        })),
    ]
}

/// 注册本模块的所有命令到全局注册表
pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("get_game_scene_tree".into(), cmd_get_game_scene_tree);
    registry.insert("get_game_node_properties".into(), cmd_get_game_node_properties);
    registry.insert("set_game_node_property".into(), cmd_set_game_node_property);
    registry.insert("capture_frames".into(), cmd_capture_frames);
    registry.insert("monitor_properties".into(), cmd_monitor_properties);
    registry.insert("execute_game_script".into(), cmd_execute_game_script);
    registry.insert("start_recording".into(), cmd_start_recording);
    registry.insert("stop_recording".into(), cmd_stop_recording);
    registry.insert("replay_recording".into(), cmd_replay_recording);
    registry.insert("find_nodes_by_script".into(), cmd_find_nodes_by_script);
    registry.insert("get_autoload".into(), cmd_get_autoload);
    registry.insert("batch_get_properties".into(), cmd_batch_get_properties);
    registry.insert("find_ui_elements".into(), cmd_find_ui_elements);
    registry.insert("click_button_by_text".into(), cmd_click_button_by_text);
    registry.insert("wait_for_node".into(), cmd_wait_for_node);
    registry.insert("find_nearby_nodes".into(), cmd_find_nearby_nodes);
    registry.insert("navigate_to".into(), cmd_navigate_to);
    registry.insert("move_to".into(), cmd_move_to);
    registry.insert("watch_signals".into(), cmd_watch_signals);
}
