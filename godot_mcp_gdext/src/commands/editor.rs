//! 编辑器工具命令模块
//!
//! 提供 13 个编辑器工具:
//! - execute_editor_script: 执行 GDScript
//! - get_editor_errors: 获取编辑器错误
//! - get_output_log: 获取输出日志
//! - get_editor_screenshot: 编辑器截图
//! - get_game_screenshot: 游戏运行截图
//! - clear_output: 清除输出
//! - reload_plugin: 重新加载插件
//! - reload_project: 重新加载项目
//! - get_signals: 获取节点信号
//! - compare_screenshots: 比较截图
//! - set_auto_dismiss: 设置自动关闭对话框
//! - get_editor_camera: 获取3D相机信息
//! - set_editor_camera: 设置3D相机

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use godot::classes::file_access::ModeFlags;
use godot::classes::{EditorInterface, Expression, FileAccess, Image, ProjectSettings};
use godot::global::Error;
use godot::obj::Gd;
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

/// 全局自动关闭对话框设置
static AUTO_DISMISS: AtomicBool = AtomicBool::new(false);

/// 收集本模块的工具定义
pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        // 1. execute_editor_script: 执行 GDScript 代码
        ToolDefinition::new("execute_editor_script", "在编辑器上下文中执行 GDScript 代码", serde_json::json!({
            "type": "object", "properties": {
                "code": { "type": "string", "description": "要执行的 GDScript 代码" }
            }, "required": ["code"]
        })),
        // 2. get_editor_errors: 获取编辑器错误列表
        ToolDefinition::new("get_editor_errors", "获取编辑器错误列表", serde_json::json!({
            "type": "object", "properties": {
                "max_lines": { "type": "integer", "description": "最大行数", "default": 50 }
            }, "required": []
        })),
        // 3. get_output_log: 获取输出日志
        ToolDefinition::new("get_output_log", "获取输出日志", serde_json::json!({
            "type": "object", "properties": {
                "max_lines": { "type": "integer", "description": "最大行数", "default": 100 },
                "filter": { "type": "string", "description": "过滤关键字" }
            }, "required": []
        })),
        // 4. get_editor_screenshot: 编辑器截图
        ToolDefinition::new("get_editor_screenshot", "获取编辑器视口截图 (返回 base64 PNG 或保存到文件)", serde_json::json!({
            "type": "object", "properties": {
                "save_path": { "type": "string", "description": "保存路径 (res:// 或 user://, 可选)" }
            }, "required": []
        })),
        // 5. get_game_screenshot: 游戏运行截图
        ToolDefinition::new("get_game_screenshot", "获取正在运行的游戏截图", serde_json::json!({
            "type": "object", "properties": {
                "save_path": { "type": "string", "description": "保存路径 (可选)" }
            }, "required": []
        })),
        // 6. clear_output: 清除输出面板
        ToolDefinition::new("clear_output", "清除编辑器输出面板", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        // 7. reload_plugin: 重新加载插件
        ToolDefinition::new("reload_plugin", "重新加载 MCP 插件", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        // 8. reload_project: 重新加载项目
        ToolDefinition::new("reload_project", "重新扫描项目文件系统", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        // 9. get_signals: 获取节点信号列表
        ToolDefinition::new("get_signals", "获取指定节点的信号列表及连接信息", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "节点路径 (相对于场景根节点)" }
            }, "required": ["node_path"]
        })),
        // 10. compare_screenshots: 比较两张截图差异
        ToolDefinition::new("compare_screenshots", "比较两张截图并生成差异图", serde_json::json!({
            "type": "object", "properties": {
                "image_a": { "type": "string", "description": "第一张图片 (路径或 base64 PNG)" },
                "image_b": { "type": "string", "description": "第二张图片 (路径或 base64 PNG)" },
                "threshold": { "type": "integer", "description": "像素差异阈值 (0-255)", "default": 10 }
            }, "required": ["image_a", "image_b"]
        })),
        // 11. set_auto_dismiss: 设置自动关闭对话框
        ToolDefinition::new("set_auto_dismiss", "设置编辑器自动关闭对话框行为", serde_json::json!({
            "type": "object", "properties": {
                "enabled": { "type": "boolean", "description": "是否启用自动关闭" }
            }, "required": ["enabled"]
        })),
        // 12. get_editor_camera: 获取编辑器3D相机信息
        ToolDefinition::new("get_editor_camera", "获取编辑器 3D 视口相机信息", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        // 13. set_editor_camera: 设置编辑器3D相机
        ToolDefinition::new("set_editor_camera", "设置编辑器 3D 视口相机参数", serde_json::json!({
            "type": "object", "properties": {
                "position": { "type": "object", "description": "位置 {x, y, z}", "additionalProperties": true },
                "rotation_degrees": { "type": "object", "description": "旋转角度 {x, y, z}", "additionalProperties": true },
                "look_at": { "type": "object", "description": "看向目标 {x, y, z}", "additionalProperties": true },
                "fov": { "type": "number", "description": "视场角 (度)" }
            }, "required": []
        })),
    ]
}

