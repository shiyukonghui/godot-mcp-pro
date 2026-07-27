//! mcp_bridge — MCP stdio ↔ TCP 桥接器
//!
//! 作用：将 MCP AI Client 的 stdio 传输转换为 TCP 连接，
//! 转发给 Godot 内部的 GDExtension TCP 服务端。
//!
//! AI Client 配置示例 (mcp.json):
//! ```json
//! {
//!   "mcpServers": {
//!     "godot-mcp": {
//!       "command": "mcp_bridge",
//!       "args": ["--port", "9876"]
//!     }
//!   }
//! }
//! ```
//!
//! 流式转换：
//!   stdin (Content-Length framing) → TCP (newline-delimited JSON)
//!   TCP (newline-delimited JSON) → stdout (Content-Length framing)

use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

/// 默认 Godot GDExtension TCP 端口
const DEFAULT_PORT: u16 = 9876;
/// 连接失败/断开后的重连间隔（秒）
const RECONNECT_INTERVAL_SECS: u64 = 10;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port = parse_port(&args).unwrap_or(DEFAULT_PORT);
    let addr = format!("127.0.0.1:{}", port);

    eprintln!("[mcp-bridge] 桥接器启动，连接目标 {}，重连间隔 {}s", addr, RECONNECT_INTERVAL_SECS);

    loop {
        eprintln!("[mcp-bridge] 正在连接 Godot MCP TCP 服务 {} ...", addr);

        let stream = match TcpStream::connect(&addr) {
            Ok(s) => {
                eprintln!("[mcp-bridge] ✅ 已连接到 Godot MCP 服务 (端口 {})", port);
                s
            }
            Err(e) => {
                eprintln!("[mcp-bridge] ❌ 连接失败: {} — {} 秒后重试", e, RECONNECT_INTERVAL_SECS);
                thread::sleep(Duration::from_secs(RECONNECT_INTERVAL_SECS));
                continue; // 重新尝试连接
            }
        };

        // 将 TCP 设置为非阻塞模式，用于后台读取线程
        if stream.set_read_timeout(Some(Duration::from_millis(100))).is_err() {
            eprintln!("[mcp-bridge] 设置 TCP 读取超时失败");
            thread::sleep(Duration::from_secs(RECONNECT_INTERVAL_SECS));
            continue;
        }

        let read_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[mcp-bridge] 克隆 TCP 流失败: {:?}", e);
                thread::sleep(Duration::from_secs(RECONNECT_INTERVAL_SECS));
                continue;
            }
        };
        let mut write_stream = stream;

        // 后台线程：从 TCP 读取数据，写入 stdout
        let stdout_handle = thread::spawn(move || {
            let mut reader = io::BufReader::new(read_stream);
            let stdout = io::stdout();
            let mut stdout_lock = stdout.lock();

            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        eprintln!("[mcp-bridge] TCP 连接已关闭");
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        let content = trimmed.as_bytes();
                        let header = format!("Content-Length: {}\r\n\r\n", content.len());

                        if stdout_lock.write_all(header.as_bytes()).is_err() {
                            break;
                        }
                        if stdout_lock.write_all(content).is_err() {
                            break;
                        }
                        if stdout_lock.write_all(b"\n").is_err() {
                            break;
                        }
                        stdout_lock.flush().ok();
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {
                        continue;
                    }
                    Err(e) => {
                        eprintln!("[mcp-bridge] TCP 读取错误: {:?}", e);
                        break;
                    }
                }
            }
        });

        // 主线程：从 stdin 读取 MCP Content-Length 格式消息，转发到 TCP
        {
            let mut stdin = io::stdin().lock();
            let mut content_length: Option<usize> = None;
            let mut buffer = String::new();

            loop {
                buffer.clear();
                match stdin.read_line(&mut buffer) {
                    Ok(0) => {
                        eprintln!("[mcp-bridge] stdin 已关闭");
                        break;
                    }
                    Ok(_) => {
                        let line = buffer.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }

                        if let Some(len_str) = line.strip_prefix("Content-Length: ") {
                            if let Ok(len) = len_str.trim().parse::<usize>() {
                                content_length = Some(len);
                            }
                            continue;
                        }

                        if line.is_empty() {
                            if let Some(len) = content_length.take() {
                                let mut body = vec![0u8; len];
                                match stdin.read_exact(&mut body) {
                                    Ok(()) => {
                                        let json_str = String::from_utf8_lossy(&body);
                                        if let Err(e) = writeln!(write_stream, "{}", json_str) {
                                            eprintln!("[mcp-bridge] TCP 写入错误: {:?}", e);
                                            break;
                                        }
                                        write_stream.flush().ok();
                                    }
                                    Err(e) => {
                                        eprintln!("[mcp-bridge] 读取消息体错误: {:?}", e);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[mcp-bridge] stdin 读取错误: {:?}", e);
                        break;
                    }
                }
            }
        }

        // 等待后台线程结束
        let _ = stdout_handle.join();
        eprintln!("[mcp-bridge] 已断开与 Godot MCP 的连接");

        // 断开后等待指定间隔后重连
        eprintln!("[mcp-bridge] {} 秒后尝试重连...", RECONNECT_INTERVAL_SECS);
        thread::sleep(Duration::from_secs(RECONNECT_INTERVAL_SECS));
    }
}

/// 从命令行参数解析端口号
fn parse_port(args: &[String]) -> Option<u16> {
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--port" || args[i] == "-p" {
            if let Some(port_str) = args.get(i + 1) {
                if let Ok(port) = port_str.parse::<u16>() {
                    return Some(port);
                }
            }
        }
        i += 1;
    }
    None
}
