@tool
## Godot MCP RS 运行时代理
## 在游戏进程中作为 Autoload 运行，通过文件 IPC 与 GDExtension 插件通信。
## 收到请求后执行命令，结果写入响应文件。
extends Node

const REQUEST_PATH := "user://mcp_game_request"
const RESPONSE_PATH := "user://mcp_game_response"

func _ready() -> void:
	process_mode = Node.PROCESS_MODE_ALWAYS


func _process(_delta: float) -> void:
	if FileAccess.file_exists(REQUEST_PATH):
		_handle_request()


func _handle_request() -> void:
	var file := FileAccess.open(REQUEST_PATH, FileAccess.READ)
	if file == null:
		return
	var text := file.get_as_text()
	file.close()
	DirAccess.remove_absolute(REQUEST_PATH)

	var parsed = JSON.parse_string(text)
	if parsed == null or not parsed is Dictionary:
		_write_response({"error": "Invalid request JSON"})
		return

	var command: String = parsed.get("command", "")
	var params: Dictionary = parsed.get("params", {})

	match command:
		"get_scene_tree":
			_cmd_get_scene_tree(params)
		"get_node_properties":
			_cmd_get_node_properties(params)
		"set_node_property":
			_cmd_set_node_property(params)
		"capture_frames":
			_cmd_capture_frames(params)
		"monitor_properties":
			_cmd_monitor_properties(params)
		"execute_script":
			_cmd_execute_script(params)
		"start_recording":
			_cmd_start_recording(params)
		"stop_recording":
			_cmd_stop_recording(params)
		"replay_recording":
			_cmd_replay_recording(params)
		"find_nodes_by_script":
			_cmd_find_nodes_by_script(params)
		"get_autoload":
			_cmd_get_autoload(params)
		"batch_get_properties":
			_cmd_batch_get_properties(params)
		"find_ui_elements":
			_cmd_find_ui_elements(params)
		"click_button_by_text":
			_cmd_click_button_by_text(params)
		"wait_for_node":
			_cmd_wait_for_node(params)
		"find_nearby_nodes":
			_cmd_find_nearby_nodes(params)
		"navigate_to":
			_cmd_navigate_to(params)
		"move_to":
			_cmd_move_to(params)
		"watch_signals":
			_cmd_watch_signals(params)
		_:
			_write_response({"error": "Unknown command: %s" % command})


# ============================================================================
# 1. get_scene_tree - 获取场景树
# ============================================================================

func _cmd_get_scene_tree(params: Dictionary) -> void:
	var root := get_tree().current_scene
	if root == null:
		_write_response({"error": "No current scene"})
		return
	var max_depth: int = params.get("max_depth", -1)
	var tree := _build_tree(root, max_depth, 0)
	_write_response({"tree": tree})


func _build_tree(node: Node, max_depth: int, depth: int) -> Dictionary:
	var result := {
		"name": node.name,
		"type": node.get_class(),
		"path": str(node.get_path()),
	}
	var script: Script = node.get_script()
	if script:
		result["script"] = script.resource_path
	if max_depth == -1 or depth < max_depth:
		var children: Array = []
		for child in node.get_children():
			children.append(_build_tree(child, max_depth, depth + 1))
		if not children.is_empty():
			result["children"] = children
	return result


# ============================================================================
# 2. get_node_properties - 获取节点属性
# ============================================================================

func _cmd_get_node_properties(params: Dictionary) -> void:
	var node_path: String = params.get("node_path", "")
	var node := _find_node(node_path)
	if node == null:
		_write_response({"error": "Node not found: %s" % node_path})
		return
	
	var prop_names: Array = params.get("properties", [])
	var result := {"node_path": node_path, "type": node.get_class(), "name": node.name}
	
	if prop_names.is_empty():
		# 返回所有属性
		for prop in node.get_property_list():
			var pname: String = prop.get("name", "")
			if pname.is_empty():
				continue
			result[pname] = _safe_get(node, pname)
	else:
		for p in prop_names:
			result[str(p)] = _safe_get(node, str(p))
	
	_write_response(result)