/// 注册本模块的命令
pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("execute_editor_script".into(), cmd_execute_editor_script);
    registry.insert("get_editor_errors".into(), cmd_get_editor_errors);
    registry.insert("get_output_log".into(), cmd_get_output_log);
    registry.insert("get_editor_screenshot".into(), cmd_get_editor_screenshot);
    registry.insert("get_game_screenshot".into(), cmd_get_game_screenshot);
    registry.insert("clear_output".into(), cmd_clear_output);
    registry.insert("reload_plugin".into(), cmd_reload_plugin);
    registry.insert("reload_project".into(), cmd_reload_project);
    registry.insert("get_signals".into(), cmd_get_signals);
    registry.insert("compare_screenshots".into(), cmd_compare_screenshots);
    registry.insert("set_auto_dismiss".into(), cmd_set_auto_dismiss);
    registry.insert("get_editor_camera".into(), cmd_get_editor_camera);
    registry.insert("set_editor_camera".into(), cmd_set_editor_camera);
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 从参数中读取可选字符串
fn opt_string(args: &serde_json::Map<String, serde_json::Value>, key: &str, default: &str) -> String {
    args.get(key).and_then(|v| v.as_str()).unwrap_or(default).to_string()
}

/// 从参数中读取可选整数
fn opt_int(args: &serde_json::Map<String, serde_json::Value>, key: &str, default: i64) -> i64 {
    args.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

/// 读取 godot.log 文件中的日志行, 支持过滤
fn read_log_file(max_lines: usize, filter: &str) -> Result<(Vec<String>, String), McpError> {
    let settings = ProjectSettings::singleton();
    let log_path = settings.globalize_path("user://logs/godot.log");

    if !FileAccess::file_exists(&log_path) {
        // 日志文件不存在时返回空列表
        return Ok((vec![], "no_log_file".into()));
    }

    let mut file = FileAccess::open(&log_path, ModeFlags::READ)
        .ok_or_else(|| McpError::internal("无法打开日志文件"))?;

    let content = file.get_as_text();
    file.close();

    // 转换为 Rust String 以便使用标准 split 方法
    let rust_content = content.to_string();
    let all_lines: Vec<&str> = rust_content.split('\n').collect();
    let start = if all_lines.len() > max_lines { all_lines.len() - max_lines } else { 0 };

    let result: Vec<String> = all_lines[start..]
        .iter()
        .filter(|line| {
            if filter.is_empty() { true }
            else { line.contains(filter) }
        })
        .map(|s| (*s).to_string())
        .collect();

    Ok((result, "log_file".into()))
}

/// 将 Image 保存为 PNG 并编码为 base64
fn image_to_base64(img: &mut Gd<Image>) -> Result<String, McpError> {
    let png_data = img.save_png_to_buffer();
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png_data.as_slice());
    Ok(b64)
}

/// 尝试获取编辑器 3D 视口相机 (使用 execute_editor_script 执行 GDScript)
fn get_editor_camera_via_script() -> Result<serde_json::Value, McpError> {
    let code = r#"var vp3d = EditorInterface.get_editor_viewport_3d()
if vp3d == null:
    return JSON.stringify({"error": "no_3d_viewport"})
var cam = vp3d.get_camera_3d()
if cam == null:
    return JSON.stringify({"error": "no_camera"})
var pos = cam.global_position
var rot = cam.rotation_degrees
return JSON.stringify({"position":{"x":pos.x,"y":pos.y,"z":pos.z},"rotation_degrees":{"x":rot.x,"y":rot.y,"z":rot.z},"fov":cam.fov,"near":cam.near,"far":cam.far})"#;
    let result = execute_expression(code)?;
    // 安全解析 JSON：不用 trim_matches，直接 serde_json 解析
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .map_err(|e| McpError::internal(&format!("解析相机数据失败: {}", e)))?;
    if parsed.get("error").is_some() {
        return Err(McpError::internal("无法获取3D视口, 请确保已打开3D场景"));
    }
    Ok(parsed)
}

