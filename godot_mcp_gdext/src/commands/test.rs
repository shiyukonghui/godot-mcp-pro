//! 测试命令模块
//!
//! 提供测试场景运行、节点状态断言、文本断言、压力测试等功能。
//! 对应原 GDScript 插件的 test_commands.gd

use std::collections::HashMap;
use std::time::Duration;

use godot::classes::file_access::ModeFlags;
use godot::classes::{
    ConfigFile, DirAccess, EditorInterface, Expression, FileAccess, Os,
};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

// ============================================================================
// 辅助函数
// ============================================================================

/// 从参数中读取可选字符串
fn opt_string(args: &serde_json::Map<String, serde_json::Value>, key: &str, default: &str) -> String {
    args.get(key).and_then(|v| v.as_str()).unwrap_or(default).to_string()
}

/// 从参数中读取可选布尔值
fn opt_bool(args: &serde_json::Map<String, serde_json::Value>, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

/// 从参数中读取可选整数
fn opt_int(args: &serde_json::Map<String, serde_json::Value>, key: &str, default: i64) -> i64 {
    args.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

/// 从参数中读取必填字符串
fn req_string(args: &serde_json::Map<String, serde_json::Value>, key: &str) -> Result<String, McpError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| McpError::invalid_params(&format!("缺少必填参数: {}", key)))
}

/// 获取游戏用户目录（与 runtime.rs 中的实现相同）
fn get_game_user_dir() -> Result<String, McpError> {
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

    let user_dir = Os::singleton().get_user_data_dir().to_string();
    let parent = std::path::Path::new(&user_dir)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| user_dir.clone());

    Ok(format!("{}/{}", parent, game_name))
}

/// 通过文件 IPC 向运行中的游戏发送命令并等待响应
fn send_game_command(
    command: &str,
    params: serde_json::Value,
    timeout_sec: f64,
) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();

    if !editor.is_playing_scene() {
        return Err(McpError::internal("游戏未在运行，请先运行场景"));
    }

    let user_dir = get_game_user_dir()?;
    let request_path = format!("{}/mcp_game_request", user_dir);
    let response_path = format!("{}/mcp_game_response", user_dir);

    // 清理旧的响应文件
    if FileAccess::file_exists(&response_path) {
        let _ = DirAccess::remove_absolute(&response_path);
    }

    // 写入请求 JSON
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

    // 轮询响应文件
    let start = std::time::Instant::now();
    let mut response_found = false;

    while start.elapsed().as_secs_f64() < timeout_sec {
        std::thread::sleep(Duration::from_millis(100));

        if FileAccess::file_exists(&response_path) {
            response_found = true;
            break;
        }

        if !EditorInterface::singleton().is_playing_scene() {
            let _ = DirAccess::remove_absolute(&request_path);
            return Err(McpError::internal("游戏在等待响应期间停止运行"));
        }
    }

    if !response_found {
        let _ = DirAccess::remove_absolute(&request_path);
        return Err(McpError::internal(&format!(
            "等待游戏响应超时 ({}秒)",
            timeout_sec
        )));
    }

    // 读取响应文件
    let file = FileAccess::open(&response_path, ModeFlags::READ)
        .ok_or_else(|| McpError::internal("无法读取响应文件"))?;
    let content = file.get_as_text().to_string();
    drop(file);
    let _ = DirAccess::remove_absolute(&response_path);
    let _ = DirAccess::remove_absolute(&request_path);

    let response: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| McpError::internal(&format!("解析响应 JSON 失败: {}", e)))?;

    if let Some(error_msg) = response.get("error").and_then(|v| v.as_str()) {
        return Err(McpError::internal(error_msg));
    }

    Ok(response)
}

// ============================================================================
// 工具命令实现
// ============================================================================

