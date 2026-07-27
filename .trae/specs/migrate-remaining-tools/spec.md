# 工具迁移至 Rust 规格文档

## Why

当前 Godot MCP 项目混合使用 GDScript (171 个工具) 和 Rust GDExtension (48 个已迁移工具)。剩余约 123 个 GDScript 工具仍依赖 GDScript 插件，存在性能瓶颈和维护成本高的问题。迁移至 Rust 可带来性能提升、类型安全和更好的错误处理。

## What Changes

- 将全部约 123 个 GDScript 工具迁移到 `godot_mcp_gdext/src/commands/` 下的 Rust 模块
- 最终目标: 完全替换 GDScript 实现，所有功能由 Rust 提供
- 按 4 个阶段分批迁移，每阶段完成后即可独立交付使用
- 新建 7 个 Rust 模块: `theme`, `animation_tree`, `navigation`, `particle`, `analysis`, `test`, `android`
- 扩展现有 17 个 Rust 模块，填充未实现工具
- 所有 Rust 工具通过 `handler.rs` 统一注册到 MCP Tool List

## 迁移策略

```
阶段进行, 每阶段可独立验证:
  Phase 1 ───► Phase 2 ───► Phase 3 ───► Phase 4
  24 tools      9 tools      19 tools      71 tools
  (高频率使用)  (批量操作)    (最复杂)      (专用模块)
```

每阶段的交付物:
1. 完整的 Rust 模块文件 (collect_tools + register + 命令函数)
2. 更新 `commands/mod.rs` 注册新模块
3. (如需) 更新 `mcp/handler.rs` 中 collect_all_tools 调用
4. 编译通过 (`cargo build`)

## Impact

- 迁移完成后 Rust 端工具数: 48 + 123 = 171（覆盖率 100%）
- 与当前 GDScript 版本完全功能对等
- 迁移完成后可完全移除 GDScript 命令实现，减少维护成本
- 后续所有新功能只需在 Rust 侧添加

## 详细迁移列表

### Phase 1: 高频率核心工具 (24 个)

#### Animation (+4 个)
| 工具 | 说明 |
|------|------|
| `add_animation_track` | 添加动画轨道 |
| `set_animation_keyframe` | 设置关键帧 |
| `get_animation_info` | 获取动画信息 |
| `remove_animation` | 删除动画 |

#### Resource (+3 个)
| 工具 | 说明 |
|------|------|
| `edit_resource` | 编辑资源文件 |
| `create_resource` | 创建新资源 |
| `get_resource_preview` | 获取资源预览 |

#### Editor (+12 个)
| 工具 | 说明 |
|------|------|
| `get_editor_errors` | 获取编辑器错误列表 |
| `get_output_log` | 获取输出日志 |
| `get_editor_screenshot` | 编辑器截图 |
| `get_game_screenshot` | 游戏运行截图 |
| `clear_output` | 清除输出面板 |
| `reload_plugin` | 重新加载插件 |
| `reload_project` | 重新加载项目 |
| `get_signals` | 获取可用信号列表 |
| `compare_screenshots` | 截图对比 |
| `set_auto_dismiss` | 设置自动关闭 |
| `get_editor_camera` | 获取编辑器相机 |
| `set_editor_camera` | 设置编辑器相机 |

#### Node (+8 个)
| 工具 | 说明 |
|------|------|
| `add_resource` | 添加资源到节点 |
| `set_anchor_preset` | 设置锚点预设 |
| `get_node_groups` | 获取节点分组 |
| `set_node_groups` | 设置节点分组 |
| `find_nodes_in_group` | 按组查找节点 |
| `get_editor_selection` | 获取编辑器选中节点 |
| `select_nodes` | 选中节点 |
| `clear_editor_selection` | 清除选中 |

---

### Phase 2: 批量操作与场景管理 (9 个)

