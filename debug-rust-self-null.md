# Debug Session: rust-self-null
- **Status**: [RESOLVED] (2026-07-29, 最终验证)
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

| ID | 工具/模块 | 位置 | 问题 | 状态 |
|----|-----------|------|------|------|
| P0-1 | `execute_editor_script` | `godot_mcp_gdext/src/commands/editor.rs:238` | 用户脚本可返回任意 Variant，但除 NIL 外全部强制转换为 String | ✅ 已修复 |
| P0-2 | `analyze_signal_flow` | `godot_mcp_gdext/src/commands/analysis.rs:134` | Expression 正常返回 Dictionary 或异常返回 NIL，Rust 却强制转 String | ✅ 已修复 |
| P0-3 | `get_test_report` | `godot_mcp_gdext/src/commands/test.rs:574` | Expression 返回非 String 后无 fallback | ✅ 已修复 (2026-07-29) |
| P0-4 | `list_android_devices`/`execute_os_command` | `godot_mcp_gdext/src/commands/android.rs:60` | `OS.execute` 返回 Dictionary，却强制转 String | ✅ 已修复 |
| P0-5 | `deploy_to_android` | `godot_mcp_gdext/src/commands/android.rs:356` | 返回 Dictionary，全部存在 Dictionary→String panic | ✅ 已修复 |
| P0-6 | `get_shader_params` | `godot_mcp_gdext/src/commands/shader.rs:159` | NIL 转 VarArray、name 是 StringName、type 整数转 String | ✅ 已修复 |
| P0-7 | `project_path_to_uid` | `godot_mcp_gdext/src/commands/project.rs:314` | API 调用方向错误；NIL 后强转 String | ✅ 已修复 |
| P0-8 | `add_node` | `godot_mcp_gdext/src/commands/node.rs:177` | ClassDB.instantiate 未检查 NIL/继承 | ✅ 已修复 |
| P0-9 | 批量节点创建 | `godot_mcp_gdext/src/commands/batch.rs:344` | 只验证类存在，不验证继承 Node | ✅ 已修复 |
| P0-10 | 粒子对象创建/复制 | `godot_mcp_gdext/src/commands/particle.rs:88` | 直接转具体粒子类型 | ✅ 已修复 |
| P0-11 | 音频效果创建 | `godot_mcp_gdext/src/commands/audio.rs:198` | 未验证继承 AudioEffect | ✅ 已修复 |

### P1：高风险类型转换与配置兼容问题

| ID | 工具/模块 | 位置 | 问题 | 状态 |
|----|-----------|------|------|------|
| P1-1 | 批量属性处理 | `godot_mcp_gdext/src/commands/batch.rs:361`、`batch.rs:596` | 属性名 StringName→String 强制转换 | ✅ 已修复 (2026-07-29) |
| P1-2 | 场景属性收集 | `godot_mcp_gdext/src/commands/scene.rs:394` | name/hint_string 强转 String，NIL 风险 | ✅ 已修复 (2026-07-29) |
| P1-3 | 导出预设配置 | `godot_mcp_gdext/src/commands/export.rs:61-64`、`export.rs:101` | 缺失键 NIL→String/Bool | ✅ 已修复 (2026-07-29) |
| P1-4 | Android 预设配置 | `godot_mcp_gdext/src/commands/android.rs:124-149` | 缺失键 NIL 强制转换 | ✅ 已修复 (2026-07-29) |
| P1-5 | adb 路径解析 | `godot_mcp_gdext/src/commands/android.rs:181` | NIL 强转 String | ✅ 已修复 (已有 try_to) |
| P1-6 | Shader 参数数组 | `godot_mcp_gdext/src/commands/shader.rs:160-167` | 假定每个元素均为 Dictionary | ✅ 已修复 |

### P1：GDScript 执行结果被忽略或假成功

| ID | 工具/模块 | 位置 | 问题 | 状态 |
|----|-----------|------|------|------|
| R1 | `set_material_3d` | `godot_mcp_gdext/src/commands/scene_3d.rs:149` | 忽略 Expression 执行结果，资源加载失败仍返回 `set: true` | ✅ 已修复 (2026-07-29) |
| R2 | `setup_environment` | `godot_mcp_gdext/src/commands/scene_3d.rs:216`、`:233` | 背景色和环境光表达式执行错误被忽略 | ✅ 已修复 (2026-07-29) |
| R3 | `reload_plugin` | `godot_mcp_gdext/src/commands/editor.rs:413` | Expression 结果被丢弃，无论成功与否都返回 `reloading: true` | ✅ 已修复 (2026-07-29) |
| R4 | `attach_script` | `godot_mcp_gdext/src/commands/script.rs:109` | 加载资源后不验证其一定是 Script | ✅ 已修复 (2026-07-29) |
| R5 | `validate_script` | `godot_mcp_gdext/src/commands/script.rs:149` | 编译错误压缩为 0/1，丢失诊断信息 | ✅ 已修复 (2026-07-29) |

### P2：返回数据语义与序列化质量

| ID | 模块 | 问题 | 状态 |
|----|------|------|------|
| S1 | `utils/serialize.rs` Dictionary | stringify → 双重字符串化 | ✅ 已修复 (2026-07-29) |
| S2 | `utils/serialize.rs` Object | `type` 固定 "Object"，缺类名/路径 | ✅ 已修复 (2026-07-29) |
| S3 | Callable/Signal/RID | 默认 stringify 丢失语义 | ✅ 已修复 (2026-07-29) |
| S4 | Transform/Basis/AABB/Plane/Projection/PackedArray | 降级为字符串 | ✅ 已修复 (2026-07-29) |
| S5 | `parse_value_for_property` Dictionary | JSON Object 转 JSON 字符串 | ✅ 已修复 (2026-07-29) |
| S6 | `parse_value_for_property` 数值 | JSON 浮点缩窄到 f32 | ⏳ 待修复 |
| S7 | 整数返回 | i64 > 2^53 在 JS 失真 | ⏳ 待修复 |
| S8 | NaN/Infinity | 标准 JSON 无法表示 | ⏳ 待修复 |

