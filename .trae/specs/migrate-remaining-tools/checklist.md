# 验证清单

## Phase 1: 高频率核心工具 (24 个)

### Animation 工具
- [ ] `add_animation_track` — 在 AnimationPlayer 的指定动画中添加轨道
- [ ] `set_animation_keyframe` — 在指定轨道的指定时间插入关键帧
- [ ] `get_animation_info` — 返回动画的时长、轨道数、关键帧数等信息
- [ ] `remove_animation` — 从 AnimationPlayer 中删除指定动画

### Resource 工具
- [ ] `edit_resource` — 加载 .tres/.res 文件并修改属性后保存
- [ ] `create_resource` — 实例化指定类型的 Resource 并保存到文件
- [ ] `get_resource_preview` — 返回资源的类型、大小、缩略图信息

### Editor 工具
- [ ] `get_editor_errors` — 返回编辑器错误面板中的所有错误
- [ ] `get_output_log` — 返回输出面板中的日志内容
- [ ] `get_editor_screenshot` — 返回编辑器窗口的 base64 截图
- [ ] `get_game_screenshot` — 返回游戏运行窗口的 base64 截图
- [ ] `clear_output` — 清除输出面板
- [ ] `reload_plugin` — 通过 EditorInterface 重新加载当前插件
- [ ] `reload_project` — 重新加载整个项目
- [ ] `get_signals` — 返回指定节点的所有可用信号
- [ ] `compare_screenshots` — 比较两张截图的差异并返回相似度
- [ ] `set_auto_dismiss` — 设置自动关闭参数
- [ ] `get_editor_camera` — 返回编辑器 3D 视口的相机信息
- [ ] `set_editor_camera` — 设置编辑器 3D 视口的相机位置/旋转

### Node 工具
- [ ] `add_resource` — 创建资源并附加到指定节点的属性上
- [ ] `set_anchor_preset` — 设置 Control 类型节点的锚点预设
- [ ] `get_node_groups` — 返回节点所属的所有组
- [ ] `set_node_groups` — 设置节点所属的组
- [ ] `find_nodes_in_group` — 按组名查找场景中所有属于该组的节点
- [ ] `get_editor_selection` — 返回编辑器当前选中的所有节点
- [ ] `select_nodes` — 选中场景中的指定节点
- [ ] `clear_editor_selection` — 清除编辑器中的所有选中

---

## Phase 2: 批量操作与场景管理 (9 个)

### Batch 工具
- [ ] `find_signal_connections` — 遍历场景树查找指定节点的信号连接
- [ ] `batch_add_nodes` — 批量创建并添加节点（支持数组参数）
- [ ] `find_node_references` — 查找场景中引用指定节点的所有节点
- [ ] `get_scene_dependencies` — 返回场景文件依赖的所有资源
- [ ] `cross_scene_set_property` — 跨多个场景文件批量设置属性

### Scene 工具
- [ ] `get_scene_file_content` — 读取 .tscn 文件内容并返回结构化数据
- [ ] `delete_scene` — 删除指定场景文件
- [ ] `add_scene_instance` — 将外部场景作为实例添加到当前编辑场景
- [ ] `get_scene_exports` — 返回场景根节点的导出变量列表

---

## Phase 3: 运行时系统 (19 个)

- [ ] Runtime IPC 通信层正常工作（Named Pipe / 共享内存）
- [ ] Runtime 工具的超时和重连机制正常工作
- [ ] `get_game_scene_tree` — 返回运行中游戏的场景树
- [ ] `get_game_node_properties` — 获取运行中游戏节点的属性
- [ ] `set_game_node_property` — 修改运行中游戏节点的属性
- [ ] `capture_frames` — 捕获运行中游戏的帧画面
- [ ] `monitor_properties` — 持续监控属性变化并返回
- [ ] `execute_game_script` — 在游戏进程中执行 GDScript
- [ ] `start_recording` / `stop_recording` / `replay_recording` — 录制和回放
- [ ] `find_nodes_by_script` — 按附加脚本查找节点
- [ ] `get_autoload` — 获取自动加载列表
- [ ] `batch_get_properties` — 批量获取多个节点的属性
- [ ] `find_ui_elements` — 按文本/类型查找 UI 元素
- [ ] `click_button_by_text` — 按文本查找按钮并模拟点击
- [ ] `wait_for_node` — 等待指定节点出现（带超时）
- [ ] `find_nearby_nodes` — 查找指定节点附近的节点
- [ ] `navigate_to` / `move_to` — 导航/移动到目标位置
- [ ] `watch_signals` — 监听指定节点的信号

