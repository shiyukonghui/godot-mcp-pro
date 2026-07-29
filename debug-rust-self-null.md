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

## 待修复问题总表（2026-07-28 子 Agent 联合审计）

以下内容合并了三个子 Agent 对 `Expression::execute()`、动态 GDScript 返回路径及全局 `Variant::to<T>()` 的审计结果。重复问题已合并，供后续继续修复。

### P0：可稳定触发 panic 的返回路径

| ID | 工具/模块 | 位置 | 问题 | 建议修复 |
|----|-----------|------|------|----------|
| P0-1 | `execute_editor_script` | `godot_mcp_gdext/src/commands/editor.rs:238` | 用户脚本可返回任意 Variant，但除 NIL 外全部强制转换为 String；返回 bool、int、Array、Dictionary、Object 等会 panic。 | 使用 `serialize_variant(&result)` 返回结构化值；保留 NIL；禁止无类型检查的 `.to::<String>()`。 |
| P0-2 | `analyze_signal_flow` | `godot_mcp_gdext/src/commands/analysis.rs:134` | Expression 正常返回 Dictionary 或异常返回 NIL，Rust 却强制转 String。 | GDScript 返回 `JSON.stringify(result)`，Rust 使用 `try_to::<String>()`；或直接安全序列化 Dictionary。 |
| P0-3 | `get_test_report` | `godot_mcp_gdext/src/commands/test.rs:574` | Expression 正常返回 Dictionary，随后执行 Dictionary→String。 | 在 GDScript 侧 JSON.stringify，Rust 侧安全解析并保留执行错误。 |
| P0-4 | `list_android_devices`/`execute_os_command` | `godot_mcp_gdext/src/commands/android.rs:60` | `OS.execute` 包装表达式返回 `{exit_code, stdout}` Dictionary，却强制转 String。 | 返回 JSON 字符串或按 Dictionary 安全序列化。 |
| P0-5 | `deploy_to_android` | `godot_mcp_gdext/src/commands/android.rs:356` | 成功、导出失败、安装失败均返回 Dictionary，全部存在 Dictionary→String panic。 | 统一返回 `JSON.stringify()`；Rust 使用 `try_to` 并区分结构化成功/失败。 |
| P0-6 | `get_shader_params` | `godot_mcp_gdext/src/commands/shader.rs:159` | Expression 可能返回 NIL，却强转 VarArray；参数 `name` 可能是 StringName；`type` 实际是整数，却强转 String。 | 检查 ARRAY/NIL；使用 AnyArray；名称兼容 StringName/String；类型按整数读取并映射。 |
| P0-7 | `project_path_to_uid` | `godot_mcp_gdext/src/commands/project.rs:314` | Expression API 调用方向疑似错误，将路径传给 `get_id_path`；失败返回 NIL 后强转 String。 | 使用 `get_id_for_path(path)` 后 `id_to_text(id)`；显式处理 NIL/无 UID。 |
| P0-8 | `add_node` | `godot_mcp_gdext/src/commands/node.rs:177` | `ClassDB.instantiate` 结果未检查 NIL/继承关系，直接转 `Gd<Node>`；合法但非 Node 类名可 panic。 | 验证 `class_exists` 和 `is_parent_class(type, "Node")`，使用安全转换并返回参数错误。 |
| P0-9 | 批量节点创建 | `godot_mcp_gdext/src/commands/batch.rs:344` | 只验证类存在，不验证继承 Node，随后对象强转 Node。 | 增加 Node 继承检查和 NIL 检查。 |
| P0-10 | 粒子对象创建/复制 | `godot_mcp_gdext/src/commands/particle.rs:88`、`particle.rs:121`、`particle.rs:236` | ClassDB 实例或 `duplicate()` 返回对象后直接转具体粒子类型。 | 检查类继承、对象存活和 `try_to::<Gd<T>>()` 结果。 |
| P0-11 | 音频效果创建 | `godot_mcp_gdext/src/commands/audio.rs:198` | 动态对象未验证继承 AudioEffect 就强转。 | 验证基类并安全转换。 |

### P1：高风险类型转换与配置兼容问题

