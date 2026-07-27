# 任务列表

> 说明: 每个 Task 对应一个可独立编译和验证的交付单元。
> 标记 [新建模块] 的需要创建新的 `.rs` 文件并注册到 `mod.rs`。
> 标记 [扩展模块] 的在已有 `.rs` 文件中追加实现。

---

## Task Dependencies

- Phase 2 依赖 Phase 1 完成 (mod.rs 路由和工具风格已确立)
- Phase 3 (Runtime) 依赖 Phase 1-2 奠定基础
- Phase 4 无前置依赖，可在任意阶段并行
- Phase 4 中的 [新建模块] 需先创建 `.rs` 文件再填充工具

---

## [x] Task 1: 基础设施 — 统一工具注册和数据模型

- [ ] SubTask 1.1: 确认现有 `mod.rs` 中工具注册模式（`collect_tools` + `register` + `command_fn`），提取为宏或辅助函数以简化新模块接入
- [ ] SubTask 1.2: 确认 `serialize.rs` 中 `parse_value_for_property` 和 `serialize_variant` 覆盖所有 GDScript 端使用的类型
- [ ] SubTask 1.3: 确认 `error.rs` 错误码覆盖所有场景（特别是截图/IPC/文件系统场景）
- [ ] SubTask 1.4: 编写基础模块模板注释文档，供后续开发参考

**验证**: `cargo build` 通过，现有 48 个工具功能不受影响。

---

## [x] Task 2: Phase 1 — Animation 工具扩展 (+4)

- [x] SubTask 2.1: 在 `animation.rs` 中实现 `add_animation_track` — 创建 AnimationTrack 并添加到动画
- [x] SubTask 2.2: 在 `animation.rs` 中实现 `set_animation_keyframe` — 在指定轨道上插入关键帧
- [x] SubTask 2.3: 在 `animation.rs` 中实现 `get_animation_info` — 返回动画时长、轨道数、关键帧数
- [x] SubTask 2.4: 在 `animation.rs` 中实现 `remove_animation` — 从 AnimationPlayer 中删除动画

---

## [x] Task 3: Phase 1 — Resource 工具扩展 (+3)

- [x] SubTask 3.1: 在 `resource.rs` 中实现 `edit_resource` — 加载资源、修改属性后保存
- [x] SubTask 3.2: 在 `resource.rs` 中实现 `create_resource` — 实例化指定类型的 Resource
- [x] SubTask 3.3: 在 `resource.rs` 中实现 `get_resource_preview` — 生成资源的缩略图/预览信息

---

## [x] Task 4: Phase 1 — Editor 工具扩展 (+12)

- [x] SubTask 4.1: 实现 `get_editor_errors` — 通过 EditorInterface 获取编辑器错误列表
- [x] SubTask 4.2: 实现 `get_output_log` — 读取 OutputPanel 的内容
- [x] SubTask 4.3: 实现 `get_editor_screenshot` — 截取编辑器窗口并返回 base64
- [x] SubTask 4.4: 实现 `get_game_screenshot` — 截取游戏运行窗口并返回 base64
- [x] SubTask 4.5: 实现 `clear_output` — 清除输出面板
- [x] SubTask 4.6: 实现 `reload_plugin` / `reload_project` — 通过 EditorInterface 重新加载
- [x] SubTask 4.7: 实现 `get_signals` — 获取指定节点的信号列表
- [x] SubTask 4.8: 实现 `compare_screenshots` — 两张截图的结构相似度对比
- [x] SubTask 4.9: 实现 `set_auto_dismiss` — 设置自动关闭参数
- [x] SubTask 4.10: 实现 `get_editor_camera` / `set_editor_camera` — 获取/设置编辑器相机

---

## [x] Task 5: Phase 1 — Node 工具扩展 (+8)

- [x] SubTask 5.1: 实现 `add_resource` — 创建资源并附加到指定节点
- [x] SubTask 5.2: 实现 `set_anchor_preset` — 设置 Control 节点的锚点预设
- [x] SubTask 5.3: 实现 `get_node_groups` / `set_node_groups` — 管理节点分组
- [x] SubTask 5.4: 实现 `find_nodes_in_group` — 按组名查找所有节点
- [x] SubTask 5.5: 实现 `get_editor_selection` / `select_nodes` / `clear_editor_selection` — 编辑器选中管理

---

## [x] Task 6: Phase 2 — Batch 工具扩展 (+5)

- [x] SubTask 6.1: 在 `batch.rs` 中实现 `find_signal_connections` — 遍历节点查找信号连接
- [x] SubTask 6.2: 在 `batch.rs` 中实现 `batch_add_nodes` — 批量创建和添加节点
- [x] SubTask 6.3: 在 `batch.rs` 中实现 `find_node_references` — 查找节点引用关系
- [x] SubTask 6.4: 在 `batch.rs` 中实现 `get_scene_dependencies` — 获取场景依赖资源
- [x] SubTask 6.5: 在 `batch.rs` 中实现 `cross_scene_set_property` — 跨场景设置属性

