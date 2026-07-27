//! MCP 协议处理器
//!
//! 处理 MCP 协议的初始化、工具列表、工具调用等核心交互。
//! 所有与 Godot API 交互的部分通过 EditorInterface 完成。
//!
//! 注意：此模块在主线程上下文执行，可以安全调用 Godot API。

// 当前 MCP 协议处理逻辑已内联在 plugin.rs 的 handle_mcp_message 中。
// 随着工具数量增长，可将协议级处理 (initialize, list_tools) 和
// 工具级处理 (call_tool 分发) 分离到此模块。

// 预留：待需要时扩展