# ============================================================================
# 3. set_node_property - 设置节点属性
# ============================================================================

func _cmd_set_node_property(params: Dictionary) -> void:
	var node_path: String = params.get("node_path", "")
	var node := _find_node(node_path)
	if node == null:
		_write_response({"error": "Node not found: %s" % node_path})
		return
	
	var property: String = params.get("property", "")
	if property.is_empty():
		_write_response({"error": "Missing property name"})
		return
	
	var value = params.get("value")
	if value == null:
		_write_response({"error": "Missing value"})
		return
	
	node.set(property, value)
	_write_response({"node_path": node_path, "property": property, "set": true})


# ============================================================================
# 4. capture_frames - 连续截取游戏帧
# ============================================================================

func _cmd_capture_frames(params: Dictionary) -> void:
	var count: int = params.get("count", 5)
	var frame_interval: int = params.get("frame_interval", 10)
	var half_resolution: bool = params.get("half_resolution", true)
	
	var viewport := get_tree().root
	var frames: Array = []
	
	for i in range(count):
		# 等待指定帧数
		for _f in range(frame_interval):
			await get_tree().process_frame
		
		var img := viewport.get_texture().get_image()
		if half_resolution:
			img.resize(img.get_width() / 2, img.get_height() / 2, Image.INTERPOLATE_LANCZOS)
		
		# 编码为 PNG base64
		var png_data := img.save_png_to_buffer()
		var b64 := Marshalls.raw_to_base64(png_data)
		frames.append({
			"index": i,
			"width": img.get_width(),
			"height": img.get_height(),
			"image_base64": b64,
		})
	
	_write_response({"frames": frames, "count": frames.size()})


# ============================================================================
# 5. monitor_properties - 监控节点属性变化
# ============================================================================

func _cmd_monitor_properties(params: Dictionary) -> void:
	var node_path: String = params.get("node_path", "")
	var prop_names: Array = params.get("properties", [])
	var frame_count: int = params.get("frame_count", 60)
	var frame_interval: int = params.get("frame_interval", 1)
	
	var node := _find_node(node_path)
	if node == null:
		_write_response({"error": "Node not found: %s" % node_path})
		return
	
	var samples: Array = []
	
	for i in range(frame_count):
		var sample := {"frame": i}
		for prop_name in prop_names:
			sample[str(prop_name)] = _safe_get(node, str(prop_name))
		samples.append(sample)
		
		for _f in range(frame_interval):
			await get_tree().process_frame
	
	_write_response({"node_path": node_path, "samples": samples, "frame_count": frame_count})


# ============================================================================
# 6. execute_script - 执行 GDScript 代码
# ============================================================================

func _cmd_execute_script(params: Dictionary) -> void:
	var code: String = params.get("code", "")
	if code.is_empty():
		_write_response({"error": "Missing code parameter"})
		return
	
	# Expression 不支持 return 关键字，自动剥离
	code = code.strip_edges()
	if code.begins_with("return "):
		code = code.substr(7)
	
	var expression := Expression.new()
	var err := expression.parse(code)
	if err != OK:
		_write_response({"error": "Parse error: %s" % error_string(err), "error_code": err})
		return
	
	# show_error=false 避免 Expression 错误在编辑器弹窗
	var result = expression.execute([], self, false)
	if expression.has_execute_failed():
		_write_response({"error": "Execute error: %s" % expression.get_error_text(), "result": str(result)})
		return
	
	_write_response({"result": str(result)})


# ============================================================================
# 7. start_recording - 开始录制输入
# ============================================================================

var _recording := false
var _recorded_events: Array = []
var _record_start_time: float = 0.0

