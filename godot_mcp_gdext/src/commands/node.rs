//! 节点 CRUD 命令模块

use std::collections::HashMap;

use godot::builtin::{Callable, NodePath, StringName, Variant};
use godot::classes::{ClassDb, Control, EditorInterface, Node};
use godot::classes::control::{LayoutPreset, LayoutPresetMode};
use godot::obj::Gd;
use godot::prelude::*;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;
use crate::utils::serialize;

pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("add_node", "向场景添加新节点", serde_json::json!({
            "type": "object", "properties": {
                "type": { "type": "string" },
                "name": { "type": "string" },
                "parent_path": { "type": "string", "default": "." },
                "properties": { "type": "object", "default": {} }
            }, "required": ["type"]
        })),
        ToolDefinition::new("delete_node", "删除节点", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" }
            }, "required": ["path"]
        })),
        ToolDefinition::new("rename_node", "重命名节点", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" }, "name": { "type": "string" }
            }, "required": ["path", "name"]
        })),
        ToolDefinition::new("update_property", "修改属性", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" }, "property": { "type": "string" }, "value": {}
            }, "required": ["path", "property", "value"]
        })),
        ToolDefinition::new("get_node_properties", "获取节点属性", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string" },
                "properties": { "type": "array", "items": { "type": "string" } }
            }, "required": ["path"]
        })),
        ToolDefinition::new("duplicate_node", "复制指定节点及其子节点", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "源节点路径" },
                "new_name": { "type": "string", "description": "新节点名称 (可选)" }
            }, "required": ["path"]
        })),
        ToolDefinition::new("connect_signal", "连接节点的信号到目标方法", serde_json::json!({
            "type": "object", "properties": {
                "source_path": { "type": "string", "description": "源节点路径" },
                "signal": { "type": "string", "description": "信号名称" },
                "target_path": { "type": "string", "description": "目标节点路径" },
                "method": { "type": "string", "description": "目标方法名" }
            }, "required": ["source_path", "signal", "method"]
        })),
        ToolDefinition::new("disconnect_signal", "断开节点的信号连接", serde_json::json!({
            "type": "object", "properties": {
                "source_path": { "type": "string", "description": "源节点路径" },
                "signal": { "type": "string", "description": "信号名称" },
                "target_path": { "type": "string", "description": "目标节点路径" },
                "method": { "type": "string", "description": "目标方法名" }
            }, "required": ["source_path", "signal", "method"]
        })),
        ToolDefinition::new("move_node", "将节点移动到新的父节点下", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "要移动的节点路径" },
                "new_parent": { "type": "string", "description": "目标父节点路径" },
                "new_name": { "type": "string", "description": "移动后的新名称 (可选)" }
            }, "required": ["path", "new_parent"]
        })),
        // === 新增的 8 个工具 ===
        ToolDefinition::new("add_resource", "创建资源并附加到节点属性", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "节点路径" },
                "property": { "type": "string", "description": "属性名称" },
                "resource_type": { "type": "string", "description": "资源类型 (如 GradientTexture1D, StyleBoxFlat)" },
                "resource_properties": { "type": "object", "description": "要设置的资源属性 (可选)" }
            }, "required": ["node_path", "property", "resource_type"]
        })),
        ToolDefinition::new("set_anchor_preset", "设置 Control 节点的锚点预设", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "节点路径" },
                "preset": { "type": "string", "description": "预设名称 (top_left, top_right, bottom_left, bottom_right, center_left, center_top, center_right, center_bottom, center, left_wide, top_wide, right_wide, bottom_wide, vcenter_wide, hcenter_wide, full_rect)" },
                "keep_offsets": { "type": "boolean", "description": "是否保持偏移量 (可选，默认 false)" }
            }, "required": ["node_path", "preset"]
        })),
        ToolDefinition::new("get_node_groups", "获取节点所属的分组列表", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "节点路径" }
            }, "required": ["node_path"]
        })),
        ToolDefinition::new("set_node_groups", "设置节点的分组", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "节点路径" },
                "groups": { "type": "array", "items": { "type": "string" }, "description": "目标分组列表" }
            }, "required": ["node_path", "groups"]
        })),
        ToolDefinition::new("find_nodes_in_group", "按组名查找所有节点", serde_json::json!({
            "type": "object", "properties": {
                "group": { "type": "string", "description": "组名" }
            }, "required": ["group"]
        })),
        ToolDefinition::new("get_editor_selection", "获取编辑器当前选中的节点", serde_json::json!({
            "type": "object", "properties": {
                "top_only": { "type": "boolean", "description": "仅返回顶层选中节点 (可选，默认 false)" }
            }
        })),
        ToolDefinition::new("select_nodes", "选中/取消选中场景中的节点", serde_json::json!({
            "type": "object", "properties": {
                "node_paths": { "type": "array", "items": { "type": "string" }, "description": "节点路径数组" },
                "node_path": { "type": "string", "description": "单个节点路径 (与 node_paths 二选一)" },
                "mode": { "type": "string", "description": "模式: replace, add, remove (可选，默认 replace)" },
                "inspect": { "type": "boolean", "description": "是否在检查器中显示 (可选，默认 true)" },
                "focus": { "type": "boolean", "description": "是否聚焦节点 (可选，默认同 inspect)" }
            }
        })),
        ToolDefinition::new("clear_editor_selection", "清除编辑器中的所有选中", serde_json::json!({
            "type": "object", "properties": {}
        })),
    ]
}

pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("add_node".into(), cmd_add_node);
    registry.insert("delete_node".into(), cmd_delete_node);
    registry.insert("rename_node".into(), cmd_rename_node);
    registry.insert("update_property".into(), cmd_update_property);
    registry.insert("get_node_properties".into(), cmd_get_node_properties);
    registry.insert("duplicate_node".into(), cmd_duplicate_node);
    registry.insert("connect_signal".into(), cmd_connect_signal);
    registry.insert("disconnect_signal".into(), cmd_disconnect_signal);
    registry.insert("move_node".into(), cmd_move_node);
    // 新增的 8 个工具注册
    registry.insert("add_resource".into(), cmd_add_resource);
    registry.insert("set_anchor_preset".into(), cmd_set_anchor_preset);
    registry.insert("get_node_groups".into(), cmd_get_node_groups);
    registry.insert("set_node_groups".into(), cmd_set_node_groups);
    registry.insert("find_nodes_in_group".into(), cmd_find_nodes_in_group);
    registry.insert("get_editor_selection".into(), cmd_get_editor_selection);
    registry.insert("select_nodes".into(), cmd_select_nodes);
    registry.insert("clear_editor_selection".into(), cmd_clear_editor_selection);
}

fn find_node(root: &Gd<Node>, path: &str) -> Option<Gd<Node>> {
    if path == "." || path == root.get_name().to_string() {
        return Some(root.clone());
    }
    // 简化: 使用 has_node + get_node (通过 NodePath 的 to_string 方式)
    let np = NodePath::from(path);
    if root.has_node(&np.to_string()) {
        return Some(root.get_node_as::<Node>(&np.to_string()));
    }
    let root_name = root.get_name().to_string();
    if let Some(rest) = path.strip_prefix(&(root_name + "/")) {
        let np2 = NodePath::from(rest);
        if root.has_node(&np2.to_string()) {
            return Some(root.get_node_as::<Node>(&np2.to_string()));
        }
    }
    None
}

fn cmd_add_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let node_type = args.get("type").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing type"))?;
    let node_name = args.get("name").and_then(|v| v.as_str()).unwrap_or(node_type);
    let parent_path = args.get("parent_path").and_then(|v| v.as_str()).unwrap_or(".");

    let mut parent = find_node(&root, parent_path)
        .ok_or_else(|| McpError::not_found(&format!("Parent '{}'", parent_path), ""))?;

    let node_var = ClassDb::singleton().instantiate(node_type);
    let mut new_node: Gd<Node> = node_var.to();

    new_node.set_name(node_name);

    if let Some(props) = args.get("properties").and_then(|v| v.as_object()) {
        for (key, val) in props {
            new_node.set(key.as_str(), &serialize::parse_value_for_property(val));
        }
    }

    parent.add_child(&new_node);
    // 通过 Variant 方式设置 owner，绕开 AsArg 的类型约束
    new_node.set("owner", &Variant::from(root.clone()));

    let path_str = root.get_path_to(&new_node).to_string();
    Ok(serde_json::json!({"node_path": path_str, "name": node_name, "type": node_type}))
}