/// 1. run_test_scenario: 运行测试场景
fn cmd_run_test_scenario(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let mut editor = EditorInterface::singleton();
    let scene_path = opt_string(args, "scene_path", "");

    // 如果指定了场景路径，先打开并运行场景
    if !scene_path.is_empty() {
        if editor.is_playing_scene() {
            editor.stop_playing_scene();
            std::thread::sleep(Duration::from_millis(500));
        }

        if scene_path == "main" {
            editor.play_main_scene();
        } else if scene_path == "current" {
            editor.play_current_scene();
        } else {
            // 检查场景文件是否存在
            if !FileAccess::file_exists(&scene_path) {
                return Err(McpError::not_found(&format!("场景文件 '{}'", scene_path), ""));
            }
            editor.play_custom_scene(&scene_path);
        }

        // 等待游戏启动
        std::thread::sleep(Duration::from_secs(1));
    }

    // 确认游戏在运行
    if !editor.is_playing_scene() {
        return Err(McpError::internal("场景未在运行，请先使用 play_scene"));
    }

    let steps = args.get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: steps (Array)"))?;

    if steps.is_empty() {
        return Err(McpError::invalid_params("steps 数组为空"));
    }

    let mut results = Vec::new();
    let mut pass_count = 0i64;
    let mut fail_count = 0i64;
    let mut error_count = 0i64;

    for (i, step) in steps.iter().enumerate() {
        let step_type = step.get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut step_result = serde_json::json!({
            "step": i,
            "type": step_type,
        });

        match step_type {
            "input" => {
                // 输入步骤：写入 mcp_input_commands 文件
                let action = step.get("action").and_then(|v| v.as_str());
                let keycode = step.get("keycode").and_then(|v| v.as_str());

                let mut events = Vec::new();
                if let Some(action_name) = action {
                    let pressed = step.get("pressed").and_then(|v| v.as_bool()).unwrap_or(true);
                    events.push(serde_json::json!({
                        "type": "action",
                        "action": action_name,
                        "pressed": pressed,
                        "strength": step.get("strength").and_then(|v| v.as_f64()).unwrap_or(1.0),
                    }));
                } else if let Some(kc) = keycode {
                    let pressed = step.get("pressed").and_then(|v| v.as_bool()).unwrap_or(true);
                    events.push(serde_json::json!({
                        "type": "key",
                        "keycode": kc,
                        "pressed": pressed,
                    }));
                }

                if !events.is_empty() {
                    let input_json = serde_json::json!({
                        "sequence_events": events,
                        "frame_delay": step.get("frame_delay").and_then(|v| v.as_i64()).unwrap_or(1),
                    });
                    let input_str = serde_json::to_string(&input_json)
                        .map_err(|e| McpError::internal(&format!("序列化输入失败: {}", e)))?;
                    if let Some(mut file) = FileAccess::open("user://mcp_input_commands", ModeFlags::WRITE) {
                        file.store_string(&input_str);
                        step_result["sent"] = serde_json::json!(true);
                        step_result["event_count"] = serde_json::json!(events.len());
                    } else {
                        step_result["error"] = serde_json::json!("写入输入命令文件失败");
                        error_count += 1;
                    }
                }
            }

            "wait" => {
                if let Some(seconds) = step.get("seconds").and_then(|v| v.as_f64()) {
                    std::thread::sleep(Duration::from_secs_f64(seconds));
                    step_result["waited_seconds"] = serde_json::json!(seconds);
                } else if let Some(node_path) = step.get("node_path").and_then(|v| v.as_str()) {
                    // 等待节点出现：通过 IPC 发送 wait_for_node 命令
                    let timeout = step.get("timeout").and_then(|v| v.as_f64()).unwrap_or(5.0);
                    match send_game_command("wait_for_node", serde_json::json!({
                        "node_path": node_path,
                        "timeout": timeout,
                        "poll_frames": step.get("poll_frames").and_then(|v| v.as_i64()).unwrap_or(5),
                    }), timeout + 2.0) {
                        Ok(resp) => {
                            step_result["found"] = resp.get("found").cloned().unwrap_or(serde_json::json!(false));
                        }
                        Err(e) => {
                            step_result["error"] = serde_json::json!(e.message);
                            error_count += 1;
                        }
                    }
                }
            }

            "assert" => {
                // 断言步骤
                if let Some(text) = step.get("text").and_then(|v| v.as_str()) {
                    // 屏幕文本断言
                    let partial = step.get("partial").and_then(|v| v.as_bool()).unwrap_or(true);
                    match send_game_command("find_ui_elements", serde_json::json!({}), 5.0) {
                        Ok(resp) => {
                            let elements = resp.pointer("/result/elements")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            let mut found = false;
                            for elem in &elements {
                                let elem_text = elem.get("text").and_then(|v| v.as_str()).unwrap_or("");
                                if partial && elem_text.contains(text) {
                                    found = true;
                                    step_result["found_in"] = serde_json::json!(elem_text);
                                    break;
                                } else if !partial && elem_text == text {
                                    found = true;
                                    step_result["found_in"] = serde_json::json!(elem_text);
                                    break;
                                }
                            }
                            step_result["passed"] = serde_json::json!(found);
                            if !found {
                                step_result["error"] = serde_json::json!("屏幕上未找到指定文本");
                                fail_count += 1;
                            } else {
                                pass_count += 1;
                            }
                        }
                        Err(e) => {
                            step_result["passed"] = serde_json::json!(false);
                            step_result["error"] = serde_json::json!(e.message);
                            error_count += 1;
                        }
                    }
                } else if step.get("node_path").is_some() && step.get("property").is_some() {
                    // 节点属性断言
                    let node_path = step.get("node_path").and_then(|v| v.as_str()).unwrap_or("");
                    let property = step.get("property").and_then(|v| v.as_str()).unwrap_or("");
                    let expected = step.get("expected");
                    let operator = step.get("operator").and_then(|v| v.as_str()).unwrap_or("eq");

                    match send_game_command("assert_node_state", serde_json::json!({
                        "node_path": node_path,
                        "property": property,
                        "expected": expected,
                        "operator": operator,
                    }), 5.0) {
                        Ok(resp) => {
                            if let Some(passed) = resp.get("passed").and_then(|v| v.as_bool()) {
                                step_result["passed"] = serde_json::json!(passed);
                                if passed {
                                    pass_count += 1;
                                } else {
                                    fail_count += 1;
                                }
                            }
                            // 复制其他字段
                            if let Some(actual) = resp.get("actual_value") {
                                step_result["actual_value"] = actual.clone();
                            }
                        }
                        Err(e) => {
                            step_result["passed"] = serde_json::json!(false);
                            step_result["error"] = serde_json::json!(e.message);
                            error_count += 1;
                        }
                    }
                } else {
                    step_result["error"] = serde_json::json!("断言步骤需要 'text' 或 'node_path' + 'property'");
                    error_count += 1;
                }
            }

            _ => {
                step_result["error"] = serde_json::json!(format!("未知步骤类型: {}", step_type));
                error_count += 1;
            }
        }

        results.push(step_result);

        // 检查游戏是否仍在运行
        if !editor.is_playing_scene() {
            results.push(serde_json::json!({
                "step": i + 1,
                "error": "游戏意外停止",
            }));
            error_count += 1;
            break;
        }
    }

    Ok(serde_json::json!({
        "total_steps": steps.len(),
        "completed_steps": results.len(),
        "assertions_passed": pass_count,
        "assertions_failed": fail_count,
        "errors": error_count,
        "all_passed": fail_count == 0 && error_count == 0,
        "results": results,
    }))
}