| ID | 工具/模块 | 位置 | 问题 | 建议修复 |
|----|-----------|------|------|----------|
| P1-1 | 批量属性处理 | `godot_mcp_gdext/src/commands/batch.rs:361`、`batch.rs:596` | 属性元数据 `name` 在 Godot 4.7 为 StringName，仍直接转 String。 | 抽取统一属性名读取函数，兼容 StringName/String/缺字段。 |
| P1-2 | 场景属性收集 | `godot_mcp_gdext/src/commands/scene.rs:394` | 属性 `name` 可能 StringName 或 NIL；`hint_string` 也可能 NIL，却直接转 String。 | 使用 `try_to`、VariantType 分支及安全默认值。 |
| P1-3 | 导出预设配置 | `godot_mcp_gdext/src/commands/export.rs:61`-`64`、`export.rs:101` | 缺失配置键返回 NIL，代码直接转 String/Bool。手工编辑或旧版配置可触发 panic。 | 为每个字段提供默认值并使用 `try_to`。 |
| P1-4 | Android 预设配置 | `godot_mcp_gdext/src/commands/android.rs:124`、`:125`、`:139`、`:148`、`:149` | 缺失键时 NIL 被强制转标量或 String。 | 安全读取并提供默认值。 |
| P1-5 | adb 路径解析 | `godot_mcp_gdext/src/commands/android.rs:181` | Expression 预期返回 String，但运行错误时 NIL 仍被强转 String。 | 使用 `try_to::<String>()` 并返回可诊断错误。 |
| P1-6 | Shader 参数数组 | `godot_mcp_gdext/src/commands/shader.rs:160`-`:167` | 除 P0 的字段问题外，还假定每个数组元素均为 Dictionary。 | 使用 AnyArray 并逐项检查 VariantType::DICTIONARY。 |

### P1：GDScript 执行结果被忽略或假成功

| ID | 工具/模块 | 位置 | 问题 | 建议修复 |
|----|-----------|------|------|----------|
| R1 | `set_material_3d` | `godot_mcp_gdext/src/commands/scene_3d.rs:149` | 忽略 Expression 执行结果，资源加载或节点类型错误仍返回 `set: true`。 | GDScript 明确返回 `{ok,error}`，Rust 校验后再报告成功。 |
| R2 | `setup_environment` | `godot_mcp_gdext/src/commands/scene_3d.rs:216`、`:233` | 背景色和环境光表达式无显式结果，执行错误被忽略。 | 返回结构化状态并检查每一步。 |
| R3 | `reload_plugin` | `godot_mcp_gdext/src/commands/editor.rs:413` | Expression 结果被丢弃，无论启停是否成功都返回 `reloading: true`。 | 返回实际启用状态和错误。 |
| R4 | `attach_script` | `godot_mcp_gdext/src/commands/script.rs:109` | 加载资源后不验证其一定是 Script，设置后直接报告成功。 | 检查资源类型、设置结果和脚本实例化错误。 |
| R5 | `validate_script` | `godot_mcp_gdext/src/commands/script.rs:149` | 编译错误被压缩为 0/1，丢失 Godot 错误枚举及行列诊断。 | 返回错误码、文本和可获得的诊断信息。 |

### P2：返回数据语义与序列化质量

| ID | 模块 | 问题 | 建议修复 |
|----|------|------|----------|
| S1 | `utils/serialize.rs` Dictionary | 当前 Dictionary 使用 stringify，避免 panic 但返回 JSON 字符串而非对象，导致双重字符串化。 | 使用 AnyDictionary 递归转换键和值，并处理非字符串键。 |
| S2 | `utils/serialize.rs` Object | `type` 固定为 `Object`，未返回实际类名；实例 ID 可能失效。 | 安全获取真实类名、路径（若为 Node）和存活状态。 |
| S3 | Callable/Signal/RID | 默认 stringify 丢失目标对象、方法、绑定参数和标识语义。 | 为常见类型添加显式结构化序列化。 |
| S4 | Transform/Basis/AABB/Plane/Projection/PackedArray | 均降级为字符串，无法结构化读取或往返。 | 分批添加显式序列化分支。 |
| S5 | `parse_value_for_property` Dictionary | 普通 JSON Object 除 Vector/Color 外被转成 JSON 字符串，Dictionary 无法往返。 | JSON Object 转 Godot Dictionary，保留结构。 |
| S6 | `parse_value_for_property` 数值 | JSON 浮点统一缩窄到 f32，可能损失精度或溢出。 | 根据目标属性元数据选择 f32/f64 或保留 Godot float 精度。 |
| S7 | 整数返回 | i64 进入 JavaScript 客户端后超过 2^53 可能失真，实例 ID/UID 风险较高。 | 大整数或 ID 使用十进制字符串。 |
| S8 | NaN/Infinity | 标准 JSON 无法表示，当前路径缺少统一策略。 | 返回带类型标记的字符串或 null+元数据。 |

