//! Android 设备管理命令模块
//!
//! 提供 Android 设备列表、导出预设信息、部署到设备等功能。
//! 对应原 GDScript 插件的 android_commands.gd

use std::collections::HashMap;

use godot::classes::{ConfigFile, Expression, FileAccess};
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

/// 通过 GDScript Expression 执行 OS 命令并获取输出
///
/// 使用 GDScript 的 OS.execute() 来执行外部命令（如 adb），
/// 因为 godot-rust 的绑定可能不直接暴露 OS.execute()。
fn execute_os_command(cmd: &str, args_list: &[&str]) -> Result<serde_json::Value, McpError> {
    let mut expr = Expression::new_gd();
    let args_json = serde_json::to_string(args_list)
        .map_err(|e| McpError::internal(&format!("序列化参数失败: {}", e)))?;

    let code = format!(
        "\
var cmd = \"{}\" \
var args = {} \
var output = [] \
var exit_code = OS.execute(cmd, args, output, true) \
return JSON.stringify({{\"exit_code\": exit_code, \"stdout\": str(output[0]) if output.size() > 0 else \"\"}})",
        cmd.replace('\\', "\\\\").replace('"', "\\\""),
        args_json
    );

    if expr.parse(&code) != godot::global::Error::OK {
        return Err(McpError::internal(&format!("解析 OS 执行表达式失败: {}", cmd)));
    }

    let result = expr.execute();
    if result.is_nil() {
        return Err(McpError::internal("OS.execute 返回空"));
    }

    let result_str = match result.try_to::<String>() {
        Ok(s) => s,
        Err(_) => result.stringify().to_string(),
    };
    serde_json::from_str(&result_str)
        .map_err(|e| McpError::internal(&format!("解析命令执行结果失败: {}", e)))
}

/// 解析 adb devices -l 输出
fn parse_adb_devices(output: &str) -> Vec<serde_json::Value> {
    let mut devices = Vec::new();
    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with("List of devices")
            || line.starts_with("* daemon")
        {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let mut dev = serde_json::json!({
            "serial": parts[0],
            "state": parts[1],
        });
        // 解析扩展属性: model:Pixel_5, product:foo, usb:3-1
        if let Some(obj) = dev.as_object_mut() {
            for i in 2..parts.len() {
                let kv = parts[i];
                if let Some(eq_pos) = kv.find(':') {
                    let key = &kv[..eq_pos];
                    let value = &kv[eq_pos + 1..];
                    obj.insert(key.to_string(), serde_json::json!(value));
                }
            }
        }
        devices.push(dev);
    }
    devices
}

/// 在 export_presets.cfg 中查找 Android 预设
fn find_android_preset(preset_name: &str, preset_index: i64) -> Result<serde_json::Value, McpError> {
    let presets_path = "res://export_presets.cfg";
    if !FileAccess::file_exists(presets_path) {
        return Ok(serde_json::json!({}));
    }

    let mut cfg = ConfigFile::new_gd();
    if cfg.load(presets_path) != godot::global::Error::OK {
        return Ok(serde_json::json!({}));
    }

    let mut idx = 0i64;
    loop {
        let section = format!("preset.{}", idx);
        if !cfg.has_section(&section) {
            break;
        }

        // 安全读取配置值：缺失键可能返回 NIL
        let platform: String = cfg.get_value(&section, "platform").try_to().unwrap_or_default();
        let name: String = cfg.get_value(&section, "name").try_to().unwrap_or_default();

        let matches = if !preset_name.is_empty() {
            name == preset_name
        } else if preset_index >= 0 {
            idx == preset_index
        } else {
            // 没有过滤条件：选择第一个 Android 预设
            platform == "Android"
        };

        if matches {
            let options_section = format!("preset.{}.options", idx);
            let package_name: String = if cfg.has_section(&options_section) {
                cfg.get_value(&options_section, "package/unique_name").try_to().unwrap_or_default()
            } else {
                String::new()
            };

            return Ok(serde_json::json!({
                "index": idx,
                "name": name,
                "platform": platform,
                "runnable": cfg.get_value(&section, "runnable").try_to::<bool>().unwrap_or(false),
                "export_path": cfg.get_value(&section, "export_path").try_to::<String>().unwrap_or_default(),
                "package_name": package_name,
            }));
        }

        idx += 1;
    }

    Ok(serde_json::json!({}))
}

// ============================================================================
// 工具命令实现
// ============================================================================

