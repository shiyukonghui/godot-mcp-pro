# Rust GDExtension 全栈 MCP 开发规划

---

## 一、总体架构

```
                  ═══ Godot Editor Process ═══
                  ┌────────────────────────────┐
                  │  Rust GDExtension           │
                  │  (godot_mcp_rs)             │
                  │                             │
                  │  ┌──────────────────────┐   │
AI ──TCP/MCP──►   │  │  MCP Protocol Layer  │   │
(9876)            │  │  TCP Server           │───┤──► EditorInterface API
                  │  │  JSON-RPC Handler     │   │
                  │  └──────┬───────────────┘   │
                  │         │                    │
                  │  ┌──────▼───────────────┐   │
                  │  │  Command Router       │   │
                  │  │  Tool definitions     │───┤──► SceneTree/Node API
                  │  │  Result serialization │   │
                  │  └──────┬───────────────┘   │
                  │         │                    │
                  │  ┌──────▼───────────────┐   │
                  │  │  Runtime Agent        │   │
                  │  │  (file IPC / shared   │───┤──► Game Process
                  │  │   memory bridge)      │   │
                  │  └──────────────────────┘   │
                  └────────────────────────────┘
```

### 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| **MCP 传输层** | TCP (端口 9876) | Godot 进程内无法安全使用 stdio |
| **AI 连接方式** | 二进制桥接器 | MCP AI Client 需要 stdio 传输 |
| **Godot 绑定库** | `godot` (gdextension) | 官方维护，成熟稳定 |
| **异步运行时** | `tokio` | 标准选择，TCP/IO 生态完善 |
| **JSON** | `serde_json` | Rust 生态最强序列化库 |
| **编辑器 API** | `EditorInterface::singleton()` | GDExtension 原生支持 |

---

## 二、组件划分

### 组件 1：`godot_mcp_gdext`（GDExtension 动态库）

**输出**: `target/release/libgodot_mcp_gdext.dll` → Godot 的 `addons/godot_mcp_rs/`

```
godot_mcp_gdext/
├── src/
│   ├── lib.rs               # GDExtension 入口
│   ├── plugin.rs             # EditorPlugin 实现
│   ├── mcp/
│   │   ├── mod.rs
│   │   ├── transport.rs      # TCP 服务器 (tokio)
│   │   ├── protocol.rs       # MCP 消息编解码
│   │   └── handler.rs        # initialize/list_tools/call_tool 处理
│   ├── commands/
│   │   ├── mod.rs            # 命令路由注册
│   │   ├── project.rs        # get_project_info 等
│   │   ├── scene.rs          # 场景操作
│   │   ├── node.rs           # 节点 CRUD
│   │   └── runtime.rs        # 运行时中继
│   └── utils/
│       ├── mod.rs
│       ├── serialize.rs      # Godot 类型 ↔ JSON 序列化
│       └── error.rs          # 错误码体系
├── Cargo.toml
└── addons/godot_mcp_rs/
    ├── plugin.cfg            # Godot 插件元数据
    └── godot_mcp_rs.gdextension  # GDExtension 配置文件
```

### 组件 2：`mcp_bridge`（CLI 桥接器）

**输出**: `target/release/mcp_bridge.exe` → 用户 PATH

```
mcp_bridge/
├── src/main.rs               # stdio ↔ TCP 双向转发
└── Cargo.toml
```

**代码量**: ~80 行 — 纯字节流透传，无业务逻辑

### 组件 3：运行时 Godot 脚本（GDScript）

**文件**: `addons/godot_mcp_rs/mcp_runtime_agent.gd`

由于 GDExtension 在游戏进程中加载时无法访问编辑器 API，运行时仍需少量 GDScript 做 Autoload。但**文件 IPC 改为共享内存或 Named Pipe**，由 GDExtension 端的 tokio 任务直接读写。

---

## 三、MCP 协议适配详情

### 传输层

1. **GDExtension 侧**：启动 tokio 运行时，在 `127.0.0.1:9876` 监听 TCP 连接
2. **桥接器侧**：stdin 来的数据 → TCP 发送给 GDExtension；TCP 来的数据 → stdout 输出
3. **AI Client 配置**：
   ```json
   {
     "mcpServers": {
       "godot-mcp": {
         "command": "mcp_bridge",
         "args": ["--port", "9876"]
       }
     }
   }
   ```

### MCP 消息流

