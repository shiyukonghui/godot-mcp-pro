# Rust GDExtension 全栈 MCP 开发规划

---

## 一、总体架构

```
                  ═══ Godot Editor Process ════════          ═══ Game Process ═══
                  ┌─────────────────────────────────┐        ┌─────────────────┐
                  │  Rust GDExtension (godot_mcp_rs)│        │  GDScript       │
                  │                                 │        │  Autoloads      │
                  │  plugin.rs                      │        │                 │
AI ──TCP/MCP──►   │   ├── _process() 轮询请求       │        │ mcp_runtime_    │
(9876)            │   ├── handle_mcp_message()      │        │ agent.gd        │
                  │   ├── handle_call_tool() ────►  │──IPC──►│  场景树/属性    │
                  │   └── build_response()          │  file  │  截图           │
                  │                                 │        │  输入模拟等     │
                  │  mcp/transport.rs                │        │                 │
                  │   ├── TCP 服务器 (tokio 线程)    │        │ mcp_screenshot_ │
                  │   └── crossbeam channel 投递     │        │ service.gd      │
                  │                                 │        │                 │
                  │  commands/ (24 个模块)            │        │ mcp_input_      │
                  │   ├── collect_all_tools() ───────┤        │ service.gd      │
                  │   └── execute_tool() ────────────┤        │                 │
                  │             │                    │        │                 │
                  │             ▼                    │        │                 │
                  │     EditorInterface API          │         └─────────────────┘
                  │     SceneTree/Node API           │
                  └─────────────────────────────────┘
```

### 通信路径

```
AI Client → mcp_bridge(stdin/Content-Length) → TCP(newline-delimited JSON)
  → transport.rs(tokio 线程) → crossbeam channel → plugin.rs::_process(Godot 主线程)
  → commands::execute_tool() → EditorInterface/Node API
  → response via crossbeam channel → TCP → mcp_bridge → stdout
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

**输出**: `target/debug/godot_mcp_gdext.dll` → Godot 的 `addons/godot_mcp_rs/`

```
godot_mcp_gdext/
├── src/
│   ├── lib.rs               # GDExtension 入口
│   ├── plugin.rs             # EditorPlugin + MCP 消息处理 (handle_mcp_message)
│   ├── mcp/
│   │   ├── mod.rs
│   │   ├── transport.rs      # TCP 服务器 (tokio 线程, crossbeam 通道)
│   │   ├── protocol.rs       # MCP/JSON-RPC 数据类型
│   │   └── handler.rs        # (预留, 当前逻辑在 plugin.rs)
│   ├── commands/
│   │   ├── mod.rs            # 命令路由: collect_all_tools / execute_tool
│   │   ├── project.rs        # 项目管理 (3 工具)
│   │   ├── scene.rs          # 场景操作 (10 工具)
│   │   ├── node.rs           # 节点 CRUD (17 工具)
│   │   ├── editor.rs         # 编辑器操作 (13 工具)
│   │   ├── runtime.rs        # 运行时 IPC 客户端 (19 工具)
│   │   ├── profiling.rs      # 性能监控 (2 工具)
│   │   ├── script.rs         # 脚本管理 (7 工具)
│   │   ├── input.rs          # 输入模拟 (6 工具)
│   │   ├── batch.rs          # 批量操作 (7 工具)
│   │   ├── animation.rs      # 动画管理 (6 工具)
│   │   ├── tilemap.rs        # 瓦片地图 (6 工具)
│   │   ├── resource.rs       # 资源管理 (6 工具)
│   │   ├── export.rs         # 导出管理 (3 工具)
│   │   ├── shader.rs         # 着色器 (6 工具)
│   │   ├── physics.rs        # 物理系统 (6 工具)
│   │   ├── scene_3d.rs       # 3D 场景 (6 工具)
│   │   ├── audio.rs          # 音频系统 (6 工具)
│   │   ├── theme.rs          # 主题管理 (7 工具)
│   │   ├── animation_tree.rs # 动画树 (8 工具)
│   │   ├── navigation.rs     # 导航系统 (5 工具)
│   │   ├── particle.rs       # 粒子系统 (5 工具)
│   │   ├── analysis.rs       # 代码分析 (6 工具)
│   │   ├── test.rs           # 测试自动化 (5 工具)
│   │   └── android.rs        # Android 部署 (3 工具)
│   └── utils/
│       ├── mod.rs
│       ├── serialize.rs      # Godot 类型 ↔ JSON 序列化
│       └── error.rs          # JSON-RPC 错误码体系
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

### 组件 3：运行时 Autoload（GDScript）

