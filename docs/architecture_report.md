# Godot MCP Pro 技术架构报告

> 撰写日期：2026-07-27
> 目标：分析项目与 Godot 编辑器的通信机制，并给出 Rust 实现 MCP 的优化建议

---

## 一、总体架构概览

Godot MCP Pro 采用 **三层架构**，通信链路如下：

```
AI Assistant ──stdio/MCP──→ Node.js MCP Server ──WebSocket:6505-6514──→ Godot Editor Plugin
                                                                              │
                                                                   (运行时)    │ 文件 IPC
                                                                              ▼
                                                                      Godot Game Process
```

### 三层职责

| 层级 | 实现语言 | 核心职责 |
|------|----------|----------|
| **MCP Server (stdio 端)** | Node.js (闭源, 付费部分) | 实现 MCP 协议层、工具注册、参数校验、错误格式化 |
| **Godot Editor Plugin** | GDScript | 通过 WebSocket 接收 JSON-RPC 命令, 操作编辑器 API |
| **Game Runtime Autoloads** | GDScript | 文件 IPC 监听器, 在运行时游戏中执行命令 |

### 关键文件组织

```
server/                          # (闭源) Node.js MCP Server
addons/godot_mcp/
├── plugin.gd                    # EditorPlugin 入口, 生命周期管理
├── websocket_server.gd          # WebSocket 客户端, JSON-RPC 消息调度
├── command_router.gd            # 命令注册与路由分发
├── commands/
│   ├── base_command.gd          # 命令基类: 错误码、参数校验、游戏 IPC 通信
│   ├── scene_commands.gd        # 场景操作
│   ├── node_commands.gd         # 节点 CRUD
│   ├── runtime_commands.gd      # 运行时中继 (转发到游戏进程)
│   └── ... (共 25+ 命令模块)
├── mcp_game_inspector_service.gd # 运行时游戏自省 Autoload
├── mcp_input_service.gd         # 运行时输入模拟 Autoload
├── mcp_screenshot_service.gd    # 运行时截图 Autoload
└── ui/status_panel.gd           # 编辑器状态面板
```

---

## 二、编辑器通信机制：WebSocket + JSON-RPC 2.0

这是整个系统的**核心通信通道**, 负责 MCP Server 与 Godot 编辑器进程间的双向实时通信。

### 2.1 连接模型

```
┌─────────────────┐          WebSocket            ┌──────────────────────┐
│  Node.js Server  │ ◄════════ JSON-RPC 2.0 ════►  │  Godot Editor Plugin │
│  (stdio→MCP)     │     ws://127.0.0.1:6505-6514  │  (WebSocket Client)  │
└─────────────────┘                                └──────────────────────┘
```

- Godot 插件作为 **WebSocket 客户端**（而非服务端），主动连接 Node.js 启动的 WebSocket 服务端。
- 端口范围 **6505-6514**：前 5 个 (6505-6509) 用于 MCP server，后 5 个 (6510-6514) 用于 CLI 工具。
- **每个 Claude Code 会话占用一个独立端口**，Godot 插件同时维护多达 10 条 WebSocket 连接。

### 2.2 连接生命周期管理

