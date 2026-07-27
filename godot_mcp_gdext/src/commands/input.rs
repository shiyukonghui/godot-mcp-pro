//! 输入模拟命令模块

use std::collections::HashMap;
use godot::builtin::Vector2;
use godot::classes::{Input, InputEventAction, InputEventKey, InputEventMouseButton, InputEventMouseMotion, InputMap, Os};
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("simulate_key", "模拟键盘按键", serde_json::json!({
            "type": "object", "properties": {
                "keycode": { "type": "string" }, "pressed": { "type": "boolean", "default": true },
                "shift": { "type": "boolean", "default": false }, "ctrl": { "type": "boolean", "default": false },
                "alt": { "type": "boolean", "default": false }
            }, "required": ["keycode"]
        })),
        ToolDefinition::new("simulate_mouse_click", "模拟鼠标点击", serde_json::json!({
            "type": "object", "properties": {
                "button": { "type": "integer", "default": 1 }, "pressed": { "type": "boolean", "default": true },
                "x": { "type": "number", "default": 0 }, "y": { "type": "number", "default": 0 }
            }, "required": []
        })),
        ToolDefinition::new("simulate_mouse_move", "模拟鼠标移动", serde_json::json!({
            "type": "object", "properties": { "x": { "type": "number", "default": 0 }, "y": { "type": "number", "default": 0 } }, "required": []
        })),
        ToolDefinition::new("simulate_action", "模拟 Input Action", serde_json::json!({
            "type": "object", "properties": { "action": { "type": "string" }, "pressed": { "type": "boolean", "default": true }, "strength": { "type": "number", "default": 1.0 } }, "required": ["action"]
        })),
        ToolDefinition::new("get_input_actions", "列出所有 Input Action", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        ToolDefinition::new("set_input_action", "创建 Input Action", serde_json::json!({
            "type": "object", "properties": { "action": { "type": "string" }, "key": { "type": "string" } }, "required": ["action"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("simulate_key".into(), cmd_simulate_key);
    registry.insert("simulate_mouse_click".into(), cmd_simulate_mouse_click);
    registry.insert("simulate_mouse_move".into(), cmd_simulate_mouse_move);
    registry.insert("simulate_action".into(), cmd_simulate_action);
    registry.insert("get_input_actions".into(), cmd_get_input_actions);
    registry.insert("set_input_action".into(), cmd_set_input_action);
}

fn find_keycode(s: &str) -> godot::global::Key { Os::singleton().find_keycode_from_string(s) }

fn cmd_simulate_key(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let key = args.get("keycode").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing keycode"))?;
    let code = find_keycode(key);
    let mut e = InputEventKey::new_gd();
    e.set("keycode", &Variant::from(code));
    e.set("pressed", &Variant::from(args.get("pressed").and_then(|v| v.as_bool()).unwrap_or(true)));
    e.set("shift_pressed", &Variant::from(args.get("shift").and_then(|v| v.as_bool()).unwrap_or(false)));
    e.set("ctrl_pressed", &Variant::from(args.get("ctrl").and_then(|v| v.as_bool()).unwrap_or(false)));
    e.set("alt_pressed", &Variant::from(args.get("alt").and_then(|v| v.as_bool()).unwrap_or(false)));
    Input::singleton().parse_input_event(&e);
    Ok(serde_json::json!({"simulated": "key", "keycode": key}))
}

fn cmd_simulate_mouse_click(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let btn = args.get("button").and_then(|v| v.as_i64()).unwrap_or(1);
    let x = args.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let y = args.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let mut e = InputEventMouseButton::new_gd();
    e.set("button_index", &Variant::from(btn as i64));
    e.set("pressed", &Variant::from(args.get("pressed").and_then(|v| v.as_bool()).unwrap_or(true)));
    e.set("position", &Variant::from(Vector2::new(x, y)));
    Input::singleton().parse_input_event(&e);
    Ok(serde_json::json!({"simulated": "click", "button": btn}))
}

fn cmd_simulate_mouse_move(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let x = args.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let y = args.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let mut e = InputEventMouseMotion::new_gd();
    e.set("position", &Variant::from(Vector2::new(x, y)));
    Input::singleton().parse_input_event(&e);
    Ok(serde_json::json!({"simulated": "mousemove", "x": x, "y": y}))
}

fn cmd_simulate_action(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let action = args.get("action").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing action"))?;
    let mut e = InputEventAction::new_gd();
    e.set("action", &Variant::from(action));
    e.set("pressed", &Variant::from(args.get("pressed").and_then(|v| v.as_bool()).unwrap_or(true)));
    e.set("strength", &Variant::from(args.get("strength").and_then(|v| v.as_f64()).unwrap_or(1.0)));
    Input::singleton().parse_input_event(&e);
    Ok(serde_json::json!({"simulated": "action", "action": action}))
}

fn cmd_get_input_actions(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let actions = InputMap::singleton().get_actions();
    let mut list: Vec<String> = Vec::new();
    for i in 0..actions.len() {
        if let Some(a) = actions.get(i) { list.push(a.to_string()); }
    }
    Ok(serde_json::json!({"actions": list, "count": list.len()}))
}

fn cmd_set_input_action(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let action = args.get("action").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing action"))?;
    let mut im = InputMap::singleton();
    if !im.has_action(action) { im.add_action(action); }
    if let Some(key) = args.get("key").and_then(|v| v.as_str()) {
        let code = find_keycode(key);
        let mut ev = InputEventKey::new_gd();
        ev.set("keycode", &Variant::from(code));
        im.action_add_event(action, &ev);
    }
    Ok(serde_json::json!({"action": action, "set": true}))
}
