//! 场景操作命令模块
//! 对应原 GDScript 插件 scene_commands.gd

use std::collections::HashMap;

use godot::classes::file_access::ModeFlags;
use godot::classes::{
    ClassDb, DirAccess, EditorInterface, FileAccess, PackedScene, ResourceLoader, ResourceSaver,
    Script,
};
use godot::global::Error;
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;
use crate::utils::serialize;

/// 收集本模块的工具定义
pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("get_scene_tree", "获取当前编辑场景的完整场景树", serde_json::json!({
            "type": "object", "properties": {
                "max_depth": { "type": "integer", "description": "最大深度 (-1 无限)", "default": -1 }
            }, "required": []
        })),
        ToolDefinition::new("get_scene_file_content", "读取 .tscn 场景文件内容", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "场景文件路径 (res://)" }
            }, "required": ["path"]
        })),
        ToolDefinition::new("open_scene", "在编辑器中打开场景", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "res:// 路径" }
            }, "required": ["path"]
        })),
        ToolDefinition::new("delete_scene", "删除场景文件", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "要删除的场景文件路径 (res://)" }
            }, "required": ["path"]
        })),
        ToolDefinition::new("add_scene_instance", "将外部场景作为实例添加到当前编辑场景", serde_json::json!({
            "type": "object", "properties": {
                "scene_path": { "type": "string", "description": "要实例化的场景路径 (res://)" },
                "parent_path": { "type": "string", "description": "父节点路径, 默认 '.'", "default": "." },
                "name": { "type": "string", "description": "实例节点名称 (可选)" }
            }, "required": ["scene_path"]
        })),
        ToolDefinition::new("get_scene_exports", "获取场景根节点的导出变量列表", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "场景文件路径 (res://)" }
            }, "required": ["path"]
        })),
        ToolDefinition::new("play_scene", "运行场景", serde_json::json!({
            "type": "object", "properties": {
                "mode": { "type": "string", "description": "main/current/路径", "default": "main" }
            }, "required": []
        })),
        ToolDefinition::new("stop_scene", "停止运行", serde_json::json!({
            "type": "object", "properties": {}, "required": []
        })),
        ToolDefinition::new("save_scene", "保存当前场景", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "保存路径 (可选)" }
            }, "required": []
        })),
        ToolDefinition::new("create_scene", "创建新场景文件", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "保存路径 (res://)" },
                "root_type": { "type": "string", "description": "根节点类型", "default": "Node2D" },
                "root_name": { "type": "string", "description": "根节点名称 (可选)" }
            }, "required": ["path"]
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("get_scene_tree".into(), cmd_get_scene_tree);
    registry.insert("get_scene_file_content".into(), cmd_get_scene_file_content);
    registry.insert("open_scene".into(), cmd_open_scene);
    registry.insert("delete_scene".into(), cmd_delete_scene);
    registry.insert("add_scene_instance".into(), cmd_add_scene_instance);
    registry.insert("get_scene_exports".into(), cmd_get_scene_exports);
    registry.insert("play_scene".into(), cmd_play_scene);
    registry.insert("stop_scene".into(), cmd_stop_scene);
    registry.insert("save_scene".into(), cmd_save_scene);
    registry.insert("create_scene".into(), cmd_create_scene);
}

/// 递归构建场景树
fn build_tree(node: &Gd<godot::classes::Node>, max_depth: i32, depth: i32) -> serde_json::Value {
    let mut tree = serde_json::Map::new();
    tree.insert("name".into(), serde_json::json!(node.get_name().to_string()));
    tree.insert("type".into(), serde_json::json!(node.get_class().to_string()));
    tree.insert("path".into(), serde_json::json!(node.get_path().to_string()));

    if max_depth == -1 || depth < max_depth {
        let count = node.get_child_count();
        if count > 0 {
            let children: Vec<serde_json::Value> = (0..count)
                .filter_map(|i| node.get_child(i))
                .map(|c| build_tree(&c, max_depth, depth + 1))
                .collect();
            tree.insert("children".into(), serde_json::Value::Array(children));
        }
    }
    serde_json::Value::Object(tree)
}