/// 1. list_android_devices: 列出已连接的 Android 设备
fn cmd_list_android_devices(_args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    // 通过 GDScript Expression 获取 adb 路径并执行
    let mut expr = Expression::new_gd();
    let adb_code = "\
var editor_settings = EditorInterface.get_editor_settings() \
var configured = \"\" \
if editor_settings.has_setting(\"export/android/adb\"): \
    configured = str(editor_settings.get_setting(\"export/android/adb\")) \
if not configured.is_empty() and FileAccess.file_exists(configured): \
    return configured \
return \"adb\"";

    if expr.parse(adb_code) != godot::global::Error::OK {
        return Err(McpError::internal("解析 adb 路径表达式失败"));
    }

    let adb_result = expr.execute();
    let adb_path: String = adb_result.try_to().unwrap_or_else(|_| "adb".into());

    // 执行 adb devices -l
    let result = execute_os_command(&adb_path, &["devices", "-l"])?;

    let exit_code = result.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);
    let stdout = result.get("stdout").and_then(|v| v.as_str()).unwrap_or("");

    if exit_code != 0 {
        return Err(McpError::internal(&format!(
            "adb 执行失败 (退出码 {})。请安装 Android platform-tools 或在编辑器设置 > 导出 > Android > Adb 中配置。",
            exit_code
        )));
    }

    let devices = parse_adb_devices(stdout);

    Ok(serde_json::json!({
        "devices": devices,
        "count": devices.len(),
        "adb_path": adb_path,
    }))
}

/// 2. get_android_preset_info: 获取 Android 导出预设信息
fn cmd_get_android_preset_info(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let preset_name = opt_string(args, "preset_name", "");
    let preset_index = opt_int(args, "preset_index", -1);

    let preset = find_android_preset(&preset_name, preset_index)?;

    if preset.is_null() || preset.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        return Err(McpError::not_found("Android 导出预设", "请先在项目 > 导出中配置 Android 预设"));
    }

    let platform = preset.get("platform").and_then(|v| v.as_str()).unwrap_or("");
    if platform != "Android" {
        return Err(McpError::internal(&format!(
            "预设 '{}' 不是 Android 预设 (platform={})",
            preset.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            platform
        )));
    }

    Ok(preset)
}