### P2：Expression/JSON 返回链脆弱点

| ID | 工具 | 位置 | 问题 | 建议修复 |
|----|------|------|------|----------|
| E1 | `get_editor_screenshot` | `godot_mcp_gdext/src/commands/editor.rs:302` | JSON 字符串使用 `trim_matches('"')`，不能正确执行 JSON 反转义。 | 使用 serde_json 解码字符串，不手工去引号。 |
| E2 | `compare_screenshots` | `godot_mcp_gdext/src/commands/editor.rs:579` | 同样手工 trim；且通过搜索 `error` 文本判断异常，可能误判。 | 严格解析结构化 JSON，检查明确字段。 |
| E3 | `get_editor_camera`/`set_editor_camera` | `godot_mcp_gdext/src/commands/editor.rs:188` | 手工拼接 JSON 浮点，NaN/Inf 或异常返回会导致解析失败。 | 使用 Dictionary + JSON.stringify 或 Rust 结构化转换。 |
| E4 | `get_signals`/`find_signal_connections` | `editor.rs:461`、`batch.rs:226` | Callable 信息只保留目标路径和方法名，绑定参数、flags、有效性丢失。 | 扩充结构化字段并安全处理失效对象。 |
| E5 | MCP 最终封装 | `godot_mcp_gdext/src/plugin.rs:223` | serde_json 结果再次放入 `content[].text`；序列化失败静默退化为 `{}`；错误有时作为成功文本返回。 | 增加 structuredContent；统一 JSON-RPC/MCP 错误语义；保留序列化错误。 |

### 运行时 GDScript 代理缺口

Rust 注册了 19 个运行时工具，但 `addons/godot_mcp_rs/mcp_runtime_agent.gd` 当前仅实现 `get_scene_tree`。其余命令（包括 `execute_game_script`）会返回 `Unknown command`。

| 范围 | 现状 | 后续建议 |
|------|------|----------|
| `execute_game_script` 等 18 个运行时工具 | Rust 侧已注册，GDScript 代理未实现。 | 补齐代理分支和结果序列化，或暂时仅注册真正可用的工具。 |
| IPC JSON 响应 | `serde_json::from_str` 安全，不会 panic；但 FileAccess 写失败会导致 Rust 超时。 | GDScript 写失败时增加可观测日志；Rust 返回明确超时上下文。 |
| `error` 字段 | Rust 只在字符串时识别，非字符串错误会被忽略。 | 接受结构化错误并统一转换为 McpError。 |

### 建议修复顺序

1. 修复 `execute_editor_script`、`analyze_signal_flow`、`get_test_report` 的任意 Variant/Dictionary 返回路径。
2. 修复 Android 两条 Dictionary 返回路径和 adb NIL 路径。
3. 修复 `get_shader_params` 的 NIL、AnyArray、StringName、整数类型字段。
4. 修复 `project_path_to_uid` 的错误 API 调用和 NIL 返回。
5. 统一修复动态 ClassDB 实例的继承验证与安全转换。
6. 统一修复 batch/scene 属性元数据 StringName 读取。
7. 修复导出和 Android 配置缺失键的 NIL 转换。
8. 处理 Expression 假成功路径与 MCP 错误语义。
9. 完善 Dictionary、Callable、RID、Transform、PackedArray 等结构化序列化。
10. 补齐运行时 GDScript 代理，或缩减工具注册范围。

### 已验证修复（避免重复处理）

- `get_node_properties`：StringName、Object、类型化 Array 已修复并通过运行时验证。
- `get_project_statistics`：Expression 的 NIL/BOOL 返回路径已修复并通过验证。
- `get_project_settings`：移除 Expression VarArray 路径，Dictionary 安全字符串化，已完整返回 36 项设置。
- 部署脚本：`scripts/deploy.ps1` 已支持构建、关闭 Godot、同步到 `F:\UE5\ly2` 和 SHA-256 校验。
