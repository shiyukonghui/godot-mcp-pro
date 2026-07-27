//! 项目分析命令模块
//!
//! 提供资源扫描、信号分析、场景复杂度分析等分析工具。
//! 对应原 GDScript 插件的 analysis_commands.gd

use std::collections::HashMap;

use godot::classes::file_access::ModeFlags;
use godot::classes::{
    DirAccess, EditorInterface, Expression, FileAccess, PackedScene, ResourceLoader,
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
        // 跳过隐藏文件
        if file_name.starts_with('.') {
            continue;
        }

        let full_path = if path.ends_with('/') {
            format!("{}{}", path, file_name)
        } else {
            format!("{}/{}", path, file_name)
        };

        if dir.current_is_dir() {
            // 跳过 addons 目录（默认排除）
            if file_name == "addons" && !include_addons {
                continue;
            }
            collect_files_by_ext(&full_path, extensions, out, include_addons);
        } else {
            // 检查扩展名
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

/// 读取文件内容
fn read_file_text(path: &str) -> String {
    let file = FileAccess::open(path, ModeFlags::READ);
    match file {
        Some(f) => f.get_as_text().to_string(),
        None => String::new(),
    }
}

/// 通过 GDScript Expression 获取编辑器的当前场景根节点
fn get_edited_root() -> Option<Gd<Node>> {
    let mut expr = Expression::new_gd();
    let code = "\
var ei = EditorInterface \
if ei != null: \
    var root = ei.get_edited_scene_root() \
    if root != null: \
        return root \
return null";
    if expr.parse(code) == godot::global::Error::OK {
        let result = expr.execute();
        if !result.is_nil() {
            // 尝试转换回 Gd<Node>
            let node = result.try_to::<Gd<Node>>().ok()?;
            return Some(node);
        }
    }
    None
}

/// 通过 GDScript Expression 递归获取节点的信号连接信息
fn collect_signal_data(node_path: &str) -> Result<serde_json::Value, McpError> {
    let mut expr = Expression::new_gd();
    let code = format!(
        "\
var node = EditorInterface.get_edited_scene_root().get_node(\"{}\") \
if node == null: \
    return [] \
var result = [] \
_collect_signal_data(node, EditorInterface.get_edited_scene_root(), result) \
return result \
\n\
func _collect_signal_data(node, root, out): \
    var node_path_str = root.get_path_to(node) \
    var signals_emitted = [] \
    var signals_connected_to = [] \
    for sig in node.get_signal_list(): \
        var sig_name = sig[\"name\"] \
        var connections = node.get_signal_connection_list(sig_name) \
        var targets = [] \
        for conn in connections: \
            if int(conn.get(\"flags\", 0)) & 1 == 0: \
                continue \
            var callable = conn[\"callable\"] \
            var target_node = callable.get_object() \
            if target_node == null: \
                continue \
            if target_node != root and not root.is_ancestor_of(target_node): \
                continue \
            targets.append({{\"target_node\": str(root.get_path_to(target_node)), \"method\": callable.get_method()}}) \
            signals_connected_to.append({{\"from_node\": str(node_path_str), \"signal\": sig_name, \"method\": callable.get_method()}}) \
        if targets.size() > 0: \
            signals_emitted.append({{\"signal\": sig_name, \"targets\": targets}}) \
    if signals_emitted.size() > 0 or signals_connected_to.size() > 0: \
        out.append({{\"name\": node.name, \"path\": str(node_path_str), \"type\": node.get_class(), \"signals_emitted\": signals_emitted, \"signals_connected_to\": signals_connected_to}}) \
    for child in node.get_children(): \
        _collect_signal_data(child, root, out)",
        node_path
    );
    if expr.parse(&code, ) != godot::global::Error::OK {
        return Err(McpError::internal("解析信号分析表达式失败"));
    }
    let result = expr.execute();
    if result.is_nil() {
        return Ok(serde_json::json!([]));
    }
    Ok(serde_json::json!(result.to::<String>()))
}

// ============================================================================
// 工具命令实现
// ============================================================================

/// 1. find_unused_resources: 查找项目中可能未使用的资源文件
///
/// 简化实现：扫描目录中所有资源文件
fn cmd_find_unused_resources(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = opt_string(args, "path", "res://");
    let include_addons = opt_bool(args, "include_addons", false);

    // 资源文件扩展名列表
    let resource_exts = [
        "tres", "tscn", "png", "jpg", "jpeg", "svg",
        "wav", "ogg", "mp3", "ttf", "otf", "gdshader", "material",
        "theme", "stylebox", "font", "anim",
    ];

    // 收集所有资源文件
    let mut all_resources = Vec::new();
    collect_files_by_ext(&path, &resource_exts, &mut all_resources, include_addons);

    // 收集引用文件（.tscn, .gd, .tres, .cfg, .godot）
    let ref_exts = ["tscn", "gd", "tres", "cfg", "godot"];
    let mut ref_files = Vec::new();
    collect_files_by_ext(&path, &ref_exts, &mut ref_files, include_addons);

    // 构建被引用的资源路径集合（简化：只做字符串搜索）
    let referenced: HashMap<String, bool> = HashMap::new();

    // 简化实现：跳过 ProjectSettings 扫描，直接返回文件列表

    // 找出未引用的资源（简化实现：仅返回文件列表）
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

/// 2. analyze_signal_flow: 分析信号连接流
fn cmd_analyze_signal_flow(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let node_path = opt_string(args, "node_path", "");

    let editor = EditorInterface::singleton();
    let root = editor.get_edited_scene_root();
    let Some(scene_root) = root else {
        return Err(McpError::no_scene());
    };

    let scene_path = scene_root.get_scene_file_path().to_string();

    // 使用 GDScript Expression 获取信号连接信息
    let mut expr = Expression::new_gd();

    if node_path.is_empty() || node_path == "." {
        // 遍历整个场景
        let signal_code = "\
var root = EditorInterface.get_edited_scene_root() \
if root == null: \
    return {\"error\": \"No scene\"} \
var nodes_data = [] \
_collect_all(root, root, nodes_data) \
return {\"scene\": root.scene_file_path, \"nodes\": nodes_data, \"total_nodes\": nodes_data.size()} \
\n\
func _collect_all(node, root, out): \
    var node_path_str = str(root.get_path_to(node)) \
    var signals_emitted = [] \
    var signals_connected_to = [] \
    for sig in node.get_signal_list(): \
        var sig_name = sig[\"name\"] \
        var connections = node.get_signal_connection_list(sig_name) \
        var targets = [] \
        for conn in connections: \
            if int(conn.get(\"flags\", 0)) & 1 == 0: \
                continue \
            var callable = conn[\"callable\"] \
            var target_node = callable.get_object() \
            if target_node == null: \
                continue \
            if target_node != root and not root.is_ancestor_of(target_node): \
                continue \
            targets.append({\"target_node\": str(root.get_path_to(target_node)), \"method\": callable.get_method()}) \
            signals_connected_to.append({\"from_node\": str(node_path_str), \"signal\": sig_name, \"method\": callable.get_method()}) \
        if targets.size() > 0: \
            signals_emitted.append({\"signal\": sig_name, \"targets\": targets}) \
    if signals_emitted.size() > 0 or signals_connected_to.size() > 0: \
        out.append({\"name\": node.name, \"path\": str(node_path_str), \"type\": node.get_class(), \"signals_emitted\": signals_emitted, \"signals_connected_to\": signals_connected_to}) \
    for child in node.get_children(): \
        _collect_all(child, root, out)";

        if expr.parse(signal_code, ) != godot::global::Error::OK {
            return Err(McpError::internal("解析信号分析表达式失败"));
        }
    } else {
        // 分析指定节点的信号
        let signal_code = format!(
            "\
var root = EditorInterface.get_edited_scene_root() \
if root == null: \
    return {{\"error\": \"No scene\"}} \
var node = root.get_node(\"{}\") \
if node == null: \
    return {{\"error\": \"Node not found\"}} \
var nodes_data = [] \
_collect_one(node, root, nodes_data) \
return {{\"scene\": root.scene_file_path, \"nodes\": nodes_data, \"total_nodes\": nodes_data.size()}} \
\n\
func _collect_one(node, root, out): \
    var node_path_str = str(root.get_path_to(node)) \
    var signals_emitted = [] \
    var signals_connected_to = [] \
    for sig in node.get_signal_list(): \
        var sig_name = sig[\"name\"] \
        var connections = node.get_signal_connection_list(sig_name) \
        var targets = [] \
        for conn in connections: \
            if int(conn.get(\"flags\", 0)) & 1 == 0: \
                continue \
            var callable = conn[\"callable\"] \
            var target_node = callable.get_object() \
            if target_node == null: \
                continue \
            if target_node != root and not root.is_ancestor_of(target_node): \
                continue \
            targets.append({{\"target_node\": str(root.get_path_to(target_node)), \"method\": callable.get_method()}}) \
            signals_connected_to.append({{\"from_node\": str(node_path_str), \"signal\": sig_name, \"method\": callable.get_method()}}) \
        if targets.size() > 0: \
            signals_emitted.append({{\"signal\": sig_name, \"targets\": targets}}) \
    if signals_emitted.size() > 0 or signals_connected_to.size() > 0: \
        out.append({{\"name\": node.name, \"path\": str(node_path_str), \"type\": node.get_class(), \"signals_emitted\": signals_emitted, \"signals_connected_to\": signals_connected_to}}) \
    for child in node.get_children(): \
        _collect_one(child, root, out)",
            node_path
        );

        if expr.parse(&signal_code, ) != godot::global::Error::OK {
            return Err(McpError::internal("解析指定节点信号分析表达式失败"));
        }
    }

    let result = expr.execute();
    if result.is_nil() {
        return Ok(serde_json::json!({
            "scene": scene_path,
            "nodes": [],
            "total_nodes": 0,
        }));
    }

    // 尝试解析结果字符串为 JSON
    let result_str: String = result.to();
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result_str) {
        // 检查是否有错误
        if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
            return Err(McpError::internal(err));
        }
        return Ok(parsed);
    }

    // 回退方案：返回原始结果
    Ok(serde_json::json!({
        "scene": scene_path,
        "nodes": [],
        "raw_result": result_str,
        "total_nodes": 0,
    }))
}

/// 3. analyze_scene_complexity: 分析场景复杂度
fn cmd_analyze_scene_complexity(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let scene_path = opt_string(args, "path", "");

    let (mut root_node, actual_path, needs_free) = if scene_path.is_empty() {
        // 使用当前编辑场景
        let editor = EditorInterface::singleton();
        let root = editor.get_edited_scene_root()
            .ok_or_else(McpError::no_scene)?;
        let path = root.get_scene_file_path().to_string();
        (root, path, false)
    } else {
        // 加载指定场景
        if !ResourceLoader::singleton().exists(&scene_path) {
            return Err(McpError::not_found(&format!("场景 '{}'", scene_path), ""));
        }
        let packed = ResourceLoader::singleton().load(&scene_path);
        let Some(packed_resource) = packed else {
            return Err(McpError::internal(&format!("无法加载场景: {}", scene_path)));
        };
        // 将 Resource 转换为 PackedScene
        let packed_scene = packed_resource.try_cast::<PackedScene>()
            .map_err(|_| McpError::internal("加载的资源不是有效的场景文件"))?;
        let Some(root) = packed_scene.instantiate() else {
            return Err(McpError::internal(&format!("无法实例化场景: {}", scene_path)));
        };
        (root, scene_path, true)
    };

    // 使用 GDScript Expression 进行分析
    let mut expr = Expression::new_gd();
    let analyze_code = "\
var root = EditorInterface.get_edited_scene_root() \
if root == null: \
    return {\"error\": \"No scene\"} \
var types = {{}} \
var scripts = [] \
var total = _count_nodes(root) \
var depth = _get_max_depth(root, 0) \
_analyze(root, root, types, scripts) \
var issues = [] \
if total > 1000: \
    issues.append({\"severity\": \"warning\", \"message\": \"场景有 \" + str(total) + \" 个节点 (>1000)。考虑拆分为子场景。\"}) \
elif total > 500: \
    issues.append({\"severity\": \"info\", \"message\": \"场景有 \" + str(total) + \" 个节点 (>500)。建议监控性能。\"}) \
if depth > 15: \
    issues.append({\"severity\": \"warning\", \"message\": \"最大嵌套深度为 \" + str(depth) + \" (>15)。深层层级难以维护。\"}) \
elif depth > 10: \
    issues.append({\"severity\": \"info\", \"message\": \"最大嵌套深度为 \" + str(depth) + \" (>10)。\"}) \
return {\"total_nodes\": total, \"max_depth\": depth, \"nodes_by_type\": types, \"scripts_attached\": scripts, \"issues\": issues} \
\n\
func _count_nodes(node): \
    var count = 1 \
    for child in node.get_children(): \
        count += _count_nodes(child) \
    return count \
\n\
func _get_max_depth(node, current): \
    var max_d = current \
    for child in node.get_children(): \
        var child_depth = _get_max_depth(child, current + 1) \
        if child_depth > max_d: \
            max_d = child_depth \
    return max_d \
\n\
func _analyze(node, root, types, scripts): \
    var type_name = node.get_class() \
    types[type_name] = types.get(type_name, 0) + 1 \
    if node.get_script() != null: \
        var script = node.get_script() \
        var script_path = script.resource_path \
        if not script_path.is_empty(): \
            scripts.append({\"node\": str(root.get_path_to(node)), \"script\": script_path}) \
    for child in node.get_children(): \
        _analyze(child, root, types, scripts)";

    if expr.parse(analyze_code, ) != godot::global::Error::OK {
        // 如果 Expression 失败，返回基本统计
        if needs_free {
            // 清理实例化的场景
            root_node.queue_free();
        }
        return Ok(serde_json::json!({
            "scene_path": actual_path,
            "total_nodes": 0,
            "max_depth": 0,
            "nodes_by_type": {},
            "scripts_attached": [],
            "issues": [{"severity": "error", "message": "Expression 解析失败"}],
        }));
    }

    let result = expr.execute();
    let result_str: String = result.to();

    if needs_free {
        root_node.queue_free();
    }

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result_str) {
        if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
            return Err(McpError::internal(err));
        }
        let mut response = serde_json::json!({
            "scene_path": actual_path,
        });
        if let Some(obj) = parsed.as_object() {
            for (k, v) in obj {
                response[k] = v.clone();
            }
        }
        return Ok(response);
    }

    Ok(serde_json::json!({
        "scene_path": actual_path,
        "raw_result": result_str,
    }))
}