#### Batch (+5 个)
| 工具 | 说明 |
|------|------|
| `find_signal_connections` | 查找信号连接 |
| `batch_add_nodes` | 批量添加节点 |
| `find_node_references` | 查找节点引用 |
| `get_scene_dependencies` | 获取场景依赖 |
| `cross_scene_set_property` | 跨场景设置属性 |

#### Scene (+4 个)
| 工具 | 说明 |
|------|------|
| `get_scene_file_content` | 获取场景文件内容 |
| `delete_scene` | 删除场景 |
| `add_scene_instance` | 添加场景实例 |
| `get_scene_exports` | 获取场景导出信息 |

---

### Phase 3: 运行时系统 (19 个)

#### Runtime (19 个)
| 工具 | 说明 | 迁移难度 |
|------|------|----------|
| `get_game_scene_tree` | 获取游戏场景树 | 高 (需 IPC) |
| `get_game_node_properties` | 获取游戏节点属性 | 高 (需 IPC) |
| `set_game_node_property` | 设置游戏节点属性 | 高 (需 IPC) |
| `capture_frames` | 捕获游戏帧 | 高 (需 IPC) |
| `monitor_properties` | 监控属性变化 | 高 (需 IPC) |
| `execute_game_script` | 执行游戏脚本 | 高 (需 IPC) |
| `start_recording` | 开始录制 | 高 (需 IPC) |
| `stop_recording` | 停止录制 | 高 (需 IPC) |
| `replay_recording` | 回放录制 | 高 (需 IPC) |
| `find_nodes_by_script` | 按脚本查找节点 | 中 |
| `get_autoload` | 获取自动加载 | 中 |
| `batch_get_properties` | 批量获取属性 | 中 |
| `find_ui_elements` | 查找 UI 元素 | 中 |
| `click_button_by_text` | 按文本点击按钮 | 中 |
| `wait_for_node` | 等待节点出现 | 中 |
| `find_nearby_nodes` | 查找附近节点 | 中 |
| `navigate_to` | 导航到目标 | 高 (需路径规划) |
| `move_to` | 移动到目标 | 高 (需路径规划) |
| `watch_signals` | 监听信号 | 中 |

---

### Phase 4: 专用模块 (约 53 个)

#### 新建模块 (无 Rust 文件, 需新建)

| 模块 | 工具数 | 说明 |
|------|--------|------|
| `theme` | 7 | create_theme, set_theme_color, set_theme_constant, set_theme_font_size, set_theme_stylebox, setup_control, get_theme_info |
| `animation_tree` | 8 | create_animation_tree, get_animation_tree_structure, add_state_machine_state, remove_state_machine_state, add_state_machine_transition, remove_state_machine_transition, set_blend_tree_node, set_tree_parameter |
| `navigation` | 5 | setup_navigation_region, bake_navigation_mesh, setup_navigation_agent, set_navigation_layers, get_navigation_info |
| `particle` | 5 | create_particles, set_particle_material, set_particle_color_gradient, apply_particle_preset, get_particle_info |
| `analysis` | 6 | find_unused_resources, analyze_signal_flow, analyze_scene_complexity, find_script_references, detect_circular_dependencies, get_project_statistics |
| `test` | 5 | run_test_scenario, assert_node_state, assert_screen_text, run_stress_test, get_test_report |
| `android` | 3 | list_android_devices, get_android_preset_info, deploy_to_android |

#### 扩展现有模块 (已在 Rust 中注册, 补充剩余工具)