fn cmd_get_scene_tree(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let max_depth = args.get("max_depth").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
    let tree = build_tree(&root, max_depth, 0);
    Ok(serde_json::json!({"scene_path": root.get_scene_file_path().to_string(), "tree": tree}))
}

fn cmd_open_scene(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    if !FileAccess::file_exists(path) {
        return Err(McpError::not_found(&format!("Scene '{}'", path), ""));
    }
    EditorInterface::singleton().open_scene_from_path(path);
    Ok(serde_json::json!({"path": path, "opened": true}))
}

fn cmd_play_scene(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("main");
    let mut editor = EditorInterface::singleton();
    match mode {
        "main" => editor.play_main_scene(),
        "current" => editor.play_current_scene(),
        custom => {
            if !FileAccess::file_exists(custom) {
                return Err(McpError::not_found(&format!("Scene '{}'", custom), ""));
            }
            editor.play_custom_scene(custom);
        }
    }
    Ok(serde_json::json!({"playing": true, "mode": mode}))
}

fn cmd_stop_scene(_: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let mut editor = EditorInterface::singleton();
    if !editor.is_playing_scene() {
        return Ok(serde_json::json!({"stopped": false, "message": "No scene playing"}));
    }
    editor.stop_playing_scene();
    Ok(serde_json::json!({"stopped": true}))
}

fn cmd_save_scene(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let mut editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).map(String::from)
        .unwrap_or_else(|| root.get_scene_file_path().to_string());

    let err = if path.is_empty() {
        editor.save_scene()
    } else {
        editor.save_scene_as(&path);
        godot::global::Error::OK
    };

    if err != godot::global::Error::OK {
        return Err(McpError::internal(&format!("场景保存失败: {:?}", err)));
    }
    Ok(serde_json::json!({"path": path, "saved": true}))
}

/// create_scene: 创建新场景文件
fn cmd_create_scene(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter: path"))?;
    let root_type = args.get("root_type").and_then(|v| v.as_str()).unwrap_or("Node2D");
    let root_name = args.get("root_name").and_then(|v| v.as_str()).unwrap_or("");

    // 实例化根节点
    let node_var = ClassDb::singleton().instantiate(root_type);
    let mut root_node: Gd<godot::classes::Node> = node_var.to();

    let name = if root_name.is_empty() {
        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Node")
    } else {
        root_name
    };
    root_node.set_name(name);

    // Pack 场景
    let mut scene = PackedScene::new_gd();
    let pack_err = scene.pack(&root_node);
    if pack_err != Error::OK {
        root_node.queue_free();
        return Err(McpError::internal(&format!("场景打包失败: {:?}", pack_err)));
    }

    // 确保目录存在
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = DirAccess::make_dir_recursive_absolute(&parent.to_string_lossy().to_string());
    }

    // 使用 ResourceSaver::save_ex().path().done() 持久化场景
    let save_err = ResourceSaver::singleton()
        .save_ex(&scene)
        .path(path)
        .done();
    root_node.queue_free();

    if save_err != Error::OK {
        return Err(McpError::internal(&format!("场景保存失败: {:?}", save_err)));
    }

    // 刷新文件系统
    if let Some(mut fs) = EditorInterface::singleton().get_resource_filesystem() {
        fs.scan();
    }

    Ok(serde_json::json!({"path": path, "root_type": root_type, "root_name": name, "created": true}))
}

// ===============================
// 新工具: get_scene_file_content
// ===============================

/// get_scene_file_content: 读取 .tscn 场景文件内容
fn cmd_get_scene_file_content(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: path"))?;

    if !FileAccess::file_exists(path) {
        return Err(McpError::not_found(&format!("场景文件 '{}'", path), ""));
    }

    // 使用 FileAccess 以只读方式打开文件
    let mut file = FileAccess::open(path, ModeFlags::READ)
        .ok_or_else(|| McpError::internal(&format!("无法读取文件: {}", path)))?;
    let content = file.get_as_text().to_string();
    file.close();
    let size = content.len();

    Ok(serde_json::json!({"path": path, "content": content, "size": size}))
}

// ===============================
// 新工具: delete_scene
// ===============================