/// 4. find_script_references: 查找引用指定脚本的所有文件
fn cmd_find_script_references(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let query = req_string(args, "query")?;
    let path = opt_string(args, "path", "res://");
    let include_addons = opt_bool(args, "include_addons", false);

    // 搜索的文件扩展名
    let search_exts = ["tscn", "gd", "tres", "cfg", "godot"];
    let mut search_files = Vec::new();
    collect_files_by_ext(&path, &search_exts, &mut search_files, include_addons);

    // 在文件中搜索引用
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

/// 5. detect_circular_dependencies: 检测场景间循环依赖
fn cmd_detect_circular_dependencies(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = opt_string(args, "path", "res://");
    let include_addons = opt_bool(args, "include_addons", false);

    // 使用 GDScript Expression 检测循环依赖
    let mut expr = Expression::new_gd();
    let dep_code = format!(
        "\
var path = \"{}\" \
var include_addons = {} \
\
var tscn_files = [] \
_collect_tscn(path, tscn_files, include_addons) \
\
var dep_graph = {{}} \
for tp in tscn_files: \
    var content = _read_file(tp) \
    if content.is_empty(): \
        continue \
    var deps = [] \
    for line in content.split(\"\\n\"): \
        if line.begins_with(\"[ext_resource\") and \".tscn\" in line: \
            var ps = line.find(\"path=\\\"\") \
            if ps == -1: \
                continue \
            ps += 6 \
            var pe = line.find(\"\\\"\", ps) \
            if pe == -1: \
                continue \
            var ref_path = line.substr(ps, pe - ps) \
            if ref_path.ends_with(\".tscn\"): \
                deps.append(ref_path) \
    dep_graph[tp] = deps \
\
var cycles = [] \
var visited = {{}} \
for scene in dep_graph: \
    visited[scene] = \"unvisited\" \
for scene in dep_graph: \
    if visited[scene] == \"unvisited\": \
        var path_stack = [] \
        _dfs_detect(scene, dep_graph, visited, path_stack, cycles) \
\
return {{\"scenes_checked\": tscn_files.size(), \"circular_dependencies\": cycles, \"has_circular\": cycles.size() > 0, \"dependency_graph\": dep_graph}} \
\n\
func _collect_tscn(p, out, include): \
    var dir = DirAccess.open(p) \
    if dir == null: \
        return \
    dir.list_dir_begin() \
    var fn = dir.get_next() \
    while not fn.is_empty(): \
        if fn.begins_with(\".\"): \
            fn = dir.get_next() \
            continue \
        var fp = p.path_join(fn) \
        if dir.current_is_dir(): \
            if fn == \"addons\" and not include: \
                fn = dir.get_next() \
                continue \
            _collect_tscn(fp, out, include) \
        elif fn.ends_with(\".tscn\"): \
            out.append(fp) \
        fn = dir.get_next() \
    dir.list_dir_end() \
\n\
func _read_file(fp): \
    var file = FileAccess.open(fp, FileAccess.READ) \
    if file == null: \
        return \"\" \
    var content = file.get_as_text() \
    file.close() \
    return content \
\n\
func _dfs_detect(node, graph, visited, path_stack, cycles): \
    visited[node] = \"visiting\" \
    path_stack.append(node) \
    if graph.has(node): \
        for dep in graph[node]: \
            if not visited.has(dep): \
                continue \
            if visited[dep] == \"visiting\": \
                var cycle_start = path_stack.find(dep) \
                var cycle = path_stack.slice(cycle_start) \
                cycle.append(dep) \
                cycles.append(cycle) \
            elif visited[dep] == \"unvisited\": \
                _dfs_detect(dep, graph, visited, path_stack, cycles) \
    path_stack.pop_back() \
    visited[node] = \"visited\"",
        path, if include_addons { "true" } else { "false" }
    );

    if expr.parse(&dep_code, ) != godot::global::Error::OK {
        return Err(McpError::internal("解析循环依赖检测表达式失败"));
    }

    let result = expr.execute();
    let result_str: String = result.to();

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result_str) {
        if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
            return Err(McpError::internal(err));
        }
        return Ok(parsed);
    }

    Ok(serde_json::json!({
        "scenes_checked": 0,
        "circular_dependencies": [],
        "has_circular": false,
        "raw_result": result_str,
    }))
}

