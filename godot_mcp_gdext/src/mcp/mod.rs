//! MCP 协议层
//!
//! 包含：
//! - protocol: MCP/JSON-RPC 数据类型定义
//! - transport_http: HTTP 服务器 (AI 助手可直接 HTTP POST /mcp)

pub mod handler;
pub mod protocol;
// pub mod transport; // TCP 模式已注释，仅保留 HTTP 模式
pub mod transport_http;