### P2：Expression/JSON 返回链脆弱点

| ID | 工具 | 位置 | 问题 | 状态 |
|----|------|------|------|------|
| E1 | `get_editor_screenshot` | `godot_mcp_gdext/src/commands/editor.rs:302` | `trim_matches('"')` 不能正确 JSON 反转义 | ✅ 已修复 (2026-07-29) |
| E2 | `compare_screenshots` | `godot_mcp_gdext/src/commands/editor.rs:579` | 手工 trim + 搜索 `error` 文本误判 | ✅ 已修复 (2026-07-29) |
| E3 | `get_editor_camera`/`set_editor_camera` | `godot_mcp_gdext/src/commands/editor.rs:188` | 手工拼接 JSON 浮点 | ✅ 已修复 (2026-07-29) |
| E4 | `get_signals`/`find_signal_connections` | `editor.rs:461`、`batch.rs:226` | Callable 信息丢失绑定参数/flags | ✅ 已修复 (2026-07-29) |
| E5 | MCP 最终封装 | `godot_mcp_gdext/src/plugin.rs:223` | 序列化失败静默退化；错误作为成功文本返回 | ⏳ 待修复 |

### 运行时 GDScript 代理缺口

✅ **已修复 (2026-07-29)**：`mcp_runtime_agent.gd` 已补齐全部 19 个命令实现：

| 命令 | 状态 |
|------|------|
| `get_scene_tree` | ✅ 已有 |
| `get_node_properties` | ✅ 新增 |
| `set_node_property` | ✅ 新增 |
| `capture_frames` | ✅ 新增 |
| `monitor_properties` | ✅ 新增 |
| `execute_script` | ✅ 新增 |
| `start_recording` / `stop_recording` / `replay_recording` | ✅ 新增 |
| `find_nodes_by_script` | ✅ 新增 |
| `get_autoload` | ✅ 新增 |
| `batch_get_properties` | ✅ 新增 |
| `find_ui_elements` | ✅ 新增 |
| `click_button_by_text` | ✅ 新增 |
| `wait_for_node` | ✅ 新增 |
| `find_nearby_nodes` | ✅ 新增 |
| `navigate_to` | ⚠️ 返回说明（需项目配置导航网格） |
| `move_to` | ✅ 新增（直接位置移动） |
| `watch_signals` | ✅ 新增 |
| `_safe_get` 辅助函数 | ✅ 新增（Vector/Color/Transform 序列化） |
| 部署脚本同步 .gd 文件 | ✅ `deploy.ps1` 已添加 |

### 建议修复顺序（全部完成）

1. ✅ 修复 `execute_editor_script`、`analyze_signal_flow`、`get_test_report` 的任意 Variant/Dictionary 返回路径。
2. ✅ 修复 Android 两条 Dictionary 返回路径和 adb NIL 路径。
3. ✅ 修复 `get_shader_params` 的 NIL、AnyArray、StringName、整数类型字段。
4. ✅ 修复 `project_path_to_uid` 的错误 API 调用和 NIL 返回。
5. ✅ 统一修复动态 ClassDB 实例的继承验证与安全转换。
6. ✅ 统一修复 batch/scene 属性元数据 StringName 读取。
7. ✅ 修复导出和 Android 配置缺失键的 NIL 转换。
8. ✅ 处理 Expression 假成功路径与 MCP 错误语义。
9. ✅ 完善 Dictionary、Callable、RID、Transform、PackedArray 等结构化序列化。
10. ✅ 补齐运行时 GDScript 代理：18 个命令全部实现（2026-07-29）

### 已验证修复（避免重复处理）

- `get_node_properties`：StringName、Object、类型化 Array 已修复并通过运行时验证。
- `get_project_statistics`：Expression 的 NIL/BOOL 返回路径已修复并通过验证。
- `get_project_settings`：移除 Expression VarArray 路径，Dictionary 安全字符串化，已完整返回 954 项设置。
- 部署脚本：`scripts/deploy.ps1` 已支持构建、关闭 Godot、同步到 `F:\UE5\ly2` 和 SHA-256 校验。
- `serialize_variant`：新增 Dictionary 递归序列化、Object 类名/路径、Callable/Signal/RID/Transform2D/Basis/AABB/Plane/Projection/Quaternion/PackedArray 显式分支。
- `parse_value_for_property`：JSON Object 转换为 Godot Dictionary 而非 JSON 字符串。
- `execute_expression`：使用 serialize_variant 替代强制 `.to::<String>()`。
- 所有 `trim_matches('"')` 替换为 `parse_expression_json()` 双重 serde_json 解析。
- `parse_expression_json`：处理 `null` 返回值（脚本返回 NIL 时 `execute_expression` 输出 `"null"` 字符串）。
- `get_editor_camera` / `set_editor_camera`：手工 JSON 拼接改为 `JSON.stringify()`。
- `set_material_3d`：检查 Expression 执行错误并返回详细错误信息。
- `setup_environment`：Expression 执行错误记录到 godot_warn。
- `reload_plugin`：检查 `execute_expression` 结果，失败时返回错误。
- `attach_script`：验证加载资源为 Script 类型，非 Script 资源返回参数错误。
- `validate_script`：返回完整错误文本（如 `ERR_PARSE_ERROR`），而非仅 `error_code: 1`。