func _input(event: InputEvent) -> void:
	if not _recording:
		return
	
	var event_data := {
		"time": Time.get_ticks_msec() / 1000.0 - _record_start_time,
	}
	
	if event is InputEventKey:
		event_data["type"] = "key"
		event_data["keycode"] = event.keycode
		event_data["pressed"] = event.pressed
	elif event is InputEventMouseButton:
		event_data["type"] = "mouse_button"
		event_data["button_index"] = event.button_index
		event_data["pressed"] = event.pressed
		event_data["position"] = {"x": event.position.x, "y": event.position.y}
	elif event is InputEventMouseMotion:
		event_data["type"] = "mouse_motion"
		event_data["position"] = {"x": event.position.x, "y": event.position.y}
		event_data["relative"] = {"x": event.relative.x, "y": event.relative.y}
	else:
		event_data["type"] = "other"
		event_data["as_text"] = event.as_text()
	
	_recorded_events.append(event_data)


func _cmd_start_recording(_params: Dictionary) -> void:
	_recording = true
	_recorded_events.clear()
	_record_start_time = Time.get_ticks_msec() / 1000.0
	_write_response({"recording": true, "start_time": _record_start_time})


func _cmd_stop_recording(_params: Dictionary) -> void:
	_recording = false
	_write_response({"events": _recorded_events, "count": _recorded_events.size(), "duration": Time.get_ticks_msec() / 1000.0 - _record_start_time})


func _cmd_replay_recording(params: Dictionary) -> void:
	var events: Array = params.get("events", [])
	var speed: float = params.get("speed", 1.0)
	
	if events.is_empty():
		_write_response({"error": "No events to replay"})
		return
	
	var start_time := Time.get_ticks_msec() / 1000.0
	var replayed := 0
	
	for event_data in events:
		var target_time: float = event_data.get("time", 0.0) / speed
		var elapsed := Time.get_ticks_msec() / 1000.0 - start_time
		if target_time > elapsed:
			await get_tree().create_timer(target_time - elapsed).timeout
		
		var event := InputEventAction.new()
		event.action = "replay_event"
		event.pressed = true
		Input.parse_input_event(event)
		replayed += 1
	
	_write_response({"replayed": replayed, "total": events.size()})


# ============================================================================
# 10. find_nodes_by_script - 按脚本查找节点
# ============================================================================

func _cmd_find_nodes_by_script(params: Dictionary) -> void:
	var script_path: String = params.get("script", "")
	var prop_names: Array = params.get("properties", [])
	
	var root := get_tree().current_scene
	if root == null:
		_write_response({"error": "No current scene"})
		return
	
	var matches: Array = []
	_find_by_script_recursive(root, script_path, prop_names, matches)
	_write_response({"nodes": matches, "count": matches.size()})


func _find_by_script_recursive(node: Node, script_path: String, prop_names: Array, matches: Array) -> void:
	var script: Script = node.get_script()
	if script and script.resource_path == script_path:
		var info := {"name": node.name, "path": str(node.get_path()), "type": node.get_class()}
		for p in prop_names:
			info[str(p)] = _safe_get(node, str(p))
		matches.append(info)
	
	for child in node.get_children():
		_find_by_script_recursive(child, script_path, prop_names, matches)


# ============================================================================
# 11. get_autoload - 获取自动加载节点
# ============================================================================

func _cmd_get_autoload(params: Dictionary) -> void:
	var name: String = params.get("name", "")
	var prop_names: Array = params.get("properties", [])
	
	var node := get_node_or_null("/root/%s" % name)
	if node == null:
		_write_response({"error": "Autoload not found: %s" % name})
		return
	
	var result := {"name": name, "type": node.get_class(), "path": str(node.get_path())}
	for p in prop_names:
		result[str(p)] = _safe_get(node, str(p))
	
	_write_response(result)


# ============================================================================
# 12. batch_get_properties - 批量获取节点属性
# ============================================================================

func _cmd_batch_get_properties(params: Dictionary) -> void:
	var nodes: Array = params.get("nodes", [])
	var results: Array = []
	
	for node_info in nodes:
		var node_path: String = node_info.get("node_path", "")
		var prop_names: Array = node_info.get("properties", [])
		var node := _find_node(node_path)
		
		if node == null:
			results.append({"node_path": node_path, "error": "Not found"})
			continue
		
		var entry := {"node_path": node_path, "type": node.get_class(), "name": node.name}
		for p in prop_names:
			entry[str(p)] = _safe_get(node, str(p))
		results.append(entry)
	
	_write_response({"results": results, "count": results.size()})


