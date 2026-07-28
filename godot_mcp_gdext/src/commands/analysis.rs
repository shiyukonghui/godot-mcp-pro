//! 项目分析命令模块
//!
//! 提供资源扫描、信号分析、场景复杂度分析等分析工具。
//! 对应原 GDScript 插件的 analysis_commands.gd
//! 纯 Rust 实现，无 GDScript Expression。

use std::collections::HashMap;

use godot::classes::file_access::ModeFlags;
use godot::classes::{
    DirAccess, EditorInterface, FileAccess, Node, PackedScene, ResourceLoader,
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

/// 从参数中读取必填字符串
fn req_string(args: &serde_json::Map<String, serde_json::Value>, key: &str) -> Result<String, McpError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| McpError::invalid_params(&format!("缺少必填参数: {}", key)))
}

/// 递归收集指定扩展名的文件
fn collect_files_by_ext(path: &str, extensions: &[&str], out: &mut Vec<String>, include_addons: bool) {
    let dir = DirAccess::open(path);
    let Some(mut dir) = dir else { return };

    dir.list_dir_begin();
    loop {
        let file_name = dir.get_next().to_string();
        if file_name.is_empty() {
            break;
        }
        if file_name.starts_with('.') {
            continue;
        }

        let full_path = if path.ends_with('/') {
            format!("{}{}", path, file_name)
        } else {
            format!("{}/{}", path, file_name)
        };

        if dir.current_is_dir() {
            if file_name == "addons" && !include_addons {
                continue;
            }
            collect_files_by_ext(&full_path, extensions, out, include_addons);
        } else {
            if let Some(dot_pos) = file_name.rfind('.') {
                let ext = &file_name[dot_pos + 1..].to_lowercase();
                if extensions.contains(&ext.as_str()) {
                    out.push(full_path);
                }
            }
        }
    }
    dir.list_dir_end();
}

/// 读取文件全部文本
fn read_file_text(path: &str) -> String {
    match FileAccess::open(path, ModeFlags::READ) {
        Some(f) => f.get_as_text().to_string(),
        None => String::new(),
    }
}

/// 递归收集节点的信号连接数据（供 analyze_signal_flow 使用）
fn collect_signal_data_recursive(
    node: &Gd<Node>,
    root: &Gd<Node>,
) -> Vec<serde_json::Value> {
    let mut nodes_data = Vec::new();
    _collect_signal_recursive(node, root, &mut nodes_data);
    nodes_data
}

fn _collect_signal_recursive(
    node: &Gd<Node>,
    root: &Gd<Node>,
    out: &mut Vec<serde_json::Value>,
) {
    // 使用 GDScript Expression 获取节点信号数据（避免 Dictionary 类型转换问题）
    let code = format!(
        "var node = EditorInterface.get_edited_scene_root().get_node(\"{}\"); \
         var root = EditorInterface.get_edited_scene_root(); \
         if node == null or root == null: return null; \
         var node_path = str(root.get_path_to(node)); \
         var sig_list = node.get_signal_list(); \
         var emitted = []; \
         var connected_to = []; \
         for sig in sig_list: \
             var sig_name = sig.get('name', ''); \
             var conns = node.get_signal_connection_list(sig_name); \
             var targets = []; \
             for c in conns: \
                 var flags = c.get('flags', 0); \
                 if int(flags) & 1 == 0: continue; \
                 var callable = c.get('callable', null); \
                 if callable == null: continue; \
                 var tgt = callable.get_object(); \
                 if tgt == null: continue; \
                 if tgt != root and not root.is_ancestor_of(tgt): continue; \
                 var tp = str(root.get_path_to(tgt)); \
                 targets.append({{'target_node': tp, 'method': callable.get_method()}}); \
                 connected_to.append({{'from_node': node_path, 'signal': sig_name, 'method': callable.get_method()}}); \
             if targets.size() > 0: \
                 emitted.append({{'signal': sig_name, 'targets': targets}}); \
         if emitted.size() > 0 or connected_to.size() > 0: \
             return {{\"name\": str(node.get_name()), \"path\": node_path, \"type\": str(node.get_class()), \"signals_emitted\": emitted, \"signals_connected_to\": connected_to}}; \
         return null",
        node.get_name().to_string()
    );
    let mut ex = godot::classes::Expression::new_gd();
    if ex.parse(&code) == godot::global::Error::OK {
        let result = ex.execute();
        if !result.is_nil() {
            let s: String = result.to();
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&s) {
                out.push(parsed);
            }
        }
    }

    // 递归遍历子节点
    let child_count = node.get_child_count();
    for i in 0..child_count {
        if let Some(child) = node.get_child(i) {
            _collect_signal_recursive(&child, root, out);
        }
    }
}

