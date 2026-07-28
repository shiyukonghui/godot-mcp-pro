@tool
extends EditorPlugin

## Godot MCP RS 插件激活脚本
##
## GDExtension 加载后将 RustMcpPlugin 类注册到 Godot 的 ClassDB。
## 本脚本负责实例化 Rust 插件并驱动其 _process 轮询。

var _rust_plugin: EditorPlugin = null
var _debug_poll_reported := false

#region debug-point A:report-instance
func _debug_report(hypothesis_id: String, location: String, message: String, data: Dictionary) -> void:
	var request := HTTPRequest.new()
	add_child(request)
	request.request_completed.connect(func(_result, _response_code, _headers, _body): request.queue_free())
	var event := {"sessionId": "rust-self-null", "runId": "pre-fix", "hypothesisId": hypothesis_id, "location": location, "msg": "[DEBUG] " + message, "data": data, "ts": Time.get_ticks_msec()}
	request.request("http://127.0.0.1:7777/event", ["Content-Type: application/json"], HTTPClient.METHOD_POST, JSON.stringify(event))
#endregion

func _enter_tree() -> void:
	if _rust_plugin != null:
		return
	if not ClassDB.class_exists("RustMcpPlugin"):
		push_error("[MCP-RS] GDExtension 未注册 RustMcpPlugin 类")
		return

	_rust_plugin = ClassDB.instantiate("RustMcpPlugin")
	if _rust_plugin == null:
		push_error("[MCP-RS] 无法实例化 RustMcpPlugin")
		return

	#region debug-point A:after-instantiate
	_debug_report("A", "plugin.gd:after-instantiate", "Rust 插件实例化结果", {"is_valid": is_instance_valid(_rust_plugin), "class": _rust_plugin.get_class(), "instance_id": _rust_plugin.get_instance_id(), "has_enter_tree": _rust_plugin.has_method("_enter_tree"), "has_poll_mcp": _rust_plugin.has_method("poll_mcp")})
	#endregion

	print("[MCP-RS] RustMcpPlugin 实例化成功")
	# 手动触发 _enter_tree（初始化 TCP 服务器和通道）
	# 由于 EditorPlugin 不在场景树中，_process 不会自动调用
	_rust_plugin._enter_tree()

func _process(delta: float) -> void:
	# GDScript EditorPlugin 在场景树中，_process 会被引擎每帧调用
	# 手动转发到 Rust 插件的 poll_mcp 处理 MCP 请求
	if _rust_plugin != null and _rust_plugin.has_method("poll_mcp"):
		#region debug-point B:before-poll
		if not _debug_poll_reported:
			_debug_poll_reported = true
			_debug_report("B", "plugin.gd:before-poll", "调用 poll_mcp 前的实例与参数状态", {"is_valid": is_instance_valid(_rust_plugin), "class": _rust_plugin.get_class(), "instance_id": _rust_plugin.get_instance_id(), "delta_type": typeof(delta), "delta": delta})
		#endregion
		_rust_plugin.poll_mcp(delta)

func _exit_tree() -> void:
	if _rust_plugin != null:
		if _rust_plugin.has_method("_exit_tree"):
			_rust_plugin._exit_tree()
		_rust_plugin.queue_free()
		_rust_plugin = null