---

## [x] Task 7: Phase 2 — Scene 工具补充 (+4)

- [x] SubTask 7.1: 在 `scene.rs` 中实现 `get_scene_file_content` — 读取场景文件的 JSON/文本内容
- [x] SubTask 7.2: 在 `scene.rs` 中实现 `delete_scene` — 删除指定场景文件
- [x] SubTask 7.3: 在 `scene.rs` 中实现 `add_scene_instance` — 将场景作为实例添加到当前场景
- [x] SubTask 7.4: 在 `scene.rs` 中实现 `get_scene_exports` — 获取场景导出变量列表

---

## [x] Task 8: Phase 3 — Runtime 运行时通信基础设施

- [x] SubTask 8.1: 设计 IPC 协议（JSON 行协议 over Named Pipe / 共享内存）
- [x] SubTask 8.2: 在 `runtime.rs` 中实现 IPC 客户端，连接运行中游戏进程的 `mcp_runtime_agent.gd`
- [x] SubTask 8.3: 实现异步请求-响应模式（tokio::sync::oneshot），处理超时和重连

**复杂度**: 高。Runtime 工具需与游戏进程通信，GDExtension 侧只能通过 IPC 间接访问。

---

## [x] Task 9: Phase 3 — Runtime 工具实现 (19 个)

- [x] SubTask 9.1: 实现场景树和属性相关: `get_game_scene_tree`, `get_game_node_properties`, `set_game_node_property`
- [x] SubTask 9.2: 实现录制/回放: `capture_frames`, `start_recording`, `stop_recording`, `replay_recording`
- [x] SubTask 9.3: 实现监控相关: `monitor_properties`, `watch_signals`
- [x] SubTask 9.4: 实现脚本执行和查找: `execute_game_script`, `find_nodes_by_script`, `get_autoload`
- [x] SubTask 9.5: 实现 UI 自动化: `find_ui_elements`, `click_button_by_text`, `wait_for_node`, `find_nearby_nodes`
- [x] SubTask 9.6: 实现批量操作: `batch_get_properties`
- [x] SubTask 9.7: 实现导航: `navigate_to`, `move_to`

---

## [x] Task 10: Phase 4 — 新建模块 (theme, animation_tree, navigation, particle)

- [x] SubTask 10.1: [新建模块] 创建 `theme.rs`，实现 7 个工具
- [x] SubTask 10.2: [新建模块] 创建 `animation_tree.rs`，实现 8 个工具
- [x] SubTask 10.3: [新建模块] 创建 `navigation.rs`，实现 5 个工具
- [x] SubTask 10.4: [新建模块] 创建 `particle.rs`，实现 5 个工具

---

## [x] Task 11: Phase 4 — 新建模块 (analysis, test, android)

- [x] SubTask 11.1: [新建模块] 创建 `analysis.rs`，实现 6 个工具
- [x] SubTask 11.2: [新建模块] 创建 `test.rs`，实现 5 个工具
- [x] SubTask 11.3: [新建模块] 创建 `android.rs`，实现 3 个工具

---

## [x] Task 12: Phase 4 — 扩展已有模块

- [x] SubTask 12.1: [project.rs] 补充 7 个工具 (filesystem搜索、项目设置)
- [x] SubTask 12.2: [shader.rs] 补充 4 个工具 (编辑、分配材质、参数管理)
- [x] SubTask 12.3: [physics.rs] 补充 5 个工具 (碰撞、物理层、物理体)
- [x] SubTask 12.4: [audio.rs] 补充 4 个工具 (音频总线管理)
- [x] SubTask 12.5: [tilemap.rs] 补充 3 个工具 (单元格操作)
- [x] SubTask 12.6: [scene_3d.rs] 补充 3 个工具 (材质、环境、GridMap)
- [x] SubTask 12.7: [export.rs] 补充 2 个工具 (预设列表、导出)
- [x] SubTask 12.8: [script.rs] 补充 2 个工具 (打开脚本、验证脚本)
- [x] SubTask 12.9: [profiling.rs] 补充 1 个工具 (编辑器性能)
- [x] SubTask 12.10: [input.rs] 补充 1 个工具 (输入序列)

---

## [ ] Task 13: 集成测试与回归验证

- [ ] SubTask 13.1: 验证 Rust 端 `list_tools` 返回全部 171 个工具
- [ ] SubTask 13.2: 每个迁移的工具至少一次端到端调用测试
- [ ] SubTask 13.3: 确认 GDScript 端无功能性退化
- [ ] SubTask 13.4: 编译 Release 版本并部署测试
