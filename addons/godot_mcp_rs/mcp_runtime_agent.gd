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
		_:
			_write_response({"error": "Unknown command: %s" % command})


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


func _write_response(data: Dictionary) -> void:
	var json := JSON.stringify(data)
	var file := FileAccess.open(RESPONSE_PATH, FileAccess.WRITE)
	if file:
		file.store_string(json)
		file.close()