// ============================================================================
// 场景复杂度分析的递归辅助函数
// ============================================================================

/// 递归统计场景节点信息
fn analyze_scene_node(
    node: &Gd<Node>,
    root: &Gd<Node>,
    depth: i64,
    type_counts: &mut serde_json::Map<String, serde_json::Value>,
    scripts: &mut Vec<serde_json::Value>,
) -> (i64, i64) {
    // total_nodes (当前子树), max_depth
    let type_name = node.get_class().to_string();
    *type_counts
        .entry(type_name)
        .or_insert(serde_json::json!(0)) = serde_json::json!(
        type_counts
            .get(&node.get_class().to_string())
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            + 1
    );

    // 检查脚本
    let script = node.get_script();
    if script.is_some() {
        if let Some(s) = script {
            let script_path = s.get_path().to_string();
            if !script_path.is_empty() {
                scripts.push(serde_json::json!({
                    "node": root.get_path_to(node).to_string(),
                    "script": script_path,
                }));
            }
        }
    }

    let mut total = 1i64;
    let mut max_depth = depth;
    let child_count = node.get_child_count();
    for i in 0..child_count {
        if let Some(child) = node.get_child(i) {
            let (sub_total, sub_depth) = analyze_scene_node(&child, root, depth + 1, type_counts, scripts);
            total += sub_total;
            if sub_depth > max_depth {
                max_depth = sub_depth;
            }
        }
    }

    (total, max_depth)
}

// ============================================================================
// 循环依赖检测
// ============================================================================

/// 从 .tscn 文件内容中解析 ext_resource 引用
fn parse_ext_resource_refs(content: &str) -> Vec<String> {
    let mut refs = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[ext_resource") && trimmed.contains(".tscn") {
            // 提取 path="..." 的值
            if let Some(ps) = trimmed.find("path=\"") {
                let start = ps + 6;
                if let Some(pe) = trimmed[start..].find('\"') {
                    let ref_path = &trimmed[start..start + pe];
                    if ref_path.ends_with(".tscn") {
                        refs.push(ref_path.to_string());
                    }
                }
            }
        }
    }
    refs
}

/// DFS 检测循环依赖
fn dfs_detect_cycle(
    node: &str,
    graph: &HashMap<String, Vec<String>>,
    visited: &mut HashMap<String, String>,
    path_stack: &mut Vec<String>,
    cycles: &mut Vec<Vec<String>>,
) {
    visited.insert(node.to_string(), "visiting".to_string());
    path_stack.push(node.to_string());

    if let Some(deps) = graph.get(node) {
        for dep in deps {
            let state = visited.get(dep).map(|s| s.as_str()).unwrap_or("unvisited");
            match state {
                "visiting" => {
                    // 找到环
                    if let Some(cycle_start) = path_stack.iter().position(|p| p == dep) {
                        let mut cycle: Vec<String> = path_stack[cycle_start..].to_vec();
                        cycle.push(dep.to_string());
                        cycles.push(cycle);
                    }
                }
                "unvisited" => {
                    dfs_detect_cycle(dep, graph, visited, path_stack, cycles);
                }
                _ => {}
            }
        }
    }

    path_stack.pop();
    visited.insert(node.to_string(), "visited".to_string());
}

// ============================================================================
// 项目统计辅助函数
// ============================================================================

/// 递归收集项目统计信息
fn collect_stats_recursive(
    path: &str,
    include_addons: bool,
    file_counts: &mut HashMap<String, i64>,
    script_lines: &mut i64,
    scene_count: &mut i64,
    resource_count: &mut i64,
) -> i64 {
    let mut total_files = 0i64;
    let dir = DirAccess::open(path);
    let Some(mut dir) = dir else { return 0 };

    dir.list_dir_begin();
    loop {
        let file_name = dir.get_next().to_string();
        if file_name.is_empty() {
            break;
        }
        if file_name.starts_with('.') {
            continue;
        }

        let full_path = if path.ends_with('/') {
            format!("{}{}", path, file_name)
        } else {
            format!("{}/{}", path, file_name)
        };

        if dir.current_is_dir() {
            if file_name == "addons" && !include_addons {
                continue;
            }
            total_files += collect_stats_recursive(&full_path, include_addons, file_counts, script_lines, scene_count, resource_count);
        } else {
            if let Some(dot_pos) = file_name.rfind('.') {
                let ext = file_name[dot_pos + 1..].to_lowercase();
                *file_counts.entry(ext.clone()).or_insert(0) += 1;

                match ext.as_str() {
                    "gd" => {
                        let content = read_file_text(&full_path);
                        if !content.is_empty() {
                            *script_lines += content.lines().count() as i64;
                        }
                    }
                    "tscn" => *scene_count += 1,
                    "tres" | "material" | "theme" | "stylebox" | "font" => *resource_count += 1,
                    _ => {}
                }
            }
            total_files += 1;
        }
    }
    dir.list_dir_end();
    total_files
}

