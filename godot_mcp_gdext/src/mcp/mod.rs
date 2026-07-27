//! MCP 协议层
//!
//! 包含：
//! - protocol: MCP/JSON-RPC 数据类型定义
//! - transport: TCP 服务器 (后台 tokio 线程, 兼容 mcp_bridge)
//! - transport_http: HTTP 服务器 (AI 助手可直接连接)

pub mod handler;
pub mod protocol;
pub mod transport;
pub mod transport_http;