# ============================================================================
# 13. find_ui_elements - 查找 UI 元素
# ============================================================================

func _cmd_find_ui_elements(params: Dictionary) -> void:
	var type_filter: String = params.get("type_filter", "")
	
	var root := get_tree().current_scene
	if root == null:
		_write_response({"error": "No current scene"})
		return
	
	var elements: Array = []
	_find_ui_recursive(root, type_filter, elements)
	_write_response({"elements": elements, "count": elements.size()})


func _find_ui_recursive(node: Node, type_filter: String, elements: Array) -> void:
	if node is Control:
		if type_filter.is_empty() or node.is_class(type_filter):
			elements.append({"name": node.name, "path": str(node.get_path()), "type": node.get_class()})
	
	for child in node.get_children():
		_find_ui_recursive(child, type_filter, elements)


# ============================================================================
# 14. click_button_by_text - 通过文本点击按钮
# ============================================================================

func _cmd_click_button_by_text(params: Dictionary) -> void:
	var text: String = params.get("text", "")
	var partial: bool = params.get("partial", true)
	
	var root := get_tree().current_scene
	if root == null:
		_write_response({"error": "No current scene"})
		return
	
	var button := _find_button_by_text(root, text, partial)
	if button == null:
		_write_response({"error": "Button with text '%s' not found" % text})
		return
	
	# 模拟按钮点击
	if button.has_signal("pressed"):
		button.emit_signal("pressed")
	if button.has_method("_pressed"):
		button._pressed()
	
	_write_response({"clicked": true, "button_path": str(button.get_path()), "button_text": button.text})


func _find_button_by_text(node: Node, text: String, partial: bool) -> Node:
	if node is BaseButton:
		var btn_text: String = node.text if "text" in node else ""
		if partial and text in btn_text:
			return node
		elif not partial and btn_text == text:
			return node
	
	for child in node.get_children():
		var found := _find_button_by_text(child, text, partial)
		if found:
			return found
	
	return null


# ============================================================================
# 15. wait_for_node - 等待节点出现
# ============================================================================

func _cmd_wait_for_node(params: Dictionary) -> void:
	var node_path: String = params.get("node_path", "")
	var timeout: float = params.get("timeout", 5.0)
	var poll_frames: int = params.get("poll_frames", 5)
	
	var start_time := Time.get_ticks_msec() / 1000.0
	
	while Time.get_ticks_msec() / 1000.0 - start_time < timeout:
		var node := _find_node(node_path)
		if node != null:
			_write_response({"found": true, "node_path": node_path, "type": node.get_class(), "name": node.name})
			return
		
		for _f in range(poll_frames):
			await get_tree().process_frame
	
	_write_response({"found": false, "node_path": node_path, "timeout": true})


# ============================================================================
# 16. find_nearby_nodes - 查找附近节点
# ============================================================================

func _cmd_find_nearby_nodes(params: Dictionary) -> void:
	var position: Dictionary = params.get("position", {})
	var radius: float = params.get("radius", 100.0)
	var type_filter: String = params.get("type_filter", "")
	var group_filter: String = params.get("group_filter", "")
	var max_results: int = params.get("max_results", 50)
	
	var root := get_tree().current_scene
	if root == null:
		_write_response({"error": "No current scene"})
		return
	
	var target_pos := Vector2(position.get("x", 0.0), position.get("y", 0.0))
	var results: Array = []
	_find_nearby_recursive(root, target_pos, radius, type_filter, group_filter, max_results, results)
	
	results.sort_custom(func(a, b): return a.distance < b.distance)
	if results.size() > max_results:
		results = results.slice(0, max_results)
	
	_write_response({"nodes": results, "count": results.size()})