**文件**: `addons/godot_mcp_rs/mcp_runtime_agent.gd`
**文件**: `addons/godot_mcp/mcp_screenshot_service.gd`（由 GDScript 插件注入）
**文件**: `addons/godot_mcp/mcp_input_service.gd`（由 GDScript 插件注入）

GDExtension 在游戏进程中加载时无法访问编辑器 API，因此运行时代理使用 GDScript Autoload 驻留游戏进程，通过**文件 IPC** 与编辑器通信。

**当前状态**：
- 编辑器端 `commands/runtime.rs` 实现 19 个运行时工具的编辑器侧 IPC 客户端
- 游戏端 `mcp_runtime_agent.gd` 已实现场景树查询（`get_scene_tree`）
- 其他 18 个运行时工具（`get_game_node_properties`、`capture_frames` 等）的游戏侧 GDScript 尚未实现
- 截图和输入服务由原 GDScript 插件的 Autoload（`mcp_screenshot_service.gd`、`mcp_input_service.gd`）提供
- IPC 协议：JSON 行协议 via `user://mcp_game_request` / `user://mcp_game_response`

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

### M1：基础架构（已完成）

```
✅  创建 Cargo workspace
✅  引入 godot/gdextension 依赖
✅  空的 GDExtension 在 Godot 中成功加载
✅  plugin.cfg + .gdextension 配置文件
✅  TCP 服务器在 9876 端口启动
✅  桥接器二进制完成
✅  AI 侧能连接并收到 initialize 响应
```

### M2：核心命令集（已完成）

```
✅  MCP list_tools 返回工具列表
✅  MCP call_tool 路由到对应 handler
✅  P0 工具全部实现
✅  错误码体系（内部错误、参数错误、未找到等）
✅  Godot 类型 ←→ serde_json 序列化
```

### M3：完整功能（已完成 — 编译通过，171/171 工具）

```
✅  全部 171 个工具 Rust 迁移完成
✅  运行时中继：文件 IPC (编辑器侧 19 工具，游戏侧基础实现)
✅  MCP notifications (tools/list_changed 等 — 预留)
✅  连接管理：心跳、超时
```

### M4：打磨与发布（进行中）

```
☐  运行时游戏侧 GDScript 补全（18 个待实现命令）
☐  UndoRedo 集成
☐  性能优化
☐  CI 构建
☐  文档 + 使用说明
```

---

## 五、技术难点与应对

| 难点 | 应对方案 | 当前状态 |
|------|----------|----------|
| GDExtension 中无法使用 stdio | 改用 TCP + 桥接器 | ✅ 已解决 |
| Godot 对象生命周期（`free()` 后访问） | 使用 `is_instance_valid()` 包装 | ✅ 已处理 |
| tokio 运行时与 Godot 主线程同步 | 用 crossbeam channel 传递任务到 `_process` | ✅ 已解决 |
| GDExtension 中 `EditorInterface` API 覆盖度不足 | 结合 `execute_editor_script` / GDScript Expression 兜底 | ✅ 已处理（少量 Expression 保留） |
| 运行时游戏通信 | 文件 IPC (`user://mcp_game_request/response`) | ✅ 基础可用，游戏侧待补全 |
| Windows 上的 Named Pipe | 暂未使用（保持文件 IPC） | ⏸ 暂缓 |

---

## 六、文件结构（最终）

```
godot-mcp-pro/
├── Cargo.toml                  # workspace root
├── godot_mcp_gdext/            # GDExtension 核心库
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs              # GDExtension 入口
│   │   ├── plugin.rs           # EditorPlugin + MCP 消息处理
│   │   ├── mcp/                # MCP 协议层
│   │   │   ├── transport.rs    # TCP 服务器 (tokio + crossbeam)
│   │   │   ├── protocol.rs     # 数据类型
│   │   │   └── handler.rs      # (预留)
│   │   ├── commands/           # 24 个模块, 171 个工具
│   │   │   ├── mod.rs          # 路由注册
│   │   │   └── *.rs            # 各模块实现
│   │   └── utils/
│   │       ├── serialize.rs
│   │       └── error.rs
│   └── addons/godot_mcp_rs/    # Godot 侧配置
│       ├── plugin.cfg
│       ├── godot_mcp_rs.gdextension
│       └── mcp_runtime_agent.gd
├── mcp_bridge/                 # stdio↔TCP 桥接器
│   ├── Cargo.toml
│   └── src/main.rs             # ~80 行, 纯字节流透传
├── addons/godot_mcp/           # 原 GDScript 插件 (保留作为备选)
│   ├── plugin.gd
│   ├── commands/               # GDScript 工具实现
│   └── ...                     # (迁移完成后可移除)
├── docs/                       # 文档
│   └── rust_dev_plan.md
└── scripts/
    └── deploy.ps1              # 构建 + 部署脚本
```
