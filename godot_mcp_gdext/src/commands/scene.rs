//! 场景操作命令模块
//! 对应原 GDScript 插件 scene_commands.gd

use std::collections::HashMap;

use godot::classes::{ClassDb, DirAccess, EditorInterface, FileAccess, PackedScene, ResourceSaver};
use godot::global::Error;
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

/// 收集本模块的工具定义
pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("get_scene_tree", "获取当前编辑场景的完整场景树", serde_json::json!({
            "type": "object", "properties": {
                "max_depth": { "type": "integer", "description": "最大深度 (-1 无限)", "default": -1 }
            }, "required": []
        })),
        ToolDefinition::new("open_scene", "在编辑器中打开场景", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "res:// 路径" }
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
    registry.insert("open_scene".into(), cmd_open_scene);
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