/// 通过 Expression 执行 GDScript 并返回字符串结果
fn execute_expression(code: &str) -> Result<String, McpError> {
    let mut expr = Expression::new_gd();
    let parse_err = expr.parse(code);
    if parse_err != Error::OK {
        return Err(McpError::invalid_params(&format!("脚本解析失败: {:?}", parse_err)));
    }
    let result = expr.execute();
    if result.is_nil() {
        Ok("null".to_string())
    } else {
        // 安全序列化：使用 serialize_variant 避免强制类型转换 panic
        Ok(crate::utils::serialize::serialize_variant(&result).to_string())
    }
}

/// 安全解析 execute_expression 返回的 JSON 字符串
/// execute_expression 返回的是 JSON 字符串字面量（带外层引号），需要先解码内层字符串再解析 JSON
fn parse_expression_json(script_result: &str) -> Result<serde_json::Value, McpError> {
    // 脚本返回 null 时，execute_expression 返回 "null"，直接返回 JSON null
    if script_result == "null" {
        return Ok(serde_json::Value::Null);
    }
    let inner: String = serde_json::from_str(script_result)
        .map_err(|e| McpError::internal(&format!("解析表达式返回值失败: {}", e)))?;
    serde_json::from_str(&inner)
        .map_err(|e| McpError::internal(&format!("解析 JSON 数据失败: {}", e)))
}

// ============================================================================
// 命令实现
// ============================================================================

/// execute_editor_script: 使用 Expression 执行 GDScript
fn cmd_execute_editor_script(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let code = args.get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: code"))?;

    let mut expr = Expression::new_gd();
    let parse_err = expr.parse(code);
    if parse_err != Error::OK {
        return Err(McpError::invalid_params(&format!("脚本解析失败 (Error code: {:?})", parse_err)));
    }

    let result = expr.execute();
    let output_str = if result.is_nil() {
        "null".to_string()
    } else {
        // 安全序列化任意 Variant 类型，避免强制转换为 String 导致 panic
        crate::utils::serialize::serialize_variant(&result).to_string()
    };

    Ok(serde_json::json!({
        "output": [output_str],
        "return_value": output_str,
    }))
}

/// get_editor_errors: 从 godot.log 读取错误信息
///
/// 简化为从日志文件读取 ERROR/SCRIPT ERROR 行
fn cmd_get_editor_errors(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let max_lines = std::cmp::max(1, opt_int(args, "max_lines", 50) as usize);

    let (lines, _source) = read_log_file(max_lines, "")?;

    // 过滤出错误行
    let errors: Vec<String> = lines.into_iter()
        .filter(|l| {
            let upper = l.to_uppercase();
            upper.contains("ERROR") || upper.contains("SCRIPT ERROR") || upper.contains("PARSE ERROR")
        })
        .collect();

    Ok(serde_json::json!({
        "errors": errors,
        "count": errors.len(),
    }))
}

/// get_output_log: 读取输出日志
fn cmd_get_output_log(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let max_lines = std::cmp::max(1, opt_int(args, "max_lines", 100) as usize);
    let filter = opt_string(args, "filter", "");

    let (lines, source) = read_log_file(max_lines, &filter)?;

    Ok(serde_json::json!({
        "lines": lines,
        "count": lines.len(),
        "source": source,
    }))
}