fn cmd_delete_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let mut node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;
    node.queue_free();
    Ok(serde_json::json!({"deleted": true, "path": path}))
}

fn cmd_rename_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing name"))?;
    let mut node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;
    node.set_name(name);
    Ok(serde_json::json!({"renamed": true, "new_name": name}))
}

fn cmd_update_property(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let property = args.get("property").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing property"))?;
    let value = args.get("value").ok_or_else(|| McpError::invalid_params("Missing value"))?;
    let mut node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;
    node.set(property, &serialize::parse_value_for_property(value));
    Ok(serde_json::json!({"node_path": path, "property": property, "updated": true}))
}

fn cmd_get_node_properties(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;

    let prop_list = node.get_property_list();
    let mut props = serde_json::Map::new();

    let filter: Option<Vec<String>> = args.get("properties")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

    let nil_var = Variant::nil();
    for i in 0..prop_list.len() {
        let entry = match prop_list.get(i) { Some(e) => e, None => continue, };
        let name = entry.get(&Variant::from("name")).unwrap_or(nil_var.clone()).to::<String>();
        if name.starts_with('_') || name == "script" { continue; }
        if let Some(ref f) = filter { if !f.contains(&name) { continue; } }

        let val = node.get(&name);
        props.insert(name, serialize::serialize_variant(&val));
    }

    Ok(serde_json::json!({
        "node_path": path, "type": node.get_class().to_string(), "properties": props
    }))
}

/// duplicate_node: 复制节点
fn cmd_duplicate_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let new_name = args.get("new_name").and_then(|v| v.as_str()).unwrap_or(path);

    let node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;
    let mut parent = node.get_parent().ok_or_else(|| McpError::internal("Node has no parent"))?;

    // duplicate() 返回 Option<Gd<Node>>，需要 unwrap
    let mut dup = node.duplicate().ok_or_else(|| McpError::internal("复制节点失败"))?;
    dup.set_name(new_name);
    parent.add_child(&dup);
    // 通过 Variant 方式设置 owner
    dup.set("owner", &Variant::from(root.clone()));

    let new_path = root.get_path_to(&dup).to_string();
    Ok(serde_json::json!({"node_path": new_path, "name": new_name, "duplicated": true}))
}

