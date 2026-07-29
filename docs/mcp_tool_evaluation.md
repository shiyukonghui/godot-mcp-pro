# MCP 工具开发实战评估报告

> 通过俄罗斯方块项目开发，系统性测试 godot-mcp 工具的可用性，记录遇到的问题和不便之处。

---

## 一、测试概况

| 项目 | 数据 |
|------|------|
| 测试场景 | 俄罗斯方块项目（tetris） |
| 涉及工具数 | 32 个 |
| 通过 | 26 个 |
| 有限制 | 5 个 |
| 不可用 | 1 个 |

### 已验证通过的工具

```
get_project_info        get_scene_tree          get_filesystem_tree
list_scripts            read_script             edit_script (search/replace)
edit_script (content)   validate_script         save_scene
play_scene              stop_scene              open_scene
add_node                delete_node             update_property
get_scene_file_content  get_editor_errors       get_scene_dependencies
get_node_properties     find_nodes_by_type      get_game_scene_tree
find_nodes_by_script    set_game_node_property  get_game_node_properties
capture_frames          get_editor_performance  get_project_statistics
connect_signal          disconnect_signal       search_files
find_node_references    get_signals
```

### 有功能限制的工具

| 工具 | 限制描述 |
|------|----------|
| `execute_game_script` | 基于 Godot Expression 类，不支持多语句、变量声明、赋值操作 |
| `simulate_key` | 仅作用于编辑器视口，无法传递给运行中的游戏窗口 |
| `search_in_files` | 搜索范围不含 .gd 脚本文件内容 |
| `get_output_log` | 日志文件路径问题，返回"无法打开日志文件" |
| `get_editor_screenshot` | 返回 null，无实际截图数据 |

### 不可用的工具

| 工具 | 原因 |
|------|------|
| `get_game_screenshot` | 需要项目中配置 MCPScreenshot Autoload，默认未启用 |

---

## 二、详细问题记录

### 2.1 参数命名不一致（高优先级）

**问题**：同类操作使用不同参数名，调用前必须逐个查看 schema。

| 工具 | 路径参数名 | 其他命名差异 |
|------|-----------|-------------|
| `read_script` | `path` | `script_path` ❌ |
| `get_scene_file_content` | `path` | `scene_path` ❌ |
| `get_node_properties` | `path` | `node_path` ❌ |
| `get_signals` | `node_path` | `path` ❌ |
| `connect_signal` | `source_path` | `node_path` ❌ |
| `execute_game_script` | -- | `script` ❌，应为 `code` |

**影响**：每次调用前必须先用 Read 工具查看 schema，增加约 30% 的交互轮次。

**建议**：统一为项目级用 `path`，节点级用 `node_path`，信号相关用 `source_path` + `target_path`。在执行层做参数别名兼容，让两种命名都能工作。

---

### 2.2 execute_game_script 的 Expression 限制（高优先级）

**问题**：底层使用 `Expression` 类而非完整 GDScript 解析器，导致严重功能受限：

- 不支持 `var` 变量声明
- 不支持赋值语句（`=`）
- 不支持 `return` 关键字（虽做了自动剥离，但限制仍然存在）
- 不支持多语句（分号分隔也不行）
- 只能写单个表达式

**实际影响**：

```gdscript
# 期望的写法（不可用）
var root = get_node("/root/Tetris")
var filled = 0
for y in range(root.BOARD_HEIGHT):
    for x in range(root.BOARD_WIDTH):
        if root.board[y][x] != null:
            filled += 1
filled

# 实际只能写（可用）
get_node("/root/Tetris").score
```

**绕过方式**：通过 `call()` 间接操作方法，但无法读取复杂状态。

```gdscript
# 可用的绕过方式
get_node("/root/Tetris").call("_hard_drop")
get_node("/root/Tetris").call("_move_piece", -1, 0)
```

**建议**：提供一个 `execute_game_script_full` 命令，内部使用 `GDScript.new()` 编译执行完整脚本代码。

---

### 2.3 simulate_key 无法到达游戏窗口（高优先级）

**问题**：`simulate_key` 在游戏运行时调用，按键事件被编辑器窗口捕获而非游戏窗口。

**实际测试**：
- 调用 `simulate_key("KEY_ENTER")` 后，游戏内 `game_started` 仍为 `false`
- 通过 `set_game_node_property` 设置 `game_started = true` 后，`_process` 中自动下落功能正常
- 说明游戏逻辑本身正确，只是输入事件未传递到游戏进程

**当前唯一的绕过方式**：`set_game_node_property` + `execute_game_script` 直接操作状态，但这完全绕过了输入系统，无法测试 `_input()` 逻辑。

**建议**：
1. 在 `mcp_runtime_agent.gd` 中增加 `cmd_simulate_key` 命令，在游戏进程内构造 `InputEventKey` 并通过 `Input.parse_input_event()` 注入
2. 或通过文件 IPC 将按键事件发送给运行时代理

---

### 2.4 get_game_screenshot 需要手动配置（中优先级）

**问题**：需要项目中存在 `MCPScreenshot` Autoload 节点才能工作，但新建项目默认不包含。

**错误信息**：
```
截图文件不存在。请确保游戏正在运行且 MCPScreenshot autoload 已激活。
路径: C:/Users/wyl/AppData/Roaming/Godot/app_userdata/新建游戏项目/mcp_screenshot.png
```

**建议**：
1. 插件初始化时自动注册 `MCPScreenshot` Autoload
2. 或改用 `capture_frames` 的实现方式（直接用 viewport 纹理截图，无需 autoload）

---

### 2.5 search_in_files 不搜索脚本文件（中优先级）