/// delete_scene: 删除指定场景文件及其 .import 文件
fn cmd_delete_scene(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: path"))?;

    if !FileAccess::file_exists(path) {
        return Err(McpError::not_found(&format!("场景文件 '{}'", path), ""));
    }

    // 使用 DirAccess::remove_absolute 删除文件
    let err = DirAccess::remove_absolute(path);
    if err != Error::OK {
        return Err(McpError::internal(&format!("删除场景失败: {:?}", err)));
    }

    // 如果存在 .import 文件也一并删除
    let import_path = format!("{}.import", path);
    if FileAccess::file_exists(&import_path) {
        let _ = DirAccess::remove_absolute(&import_path);
    }

    // 刷新文件系统
    if let Some(mut fs) = EditorInterface::singleton().get_resource_filesystem() {
        fs.scan();
    }

    Ok(serde_json::json!({"path": path, "deleted": true}))
}

// ===============================
// 新工具: add_scene_instance
// ===============================

/// add_scene_instance: 将外部场景作为实例添加到当前编辑场景
fn cmd_add_scene_instance(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let scene_path = args.get("scene_path").and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: scene_path"))?;
    let parent_path = args.get("parent_path").and_then(|v| v.as_str()).unwrap_or(".");
    let instance_name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");

    // 获取编辑器场景根节点
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    if !FileAccess::file_exists(scene_path) {
        return Err(McpError::not_found(&format!("场景文件 '{}'", scene_path), ""));
    }

    // 查找父节点, 默认 "." 表示场景根节点
    let mut parent = if parent_path == "." {
        root.clone()
    } else {
        // 先检查节点是否存在, get_node_as 返回 Gd 而非 Option
        if !root.has_node(parent_path) {
            return Err(McpError::not_found(&format!("父节点 '{}'", parent_path), "使用 get_scene_tree 查看可用节点"));
        }
        root.get_node_as::<godot::classes::Node>(parent_path)
    };

    // 使用 ResourceLoader 加载 PackedScene
    let mut rl = ResourceLoader::singleton();
    let packed = rl.load(scene_path)
        .and_then(|res| res.try_cast::<PackedScene>().ok())
        .ok_or_else(|| McpError::internal(&format!("无法加载场景: {}", scene_path)))?;

    // 实例化场景
    let mut instance = packed.instantiate()
        .ok_or_else(|| McpError::internal("无法实例化场景"))?;

    // 设置自定义名称 (如果提供)
    if !instance_name.is_empty() {
        instance.set_name(instance_name);
    }

    // 添加到父节点 (不使用 UndoRedo)
    parent.add_child(&instance);
    // 设置 owner 为场景根节点
    instance.set("owner", &Variant::from(root.clone()));

    let node_path = root.get_path_to(&instance).to_string();
    let name = instance.get_name().to_string();

    Ok(serde_json::json!({"node_path": node_path, "scene_path": scene_path, "name": name}))
}

// ===============================
// 新工具: get_scene_exports
// ===============================

/// 属性使用标志常量
const PROPERTY_USAGE_EDITOR: i64 = 2;
const PROPERTY_USAGE_SCRIPT_VARIABLE: i64 = 1024;

/// get_scene_exports: 获取场景中所有带脚本节点的导出变量列表
fn cmd_get_scene_exports(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = args.get("path").and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("缺少必填参数: path"))?;

    if !FileAccess::file_exists(path) {
        return Err(McpError::not_found(&format!("场景文件 '{}'", path), ""));
    }

    // 加载 PackedScene
    let mut rl = ResourceLoader::singleton();
    let packed = rl.load(path)
        .and_then(|res| res.try_cast::<PackedScene>().ok())
        .ok_or_else(|| McpError::internal(&format!("无法加载场景: {}", path)))?;

    // 实例化场景
    let mut instance = packed.instantiate()
        .ok_or_else(|| McpError::internal("无法实例化场景"))?;

    // 递归收集导出变量
    let mut nodes_data: Vec<serde_json::Value> = Vec::new();
    collect_exports_recursive(&instance, &instance, &mut nodes_data);

    // 清理实例
    instance.queue_free();

    let count = nodes_data.len();
    Ok(serde_json::json!({"path": path, "nodes": nodes_data, "count": count}))
}