/// move_node: 移动节点到新父节点 (reparent)
fn cmd_move_node(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing path"))?;
    let new_parent_path = args.get("new_parent").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing new_parent"))?;
    let new_name = args.get("new_name").and_then(|v| v.as_str());

    let mut node = find_node(&root, path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", path), ""))?;
    let mut new_parent = find_node(&root, new_parent_path)
        .ok_or_else(|| McpError::not_found(&format!("Parent '{}'", new_parent_path), ""))?;

    // 从当前父节点移除
    if let Some(mut old_parent) = node.get_parent() {
        old_parent.remove_child(&node);
    }
    // 添加到新父节点
    new_parent.add_child(&node);
    node.set("owner", &Variant::from(root.clone()));

    if let Some(name) = new_name {
        node.set_name(name);
    }

    let final_path = root.get_path_to(&node).to_string();
    Ok(serde_json::json!({"node_path": final_path, "moved": true}))
}

/// connect_signal: 连接节点信号
fn cmd_connect_signal(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let src_path = args.get("source_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing source_path"))?;
    let signal = args.get("signal").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing signal"))?;
    let method = args.get("method").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing method"))?;
    let tgt_path = args.get("target_path").and_then(|v| v.as_str());

    let mut source = find_node(&root, src_path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", src_path), ""))?;

    if let Some(tp) = tgt_path {
        let target = find_node(&root, tp).ok_or_else(|| McpError::not_found(&format!("Node '{}'", tp), ""))?;
        let callable = Callable::from_object_method(&target, method);
        source.connect(signal, &callable);
        Ok(serde_json::json!({"connected": true, "signal": signal, "source": src_path, "target": tp}))
    } else {
        // 无目标节点时连接到场景根节点
        let callable = Callable::from_object_method(&root, method);
        source.connect(signal, &callable);
        Ok(serde_json::json!({"connected": true, "signal": signal, "source": src_path}))
    }
}

/// disconnect_signal: 断开信号连接
fn cmd_disconnect_signal(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let src_path = args.get("source_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing source_path"))?;
    let signal = args.get("signal").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing signal"))?;
    let method = args.get("method").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing method"))?;

    let mut source = find_node(&root, src_path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", src_path), ""))?;

    // disconnect 需要一个 Callable 来匹配要断开的连接
    let callable = Callable::from_object_method(&root, method);
    source.disconnect(signal, &callable);
    Ok(serde_json::json!({"disconnected": true, "signal": signal, "source": src_path}))
}

// =============================================================================
// 新增的 8 个节点工具
// =============================================================================

/// add_resource: 创建资源并附加到节点属性
fn cmd_add_resource(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let property = args.get("property").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing property"))?;
    let resource_type = args.get("resource_type").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing resource_type"))?;

    let mut node = find_node(&root, node_path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", node_path), ""))?;

    // 验证资源类型是否存在于 ClassDB 中
    let class_db = ClassDb::singleton();
    let res_type_name = StringName::from(resource_type);
    let resource_base_name = StringName::from("Resource");
    if !class_db.class_exists(&res_type_name) {
        return Err(McpError::invalid_params(&format!("Unknown resource type: {}", resource_type)));
    }
    if !class_db.is_parent_class(&res_type_name, &resource_base_name) {
        return Err(McpError::invalid_params(&format!("'{}' is not a Resource type", resource_type)));
    }

    // 创建资源实例
    let resource_var = class_db.instantiate(resource_type);
    let mut resource: Gd<Resource> = resource_var.to();

    // 应用资源属性（如果有）
    if let Some(resource_props) = args.get("resource_properties").and_then(|v| v.as_object()) {
        for (key, val) in resource_props {
            resource.set(key.as_str(), &serialize::parse_value_for_property(val));
        }
    }

    // 将资源赋值给节点属性
    node.set(property, &Variant::from(resource));

    let path_str = root.get_path_to(&node).to_string();
    Ok(serde_json::json!({
        "node_path": path_str,
        "property": property,
        "resource_type": resource_type,
    }))
}

/// set_anchor_preset: 设置 Control 节点的锚点预设
fn cmd_set_anchor_preset(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let preset_name = args.get("preset").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing preset"))?;
    let keep_offsets = args.get("keep_offsets").and_then(|v| v.as_bool()).unwrap_or(false);

    let node = find_node(&root, node_path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", node_path), ""))?;

    // 检查是否为 Control 类型节点
    if !node.is_class(&GString::from("Control")) {
        return Err(McpError::invalid_params(&format!(
            "Node '{}' is not a Control (is {})", node_path, node.get_class().to_string()
        )));
    }

    // 预设名称到 LayoutPreset 的映射
    let preset = match preset_name {
        "top_left" => LayoutPreset::TOP_LEFT,
        "top_right" => LayoutPreset::TOP_RIGHT,
        "bottom_left" => LayoutPreset::BOTTOM_LEFT,
        "bottom_right" => LayoutPreset::BOTTOM_RIGHT,
        "center_left" => LayoutPreset::CENTER_LEFT,
        "center_top" => LayoutPreset::CENTER_TOP,
        "center_right" => LayoutPreset::CENTER_RIGHT,
        "center_bottom" => LayoutPreset::CENTER_BOTTOM,
        "center" => LayoutPreset::CENTER,
        "left_wide" => LayoutPreset::LEFT_WIDE,
        "top_wide" => LayoutPreset::TOP_WIDE,
        "right_wide" => LayoutPreset::RIGHT_WIDE,
        "bottom_wide" => LayoutPreset::BOTTOM_WIDE,
        "vcenter_wide" => LayoutPreset::VCENTER_WIDE,
        "hcenter_wide" => LayoutPreset::HCENTER_WIDE,
        "full_rect" => LayoutPreset::FULL_RECT,
        _ => return Err(McpError::invalid_params(&format!(
            "Unknown preset: '{}'. Available: top_left, top_right, bottom_left, bottom_right, center_left, center_top, center_right, center_bottom, center, left_wide, top_wide, right_wide, bottom_wide, vcenter_wide, hcenter_wide, full_rect",
            preset_name
        ))),
    };

    // 尝试转换为 Control 以调用 set_anchors_and_offsets_preset
    let mut control = node.try_cast::<Control>().map_err(|_| {
        McpError::invalid_params("Failed to cast node to Control")
    })?;

    // 根据 keep_offsets 选择对应的 LayoutPresetMode
    let mode = if keep_offsets {
        LayoutPresetMode::KEEP_SIZE
    } else {
        LayoutPresetMode::MINSIZE
    };
    control.set_anchors_and_offsets_preset_ex(preset).resize_mode(mode).done();

    let path_str = root.get_path_to(&control).to_string();
    Ok(serde_json::json!({
        "node_path": path_str,
        "preset": preset_name,
    }))
}

/// get_node_groups: 获取节点分组列表
fn cmd_get_node_groups(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;

    let node = find_node(&root, node_path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", node_path), ""))?;

    let groups_array = node.get_groups();
    let mut groups: Vec<String> = Vec::new();

    for i in 0..groups_array.len() {
        if let Some(sn) = groups_array.get(i) {
            let group_str = sn.to_string();
            // 过滤掉以 "_" 开头的内部组
            if !group_str.starts_with('_') {
                groups.push(group_str);
            }
        }
    }

    let path_str = root.get_path_to(&node).to_string();
    Ok(serde_json::json!({
        "node_path": path_str,
        "groups": groups,
        "count": groups.len(),
    }))
}

/// set_node_groups: 设置节点分组
fn cmd_set_node_groups(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let node_path = args.get("node_path").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing node_path"))?;
    let desired_groups = args.get("groups").and_then(|v| v.as_array()).ok_or_else(|| {
        McpError::invalid_params("Missing or invalid 'groups' array")
    })?;

    let mut node = find_node(&root, node_path).ok_or_else(|| McpError::not_found(&format!("Node '{}'", node_path), ""))?;

    // 获取当前非内部组列表
    let current_array = node.get_groups();
    let mut current_groups: Vec<String> = Vec::new();
    for i in 0..current_array.len() {
        if let Some(sn) = current_array.get(i) {
            let g = sn.to_string();
            if !g.starts_with('_') {
                current_groups.push(g);
            }
        }
    }

    // 计算要移除的组
    let mut removed: Vec<String> = Vec::new();
    for g in &current_groups {
        if !desired_groups.iter().any(|v| v.as_str() == Some(g.as_str())) {
            removed.push(g.clone());
        }
    }

    // 计算要添加的组
    let mut added: Vec<String> = Vec::new();
    for v in desired_groups {
        if let Some(g) = v.as_str() {
            if !current_groups.contains(&g.to_string()) {
                added.push(g.to_string());
            }
        }
    }

    // 执行添加操作
    for g in &added {
        let sn = StringName::from(g.as_str());
        node.add_to_group(&sn);
    }

    // 执行移除操作
    for g in &removed {
        let sn = StringName::from(g.as_str());
        node.remove_from_group(&sn);
    }

    let path_str = root.get_path_to(&node).to_string();
    Ok(serde_json::json!({
        "node_path": path_str,
        "groups": desired_groups,
        "added": added,
        "removed": removed,
    }))
}

/// 递归查找组内节点的辅助函数
fn find_in_group_recursive(node: &Gd<Node>, root: &Gd<Node>, group_name: &str, matches: &mut Vec<serde_json::Value>) {
    // 检查当前节点是否在目标组中
    let gn = StringName::from(group_name);
    if node.is_in_group(&gn) {
        matches.push(serde_json::json!({
            "name": node.get_name().to_string(),
            "path": root.get_path_to(node).to_string(),
            "type": node.get_class().to_string(),
        }));
    }

    // 递归遍历子节点
    for i in 0..node.get_child_count() {
        if let Some(child) = node.get_child(i) {
            find_in_group_recursive(&child, root, group_name, matches);
        }
    }
}

/// find_nodes_in_group: 按组名递归查找所有节点
fn cmd_find_nodes_in_group(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let group = args.get("group").and_then(|v| v.as_str()).ok_or_else(|| McpError::invalid_params("Missing group"))?;

    let mut matches: Vec<serde_json::Value> = Vec::new();
    find_in_group_recursive(&root, &root, group, &mut matches);

    Ok(serde_json::json!({
        "group": group,
        "nodes": matches,
        "count": matches.len(),
    }))
}

/// 序列化选中节点列表的辅助函数
fn serialize_selection_nodes(root: &Gd<Node>, nodes: Array<Gd<Node>>) -> Vec<serde_json::Value> {
    let mut result: Vec<serde_json::Value> = Vec::new();
    for i in 0..nodes.len() {
        if let Some(node) = nodes.get(i) {
            // 跳过不在当前场景中的节点
            let is_root = matches!(root.get_path_to(&node).to_string().as_str(), ".");
            if is_root || root.is_ancestor_of(&node) {
                let path_str = if is_root {
                    ".".to_string()
                } else {
                    root.get_path_to(&node).to_string()
                };
                result.push(serde_json::json!({
                    "name": node.get_name().to_string(),
                    "path": path_str,
                    "type": node.get_class().to_string(),
                }));
            }
        }
    }
    result
}

/// get_editor_selection: 获取编辑器当前选中的节点
fn cmd_get_editor_selection(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let top_only = args.get("top_only").and_then(|v| v.as_bool()).unwrap_or(false);

    let selection = editor.get_selection().ok_or_else(|| {
        McpError::internal("Failed to get editor selection")
    })?;

    let selected_nodes = if top_only {
        selection.get_top_selected_nodes()
    } else {
        selection.get_selected_nodes()
    };

    let serialized = serialize_selection_nodes(&root, selected_nodes);

    Ok(serde_json::json!({
        "nodes": serialized,
        "count": serialized.len(),
        "top_only": top_only,
    }))
}

/// select_nodes: 选中/取消选中场景中的节点
fn cmd_select_nodes(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let mut editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    // 解析 node_paths（支持数组和单字符串两种格式）
    let node_paths: Vec<String> = if let Some(paths) = args.get("node_paths").and_then(|v| v.as_array()) {
        paths.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    } else if let Some(path) = args.get("node_path").and_then(|v| v.as_str()) {
        vec![path.to_string()]
    } else {
        return Err(McpError::invalid_params("Missing node_paths or node_path"));
    };

    let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("replace");
    if mode != "replace" && mode != "add" && mode != "remove" {
        return Err(McpError::invalid_params("mode must be one of: replace, add, remove"));
    }

    let inspect = args.get("inspect").and_then(|v| v.as_bool()).unwrap_or(true);
    let focus = args.get("focus").and_then(|v| v.as_bool()).unwrap_or(inspect);

    // 解析所有节点路径为 Gd<Node>
    let mut resolved_nodes: Vec<Gd<Node>> = Vec::new();
    for path_str in &node_paths {
        let node = find_node(&root, path_str)
            .ok_or_else(|| McpError::not_found(&format!("Node '{}'", path_str), ""))?;
        resolved_nodes.push(node);
    }

    let mut selection = editor.get_selection().ok_or_else(|| {
        McpError::internal("Failed to get editor selection")
    })?;

    // 如果是 replace 模式，先清空选中
    if mode == "replace" {
        selection.clear();
    }

    // 添加或移除节点
    for node in &resolved_nodes {
        if mode == "remove" {
            selection.remove_node(node);
        } else {
            selection.add_node(node);
        }
    }

    // 当选中单个节点时，支持聚焦和检查器显示
    if mode != "remove" && resolved_nodes.len() == 1 {
        let edited_node = &resolved_nodes[0];
        if focus {
            editor.edit_node(edited_node);
        }
        if inspect {
            editor.inspect_object(edited_node);
        }
    }

    let current_selection = selection.get_selected_nodes();
    let serialized = serialize_selection_nodes(&root, current_selection);

    Ok(serde_json::json!({
        "mode": mode,
        "selected": serialized,
        "count": serialized.len(),
    }))
}

/// clear_editor_selection: 清除编辑器中的所有选中
fn cmd_clear_editor_selection(_args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let _root = editor.get_edited_scene_root().ok_or_else(|| McpError::no_scene())?;

    let mut selection = editor.get_selection().ok_or_else(|| {
        McpError::internal("Failed to get editor selection")
    })?;

    let before_count = selection.get_selected_nodes().len();
    selection.clear();

    Ok(serde_json::json!({
        "cleared": before_count,
        "selected": [],
        "count": 0,
    }))
}