---

## Phase 4: 专用模块 (约 53 个)

### 新建模块 — theme
- [ ] `create_theme` — 创建新的 Theme 资源
- [ ] `set_theme_color` — 设置主题颜色
- [ ] `set_theme_constant` — 设置主题常量
- [ ] `set_theme_font_size` — 设置主题字体大小
- [ ] `set_theme_stylebox` — 设置主题样式盒
- [ ] `setup_control` — 对 Control 节点应用主题配置
- [ ] `get_theme_info` — 获取主题信息

### 新建模块 — animation_tree
- [ ] `create_animation_tree` — 创建 AnimationTree 节点
- [ ] `get_animation_tree_structure` — 获取动画树结构
- [ ] `add_state_machine_state` / `remove_state_machine_state` — 管理状态机状态
- [ ] `add_state_machine_transition` / `remove_state_machine_transition` — 管理状态机过渡
- [ ] `set_blend_tree_node` — 设置混合树节点参数
- [ ] `set_tree_parameter` — 设置动画树参数

### 新建模块 — navigation
- [ ] `setup_navigation_region` — 设置导航区域
- [ ] `bake_navigation_mesh` — 烘焙导航网格
- [ ] `setup_navigation_agent` — 设置导航代理
- [ ] `set_navigation_layers` — 设置导航层
- [ ] `get_navigation_info` — 获取导航信息

### 新建模块 — particle
- [ ] `create_particles` — 创建粒子系统节点
- [ ] `set_particle_material` — 设置粒子材质
- [ ] `set_particle_color_gradient` — 设置粒子颜色渐变
- [ ] `apply_particle_preset` — 应用粒子预设
- [ ] `get_particle_info` — 获取粒子系统信息

### 新建模块 — analysis
- [ ] `find_unused_resources` — 查找未使用的资源
- [ ] `analyze_signal_flow` — 分析信号连接流程
- [ ] `analyze_scene_complexity` — 分析场景复杂度
- [ ] `find_script_references` — 查找脚本引用
- [ ] `detect_circular_dependencies` — 检测循环依赖
- [ ] `get_project_statistics` — 获取项目统计信息

### 新建模块 — test
- [ ] `run_test_scenario` — 运行测试场景
- [ ] `assert_node_state` — 断言节点状态
- [ ] `assert_screen_text` — 断言屏幕文本
- [ ] `run_stress_test` — 运行压力测试
- [ ] `get_test_report` — 获取测试报告

### 新建模块 — android
- [ ] `list_android_devices` — 列出连接的 Android 设备
- [ ] `get_android_preset_info` — 获取 Android 导出预设信息
- [ ] `deploy_to_android` — 部署到 Android 设备

### 扩展模块
- [ ] [project] `get_filesystem_tree`, `search_files`, `search_in_files`
- [ ] [project] `get_project_settings`, `set_project_setting`
- [ ] [project] `uid_to_project_path`, `project_path_to_uid`
- [ ] [shader] `edit_shader`, `assign_shader_material`
- [ ] [shader] `set_shader_param`, `get_shader_params`
- [ ] [physics] `setup_collision`, `set_physics_layers`, `get_physics_layers`
- [ ] [physics] `setup_physics_body`, `get_collision_info`
- [ ] [audio] `get_audio_bus_layout`, `add_audio_bus`, `set_audio_bus`, `add_audio_bus_effect`
- [ ] [tilemap] `tilemap_set_cell`, `tilemap_fill_rect`, `tilemap_get_cell`
- [ ] [scene_3d] `set_material_3d`, `setup_environment`, `add_gridmap`
- [ ] [export] `list_export_presets`, `export_project`
- [ ] [script] `get_open_scripts`, `validate_script`
- [ ] [profiling] `get_editor_performance`
- [ ] [input] `simulate_sequence`

---

## 集成验证
- [ ] `cargo build` 编译通过（所有阶段）
- [ ] `list_tools` 返回全部 171 个工具（Rust 端）
- [ ] 所有 48 个已迁移工具功能未退化
- [ ] 迁移完成后 GDScript 命令文件不再被依赖，可安全移除
