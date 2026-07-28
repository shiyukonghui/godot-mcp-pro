# Debug Session: rust-self-null
- **Status**: [OPEN]
- **Issue**: GDScript 调用 RustMcpPlugin::poll_mcp() 时实例 self 为 null，并触发 NIL 到 ARRAY 转换失败
- **Debug Server**: http://127.0.0.1:7777/event
- **Log File**: .dbg/trae-debug-log-rust-self-null.ndjson

## Reproduction Steps
1. 在 Godot 编辑器中启用 Godot MCP RS 插件。
2. 等待插件执行每帧轮询。
3. 观察 plugin.gd:32 重复报告 Invalid call error code 1337。

## Hypotheses & Verification
| ID | Hypothesis | Likelihood | Effort | Evidence |
|----|------------|------------|--------|----------|
| A | ClassDB.instantiate 创建的 EditorPlugin 没有有效实例绑定 | High | Low | Rejected |
| B | poll_mcp 的 delta 参数触发错误的参数数组转换 | Medium | Low | Rejected |
| C | GDScript 静态类型或动态分派导致实例绑定丢失 | Medium | Low | Rejected |
| D | 手动调用 _enter_tree 使 Rust 实例生命周期异常 | High | Medium | Rejected |
| E | 已加载 DLL 与脚本或构建产物不一致 | Medium | Low | Rejected |
| F | 工具执行后的 Output 面板递归遍历遇到失效编辑器节点 | High | Low | Rejected |
| G | 属性元数据 name 是 StringName，但代码强制转换为 String | High | Low | Confirmed |
| H | 属性值 StringName 在通用序列化器中落入 String 强制转换 | High | Low | Confirmed |
| I | OBJECT 和其他未覆盖 Variant 类型被强制转换为 String | High | Low | Confirmed |
| J | 类型化 Array 被强制转换为无类型 VarArray | High | Low | Confirmed |
| K | 项目统计 Expression 的 NIL/BOOL 结果被强制转换为 String | High | Low | Confirmed |
| L | 项目设置 Expression 的 NIL 结果被强制转换为 VarArray | High | Low | Confirmed |
| M | Dictionary 设置值被通用序列化器强制转换为 String | High | Low | Confirmed |

## Log Evidence
- `plugin.gd:after-instantiate`: RustMcpPlugin 有效，instance_id=1514093483818，且已注册 `_enter_tree` 与 `poll_mcp`。
- `plugin.gd:before-poll`: 调用前实例仍有效且 instance_id 未变化，delta 为 FLOAT。
- MCP `tools/list` 已通过 `poll_mcp` 返回完整工具列表。
- MCP `tools/call(get_project_info)` 已通过 `poll_mcp` 返回项目数据。
- MCP `tools/call(get_output_log)` 返回日志文件内容。
- MCP `tools/call(get_editor_errors)` 返回零条结构化错误。
- MCP `tools/call(get_node_properties, UI/ScoreLabel)` 超时，并稳定复现 panic。
- Godot 报错明确指出 `cannot convert from STRING_NAME to STRING: &"ScoreLabel"`。
- 对应代码为 `node.rs` 中属性列表 `name` 的 `.to::<String>()` 强制转换。
- 首次修复后仍显示 `ScoreLabel` 转换失败；代码审计确认已越过元数据读取，但 `serialize_variant` 缺少 STRING_NAME 分支，序列化节点 `name` 属性时再次执行 String 强制转换。
- 第二次修复后错误推进为 `cannot convert from OBJECT to STRING: VariantGd { class: Node2D }`，确认通用序列化器的 OBJECT/default 分支存在相同问题。
- 第三次修复后错误推进为 `expected array of type Untyped, got Builtin(NODE_PATH)`，确认类型化数组不能按 VarArray 读取。
- 扩展测试中 `get_project_statistics` 触发 `NIL to STRING`；定位到 Expression 结果的无条件字符串转换，且同函数还存在 BOOL to STRING 风险。
- `get_project_settings` 触发 `NIL to ARRAY`；定位到 Expression 结果无条件转换为 VarArray，已改为直接遍历 ProjectSettings 属性列表。
- 项目设置列表修复后错误推进为 `DICTIONARY to STRING`；定位到通用序列化器 Dictionary 分支，已改用 Variant::stringify()。

## Verification Conclusion
根因是通用序列化器对 Godot 4.7 严格 Variant 类型处理不足。

- Pre-fix：`get_node_properties("UI/ScoreLabel")` 依次触发 StringName→String、Object→String、Array[NodePath]→VarArray panic，HTTP 请求超时。
- Post-fix：相同请求完整返回 Label 的全部属性，包括 StringName、Object 和空的 Array[NodePath]。
- 扩展验证：根节点 `.` 与 `UI` 节点的完整属性查询均成功返回，无超时。
- 当前结论：修复已通过运行时验证，等待用户确认后清理调试插桩、记录和服务器。
- 扩展结论：节点与场景工具已通过；项目统计暴露独立 Expression 类型转换问题，已实施安全转换，等待部署验证。
- 项目统计与项目设置已在修复后完整返回。
- 扩展通过：文件系统树、脚本列表、循环依赖、输入动作、音频信息、音频总线、导航信息、碰撞信息、节点组、未使用资源扫描。
