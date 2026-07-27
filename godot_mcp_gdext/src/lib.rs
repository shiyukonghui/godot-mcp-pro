//! Godot MCP RS — Rust GDExtension 实现的全栈 MCP 服务
//!
//! 架构说明：
//! - 作为 Godot EditorPlugin 加载，在 Godot 进程内部运行
//! - 启动 TCP 服务器监听 9876 端口，接收 MCP 协议的 JSON-RPC 请求
//! - 通过 GDExtension API 直接调用编辑器功能，无需外部进程
//!
//! 通信路径：
//!   AI Client → mcp_bridge(stdin) → TCP → GDExtension → EditorInterface

use godot::prelude::*;

mod plugin;
mod mcp;
mod commands;
mod utils;

/// GDExtension 入口点
/// 由 Godot 引擎在加载 native 插件时自动调用
struct GodotMcpGdext;

#[gdextension]
unsafe impl ExtensionLibrary for GodotMcpGdext {}