func _find_nearby_recursive(node: Node, pos: Vector2, radius: float, type_filter: String, group_filter: String, max_results: int, results: Array) -> void:
	if results.size() >= max_results:
		return
	
	var skip := false
	if not type_filter.is_empty() and not node.is_class(type_filter):
		skip = true
	if not group_filter.is_empty() and not node.is_in_group(group_filter):
		skip = true
	
	if not skip:
		var node_pos := Vector2.ZERO
		if "global_position" in node:
			node_pos = node.global_position
		elif "position" in node:
			node_pos = node.position
		
		var distance := pos.distance_to(node_pos)
		if distance <= radius:
			results.append({"name": node.name, "path": str(node.get_path()), "type": node.get_class(), "distance": distance})
	
	for child in node.get_children():
		_find_nearby_recursive(child, pos, radius, type_filter, group_filter, max_results, results)


# ============================================================================
# 17. navigate_to - 导航到目标（使用导航网格）
# ============================================================================

func _cmd_navigate_to(_params: Dictionary) -> void:
	_write_response({"error": "navigate_to 需要项目中配置 NavigationAgent 和导航网格。请在游戏脚本中使用 NavigationAgent2D/3D 实现移动。"})


# ============================================================================
# 18. move_to - 直接移动到目标
# ============================================================================

func _cmd_move_to(params: Dictionary) -> void:
	var target = params.get("target", {})
	var player_path: String = params.get("player_path", "")
	var move_speed: float = params.get("move_speed", 200.0)
	var arrival_radius: float = params.get("arrival_radius", 10.0)
	var timeout: float = params.get("timeout", 15.0)
	var look_at_target: bool = params.get("look_at_target", false)
	
	var player: Node
	if not player_path.is_empty():
		player = _find_node(player_path)
	else:
		# 尝试查找常见的玩家节点
		player = _find_node("Player")
		if player == null:
			player = _find_node("CharacterBody2D")
		if player == null:
			player = _find_node("CharacterBody3D")
	
	if player == null:
		_write_response({"error": "未找到玩家节点。请指定 player_path 参数指向场景中的可移动角色节点。"})
		return
	
	var target_pos: Vector2
	if target is String:
		var target_node := _find_node(target)
		if target_node and "global_position" in target_node:
			target_pos = target_node.global_position
		elif target_node and "position" in target_node:
			target_pos = target_node.position
		else:
			_write_response({"error": "目标节点未找到或没有位置属性: %s" % target})
			return
	elif target is Dictionary:
		target_pos = Vector2(target.get("x", 0.0), target.get("y", 0.0))
	else:
		_write_response({"error": "无效的目标参数"})
		return
	
	if not "position" in player and not "global_position" in player:
		_write_response({"error": "玩家节点没有 position 属性"})
		return
	
	var start_time := Time.get_ticks_msec() / 1000.0
	var start_pos: Vector2 = player.global_position if "global_position" in player else player.position
	
	while Time.get_ticks_msec() / 1000.0 - start_time < timeout:
		var current_pos: Vector2 = player.global_position if "global_position" in player else player.position
		
		if current_pos.distance_to(target_pos) <= arrival_radius:
			_write_response({"reached": true, "node_path": str(player.get_path()), "distance_traveled": start_pos.distance_to(current_pos), "time": Time.get_ticks_msec() / 1000.0 - start_time})
			return
		
		var direction := (target_pos - current_pos).normalized()
		var new_pos := current_pos + direction * move_speed * get_process_delta_time()
		player.position = new_pos
		
		if look_at_target:
			player.rotation = direction.angle()
		
		await get_tree().process_frame
	
	_write_response({"reached": false, "timeout": true, "node_path": str(player.get_path())})


# ============================================================================
# 19. watch_signals - 监听信号
# ============================================================================

var _watched_signals: Dictionary = {}