```
AI → bridge → TCP → GDExtension:

  1. JSON-RPC 2.0 行协议 (按 \n 分割)
  2. 格式: Content-Length: N\r\n\r\n{payload}  (标准 MCP HTTP 风格)
     或直接用 \n 分隔的 JSON 行 (更轻量)

GDExtension → TCP → bridge → AI:

  1. 同样格式返回
  2. 工具结果中大的 base64 截图等使用 tokio 的 write_all 无需额外处理
```

### 工具注册

MCP `ListToolsResult` 中的 `Tool` 结构：

```rust
struct Tool {
    name: String,
    description: String,
    input_schema: serde_json::Value,  // JSON Schema
}
```

每个命令模块提供一个 `Vec<ToolDefinition>` 数组，由 Command Router 统一收集。

**首批实现工具（20 个核心，覆盖 80% 使用场景）**：

| 优先级 | 工具名 | 所属模块 | 备注 |
|--------|--------|----------|------|
| P0 | `get_project_info` | project | 基础信息，最简单 |
| P0 | `get_scene_tree` | scene | 核心查看功能 |
| P0 | `add_node` | node | 最常用操作 |
| P0 | `delete_node` | node | 常用 CRUD |
| P0 | `update_property` | node | 核心编辑能力 |
| P0 | `play_scene` / `stop_scene` | scene | 运行控制 |
| P0 | `execute_editor_script` | editor | 兜底能力 |
| P0 | `save_scene` | scene | 保存 |
| P0 | `get_node_properties` | node | 查看属性 |
| P1 | `rename_node` | node | 基础 CRUD |
| P1 | `move_node` | node | 重排父子 |
| P1 | `duplicate_node` | node | 复制 |
| P1 | `open_scene` | scene | 切换场景 |
| P1 | `create_scene` | scene | 新建 |
| P1 | `get_editor_errors` | editor | 调试 |
| P1 | `get_output_log` | editor | 查看日志 |
| P1 | `get_performance_monitors` | profiling | 性能监控 |
| P2 | `list_scripts` / `read_script` | script | 脚本操作 |
| P2 | `connect_signal` | node | 信号连接 |
| P2 | `find_nodes_by_type` | batch | 批量查询 |

---

## 四、项目里程碑

### M1：基础架构（本阶段完成）

```
□  创建 Cargo workspace
□  引入 godot/gdextension 依赖
□  空的 GDExtension 在 Godot 中成功加载
□  plugin.cfg + .gdextension 配置文件
□  TCP 服务器在 9876 端口启动
□  桥接器二进制完成
□  AI 侧能连接并收到 initialize 响应
```

### M2：核心命令集

```
□  MCP list_tools 返回工具列表
□  MCP call_tool 路由到对应 handler
□  P0 工具全部实现（~10 个）
□  错误码体系（内部错误、参数错误、未找到等）
□  Godot 类型 ←→ serde_json 序列化
```

### M3：完整功能

```
□  P1 工具全部实现（~15 个）
□  运行时中继：文件 IPC / Named Pipe
□  P2 工具
□  MCP notifications (tools/list_changed 等)
□  连接管理：自动重连、心跳、超时
```

### M4：打磨与发布

```
□  错误信息本地化、suggestion 提示
□  UndoRedo 集成
□  性能优化
□  CI 构建
□  文档 + 使用说明
```

---

## 五、技术难点与应对

| 难点 | 应对方案 |
|------|----------|
| GDExtension 中无法使用 stdio | 改用 TCP + 桥接器 |
| Godot 对象生命周期（`free()` 后访问） | 使用 `is_instance_valid()` 包装 |
| tokio 运行时与 Godot 主线程同步 | 用 `channel` 传递任务到 `_process` |
| GDExtension 中 `EditorInterface` API 覆盖度不足 | 结合 `execute_editor_script` 兜底 |
| 运行时游戏通信 | 初期复用文件 IPC，后续升级 Named Pipe |
| Windows 上的 Named Pipe | 使用 `tokio::net::windows::named_pipe` |

---

## 六、文件结构（最终）

```
godot-mcp-pro/
├── Cargo.workspace            # workspace root
├── godot_mcp_gdext/            # GDExtension 核心库
│   ├── Cargo.toml
│   └── src/
├── mcp_bridge/                 # stdio↔TCP 桥接器
│   ├── Cargo.toml
│   └── src/main.rs
└── addons/godot_mcp_rs/        # Godot 侧配置
    ├── plugin.cfg
    └── godot_mcp_rs.gdextension
```