| 模块 | 当前/总计 | 待迁移 | 待迁移工具 |
|------|-----------|--------|------------|
| `project` | 3/10 | 7 | get_filesystem_tree, search_files, search_in_files, get_project_settings, set_project_setting, uid_to_project_path, project_path_to_uid |
| `shader` | 2/6 | 4 | edit_shader, assign_shader_material, set_shader_param, get_shader_params |
| `physics` | 1/6 | 5 | setup_collision, set_physics_layers, get_physics_layers, setup_physics_body, get_collision_info |
| `audio` | 2/6 | 4 | get_audio_bus_layout, add_audio_bus, set_audio_bus, add_audio_bus_effect |
| `tilemap` | 3/6 | 3 | tilemap_set_cell, tilemap_fill_rect, tilemap_get_cell |
| `scene_3d` | 3/6 | 3 | set_material_3d, setup_environment, add_gridmap |
| `export` | 1/3 | 2 | list_export_presets, export_project |
| `script` | 5/7 | 2 | get_open_scripts, validate_script |
| `profiling` | 1/2 | 1 | get_editor_performance |
| `input` | 4/5 | 1 | simulate_sequence |

---

## ADDED Requirements

### Phase 1: 高频率核心工具迁移

#### Requirement: Animation 工具扩展
- **WHEN** 用户调用 `add_animation_track`
- **THEN** Rust handler 创建动画轨道并返回结果

#### Requirement: Resource 工具扩展
- **WHEN** 用户调用 `edit_resource`
- **THEN** Rust handler 加载并编辑资源文件

#### Requirement: Editor 工具扩展
- **WHEN** 用户调用 `get_editor_errors`
- **THEN** Rust handler 从 EditorInterface 获取错误列表
- **WHEN** 用户调用 `get_output_log`
- **THEN** Rust handler 读取输出面板内容
- **WHEN** 用户调用截图相关工具
- **THEN** Rust handler 截取编辑器/游戏画面并返回 base64

#### Requirement: Node 工具扩展
- **WHEN** 用户调用 `add_resource`
- **THEN** Rust handler 创建资源并附加到节点
- **WHEN** 用户调用节点分组工具
- **THEN** Rust handler 通过 Node API 管理分组

### Phase 2: 批量操作与场景管理迁移

#### Requirement: Batch 工具扩展
- **WHEN** 用户调用 `find_signal_connections`
- **THEN** Rust handler 遍历节点树查找信号连接
- **WHEN** 用户调用跨场景工具
- **THEN** Rust handler 跨场景操作资源

#### Requirement: Scene 工具补充
- **WHEN** 用户调用 `delete_scene` / `add_scene_instance`
- **THEN** Rust handler 通过 EditorInterface 管理场景文件

### Phase 3: 运行时系统迁移

#### Requirement: Runtime 运行时通信
- **WHEN** 用户调用运行时工具
- **THEN** Rust handler 通过 IPC (文件/命名管道) 与运行中的游戏进程通信
- **NOTE**: 运行时 GDScript 脚本 `mcp_runtime_agent.gd` 作为 Autoload 驻留游戏进程
- **NOTE**: IPC 协议使用 JSON 行协议通过 Named Pipe 传输

### Phase 4: 专用模块迁移

#### Requirement: Theme 主题管理
- **WHEN** 用户调用主题工具
- **THEN** Rust handler 创建和编辑 Theme 资源

#### Requirement: Navigation 导航系统
- **WHEN** 用户调用导航工具
- **THEN** Rust handler 创建 NavigationRegion3D, NavigationAgent 等

#### Requirement: Particle 粒子系统
- **WHEN** 用户调用粒子工具
- **THEN** Rust handler 创建 GPUParticles2D/3D 并设置材质

#### Requirement: Analysis 代码分析
- **WHEN** 用户调用分析工具
- **THEN** Rust handler 遍历文件系统进行静态分析

#### Requirement: Test 测试自动化
- **WHEN** 用户调用测试工具
- **THEN** Rust handler 执行测试场景并断言节点状态

#### Requirement: Android 部署
- **WHEN** 用户调用 Android 工具
- **THEN** Rust handler 通过 EditorExportPlatformAndroid API 管理部署

## REMOVED Requirements

无——迁移过程中 GDScript 实现保持可用；迁移完成后所有 GDScript 命令文件 (`addons/godot_mcp/commands/*.gd`) 将被移除，仅保留 Rust 实现。
