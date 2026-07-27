@tool
extends EditorPlugin

## Godot MCP RS 插件激活脚本
##
## GDExtension 加载后将 RustMcpPlugin 类注册到 Godot 的 ClassDB。
## 本脚本负责实例化 Rust 插件并驱动其 _process 轮询。

var _rust_plugin: EditorPlugin = null

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

	print("[MCP-RS] RustMcpPlugin 实例化成功")
	# 手动触发 _enter_tree（初始化 TCP 服务器和通道）
	# 由于 EditorPlugin 不在场景树中，_process 不会自动调用
	_rust_plugin._enter_tree()

func _process(delta: float) -> void:
	# GDScript EditorPlugin 在场景树中，_process 会被引擎每帧调用
	# 手动转发到 Rust 插件的 _process 处理 MCP 请求
	if _rust_plugin != null and _rust_plugin.has_method("_process"):
		_rust_plugin._process(delta)

func _exit_tree() -> void:
	if _rust_plugin != null:
		if _rust_plugin.has_method("_exit_tree"):
			_rust_plugin._exit_tree()
		_rust_plugin.queue_free()
		_rust_plugin = null