func _cmd_watch_signals(params: Dictionary) -> void:
	var node_paths: Array = params.get("node_paths", [])
	var signal_filter: Array = params.get("signal_filter", [])
	var duration_ms: int = params.get("duration_ms", 5000)
	
	# 清空之前的监听
	_watched_signals.clear()
	
	var emitted: Array = []
	
	# 为每个节点连接信号
	for np in node_paths:
		var node := _find_node(str(np))
		if node == null:
			continue
		
		for sig in node.get_signal_list():
			var sig_name: String = sig.get("name", "")
			if sig_name.is_empty():
				continue
			# 跳过内置信号
			if sig_name.begins_with("_"):
				continue
			if not signal_filter.is_empty() and not signal_filter.has(sig_name):
				continue
			
			# 用闭包捕获信号参数
			var _callback := func(args = null):
				var entry := {
					"signal": sig_name,
					"node": str(node.get_path()),
					"time": Time.get_ticks_msec() / 1000.0,
				}
				if args != null and args is Array:
					entry["args"] = str(args)
				emitted.append(entry)
			
			# 连接信号（不绑定参数）
			var conn_err := node.connect(sig_name, Callable.create(_callback, "call"))
			if conn_err == OK:
				_watched_signals[str(node.get_path()) + ":" + sig_name] = {"node": node, "signal": sig_name}
	
	# 等待指定时间
	await get_tree().create_timer(duration_ms / 1000.0).timeout
	
	# 断开所有连接
	for key in _watched_signals:
		var info = _watched_signals[key]
		info.node.disconnect(info.signal, Callable.create(func(): pass, "call"))
	_watched_signals.clear()
	
	_write_response({"emitted": emitted, "count": emitted.size(), "duration_ms": duration_ms})


# ============================================================================
# 辅助函数
# ============================================================================

func _find_node(node_path: String) -> Node:
	var root := get_tree().current_scene
	if root == null:
		return null
	if node_path == "." or node_path.is_empty():
		return root
	# 尝试从场景根解析路径
	if node_path.begins_with("/root/"):
		return get_node_or_null(node_path)
	# 相对路径
	var node := root.get_node_or_null(node_path)
	if node:
		return node
	# 递归搜索（不区分大小写的名称匹配）
	return _find_by_name_recursive(root, node_path)


func _find_by_name_recursive(node: Node, name: String) -> Node:
	if node.name == name:
		return node
	for child in node.get_children():
		var found := _find_by_name_recursive(child, name)
		if found:
			return found
	return null


func _safe_get(node: Node, property: String):
	# 安全获取属性值，避免不存在时崩溃
	var value = node.get(property)
	if value == null:
		return null
	# 对常见类型做串行化处理
	if value is Vector2 or value is Vector2i:
		return {"x": value.x, "y": value.y}
	if value is Vector3 or value is Vector3i:
		return {"x": value.x, "y": value.y, "z": value.z}
	if value is Color:
		return {"r": value.r, "g": value.g, "b": value.b, "a": value.a, "html": value.to_html()}
	if value is Transform2D:
		return {"x": {"x": value.x.x, "y": value.x.y}, "y": {"x": value.y.x, "y": value.y.y}, "origin": {"x": value.origin.x, "y": value.origin.y}}
	if value is Transform3D:
		return {"basis": {"x": {"x": value.basis.x.x, "y": value.basis.x.y, "z": value.basis.x.z}, "y": {"x": value.basis.y.x, "y": value.basis.y.y, "z": value.basis.y.z}, "z": {"x": value.basis.z.x, "y": value.basis.z.y, "z": value.basis.z.z}}, "origin": {"x": value.origin.x, "y": value.origin.y, "z": value.origin.z}}
	if value is String or value is StringName:
		return str(value)
	if value is int or value is float or value is bool:
		return value
	if value is Object:
		return {"instance_id": value.get_instance_id(), "class": value.get_class()}
	return str(value)


func _write_response(data: Dictionary) -> void:
	var json := JSON.stringify(data)
	var file := FileAccess.open(RESPONSE_PATH, FileAccess.WRITE)
	if file:
		file.store_string(json)
		file.close()