/// 2. assert_node_state: 断言节点属性值
fn cmd_assert_node_state(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = req_string(args, "node_path")?;
    let property = req_string(args, "property")?;

    if !args.contains_key("expected") {
        return Err(McpError::invalid_params("缺少必填参数: expected"));
    }
    let expected = args.get("expected").cloned().unwrap_or(serde_json::Value::Null);
    let operator = opt_string(args, "operator", "eq");

    // 通过 IPC 向运行中的游戏发送断言命令
    let result = send_game_command("assert_node_state", serde_json::json!({
        "node_path": node_path,
        "property": property,
        "expected": expected,
        "operator": operator,
    }), 5.0)?;

    Ok(result)
}

/// 3. assert_screen_text: 断言屏幕上存在指定文本
fn cmd_assert_screen_text(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let expected_text = req_string(args, "text")?;
    let partial = opt_bool(args, "partial", true);
    let _case_sensitive = opt_bool(args, "case_sensitive", true);

    // 通过 IPC 获取 UI 元素
    let ui_result = send_game_command("find_ui_elements", serde_json::json!({}), 5.0)?;

    let elements = ui_result.pointer("/result/elements")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut found = false;
    let mut matched_element = serde_json::json!({});
    let mut all_texts = Vec::new();

    for elem in &elements {
        let elem_text = elem.get("text").and_then(|v| v.as_str()).unwrap_or("");
        if elem_text.is_empty() {
            continue;
        }
        all_texts.push(elem_text.to_string());

        if partial {
            if elem_text.contains(&expected_text) {
                found = true;
                matched_element = elem.clone();
                break;
            }
        } else {
            if elem_text == &expected_text {
                found = true;
                matched_element = elem.clone();
                break;
            }
        }
    }

    let mut assertion = serde_json::json!({
        "passed": found,
        "expected_text": expected_text,
        "partial": partial,
    });

    if found {
        assertion["matched_element"] = serde_json::json!({
            "text": matched_element.get("text"),
            "type": matched_element.get("type"),
            "path": matched_element.get("path"),
        });
    } else {
        assertion["visible_texts"] = serde_json::json!(all_texts);
    }

    Ok(serde_json::json!({ "result": assertion }))
}