// ============================================================================
// 工具命令实现 — 全部为纯 Rust，无 GDScript Expression
// ============================================================================

/// 1. find_unused_resources: 查找项目中可能未使用的资源文件
fn cmd_find_unused_resources(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = opt_string(args, "path", "res://");
    let include_addons = opt_bool(args, "include_addons", false);

    let resource_exts = [
        "tres", "tscn", "png", "jpg", "jpeg", "svg",
        "wav", "ogg", "mp3", "ttf", "otf", "gdshader", "material",
        "theme", "stylebox", "font", "anim",
    ];

    // 收集所有资源文件和引用文件
    let mut all_resources = Vec::new();
    collect_files_by_ext(&path, &resource_exts, &mut all_resources, include_addons);

    let ref_exts = ["tscn", "gd", "tres", "cfg", "godot"];
    let mut ref_files = Vec::new();
    collect_files_by_ext(&path, &ref_exts, &mut ref_files, include_addons);

    // 从引用文件中提取被引用的资源路径
    let mut referenced: HashMap<String, bool> = HashMap::new();
    for rf in &ref_files {
        let content = read_file_text(rf);
        for line in content.lines() {
            if line.starts_with("[ext_resource") {
                // 提取 path="..." 的值
                if let Some(ps) = line.find("path=\"") {
                    let start = ps + 6;
                    if let Some(pe) = line[start..].find('\"') {
                        let ref_path = &line[start..start + pe];
                        referenced.insert(ref_path.to_string(), true);
                    }
                }
            }
        }
    }

    // 找出未被引用的资源
    let unused: Vec<String> = all_resources.iter()
        .filter(|r| !referenced.contains_key(r.as_str()))
        .cloned()
        .collect();

    Ok(serde_json::json!({
        "unused_resources": unused,
        "unused_count": unused.len(),
        "total_resources_scanned": all_resources.len(),
        "total_files_checked": ref_files.len(),
    }))
}

/// 2. analyze_signal_flow: 分析信号连接流（纯 Rust 递归遍历）
fn cmd_analyze_signal_flow(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root().ok_or_else(McpError::no_scene)?;
    let scene_path = root.get_scene_file_path().to_string();

    let node_filter = opt_string(args, "node_path", "");

    let nodes_data = if node_filter.is_empty() || node_filter == "." {
        // 分析整个场景
        collect_signal_data_recursive(&root, &root)
    } else {
        // 分析指定节点及其子树
        if !root.has_node(&node_filter) {
            return Err(McpError::not_found(&format!("节点 '{}'", node_filter), ""));
        }
        let node = root.get_node_as::<Node>(&node_filter);
        collect_signal_data_recursive(&node, &root)
    };

    Ok(serde_json::json!({
        "scene": scene_path,
        "nodes": nodes_data,
        "total_nodes": nodes_data.len(),
    }))
}