/// 递归遍历节点树, 收集带脚本节点的导出变量
fn collect_exports_recursive(node: &Gd<godot::classes::Node>, root: &Gd<godot::classes::Node>, nodes_data: &mut Vec<serde_json::Value>) {
    // 获取节点脚本
    let script_var = node.get("script");
    if !script_var.is_nil() {
        // 获取属性列表并过滤导出变量
        let prop_list = node.get_property_list();
        let mut exports = serde_json::Map::new();
        let nil_var = Variant::nil();

        for i in 0..prop_list.len() {
            let entry = match prop_list.get(i) {
                Some(e) => e,
                None => continue,
            };

            // 检查 usage 标志: PROPERTY_USAGE_EDITOR (2) + PROPERTY_USAGE_SCRIPT_VARIABLE (1024)
            let usage = entry.get(&Variant::from("usage")).unwrap_or(nil_var.clone()).to::<i64>();
            if (usage & PROPERTY_USAGE_EDITOR) == 0 || (usage & PROPERTY_USAGE_SCRIPT_VARIABLE) == 0 {
                continue;
            }

            // 安全读取属性名：兼容 StringName / String（Godot 4.7 为 StringName）
            let name_var = entry.get(&Variant::from("name")).unwrap_or(nil_var.clone());
            let prop_name: String = match name_var.get_type() {
                godot::builtin::VariantType::STRING_NAME => name_var.to::<godot::builtin::StringName>().to_string(),
                godot::builtin::VariantType::STRING => name_var.to::<godot::prelude::GString>().to_string(),
                _ => continue, // NIL 或非文本类型，跳过
            };
            let prop_type = entry.get(&Variant::from("type")).unwrap_or(nil_var.clone()).to::<i64>();
            let hint = entry.get(&Variant::from("hint")).unwrap_or(nil_var.clone()).to::<i64>();
            // hint_string 可能为 NIL 或 StringName，安全读取
            let hint_string_var = entry.get(&Variant::from("hint_string")).unwrap_or(nil_var.clone());
            let hint_string: String = match hint_string_var.get_type() {
                godot::builtin::VariantType::STRING_NAME => hint_string_var.to::<godot::builtin::StringName>().to_string(),
                godot::builtin::VariantType::STRING => hint_string_var.to::<godot::prelude::GString>().to_string(),
                _ => String::new(), // NIL 或其他类型，使用空字符串
            };

            // 序列化属性值
            let val = node.get(&prop_name);
            exports.insert(prop_name, serde_json::json!({
                "value": serialize::serialize_variant(&val),
                "type": prop_type,
                "hint": hint,
                "hint_string": hint_string,
            }));
        }

        if !exports.is_empty() {
            // 将 Variant 转为 Gd<Script> 以访问脚本属性
            let script_gd: Gd<Script> = script_var.to();
            // 获取脚本资源路径 (resource_path 继承自 Resource，Godot 4.7 为 StringName)
            let script_path = {
                let rp = script_gd.get("resource_path");
                match rp.get_type() {
                    godot::builtin::VariantType::STRING_NAME => rp.to::<godot::builtin::StringName>().to_string(),
                    godot::builtin::VariantType::STRING => rp.to::<godot::prelude::GString>().to_string(),
                    _ => String::new(),
                }
            };

            // 计算节点路径 (通过 instance_id 比较判断是否为根节点)
            let is_root = node.instance_id() == root.instance_id();
            let node_path = if is_root {
                ".".to_string()
            } else {
                root.get_path_to(node).to_string()
            };

            nodes_data.push(serde_json::json!({
                "node_path": node_path,
                "node_name": node.get_name().to_string(),
                "node_type": node.get_class().to_string(),
                "script_path": script_path,
                "exports": exports,
            }));
        }
    }

    // 递归遍历子节点
    let child_count = node.get_child_count();
    for i in 0..child_count {
        if let Some(child) = node.get_child(i) {
            collect_exports_recursive(&child, root, nodes_data);
        }
    }
}