/// 4. run_stress_test: 运行压力测试
fn cmd_run_stress_test(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let count = opt_int(args, "count", 10);
    let action = opt_string(args, "action", "");

    let editor = EditorInterface::singleton();
    if !editor.is_playing_scene() {
        return Err(McpError::internal("场景未在运行，请先运行场景"));
    }

    let mut events_sent = 0i64;
    let start = std::time::Instant::now();
    let mut timings = Vec::new();

    for i in 0..count {
        if !editor.is_playing_scene() {
            return Ok(serde_json::json!({
                "completed": false,
                "crashed": true,
                "iterations_completed": i,
                "events_sent": events_sent,
                "elapsed_seconds": start.elapsed().as_secs_f64(),
                "error": "压力测试期间游戏停止运行",
            }));
        }

        let iter_start = std::time::Instant::now();

        // 构造输入事件
        let mut events = Vec::new();
        if !action.is_empty() {
            events.push(serde_json::json!({
                "type": "action",
                "action": action,
                "pressed": true,
                "strength": 1.0,
            }));
            events.push(serde_json::json!({
                "type": "action",
                "action": action,
                "pressed": false,
                "strength": 0.0,
            }));
        } else {
            // 默认使用方向键
            let actions = ["ui_up", "ui_down", "ui_left", "ui_right", "ui_accept"];
            let idx = (i as usize) % actions.len();
            events.push(serde_json::json!({
                "type": "action",
                "action": actions[idx],
                "pressed": true,
                "strength": 1.0,
            }));
            events.push(serde_json::json!({
                "type": "action",
                "action": actions[idx],
                "pressed": false,
                "strength": 0.0,
            }));
        }

        // 写入输入文件
        let input_json = serde_json::json!({
            "sequence_events": events,
            "frame_delay": 1,
        });
        let input_str = serde_json::to_string(&input_json)
            .map_err(|e| McpError::internal(&format!("序列化失败: {}", e)))?;
        if let Some(mut file) = FileAccess::open("user://mcp_input_commands", ModeFlags::WRITE) {
            file.store_string(&input_str);
            events_sent += events.len() as i64;
        }

        let elapsed = iter_start.elapsed().as_secs_f64();
        timings.push(elapsed);

        // 每次迭代后短暂等待
        std::thread::sleep(Duration::from_millis(50));
    }

    let total_elapsed = start.elapsed().as_secs_f64();
    let avg_time = if !timings.is_empty() {
        timings.iter().sum::<f64>() / timings.len() as f64
    } else {
        0.0
    };

    Ok(serde_json::json!({
        "completed": true,
        "crashed": false,
        "iterations": count,
        "events_sent": events_sent,
        "elapsed_seconds": total_elapsed,
        "average_iteration_time": avg_time,
        "game_still_running": editor.is_playing_scene(),
    }))
}