/// get_editor_screenshot: 获取编辑器视口截图
///
/// 通过 get_base_control 获取编辑器根控件, 再获取其视口进行截图
fn cmd_get_editor_screenshot(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let _base_control = editor.get_base_control()
        .ok_or_else(|| McpError::internal("无法访问编辑器基控件"))?;

    // 使用 execute_editor_script 获取视口截图 (因为 ViewportTexture API 在 Rust 中可能受限)
    let code = format!(
        r#"var vp = EditorInterface.get_base_control().get_viewport()
var tex = vp.get_texture()
var img = tex.get_image()
var buf = img.save_png_to_buffer()
var b64 = Marshalls.raw_to_base64(buf)
var result = {{"width": img.get_width(), "height": img.get_height(), "image_base64": b64}}
return JSON.stringify(result)"#
    );

    let script_result = execute_expression(&code)?;
    // 安全解析 JSON（不再使用 trim_matches）
    match parse_expression_json(&script_result) {
        Ok(mut data) => {
            // 检查是否需要保存到文件
            let save_path = opt_string(args, "save_path", "");
            if !save_path.is_empty() {
                // 从 base64 还原并保存
                if let Some(b64) = data.get("image_base64").and_then(|v| v.as_str()) {
                    use base64::Engine as _;
                    let buf = base64::engine::general_purpose::STANDARD.decode(b64)
                        .map_err(|e| McpError::internal(&format!("base64 解码失败: {}", e)))?;
                    let mut pba = godot::builtin::PackedByteArray::new();
                    for b in &buf { pba.push(*b); }
                    let mut img = Image::new_gd();
                    let err = img.load_png_from_buffer(&pba);
                    if err == Error::OK {
                        let abs_path = ProjectSettings::singleton().globalize_path(&save_path);
                        let abs_gstr: GString = abs_path.into();
                        let save_err = img.save_png(&abs_gstr);
                        if save_err == Error::OK {
                            data["saved_path"] = serde_json::json!(save_path);
                        }
                    }
                }
                data.as_object_mut().map(|m| m.remove("image_base64"));
            }
            Ok(data)
        }
        Err(_) => {
            // 备用方案: 直接通过 GDScript 返回 JSON
            Ok(serde_json::json!({
                "width": 0,
                "height": 0,
                "error": "截图获取失败, 请重试"
            }))
        }
    }
}

/// get_game_screenshot: 获取游戏运行截图
///
/// 检查游戏是否在运行, 尝试读取 user://mcp_screenshot.png
fn cmd_get_game_screenshot(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();

    if !editor.is_playing_scene() {
        return Err(McpError::internal("当前没有游戏在运行"));
    }

    // 尝试使用 GDScript 获取截图 (通过文件 IPC)
    // 首先检查是否有已有的截图文件
    let screenshot_path = "user://mcp_screenshot.png";
    let settings = ProjectSettings::singleton();
    let abs_screenshot = settings.globalize_path(screenshot_path);

    if !FileAccess::file_exists(screenshot_path) {
        return Err(McpError::internal(&format!(
            "截图文件不存在。请确保游戏正在运行且 MCPScreenshot autoload 已激活。路径: {}",
            abs_screenshot
        )));
    }

    // 加载图片
    let mut img = Image::new_gd();
    let err = img.load(screenshot_path);
    if err != Error::OK {
        return Err(McpError::internal("无法加载截图文件"));
    }

    let width = img.get_width();
    let height = img.get_height();

    let save_path = opt_string(args, "save_path", "");
    if !save_path.is_empty() {
        let abs_path = settings.globalize_path(&save_path);
        let abs_gstr: GString = abs_path.into();
        let save_err = img.save_png(&abs_gstr);
        if save_err != Error::OK {
            return Err(McpError::internal(&format!("保存截图失败: {:?}", save_err)));
        }
        return Ok(serde_json::json!({
            "saved_path": save_path,
            "width": width,
            "height": height,
            "format": "png",
        }));
    }

    let b64 = image_to_base64(&mut img)?;
    Ok(serde_json::json!({
        "image_base64": b64,
        "width": width,
        "height": height,
        "format": "png",
    }))
}

/// clear_output: 清除输出面板
fn cmd_clear_output(_args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    // 在 Rust 中通过打印空行来模拟清除
    godot_print!("\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n");
    Ok(serde_json::json!({"cleared": true}))
}

/// reload_plugin: 重新加载插件
///
/// 通过 set_plugin_enabled 禁用再启用插件
fn cmd_reload_plugin(_args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    // 使用 call_deferred 方式延迟执行
    let code = r#"EditorInterface.set_plugin_enabled("godot_mcp", false)
EditorInterface.set_plugin_enabled("godot_mcp", true)
return "ok""#;
    match execute_expression(code) {
        Ok(_) => Ok(serde_json::json!({
            "reloading": true,
            "message": "插件将重新加载, 连接会短暂断开并自动重连"
        })),
        Err(e) => Err(McpError::internal(&format!("插件重载失败: {:?}", e))),
    }
}