/// 3. analyze_scene_complexity: 分析场景复杂度（纯 Rust 递归）
fn cmd_analyze_scene_complexity(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let scene_path = opt_string(args, "path", "");

    let (mut root_node, actual_path, needs_free) = if scene_path.is_empty() {
        let editor = EditorInterface::singleton();
        let root = editor.get_edited_scene_root()
            .ok_or_else(McpError::no_scene)?;
        let path = root.get_scene_file_path().to_string();
        (root, path, false)
    } else {
        if !ResourceLoader::singleton().exists(&scene_path) {
            return Err(McpError::not_found(&format!("场景 '{}'", scene_path), ""));
        }
        let packed = ResourceLoader::singleton().load(&scene_path);
        let Some(packed_resource) = packed else {
            return Err(McpError::internal(&format!("无法加载场景: {}", scene_path)));
        };
        let packed_scene = packed_resource.try_cast::<PackedScene>()
            .map_err(|_| McpError::internal("加载的资源不是有效的场景文件"))?;
        let Some(root) = packed_scene.instantiate() else {
            return Err(McpError::internal(&format!("无法实例化场景: {}", scene_path)));
        };
        (root, scene_path, true)
    };

    // 纯 Rust 递归分析
    let mut type_counts = serde_json::Map::new();
    let mut scripts = Vec::new();

    let (total_nodes, max_depth) = analyze_scene_node(&root_node, &root_node, 0, &mut type_counts, &mut scripts);

    // 生成建议
    let mut issues: Vec<serde_json::Value> = Vec::new();
    if total_nodes > 1000 {
        issues.push(serde_json::json!({
            "severity": "warning",
            "message": format!("场景有 {} 个节点 (>1000)。考虑拆分为子场景。", total_nodes),
        }));
    } else if total_nodes > 500 {
        issues.push(serde_json::json!({
            "severity": "info",
            "message": format!("场景有 {} 个节点 (>500)。建议监控性能。", total_nodes),
        }));
    }
    if max_depth > 15 {
        issues.push(serde_json::json!({
            "severity": "warning",
            "message": format!("最大嵌套深度为 {} (>15)。深层层级难以维护。", max_depth),
        }));
    } else if max_depth > 10 {
        issues.push(serde_json::json!({
            "severity": "info",
            "message": format!("最大嵌套深度为 {} (>10)。", max_depth),
        }));
    }

    if needs_free {
        root_node.queue_free();
    }

    Ok(serde_json::json!({
        "scene_path": actual_path,
        "total_nodes": total_nodes,
        "max_depth": max_depth,
        "nodes_by_type": type_counts,
        "scripts_attached": scripts,
        "issues": issues,
    }))
}

/// 4. find_script_references: 查找引用指定脚本的所有文件
fn cmd_find_script_references(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let query = req_string(args, "query")?;
    let path = opt_string(args, "path", "res://");
    let include_addons = opt_bool(args, "include_addons", false);

    let search_exts = ["tscn", "gd", "tres", "cfg", "godot"];
    let mut search_files = Vec::new();
    collect_files_by_ext(&path, &search_exts, &mut search_files, include_addons);

    let mut references = Vec::new();
    for file_path in &search_files {
        let content = read_file_text(file_path);
        if content.is_empty() {
            continue;
        }
        for (line_num, line) in content.lines().enumerate() {
            if line.contains(&query) {
                references.push(serde_json::json!({
                    "file": file_path,
                    "line": line_num + 1,
                    "content": line.trim(),
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "query": query,
        "references": references,
        "reference_count": references.len(),
        "files_searched": search_files.len(),
    }))
}

/// 5. detect_circular_dependencies: 检测场景间循环依赖（纯 Rust 实现）
fn cmd_detect_circular_dependencies(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = opt_string(args, "path", "res://");
    let include_addons = opt_bool(args, "include_addons", false);

    // 收集所有 .tscn 文件
    let mut tscn_files = Vec::new();
    collect_files_by_ext(&path, &["tscn"], &mut tscn_files, include_addons);

    // 构建依赖图
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    for tp in &tscn_files {
        let content = read_file_text(tp);
        if content.is_empty() {
            continue;
        }
        let deps = parse_ext_resource_refs(&content);
        graph.insert(tp.clone(), deps);
    }

    // DFS 检测循环依赖
    let mut visited: HashMap<String, String> = HashMap::new();
    let mut cycles: Vec<Vec<String>> = Vec::new();
    let mut path_stack = Vec::new();

    for scene in graph.keys() {
        visited.insert(scene.clone(), "unvisited".to_string());
    }
    for scene in graph.keys() {
        if visited.get(scene).map(|s| s.as_str()) == Some("unvisited") {
            dfs_detect_cycle(scene, &graph, &mut visited, &mut path_stack, &mut cycles);
        }
    }

    Ok(serde_json::json!({
        "scenes_checked": tscn_files.len(),
        "circular_dependencies": cycles,
        "has_circular": !cycles.is_empty(),
        "dependency_graph": graph,
    }))
}

/// 6. get_project_statistics: 获取项目统计信息（纯 Rust 实现）
fn cmd_get_project_statistics(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = opt_string(args, "path", "res://");
    let include_addons = opt_bool(args, "include_addons", false);

    let mut file_counts: HashMap<String, i64> = HashMap::new();
    let mut total_script_lines = 0i64;
    let mut scene_count = 0i64;
    let mut resource_count = 0i64;

    let total_files = collect_stats_recursive(
        &path, include_addons,
        &mut file_counts, &mut total_script_lines,
        &mut scene_count, &mut resource_count,
    );

    // 序列化 file_counts
    let file_counts_json: serde_json::Map<String, serde_json::Value> = file_counts
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::json!(v)))
        .collect();

    // 收集 autoloads - 使用 GDScript Expression 获取
    let mut autoloads = serde_json::Map::new();
    let autoload_code = "\
var ps = ProjectSettings.get_singleton(); \
var list = ps.get_property_list(); \
var result = {}; \
for p in list: \
    var name = str(p.get('name', '')); \
    if name.begins_with('autoload/'): \
        result[name.substr(9)] = str(ps.get_setting(name)); \
return JSON.stringify(result)";
    let mut al_expr = godot::classes::Expression::new_gd();
    if al_expr.parse(autoload_code) == godot::global::Error::OK {
        let result = al_expr.execute();
        if let Ok(s) = result.try_to::<String>() {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&s) {
                if let Some(obj) = parsed.as_object() {
                    autoloads = obj.clone();
                }
            }
        }
    }

    // 收集插件信息
    let mut plugins = Vec::new();
    let plugin_dir = DirAccess::open("res://addons");
    if let Some(mut pd) = plugin_dir {
        pd.list_dir_begin();
        loop {
            let dn = pd.get_next().to_string();
            if dn.is_empty() { break; }
            if dn.starts_with('.') { continue; }
            if pd.current_is_dir() {
                let cfg_path = format!("res://addons/{}/plugin.cfg", dn);
                if FileAccess::file_exists(&cfg_path) {
                    // 检查插件是否在启用列表中 - 使用 GDScript Expression
                    let plugin_check_code = format!(
                        "var ep = ProjectSettings.get_setting('editor_plugins/enabled', []); \
                         return '{}' in ep",
                        cfg_path
                    );
                    let mut plug_expr = godot::classes::Expression::new_gd();
                    let enabled = if plug_expr.parse(&plugin_check_code) == godot::global::Error::OK {
                        let r = plug_expr.execute();
                        r.try_to::<bool>().unwrap_or(false)
                    } else {
                        false
};
                    plugins.push(serde_json::json!({
                        "name": dn,
                        "enabled": enabled,
                    }));
                }
            }
        }
        pd.list_dir_end();
    }

    Ok(serde_json::json!({
        "file_counts_by_extension": file_counts_json,
        "total_files": total_files,
        "total_script_lines": total_script_lines,
        "scene_count": scene_count,
        "resource_count": resource_count,
        "autoloads": autoloads,
        "plugins": plugins,
    }))
}