核心实现在 [websocket_server.gd](file:///f:/RustProjects/godot-mcp-pro/addons/godot_mcp/websocket_server.gd)：

#### 自动重连
```gdscript
# 每 3 秒尝试重连所有未连接的端口
const RECONNECT_INTERVAL := 3.0
func _process(delta):
    for p in range(BASE_PORT, MAX_PORT + 1):
        if ws == null:
            _timers[p] += delta
            if _timers[p] >= RECONNECT_INTERVAL:
                _try_connect(p)
```

#### 心跳保活
- 每 5 秒发送 `{"jsonrpc": "2.0", "method": "ping"}` 心跳帧
- 服务端回复 `pong`，收到回复重置活动计时器
- 如果 **30 秒内无任何消息到达**，强制关闭连接并标记为 `stale`

#### 缓冲区配置
- 入站/出站缓冲区均为 **16MB**，应对大型场景数据（如截图 base64）

### 2.3 消息协议：JSON-RPC 2.0

所有消息遵循 JSON-RPC 2.0 规范：

**请求格式（服务端 → 插件）：**
```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "method": "get_scene_tree",
  "params": {"max_depth": -1}
}
```

**成功响应（插件 → 服务端）：**
```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "result": {"scene_path": "res://main.tscn", "tree": {...}}
}
```

**错误响应：**
```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "error": {
    "code": -32001,
    "message": "Node not found: Nonexistent",
    "data": {"suggestion": "Use get_scene_tree to see available nodes"}
  }
}
```

### 2.4 消息分发流程

```
websocket_server.gd                  command_router.gd
┌─────────────────┐                  ┌────────────────────┐
│ _dispatch_message│                 │ execute(method,    │
│  → 解析 JSON      │                 │   params) → await  │
│  → 处理 ping/pong │                 │  → _command_handlers│
│  → call_deferred  │── await ──────►│     [method].call() │
│    _execute_command│                │  → result / error   │
│  → _send_response  │◄── result ────│                     │
└─────────────────┘                  └────────────────────┘
```

关键设计决策：
- **`call_deferred`**：将命令执行推迟到 Godot 的下一帧，避免阻塞 WebSocket 消息循环
- **`await`**：命令函数内部可 `await`（如等待场景加载），command_router 的 `execute()` 是异步方法
- **结果直连**：直接写入该端口的 WebSocket 连接，不与其它端口的消息混在一起

---

## 三、运行时游戏通信：文件 IPC

编辑器命令需要与**正在运行的游戏进程**通信时（如获取运行时场景树、模拟输入、截图），采用基于文件的 IPC 机制。

### 3.1 架构示意图

```
┌──────────────────────┐         ┌──────────────────────────┐
│  Godot Editor Plugin │         │  Game Process (运行时)    │
│                      │         │                          │
│  base_command.gd     │         │  mcp_game_inspector.gd   │
│  send_game_command() │         │  mcp_input_service.gd    │
│                      │         │  mcp_screenshot_service  │
│  1. 写入请求文件      │         │                          │
│  2. 轮询等待响应       │         │  1. _process() 检测文件   │
│  3. 读取响应文件       │         │  2. 执行命令              │
│  4. 清理临时文件       │         │  3. 写入响应文件          │
└──────────────────────┘         └──────────────────────────┘
```

### 3.2 请求/响应协议（以游戏自省为例）

**步骤详解** [base_command.gd:201-263](file:///f:/RustProjects/godot-mcp-pro/addons/godot_mcp/commands/base_command.gd#L201-L263)：

1. **编辑器端** 写入 `user://mcp_game_request` 文件：
   ```json
   {"command": "get_scene_tree", "params": {"max_depth": -1}}
   ```

2. **游戏端** `mcp_game_inspector_service.gd` 在 `_process()` 中轮询检测到请求文件

3. 游戏端处理命令，写入 `user://mcp_game_response` 文件

4. **编辑器端** 每 0.1 秒轮询一次响应文件，超时时间默认 5 秒

5. 读取并清理临时文件，返回结果

### 3.3 三条独立文件 IPC 通道

| 通道 | 请求文件 | 响应文件 | 对应 Autoload |
|------|----------|----------|---------------|
| 游戏自省 | `mcp_game_request` | `mcp_game_response` | `MCPGameInspector` |
| 输入模拟 | `mcp_input_commands` | （无响应） | `MCPInputService` |
| 截图 | `mcp_screenshot_request` | `mcp_screenshot.png` | `MCPScreenshot` |

**重要考量**：文件 IPC 是单向的（编辑器 → 游戏），输入模拟和截图不需要游戏返回数据；只有游戏自省类命令需要双向通信。

### 3.4 调试器暂停恢复机制

文件 IPC 的一个重要问题是游戏可能因运行时错误而暂停。`base_command.gd` 实现了完善的诊断：

1. 超时后自动尝试按"继续"按钮恢复调试器
2. 从调试器错误面板收集最近的运行时错误
3. 返回带有详细上下文的错误信息，区分"游戏未运行"与"游戏已暂停"

### 3.5 运行时 Autoload 注入

[plugin.gd:4-8](file:///f:/RustProjects/godot-mcp-pro/addons/godot_mcp/plugin.gd#L4-L8) 插件激活时，会向 `ProjectSettings` 注入 3 个 Autoload：

- `MCPScreenshotService` — 运行时截图
- `MCPInputService` — 输入模拟
- `MCPGameInspector` — 运行时场景树/属性自省

插件退出时**只清理本次会话注入的 Autoload**，保留项目原有的 Autoload。

---

## 四、命令路由机制

### 4.1 命令注册

[command_router.gd:17-54](file:///f:/RustProjects/godot-mcp-pro/addons/godot_mcp/command_router.gd#L17-L54) 在 `_ready()` 中加载所有命令模块，将每个模块的 `get_commands()` 返回的 `{方法名 → Callable}` 字典全部合并到 `_command_handlers` 全局字典中：

```gdscript
# 注册 25 个命令模块, 约 175 个方法
var command_classes := [
    preload(...), preload(...),  # 25 个模块
]
for cmd_class in command_classes:
    var cmd := cmd_class.new()
    cmd.editor_plugin = editor_plugin
    add_child(cmd)
    var methods := cmd.get_commands()
    for method_name in methods:
        _command_handlers[method_name] = methods[method_name]
```

### 4.2 命令执行路由

```
websocket_server._execute_command()
  │
  ├─ 解析 method, params, id, source_port
  │
  └─ await command_router.execute(method, params)
       │
       ├─ 检查 method 是否注册 → 否: {"error": -32601, "available_methods": [...]}
       ├─ 检查是否被禁用     → 是: {"error": 工具禁用}
       │
       └─ await handler.call(params)
            │
            ├─ scene_commands.gd: _get_scene_tree(params)
            ├─ node_commands.gd: _add_node(params)
            ├─ runtime_commands.gd → send_game_command("get_scene_tree")
            └─ ...
```

### 4.3 单一职责设计

每个命令模块继承 [base_command.gd](file:///f:/RustProjects/godot-mcp-pro/addons/godot_mcp/commands/base_command.gd)，提供：

- **工具函数**：`success()`, `error()`, `require_string()`, `optional_string()` 等
- **编辑器访问**：`get_editor()`, `get_edited_root()`, `get_undo_redo()`
- **游戏 IPC**：`send_game_command()` — 封装文件读写 + 轮询 + 超时 + 调试器恢复

---

## 五、现存架构的局限与瓶颈

### 5.1 WebSocket 通信瓶颈

| 问题 | 说明 |
|------|------|
| **单线程处理** | Godot 的 GDScript 运行在主线程，所有 WebSocket 消息和命令执行都在同一个 `_process` 循环中 |
| `call_deferred` 延迟 | 命令执行通过 `call_deferred` 推迟到下一帧，增加约 1 帧(16ms) 的延迟 |
| **JSON 序列化开销** | 每个消息要经过 `JSON.stringify` / `JSON.parse`，大量数据（截图 base64）开销显著 |
| **无批量处理** | 每条消息立即处理，没有请求合并或批量处理机制 |
| **10 端口轮询** | 每个 `_process` 帧要遍历 10 个端口的 WebSocketPeer，O(n) 复杂度 |

### 5.2 文件 IPC 瓶颈

| 问题 | 说明 |
|------|------|
| **轮询延迟** | 编辑器端每 100ms 轮询，游戏端每帧轮询，导致命令延迟约 100-200ms |
| **文件系统开销** | 每次通信涉及 2 次文件写入 + 1-2 次文件读取 + 文件清理 |
| **无并行能力** | 同一时刻只能处理一个游戏命令（`_pending_command` 标志互斥） |
| **无流式响应** | 所有结果必须一次性生成后写入文件，无法流式返回 |

### 5.3 GDScript 的性能局限

- GDScript 是解释型语言，JSON 序列化/反序列化开销大
- 场景树遍历、节点属性采集等操作在大场景下性能不佳
- 没有真正的异步并行能力，`await` 只是协程级别的暂停

---

## 六、Rust 实现 MCP Server 的优化建议

如果要从零在 Rust 中实现 Godot MCP Server（替代现有 Node.js Server），以下是从通信效率和并行化角度的核心建议。

### 6.1 核心架构设计

```
AI Assistant ──stdio/MCP──→ Rust MCP Server ──WebSocket──→ Godot Editor Plugin
                                  │
                           (代替 Node.js Server)
```

**Rust 的优势**：
- 零成本抽象，无 GC 停顿
- tokio 异步运行时成熟的 IO 并发模型
- 优秀的 JSON 序列化性能（`serde_json`）
- 与 Godot 的 GDNative/GDExtension 的 C API 无缝对接（可选）

### 6.2 建议一：采用 Actor 模型处理多连接

当前 Node.js 版本的 Godot 插件**主动连接 10 个端口**，而 Rust 服务端应作为 WebSocket **监听端**。

```rust
// 推荐架构：基于 tokio + Actor 模型
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

#[tokio::main]
async fn main() {
    // 1. 启动 stdio MCP 监听（接收 AI 请求）
    let mcp_handle = tokio::spawn(mcp_stdio_listener());
    
    // 2. 启动 WebSocket 服务端（等待 Godot 连接）
    let ws_handle = tokio::spawn(websocket_server());
    
    // 3. 启动 10 个端口监听
    for port in 6505..=6514 {
        tokio::spawn(listen_port(port));
    }
}
```

**关键优化**：
- 每个端口一个独立的 `tokio::spawn` 任务，天然并行
- `Actor` 封装每个 Godot 连接的状态，通过 `mpsc channel` 与 MCP 层通信
- 无锁设计：通过消息传递而非共享状态

### 6.3 建议二：WebSocket 消息批量处理

当前架构中每条消息立即序列化发送。Rust 可实现批量处理：

```rust
/// 消息批处理器
struct BatchProcessor {
    buffer: Vec<JsonRpcMessage>,
    max_batch_size: usize,
    flush_interval: Duration,
}

impl BatchProcessor {
    /// 积累消息，达到阈值或超时时批量发送
    async fn push(&mut self, msg: JsonRpcMessage, sink: &mut WebSocketSink) {
        self.buffer.push(msg);
        if self.buffer.len() >= self.max_batch_size {
            self.flush(sink).await;
        }
    }
    
    /// 批量发送：合并为 JSON-RPC 批量请求
    async fn flush(&mut self, sink: &mut WebSocketSink) {
        let batch = std::mem::take(&mut self.buffer);
        let json = serde_json::to_string(&batch)?; // 批量序列化
        sink.send(Message::Text(json)).await;
    }
}
```

### 6.4 建议三：命令行级流水线并行

当前系统每条命令是同步串行的（GDScript 中 `await` 等待结果）。Rust 可将命令拆分为"发送 → 等待 → 解析"三阶段流水线：

```
┌─────────┐   ┌─────────┐   ┌─────────┐
│ 发送     │ → │ 等待     │ → │ 解析     │
│ WebSocket│   │ 响应     │   │ 结果     │
└─────────┘   └─────────┘   └─────────┘
     ↑               ↑              ↑
  任务流1          任务流2         任务流3
  (tokio::spawn)
```

**具体实现**：
- 为每个待处理命令生成一个 `oneshot::channel`
- 将 channel 的 sender 注册到"待处理"哈希表中，key 为 JSON-RPC id
- WebSocket 收到响应后，查找对应 channel 并 send
- 这样发送端不必等待，可同时发送多条命令给 Godot

```rust
/// 并行的命令-响应匹配器
struct ResponseMatcher {
    pending: HashMap<u64, oneshot::Sender<JsonRpcResponse>>,
    next_id: AtomicU64,
}

impl ResponseMatcher {
    /// 发送命令并立即返回 Future
    async fn send_command(&mut self, ws: &mut WebSocketStream, 
                          method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        
        self.pending.insert(id, tx);
        ws.send(build_request(id, method, params)).await;
        
        // 异步等待响应（不阻塞其他命令）
        let response = rx.await?;
        Ok(response.result)
    }
}
```

### 6.5 建议四：用 GDExtension 替代文件 IPC

当前的文件 IPC 是架构中最大的性能瓶颈。Rust 可以通过 **GDExtension**（Godot 4 的 C 语言扩展接口）直接在 Rust 中实现运行时代理，**彻底消除文件轮询**：

```rust
// Rust GDExtension 运行时代理
#[gdextension]
struct RuntimeAgent {
    // 通过共享内存或 Unix Domain Socket 与 MCP Server 通信
    ipc_channel: LocalSocket,
    command_buffer: Vec<u8>,
}

#[godot_api]
impl RuntimeAgent {
    #[func]
    fn _process(&mut self, _delta: f64) {
        // 非阻塞读取，无需文件轮询
        if let Some(cmd) = self.ipc_channel.try_recv() {
            let result = self.execute_command(cmd);
            self.ipc_channel.send(result);
        }
    }
}
```

**替代方案（更低侵入性）**：
- 使用 **命名管道 (Named Pipe)** 替代文件 IPC
- 在游戏端通过 GDScript 的 `SimpleWebSocketClient` 直接建立 WebSocket 连接

### 6.6 建议五：JSON-RPC 批处理与压缩

```rust
// 对截图等大数据使用 binary WebSocket frame + 压缩
use flate2::write::GzEncoder;

fn send_large_result(sink: &mut WebSocketStream, data: &[u8]) {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(data).unwrap();
    let compressed = encoder.finish().unwrap();
    
    // 发送 binary frame，减少 ~70% 体积
    sink.send(Message::Binary(compressed));
}
```

### 6.7 建议六：工具热缓存

许多命令（如 `get_project_info`、`get_editor_performance`）的结果变化频率低。Rust 可实现缓存层：

```rust
struct CommandCache {
    cache: HashMap<&'static str, (Value, Instant)>,
    ttl: Duration,
}

impl CommandCache {
    fn get_or_compute(&mut self, key: &str, 
                      compute: impl Future<Output=Value>) -> Value {
        if let Some((value, time)) = self.cache.get(key) {
            if time.elapsed() < self.ttl {
                return value.clone();
            }
        }
        let value = compute.await;
        self.cache.insert(key, (value.clone(), Instant::now()));
        value
    }
}
```

### 6.8 建议七：MCP 协议层优化

当前 Node.js 服务端需要通过 `--lite` / `--minimal` 等参数手动裁剪工具列表以适配不同客户端的工具数量限制。Rust 可以内置更智能的协议处理：

```rust
enum ToolMode {
    /// 标准模式：注册全部 175+ 工具
    Full,
    /// 延迟加载：仅在请求时注册工具元数据
    Lazy { loaded: HashSet<String> },
    /// 分批注册：工具列表自动分组，按需注册
    Paginated { page_size: usize },
}

/// 实现 MCP 的 ListTools 响应的动态分页
async fn handle_list_tools(&self, params: ListToolsParams) -> ListToolsResult {
    let page = params.cursor.unwrap_or(0);
    let tools: Vec<Tool> = self.all_tools
        .iter()
        .skip(page * self.page_size)
        .take(self.page_size)
        .map(|t| t.to_protocol_tool())
        .collect();
    
    ListToolsResult {
        tools,
        next_cursor: if tools.len() == self.page_size {
            Some(page + 1)
        } else {
            None
        }
    }
}
```

### 6.9 建议八：错误恢复策略升级

当前项目中 Godot 侧实现了指数退避重连、心跳、stale 连接检测等机制。在 Rust 中应将这些提升为架构级能力：

```rust
/// 连接管理器：自动重连 + 指数退避 + 健康检查
struct ConnectionManager {
    backoff: ExponentialBackoff,
    health_check: HealthChecker,
    state: ConnectionState,
}

impl ConnectionManager {
    async fn maintain_connection(&mut self) {
        loop {
            match self.connect().await {
                Ok(stream) => {
                    self.state = ConnectionState::Connected;
                    // 健康检查任务与消息处理任务并行
                    tokio::select! {
                        _ = self.health_check.run(&stream) => {},
                        _ = self.message_loop(&stream) => {},
                    }
                }
                Err(e) => {
                    let delay = self.backoff.next_backoff();
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}
```

---

## 七、总结：Rust 实现收益量化

| 维度 | Node.js 现状 | Rust 优化预期 |
|------|-------------|--------------|
| **消息延迟** | ~16ms (受 `call_deferred` 限制) | ~1-2ms (零延迟发送) |
| **并发能力** | 单线程轮询 10 端口 | 10 个独立 tokio 任务并行 |
| **JSON 吞吐** | 大场景 JSON 序列化卡顿 | `simd-json` 或 `serde_json` 零拷贝解析 |
| **文件 IPC 延迟** | 100-200ms (轮询 + 文件 IO) | ~1ms (共享内存或命名管道) |
| **缓存能力** | 无缓存，每次重新采集 | 支持 TTL 缓存，热点命令 0 延迟 |
| **批量处理** | 无 | 支持消息批次合并 + 流水线并行 |
| **内存安全** | Node.js GC 不确定暂停 | 零 GC 暂停，确定性延迟 |
| **包体积** | 需 Node.js 运行时 + npm 依赖 | 单文件静态链接，<10MB |

### 实施优先级建议

1. **P0 - 通信层**：Actor 模型 WebSocket 服务 + 异步命令匹配器（替换 Node.js 的单线程事件循环）
2. **P1 - 运行时代理**：GDExtension 直接替代文件 IPC（消除最大延迟源头）
3. **P2 - 智能缓存**：热点命令结果缓存 + JSON-RPC 批量请求
4. **P3 - 协议增强**：二进制帧压缩 + 工具延迟加载/分页

---

*本文档基于 `godot-mcp-pro` 项目的 GDScript 插件部分（开源）逆向分析形成。Node.js MCP Server 部分为闭源，文档中的推断基于插件侧接口约定和标准 MCP 协议行为。*