/// reload_project: 重新扫描文件系统
fn cmd_reload_project(_args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    if let Some(mut fs) = editor.get_resource_filesystem() {
        fs.scan();
    }
    Ok(serde_json::json!({
        "reloaded": true,
        "message": "文件系统已重新扫描"
    }))
}

/// get_signals: 获取节点信号列表
///
/// 使用 GDScript 获取完整信号信息 (因为 get_signal_list 返回 Array of Dictionary,
/// 在 Rust 绑定中处理 Dictionary 较为复杂)
fn cmd_get_signals(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = args.get("node_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: node_path"))?;

    let editor = EditorInterface::singleton();
    let _root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    // 使用 GDScript 获取信号数据, 返回 JSON 字符串
    let code = format!(
        r#"var root = EditorInterface.get_edited_scene_root()
var node = root.find_node("{}", true, false)
if node == null:
    return "{{\"error\": \"node_not_found\"}}"
var signals = []
for sig in node.get_signal_list():
    var sig_info = {{"name": sig["name"], "args": [], "connections": []}}
    for arg in sig["args"]:
        sig_info["args"].append({{"name": arg["name"], "type": str(arg["type"])}})
    for conn in node.get_signal_connection_list(sig["name"]):
        var conn_obj = conn["callable"].get_object()
        var target_path = root.get_path_to(conn_obj).to_string() if conn_obj else ""
        sig_info["connections"].append({{
            "target": target_path,
            "method": conn["callable"].get_method()
        }})
    signals.append(sig_info)
var result = {{
    "node_path": root.get_path_to(node).to_string(),
    "type": node.get_class(),
    "signals": signals,
    "count": signals.size()
}}
return JSON.stringify(result)"#,
        node_path.replace('"', "\\\"")
    );

    let script_result = execute_expression(&code)?;
    // 安全解析 JSON
    let parsed = parse_expression_json(&script_result)?;
    if let Some(error_msg) = parsed.get("error").and_then(|v| v.as_str()) {
        if error_msg.contains("node_not_found") {
            return Err(McpError::not_found(&format!("Node '{}'", node_path), ""));
        }
        return Err(McpError::internal(error_msg));
    }
    Ok(parsed)
}

/// compare_screenshots: 比较两张截图
///
/// 逐像素比较两张图片, 生成差异图
fn cmd_compare_screenshots(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let image_a = args.get("image_a")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: image_a"))?;
    let image_b = args.get("image_b")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: image_b"))?;
    let threshold = opt_int(args, "threshold", 10) as i32;

    // 使用 GDScript 执行图片比较 (因为 Image API 在 Rust 中操作像素较为复杂)
    // 对特殊字符进行转义处理
    let escaped_a = image_a.replace('\\', "\\\\").replace('"', "\\\"");
    let escaped_b = image_b.replace('\\', "\\\\").replace('"', "\\\"");

    let code = format!(
        r#"func load_img(val):
    var img = Image.new()
    if val.begins_with("res://") or val.begins_with("user://"):
        var err = img.load(val)
        if err != OK:
            return [null, "load_failed"]
    else:
        var buf = Marshalls.base64_to_raw(val)
        if buf.is_empty():
            # 尝试直接加载
            var err = img.load(val)
            if err != OK:
                return [null, "decode_failed"]
        else:
            var err = img.load_png_from_buffer(buf)
            if err != OK:
                return [null, "png_decode_failed"]
    return [img, null]

var load_a = load_img("{}")
if load_a[1] != null:
    return "{{\"error\": \"image_a_load_failed\"}}"
var img_a = load_a[0]

var load_b = load_img("{}")
if load_b[1] != null:
    return "{{\"error\": \"image_b_load_failed\"}}"
var img_b = load_b[0]

if img_a.get_size() != img_b.get_size():
    return "{{\"error\": \"size_mismatch\", \"size_a\": \"\" + str(img_a.get_size()) + \"\", \"size_b\": \"\" + str(img_b.get_size()) + \"\"}}"

var width = img_a.get_width()
var height = img_a.get_height()
var diff_img = Image.create(width, height, false, Image.FORMAT_RGBA8)
var changed = 0
var total = width * height
var thr = {}

for y in range(height):
    for x in range(width):
        var ca = img_a.get_pixel(x, y)
        var cb = img_b.get_pixel(x, y)
        var dr = absi(int(ca.r8) - int(cb.r8))
        var dg = absi(int(ca.g8) - int(cb.g8))
        var db = absi(int(ca.b8) - int(cb.b8))
        var max_d = maxi(dr, maxi(dg, db))
        if max_d > thr:
            changed += 1
            diff_img.set_pixel(x, y, Color(1, 0, 0, clampf(float(max_d) / 255.0, 0.3, 1.0)))
        else:
            diff_img.set_pixel(x, y, Color(ca.r * 0.3, ca.g * 0.3, ca.b * 0.3, 1.0))

var diff_pct = (float(changed) / float(total)) * 100.0
var diff_buf = diff_img.save_png_to_buffer()
var diff_b64 = Marshalls.raw_to_base64(diff_buf)

var result = {{
    "identical": changed == 0,
    "changed_pixels": changed,
    "total_pixels": total,
    "diff_percentage": snappedf(diff_pct, 0.01),
    "threshold": thr,
    "width": width,
    "height": height,
    "diff_image_base64": diff_b64
}}
return JSON.stringify(result)"#,
        escaped_a, escaped_b, threshold
    );

    let script_result = execute_expression(&code)?;
    // 安全解析 JSON
    let parsed = parse_expression_json(&script_result)?;
    if let Some(error_msg) = parsed.get("error").and_then(|v| v.as_str()) {
        if error_msg.contains("size_mismatch") {
            return Err(McpError::invalid_params("图片尺寸不一致"));
        }
        return Err(McpError::invalid_params("图片加载失败"));
    }
    Ok(parsed)
}