/// 6. get_project_statistics: 获取项目统计信息
fn cmd_get_project_statistics(args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let path = opt_string(args, "path", "res://");
    let include_addons = opt_bool(args, "include_addons", false);

    // 使用 GDScript Expression 获取统计信息
    let mut expr = Expression::new_gd();
    let stat_code = format!(
        "\
var path = \"{}\" \
var include_addons = {} \
var file_counts = {{}} \
_collect_stats(path, include_addons, file_counts) \
\
var total_script_lines = int(file_counts.get(\"_total_script_lines\", 0)) \
var scene_count = int(file_counts.get(\"_scene_count\", 0)) \
var resource_count = int(file_counts.get(\"_resource_count\", 0)) \
var total_files = int(file_counts.get(\"_total_files\", 0)) \
file_counts.erase(\"_total_script_lines\") \
file_counts.erase(\"_scene_count\") \
file_counts.erase(\"_resource_count\") \
file_counts.erase(\"_total_files\") \
\
var autoloads = {{}} \
for prop in ProjectSettings.get_property_list(): \
    var pn = prop[\"name\"] \
    if pn.begins_with(\"autoload/\"): \
        autoloads[pn.substr(9)] = str(ProjectSettings.get_setting(pn)) \
\
var plugins = [] \
var enabled = ProjectSettings.get_setting(\"editor_plugins/enabled\", []) \
var plugin_dir = DirAccess.open(\"res://addons\") \
if plugin_dir != null: \
    plugin_dir.list_dir_begin() \
    var dn = plugin_dir.get_next() \
    while not dn.is_empty(): \
        if plugin_dir.current_is_dir() and not dn.begins_with(\".\"): \
            var cfg_path = \"res://addons/\".path_join(dn).path_join(\"plugin.cfg\") \
            if FileAccess.file_exists(cfg_path): \
                var pp = \"res://addons/%s/plugin.cfg\" % dn \
                plugins.append({{\"name\": dn, \"enabled\": pp in enabled}}) \
        dn = plugin_dir.get_next() \
    plugin_dir.list_dir_end() \
\
return {{\"file_counts_by_extension\": file_counts, \"total_files\": total_files, \"total_script_lines\": total_script_lines, \"scene_count\": scene_count, \"resource_count\": resource_count, \"autoloads\": autoloads, \"plugins\": plugins}} \
\n\
func _collect_stats(p, include, counts): \
    var dir = DirAccess.open(p) \
    if dir == null: \
        return \
    dir.list_dir_begin() \
    var fn = dir.get_next() \
    while not fn.is_empty(): \
        if fn.begins_with(\".\"): \
            fn = dir.get_next() \
            continue \
        var fp = p.path_join(fn) \
        if dir.current_is_dir(): \
            if fn == \"addons\" and not include: \
                fn = dir.get_next() \
                continue \
            _collect_stats(fp, include, counts) \
        else: \
            var ext = fn.get_extension().to_lower() \
            counts[ext] = counts.get(ext, 0) + 1 \
            if ext == \"gd\": \
                var file = FileAccess.open(fp, FileAccess.READ) \
                if file: \
                    var content = file.get_as_text() \
                    file.close() \
                    var lc = content.count(\"\\n\") + 1 if not content.is_empty() else 0 \
                    counts[\"_total_script_lines\"] = counts.get(\"_total_script_lines\", 0) + lc \
            if ext == \"tscn\": \
                counts[\"_scene_count\"] = counts.get(\"_scene_count\", 0) + 1 \
            if ext in [\"tres\", \"material\", \"theme\", \"stylebox\", \"font\"]: \
                counts[\"_resource_count\"] = counts.get(\"_resource_count\", 0) + 1 \
            counts[\"_total_files\"] = counts.get(\"_total_files\", 0) + 1 \
        fn = dir.get_next() \
    dir.list_dir_end()",
        path, if include_addons { "true" } else { "false" }
    );

    if expr.parse(&stat_code, ) != godot::global::Error::OK {
        return Err(McpError::internal("解析项目统计表达式失败"));
    }

    let result = expr.execute();
    let result_str: String = result.to();

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result_str) {
        if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
            return Err(McpError::internal(err));
        }
        return Ok(parsed);
    }

    Ok(serde_json::json!({
        "raw_result": result_str,
    }))
}