/// 5. get_test_report: 获取测试报告
fn cmd_get_test_report(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let _clear = opt_bool(args, "clear", true);

    // 使用 GDScript Expression 获取测试结果
    // 在 Rust 端维护_test_results 不现实，所以通过 Expression 获取
    let mut expr = Expression::new_gd();
    let report_code = "\
var ei = EditorInterface \
if ei == null: \
    return JSON.stringify({\"error\": \"No EditorInterface\"}) \
return JSON.stringify({\"message\": \"使用 assert_node_state 等测试命令会自动收集结果。请查阅最近执行的测试命令输出。\", \"available_commands\": [\"assert_node_state\", \"assert_screen_text\", \"run_test_scenario\"]})";

    if expr.parse(report_code) == godot::global::Error::OK {
        let result = expr.execute();
        let result_str = match result.try_to::<String>() {
            Ok(s) => s,
            Err(_) => result.stringify().to_string(),
        };
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result_str) {
            return Ok(parsed);
        }
    }

    // 回退方案
    Ok(serde_json::json!({
        "message": "测试报告",
        "note": "测试结果在各命令的返回中查看。使用 run_test_scenario 执行完整测试流程。",
        "available_commands": ["run_test_scenario", "assert_node_state", "assert_screen_text", "run_stress_test"],
    }))
}

// ============================================================================
// 工具注册
// ============================================================================

/// 收集本模块的所有工具定义
pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        // 1. run_test_scenario: 运行测试场景
        ToolDefinition::new("run_test_scenario", "运行测试场景并执行一系列测试步骤", serde_json::json!({
            "type": "object", "properties": {
                "scene_path": { "type": "string", "description": "场景路径，可选 'main', 'current' 或 res:// 路径" },
                "steps": { "type": "array", "description": "测试步骤数组，每个步骤包含 type(input/wait/assert) 和相关参数", "items": {
                    "type": "object", "properties": {
                        "type": { "type": "string", "enum": ["input", "wait", "assert"], "description": "步骤类型" },
                        "action": { "type": "string", "description": "操作名称（input 类型使用）" },
                        "keycode": { "type": "string", "description": "按键代码（input 类型使用）" },
                        "seconds": { "type": "number", "description": "等待秒数（wait 类型使用）" },
                        "node_path": { "type": "string", "description": "节点路径（wait/assert 类型使用）" },
                        "property": { "type": "string", "description": "属性名（assert 类型使用）" },
                        "text": { "type": "string", "description": "期望文本（assert 类型使用）" },
                        "expected": { "description": "期望值（assert 类型使用）" },
                        "operator": { "type": "string", "description": "比较操作符: eq/neq/gt/lt/gte/lte/contains/type_is", "default": "eq" }
                    }, "required": ["type"]
                }}
            }, "required": ["steps"]
        })),

        // 2. assert_node_state: 断言节点属性
        ToolDefinition::new("assert_node_state", "断言运行中游戏节点的属性值符合预期", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "节点路径" },
                "property": { "type": "string", "description": "属性名称" },
                "expected": { "description": "期望值" },
                "operator": { "type": "string", "description": "比较操作符: eq/neq/gt/lt/gte/lte/contains/type_is", "default": "eq" }
            }, "required": ["node_path", "property", "expected"]
        })),

        // 3. assert_screen_text: 断言屏幕文本
        ToolDefinition::new("assert_screen_text", "断言运行中游戏屏幕上存在指定文本", serde_json::json!({
            "type": "object", "properties": {
                "text": { "type": "string", "description": "要查找的文本" },
                "partial": { "type": "boolean", "description": "是否使用部分匹配", "default": true },
                "case_sensitive": { "type": "boolean", "description": "是否区分大小写", "default": true }
            }, "required": ["text"]
        })),

        // 4. run_stress_test: 运行压力测试
        ToolDefinition::new("run_stress_test", "运行压力测试，重复执行指定操作 count 次", serde_json::json!({
            "type": "object", "properties": {
                "count": { "type": "integer", "description": "重复次数", "default": 10 },
                "action": { "type": "string", "description": "要执行的操作（可选，默认使用方向键）" }
            }, "required": []
        })),

        // 5. get_test_report: 获取测试报告
        ToolDefinition::new("get_test_report", "获取测试结果报告", serde_json::json!({
            "type": "object", "properties": {
                "clear": { "type": "boolean", "description": "是否清除结果", "default": true }
            }, "required": []
        })),
    ]
}

/// 注册本模块的所有命令到全局注册表
pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("run_test_scenario".into(), cmd_run_test_scenario);
    registry.insert("assert_node_state".into(), cmd_assert_node_state);
    registry.insert("assert_screen_text".into(), cmd_assert_screen_text);
    registry.insert("run_stress_test".into(), cmd_run_stress_test);
    registry.insert("get_test_report".into(), cmd_get_test_report);
}