**问题**：搜索 "PieceData" 返回 0 结果，但 `tetris.gd` 中大量使用 `PieceData`。

**对比**：
- `find_node_references("ScoreLabel")` → 正确找到 `tetris.gd:22` 和 `tetris.tscn:16`
- `search_in_files("PieceData")` → 0 结果

`find_node_references` 能搜脚本文件，但 `search_in_files` 不行。

**建议**：统一底层搜索实现，或明确文档说明 `search_in_files` 的搜索范围。

---

### 2.6 edit_script 搜索字符串中的转义问题（中优先级）

**问题**：通过 `read_script` 获取的代码内容中，tab 显示为 `\t`（JSON 转义形式），用此作为 `edit_script` 的 `search` 参数时需精确匹配。

**示例**：
```json
// read_script 返回的内容
{"content": "...\t\tKEY_DOWN:\n\t\t\t_move_piece(0, 1)..."}

// edit_script 的 search 参数必须使用完全相同的 \t 转义
// 如果手写 tab 字符而非 \t，会匹配失败 (replacements: 0)
```

**实际遇到**：在一次 search/replace 中返回 `replacements: 0`，需重新 `read_script` 确认准确格式后才能匹配成功。

**建议**：
1. `edit_script` 支持模糊匹配（自动处理 tab/space 差异）
2. 或在找不到匹配时返回上下文帮助定位

---

### 2.7 connect_signal 无类型校验（低优先级）

**问题**：允许对 Label 节点连接 `pressed` 信号，但 Label 无此信号（`pressed` 属于 BaseButton）。

**结果**：
```
ERROR: In Object of type 'Label': Attempt to connect nonexistent signal 'pressed' 
to callable 'Node2D(tetris.gd)::_on_score_pressed'.
```

**连锁反应**：该错误触发了 Rust 插件的 re-entrancy bug，导致 `Gd<T>::bind_mut()` panic 循环。

**建议**：连接前校验信号是否存在于目标节点类型。

---

### 2.8 场景树路径冗长（低优先级）

**问题**：`get_scene_tree` 返回的 path 极长：

```
/root/@EditorNode@19545/@Panel@14/@VBoxContainer@15/DockVSplitMain/.../
@SubViewport@9904/Tetris/UI/ScoreLabel
```

**建议**：增加 `relative=true` 参数，返回相对于场景根的短路径（如 `Tetris/UI/ScoreLabel`）。

---

### 2.9 edit_script 缺少增量编辑能力（低优先级）

**问题**：`edit_script` 只有两种模式：
1. **search/replace**：精确文本替换，一次一个匹配
2. **content**：完整文件内容替换

无法做"在第 N 行后插入"或"删除第 N 到 M 行"等操作。

**建议**：增加 `insert_after_line` 和 `delete_lines` 参数，支持行级精确定位编辑。

---

### 2.10 运行时代理与编辑器代理的职责分离不清晰（低优先级）

**问题**：
- `get_game_scene_tree` 通过文件 IPC 走运行时代理（`mcp_runtime_agent.gd`）
- `execute_game_script` 也走运行时代理，但受 Expression 限制
- `simulate_key` 走编辑器代理，无法触达游戏进程

同一类"游戏操作"分散在不同代理中，行为不一致。

**建议**：所有游戏运行时操作统一走运行时代理，编辑器代理专注于场景编辑操作。

---

## 三、开发流程体验总结

### 优点

1. **场景编辑流畅**：`add_node`、`update_property`、`delete_node` 的增删改操作响应迅速，链式操作无需等待。
2. **脚本编译快速验证**：`validate_script` 即时反馈编译结果，编辑安全有保障。
3. **游戏状态探查**：`execute_game_script` + `set_game_node_property` 组合可绕过输入限制直接操控游戏状态。
4. **帧截图实用**：`capture_frames` 的 base64 图片可嵌入分析结果，视觉验证效果好。

### 痛点

1. **参数命名学习成本高**：32 个工具，约 6 个存在命名不一致，每次使用前必须读 schema。
2. **游戏输入测试不可行**：`simulate_key` 是最基本的功能需求，但实际不可用，导致无法端到端测试。
3. **Expression 限制是最大瓶颈**：无法在游戏运行时写调试脚本、遍历数组、聚合数据。
4. **错误恢复能力弱**：一次 connect_signal 误操作触发了 Rust 插件 panic 循环，需要 reload_plugin 才能恢复。

### 效率数据

| 指标 | 数据 |
|------|------|
| 完成功能数 | 6 项（bug 修复 + 功能添加 + 代码优化 + UI 优化） |
| MCP 工具调用次数 | 约 55 次 |
| 工具 schema 查询次数 | 约 8 次（占总调用 15%） |
| 因参数名错误重试次数 | 约 5 次 |
| 总交互轮次 | 约 10 轮 |

---

## 四、改进建议优先级排序

| 优先级 | 建议 | 影响范围 |
|--------|------|----------|
| P0 | `execute_game_script` 支持完整脚本语法 | 阻塞调试和测试 |
| P0 | `simulate_key` 在游戏进程中注入事件 | 阻塞输入测试 |
| P1 | 统一参数命名 + 添加别名兼容 | 降低学习成本 |
| P1 | `get_game_screenshot` 自动注册 autoload | 开箱即用体验 |
| P2 | `search_in_files` 覆盖脚本文件 | 代码搜索完整性 |
| P2 | `edit_script` 增加行级编辑能力 | 精确编辑体验 |
| P3 | `connect_signal` 增加信号存在性校验 | 防止误操作 |
| P3 | 场景树路径增加相对路径选项 | 可读性 |