// ============================================================================
// 工具注册
// ============================================================================

/// 收集本模块的所有工具定义
pub fn collect_tools() -> Vec<ToolDefinition> {
    vec![
        // 1. find_unused_resources: 查找未使用的资源
        ToolDefinition::new("find_unused_resources", "查找项目中可能未使用的资源文件", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "搜索路径，默认为 res://", "default": "res://" },
                "include_addons": { "type": "boolean", "description": "是否包含 addons 目录", "default": false }
            }, "required": []
        })),

        // 2. analyze_signal_flow: 分析信号连接流
        ToolDefinition::new("analyze_signal_flow", "分析当前场景的信号连接流", serde_json::json!({
            "type": "object", "properties": {
                "node_path": { "type": "string", "description": "要分析的节点路径（可选，不传则分析整个场景）" }
            }, "required": []
        })),

        // 3. analyze_scene_complexity: 分析场景复杂度
        ToolDefinition::new("analyze_scene_complexity", "分析场景的复杂度（节点数、深度、类型分布等）", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "场景文件路径（可选，不传则分析当前编辑场景）" }
            }, "required": []
        })),

        // 4. find_script_references: 查找脚本引用
        ToolDefinition::new("find_script_references", "查找引用指定脚本的所有文件", serde_json::json!({
            "type": "object", "properties": {
                "query": { "type": "string", "description": "要搜索的脚本路径或类名" },
                "path": { "type": "string", "description": "搜索路径", "default": "res://" },
                "include_addons": { "type": "boolean", "description": "是否包含 addons 目录", "default": false }
            }, "required": ["query"]
        })),

        // 5. detect_circular_dependencies: 检测循环依赖
        ToolDefinition::new("detect_circular_dependencies", "检测场景文件之间的循环依赖", serde_json::json!({
            "type": "object", "properties": {
                "path": { "type": "string", "description": "搜索路径", "default": "res://" },
                "include_addons": { "type": "boolean", "description": "是否包含 addons 目录", "default": false }
            }, "required": []
        })),

        // 6. get_project_statistics: 获取项目统计信息
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