/// set_auto_dismiss: 设置自动关闭对话框
///
/// 存储在全局静态变量中
fn cmd_set_auto_dismiss(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let enabled = args.get("enabled")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: enabled"))?;

    AUTO_DISMISS.store(enabled, Ordering::Relaxed);

    let msg = if enabled { "已启用" } else { "已禁用" };
    Ok(serde_json::json!({
        "auto_dismiss": enabled,
        "message": format!("自动关闭对话框{}", msg),
    }))
}

/// get_editor_camera: 获取编辑器 3D 相机信息
///
/// 使用 GDScript 获取, 因为 get_editor_viewport_3d 在 Rust 绑定中可能不直接可用
fn cmd_get_editor_camera(_args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    get_editor_camera_via_script()
}

/// set_editor_camera: 设置编辑器 3D 相机
fn cmd_set_editor_camera(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    // 构建 GDScript 代码来设置相机
    let mut script_parts = Vec::new();

    script_parts.push(
        r#"var vp3d = EditorInterface.get_editor_viewport_3d()
if vp3d == null:
    return "{\"error\":\"no_3d_viewport\"}"
var cam = vp3d.get_camera_3d()
if cam == null:
    return "{\"error\":\"no_camera\"}"
"#.to_string()
    );

    // 设置位置
    if let Some(pos) = args.get("position").and_then(|v| v.as_object()) {
        let x = pos.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = pos.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let z = pos.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0);
        script_parts.push(format!("cam.global_position = Vector3({}, {}, {})\n", x, y, z));
    }

    // 设置旋转
    if let Some(rot) = args.get("rotation_degrees").and_then(|v| v.as_object()) {
        let x = rot.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = rot.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let z = rot.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0);
        script_parts.push(format!("cam.rotation_degrees = Vector3({}, {}, {})\n", x, y, z));
    }

    // 设置看向目标
    if let Some(look) = args.get("look_at").and_then(|v| v.as_object()) {
        let x = look.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = look.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let z = look.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0);
        script_parts.push(format!("cam.look_at(Vector3({}, {}, {}))\n", x, y, z));
    }

    // 设置 FOV
    if let Some(fov) = args.get("fov").and_then(|v| v.as_f64()) {
        script_parts.push(format!("cam.fov = {}\n", fov));
    }

    // 返回结果 - 使用 JSON.stringify 避免手工拼接
    script_parts.push(
        r#"var pos = cam.global_position
var rot = cam.rotation_degrees
return JSON.stringify({"position":{"x":pos.x,"y":pos.y,"z":pos.z},"rotation_degrees":{"x":rot.x,"y":rot.y,"z":rot.z},"fov":cam.fov})"#.to_string()
    );

    let code = script_parts.concat();
    let script_result = execute_expression(&code)?;
    // 安全解析 JSON
    let parsed = parse_expression_json(&script_result)?;
    if parsed.get("error").is_some() {
        return Err(McpError::internal("无法获取3D视口, 请确保已打开3D场景"));
    }
    Ok(parsed)
}