/// 3. deploy_to_android: 部署到 Android 设备
fn cmd_deploy_to_android(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let preset_name = opt_string(args, "preset_name", "");
    let _preset_index = opt_int(args, "preset_index", -1);
    let device_id = opt_string(args, "device_id", "");
    let debug = opt_bool(args, "debug", true);
    let launch = opt_bool(args, "launch", true);
    let skip_export = opt_bool(args, "skip_export", false);

    let preset = find_android_preset(&preset_name, -1)?;

    if preset.is_null() || preset.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        return Err(McpError::not_found("Android 导出预设", "请先在项目 > 导出中配置 Android 预设"));
    }

    let platform = preset.get("platform").and_then(|v| v.as_str()).unwrap_or("");
    if platform != "Android" {
        return Err(McpError::internal(&format!(
            "预设 '{}' 不是 Android 预设",
            preset.get("name").and_then(|v| v.as_str()).unwrap_or("")
        )));
    }

    let export_path_res: String = preset.get("export_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if export_path_res.is_empty() {
        return Err(McpError::internal(&format!(
            "预设 '{}' 未配置导出路径",
            preset.get("name").and_then(|v| v.as_str()).unwrap_or("")
        )));
    }

    // 通过 GDScript Expression 执行部署
    let mut expr = Expression::new_gd();
    let deploy_code = format!(
        "\
var preset_name = \"{}\" \
var device_serial = \"{}\" \
var debug = {} \
var launch = {} \
var skip_export = {} \
\
var export_path_res = \"{}\" \
var export_path_abs = export_path_res \
if export_path_res.begins_with(\"res://\"): \
    export_path_abs = ProjectSettings.globalize_path(export_path_res) \
\
var steps = [] \
var adb = \"adb\" \
\
var editor_settings = EditorInterface.get_editor_settings() \
var configured = \"\" \
if editor_settings.has_setting(\"export/android/adb\"): \
    configured = str(editor_settings.get_setting(\"export/android/adb\")) \
if not configured.is_empty() and FileAccess.file_exists(configured): \
    adb = configured \
\
if not skip_export: \
    var godot_bin = OS.get_executable_path() \
    var project_dir = ProjectSettings.globalize_path(\"res://\") \
    var export_flag = \"--export-debug\" if debug else \"--export-release\" \
    var export_args = [\"--headless\", \"--path\", project_dir, export_flag, preset_name, export_path_abs] \
    var export_output = [] \
    var export_exit = OS.execute(godot_bin, export_args, export_output, true) \
    steps.append({{\"step\": \"export\", \"exit_code\": export_exit}}) \
    if export_exit != 0: \
        return JSON.stringify({{\"error\": \"Godot 导出失败 (退出码 \" + str(export_exit) + \")\", \"steps\": steps}}) \
\
var install_args = [] \
if not device_serial.is_empty(): \
    install_args.append(\"-s\") \
    install_args.append(device_serial) \
install_args.append(\"install\") \
install_args.append(\"-r\") \
install_args.append(export_path_abs) \
var install_output = [] \
var install_exit = OS.execute(adb, install_args, install_output, true) \
steps.append({{\"step\": \"install\", \"exit_code\": install_exit}}) \
if install_exit != 0: \
    return JSON.stringify({{\"error\": \"adb install 失败 (退出码 \" + str(install_exit) + \")\", \"steps\": steps}}) \
\
if launch: \
    var package_name = \"\" \
    var cfg = ConfigFile.new() \
    if cfg.load(\"res://export_presets.cfg\") == OK: \
        var idx = 0 \
        while cfg.has_section(\"preset.\" + str(idx)): \
            var name = str(cfg.get_value(\"preset.\" + str(idx), \"name\", \"\")) \
            if name == preset_name: \
                var opt_section = \"preset.\" + str(idx) + \".options\" \
                if cfg.has_section(opt_section): \
                    package_name = str(cfg.get_value(opt_section, \"package/unique_name\", \"\")) \
                break \
            idx += 1 \
    if not package_name.is_empty(): \
        var launch_args = [] \
        if not device_serial.is_empty(): \
            launch_args.append(\"-s\") \
            launch_args.append(device_serial) \
        launch_args.append(\"shell\") \
        launch_args.append(\"monkey\") \
        launch_args.append(\"-p\") \
        launch_args.append(package_name) \
        launch_args.append(\"-c\") \
        launch_args.append(\"android.intent.category.LAUNCHER\") \
        launch_args.append(\"1\") \
        var launch_output = [] \
        var launch_exit = OS.execute(adb, launch_args, launch_output, true) \
        steps.append({{\"step\": \"launch\", \"exit_code\": launch_exit}}) \
    else: \
        steps.append({{\"step\": \"launch\", \"skipped\": true, \"reason\": \"未找到 package_name\"}}) \
\
return JSON.stringify({{\"result\": {{\"preset\": preset_name, \"apk_path\": export_path_abs, \"device\": device_serial if not device_serial.is_empty() else \"(default)\", \"steps\": steps}}}})",
        preset_name.replace('"', "\\\""),
        device_id.replace('"', "\\\""),
        if debug { "true" } else { "false" },
        if launch { "true" } else { "false" },
        if skip_export { "true" } else { "false" },
        export_path_res.replace('"', "\\\""),
    );

    if expr.parse(&deploy_code) != godot::global::Error::OK {
        return Err(McpError::internal("解析部署表达式失败"));
    }

    let result = expr.execute();
    let result_str = match result.try_to::<String>() {
        Ok(s) => s,
        Err(_) => result.stringify().to_string(),
    };

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result_str) {
        // 检查是否有错误
        if let Some(error_msg) = parsed.get("error").and_then(|v| v.as_str()) {
            return Err(McpError::internal(error_msg));
        }
        // 解包 result 字段
        if let Some(inner) = parsed.get("result") {
            return Ok(inner.clone());
        }
        return Ok(parsed);
    }

    Ok(serde_json::json!({
        "preset": preset_name,
        "export_path": export_path_res,
        "raw_output": result_str,
    }))
}

// ============================================================================
// 工具注册
// ============================================================================

/// 收集本模块的所有工具定义
pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        // 1. list_android_devices: 列出 Android 设备
        ToolDefinition::new("list_android_devices", "列出已连接的 Android 设备", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),

        // 2. get_android_preset_info: 获取 Android 预设信息
        ToolDefinition::new("get_android_preset_info", "获取 Android 导出预设信息", serde_json::json!({
            "type": "object", "properties": {
                "preset_name": { "type": "string", "description": "预设名称（可选）" },
                "preset_index": { "type": "integer", "description": "预设索引（可选）" }
            }, "required": []
        })),

        // 3. deploy_to_android: 部署到 Android 设备
        ToolDefinition::new("deploy_to_android", "将项目导出并部署到 Android 设备", serde_json::json!({
            "type": "object", "properties": {
                "preset_name": { "type": "string", "description": "Android 导出预设名称" },
                "device_id": { "type": "string", "description": "目标设备 serial（可选，部署到第一个可用设备）" },
                "debug": { "type": "boolean", "description": "是否以 debug 模式导出", "default": true },
                "launch": { "type": "boolean", "description": "安装后是否启动应用", "default": true },
                "skip_export": { "type": "boolean", "description": "是否跳过导出步骤（直接安装已有 APK）", "default": false }
            }, "required": ["preset_name"]
        })),
    ]
}

/// 注册本模块的所有命令到全局注册表
pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("list_android_devices".into(), cmd_list_android_devices);
    registry.insert("get_android_preset_info".into(), cmd_get_android_preset_info);
    registry.insert("deploy_to_android".into(), cmd_deploy_to_android);
}
