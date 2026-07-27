//! MCP 协议层
//!
//! 包含：
//! - protocol: MCP/JSON-RPC 数据类型定义
//! - transport: TCP 服务器 (后台 tokio 线程)
//! - handler: MCP 协议方法处理

pub mod handler;
pub mod protocol;
pub mod transport;