// ============================================================================
// 工具注册
// ============================================================================

/// 收集本模块的所有工具定义
pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new("find_unused_resources", "查找项目中可能未使用的资源文件", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "搜索路径，默认为 res://", "default": "res://" },
                "include_addons": { "type": "boolean", "description": "是否包含 addons 目录", "default": false }
            }, "required": []
        })),
        ToolDefinition::new("analyze_signal_flow", "分析当前场景的信号连接流", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "要分析的节点路径（可选，不传则分析整个场景）" }
            }, "required": []
        })),
        ToolDefinition::new("analyze_scene_complexity", "分析场景的复杂度（节点数、深度、类型分布等）", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "场景文件路径（可选，不传则分析当前编辑场景）" }
            }, "required": []
        })),
        ToolDefinition::new("find_script_references", "查找引用指定脚本的所有文件", serde_json::json!({
            "type": "object", "properties": {
                "query": { "type": "string", "description": "要搜索的脚本路径或类名" },
                "path": { "type": "string", "description": "搜索路径", "default": "res://" },
                "include_addons": { "type": "boolean", "description": "是否包含 addons 目录", "default": false }
            }, "required": ["query"]
        })),
        ToolDefinition::new("detect_circular_dependencies", "检测场景文件之间的循环依赖", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "搜索路径", "default": "res://" },
                "include_addons": { "type": "boolean", "description": "是否包含 addons 目录", "default": false }
            }, "required": []
        })),
        ToolDefinition::new("get_project_statistics", "获取项目统计信息（文件数、脚本行数、场景数等）", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "统计路径", "default": "res://" },
                "include_addons": { "type": "boolean", "description": "是否包含 addons 目录", "default": false }
            }, "required": []
        })),
    ]
}

/// 注册本模块的所有命令到全局注册表
pub fn register(registry: &mut HashMap<String, fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>>) {
    registry.insert("find_unused_resources".into(), cmd_find_unused_resources);
    registry.insert("analyze_signal_flow".into(), cmd_analyze_signal_flow);
    registry.insert("analyze_scene_complexity".into(), cmd_analyze_scene_complexity);
    registry.insert("find_script_references".into(), cmd_find_script_references);
    registry.insert("detect_circular_dependencies".into(), cmd_detect_circular_dependencies);
    registry.insert("get_project_statistics".into(), cmd_get_project_statistics);
}
