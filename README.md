# Godot MCP Pro

Rust 实现的 MCP (Model Context Protocol) 服务，为 AI 驱动的 Godot 游戏开发提供支持。AI 助手通过 HTTP 直接连接到 Godot 编辑器，内置 **175+ 个工具**。

> 本项目基于 [Godot MCP Pro](https://buymeacoffee.com/y1uda/extras) (作者 y1uda) 的 Rust 重实现，感谢原作者的开创性工作。

## 架构

```
AI Assistant —HTTP POST /mcp—→ Godot GDExtension (内置 HTTP 服务器 :9877)
                                   └─ 主线程处理队列 → Godot Editor API
```

- **零中介**: 无需 Node.js 服务器、无需桥接器，插件内置 HTTP 服务器
- **直接集成**: 完整访问 Godot Editor API、UndoRedo 系统、场景树
- **JSON-RPC 2.0**: 标准协议，完善的错误码和响应

## 仓库内容

本项目是 **Godot MCP Pro** 的 Rust 重实现（GDExtension），包含：

- `godot_mcp_gdext/` — Rust GDExtension 源码，内置 HTTP MCP 服务器
- `addons/godot_mcp_rs/` — Godot 插件目录（编译后的 DLL + 配置）
- `mcp_bridge/` — （可选）兼容 stdio 模式的桥接器，仅在需要 stdio 传输时使用

> 本实现基于 [Godot MCP Pro](https://buymeacoffee.com/y1uda/extras) (原作者 y1uda) 的 GDScript/Node.js 版本重写为纯 Rust，感谢原作者的开创性工作。

## 快速开始

### 1. 部署插件

将 `addons/godot_mcp_rs/` 目录复制到你的 Godot 项目的 `addons/` 目录下。

> 如果是首次使用，需要先编译 GDExtension。运行部署脚本：
> ```bash
> # 构建 Rust 并复制 DLL 到插件目录
> powershell -File scripts/deploy.ps1
> ```

### 2. 启用插件

打开 Godot 编辑器，进入 **Project → Project Settings → Plugins**，找到 **Godot MCP RS**，点击 **Enable**。

启用后，输出面板会显示启动信息：

```
[MCP-RS] === Godot MCP RS 启动完成 ===
[MCP-RS] HTTP 端口 9877, 已注册 174 个工具
[MCP-RS] 🌐 HTTP 服务器监听 127.0.0.1:9877 (AI 助手可直接 POST /mcp)
```

### 3. 配置 AI 客户端

AI 助手通过 HTTP POST 请求与 Godot 插件通信，端点为：

```
POST http://127.0.0.1:9877/mcp
Content-Type: application/json

{ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }
```

#### Claude Code / Cline 配置

如果 AI 客户端支持 MCP over HTTP，可以直接配置为 HTTP 端点：

```json
{
  "mcpServers": {
    "godot-mcp": {
      "type": "http",
      "url": "http://127.0.0.1:9877/mcp"
    }
  }
}
```

#### 通用 HTTP 客户端

任何支持 HTTP 的 AI 助手均可直接调用。例如在 Python 中测试：

```python
import requests, json

# 列出所有工具
rsp = requests.post("http://127.0.0.1:9877/mcp", json={
    "jsonrpc": "2.0", "id": 1, "method": "tools/list"
})
print(rsp.json())
```

### 4. 配置端口（可选）

默认端口为 **9876**（MCP 端口），HTTP 端口为 **9877**（MCP 端口 + 1）。

可通过 Project Settings 添加自定义端口：

```
godot_mcp/port = 9876   # MCP 端口
                          # HTTP 端口自动为 9877
```

### 5. 使用 AI 助手操作 Godot 编辑器

插件启用后，AI 助手即可：

- 查询项目信息、场景树
- 创建/删除/修改节点和属性
- 编辑脚本和 Shader
- 管理动画、TileMap、导航等
- 运行时调试和监控

> **无需桥接器**：插件内置 HTTP 服务器，AI 助手直接 POST 请求即可通信，无需 `mcp_bridge` 或 Node.js 环境。

## All 175 Tools

### Project Tools (7)
| Tool | Description |
|------|-------------|
| `get_project_info` | Project metadata, version, viewport, autoloads |
| `get_filesystem_tree` | Recursive file tree with filtering |
| `search_files` | Fuzzy/glob file search |
| `get_project_settings` | Read project.godot settings |
| `set_project_setting` | Set project settings via editor API |
| `uid_to_project_path` | UID → res:// conversion |
| `project_path_to_uid` | res:// → UID conversion |

### Scene Tools (9)
| Tool | Description |
|------|-------------|
| `get_scene_tree` | Live scene tree with hierarchy |
| `get_scene_file_content` | Raw .tscn file content |
| `create_scene` | Create new scene files |
| `open_scene` | Open scene in editor |
| `delete_scene` | Delete scene file |
| `add_scene_instance` | Instance scene as child node |
| `play_scene` | Run scene (main/current/custom) |
| `stop_scene` | Stop running scene |
| `save_scene` | Save current scene to disk |

### Node Tools (17)
| Tool | Description |
|------|-------------|
| `add_node` | Add node with type and properties |
| `delete_node` | Delete node (with undo support) |
| `duplicate_node` | Duplicate node and children |
| `move_node` | Move/reparent node |
| `update_property` | Set any property (auto type parsing) |
| `get_node_properties` | Get all node properties |
| `add_resource` | Add Shape/Material/etc to node |
| `set_anchor_preset` | Set Control anchor preset |
| `rename_node` | Rename a node in the scene |
| `connect_signal` | Connect signal between nodes |
| `disconnect_signal` | Disconnect signal connection |
| `get_node_groups` | Get groups a node belongs to |
| `set_node_groups` | Set node group membership |
| `find_nodes_in_group` | Find all nodes in a group |
| `get_editor_selection` | Get currently selected scene nodes |
| `select_nodes` | Select, focus, and inspect scene nodes |
| `clear_editor_selection` | Clear the editor scene selection |

### Script Tools (8)
| Tool | Description |
|------|-------------|
| `list_scripts` | List all scripts with class info |
| `read_script` | Read script content |
| `create_script` | Create new script with template |
| `edit_script` | Search/replace or full edit |
| `attach_script` | Attach script to node |
| `get_open_scripts` | List scripts open in editor |
| `validate_script` | Validate GDScript syntax |
| `search_in_files` | Search content in project files |

### Editor Tools (9)
| Tool | Description |
|------|-------------|
| `get_editor_errors` | Get errors and stack traces |
| `get_editor_screenshot` | Capture editor viewport |
| `get_game_screenshot` | Capture running game |
| `execute_editor_script` | Run arbitrary GDScript in editor |
| `clear_output` | Clear output panel |
| `get_signals` | Get all signals of a node with connections |
| `reload_plugin` | Reload the MCP plugin (auto-reconnect) |
| `reload_project` | Rescan filesystem and reload scripts |
| `get_output_log` | Get output panel content |

### Input Tools (7)
| Tool | Description |
|------|-------------|
| `simulate_key` | Simulate keyboard key press/release |
| `simulate_mouse_click` | Simulate mouse click at position |
| `simulate_mouse_move` | Simulate mouse movement |
| `simulate_action` | Simulate Godot Input Action |
| `simulate_sequence` | Sequence of input events with frame delays |
| `get_input_actions` | List all input actions |
| `set_input_action` | Create/modify input action |

### Runtime Tools (19)
| Tool | Description |
|------|-------------|
| `get_game_scene_tree` | Scene tree of running game |
| `get_game_node_properties` | Node properties in running game |
| `set_game_node_property` | Set node property in running game |
| `execute_game_script` | Run GDScript in game context |
| `capture_frames` | Multi-frame screenshot capture |
| `monitor_properties` | Record property values over time |
| `start_recording` | Start input recording |
| `stop_recording` | Stop input recording |
| `replay_recording` | Replay recorded input |
| `find_nodes_by_script` | Find game nodes by script |
| `get_autoload` | Get autoload node properties |
| `batch_get_properties` | Batch get multiple node properties |
| `find_ui_elements` | Find UI elements in game |
| `click_button_by_text` | Click button by text content |
| `wait_for_node` | Wait for node to appear |
| `find_nearby_nodes` | Find nodes near position |
| `navigate_to` | Navigate to target position |
| `move_to` | Walk character to target |

### Animation Tools (6)
| Tool | Description |
|------|-------------|
| `list_animations` | List all animations in AnimationPlayer |
| `create_animation` | Create new animation |
| `add_animation_track` | Add track (value/position/rotation/method/bezier) |
| `set_animation_keyframe` | Insert keyframe into track |
| `get_animation_info` | Detailed animation info with all tracks/keys |
| `remove_animation` | Remove an animation |

### TileMap Tools (6)
| Tool | Description |
|------|-------------|
| `tilemap_set_cell` | Set a single tile cell |
| `tilemap_fill_rect` | Fill rectangular region with tiles |
| `tilemap_get_cell` | Get tile data at cell |
| `tilemap_clear` | Clear all cells |
| `tilemap_get_info` | TileMapLayer info and tile set sources |
| `tilemap_get_used_cells` | List of used cells |

### Theme & UI Tools (6)
| Tool | Description |
|------|-------------|
| `create_theme` | Create Theme resource file |
| `set_theme_color` | Set theme color override |
| `set_theme_constant` | Set theme constant override |
| `set_theme_font_size` | Set theme font size override |
| `set_theme_stylebox` | Set StyleBoxFlat override |
| `get_theme_info` | Get theme overrides info |

### Profiling Tools (2)
| Tool | Description |
|------|-------------|
| `get_performance_monitors` | All performance monitors (FPS, memory, physics, etc.) |
| `get_editor_performance` | Quick performance summary |

### Batch & Refactoring Tools (8)
| Tool | Description |
|------|-------------|
| `find_nodes_by_type` | Find all nodes of a type |
| `find_signal_connections` | Find all signal connections in scene |
| `batch_set_property` | Set property on all nodes of a type |
| `find_node_references` | Search project files for pattern |
| `get_scene_dependencies` | Get resource dependencies |
| `cross_scene_set_property` | Set property across all scenes |
| `find_script_references` | Find where script/resource is used |
| `detect_circular_dependencies` | Find circular scene dependencies |

### Shader Tools (6)
| Tool | Description |
|------|-------------|
| `create_shader` | Create shader with template |
| `read_shader` | Read shader file |
| `edit_shader` | Edit shader (replace/search-replace) |
| `assign_shader_material` | Assign ShaderMaterial to node |
| `set_shader_param` | Set shader parameter |
| `get_shader_params` | Get all shader parameters |

### Export Tools (3)
| Tool | Description |
|------|-------------|
| `list_export_presets` | List export presets |
| `export_project` | Get export command for preset |
| `get_export_info` | Export-related project info |

### Resource Tools (6)
| Tool | Description |
|------|-------------|
| `read_resource` | Read .tres resource properties |
| `edit_resource` | Edit resource properties |
| `create_resource` | Create new .tres resource |
| `get_resource_preview` | Get resource thumbnail |
| `add_autoload` | Register autoload singleton |
| `remove_autoload` | Remove autoload singleton |

### Physics Tools (6)
| Tool | Description |
|------|-------------|
| `setup_physics_body` | Configure physics body properties |
| `setup_collision` | Add collision shapes to nodes |
| `set_physics_layers` | Set collision layer/mask |
| `get_physics_layers` | Get collision layer/mask info |
| `get_collision_info` | Get collision shape details |
| `add_raycast` | Add RayCast2D/3D node |

### 3D Scene Tools (6)
| Tool | Description |
|------|-------------|
| `add_mesh_instance` | Add MeshInstance3D with primitive mesh |
| `setup_camera_3d` | Configure Camera3D properties |
| `setup_lighting` | Add/configure light nodes |
| `setup_environment` | Configure WorldEnvironment |
| `add_gridmap` | Set up GridMap node |
| `set_material_3d` | Set StandardMaterial3D properties |

### Particle Tools (5)
| Tool | Description |
|------|-------------|
| `create_particles` | Create GPUParticles2D/3D |
| `set_particle_material` | Configure ParticleProcessMaterial |
| `set_particle_color_gradient` | Set color gradient for particles |
| `apply_particle_preset` | Apply preset (fire, smoke, sparks, etc.) |
| `get_particle_info` | Get particle system details |

### Navigation Tools (6)
| Tool | Description |
|------|-------------|
| `setup_navigation_region` | Configure NavigationRegion |
| `setup_navigation_agent` | Configure NavigationAgent |
| `bake_navigation_mesh` | Bake navigation mesh |
| `set_navigation_layers` | Set navigation layers |
| `get_navigation_info` | Get navigation setup info |

### Audio Tools (6)
| Tool | Description |
|------|-------------|
| `add_audio_player` | Add AudioStreamPlayer node |
| `add_audio_bus` | Add audio bus |
| `add_audio_bus_effect` | Add effect to audio bus |
| `set_audio_bus` | Configure audio bus properties |
| `get_audio_bus_layout` | Get audio bus layout info |
| `get_audio_info` | Get audio-related node info |

### AnimationTree Tools (4)
| Tool | Description |
|------|-------------|
| `create_animation_tree` | Create AnimationTree |
| `get_animation_tree_structure` | Get tree structure |
| `set_tree_parameter` | Set AnimationTree parameter |
| `add_state_machine_state` | Add state to state machine |

### State Machine Tools (3)
| Tool | Description |
|------|-------------|
| `remove_state_machine_state` | Remove state from state machine |
| `add_state_machine_transition` | Add transition between states |
| `remove_state_machine_transition` | Remove state transition |

### Blend Tree Tools (1)
| Tool | Description |
|------|-------------|
| `set_blend_tree_node` | Configure blend tree nodes |

### Analysis & Search Tools (4)
| Tool | Description |
|------|-------------|
| `analyze_scene_complexity` | Analyze scene performance |
| `analyze_signal_flow` | Map signal connections |
| `find_unused_resources` | Find unreferenced resources |
| `get_project_statistics` | Get project-wide statistics |

### Testing & QA Tools (6)
| Tool | Description |
|------|-------------|
| `run_test_scenario` | Run automated test scenario |
| `assert_node_state` | Assert node property values |
| `assert_screen_text` | Check for text on screen |
| `compare_screenshots` | Compare two screenshots |
| `run_stress_test` | Run performance stress test |
| `get_test_report` | Get test results report |

## 核心特性

- **零中介架构**: HTTP 服务器直接集成在 GDExtension 中，无需 Node.js/WebSocket 桥接
- **UndoRedo 集成**: 所有节点/属性操作支持 Ctrl+Z 撤销
- **智能类型解析**: `"Vector2(100, 200)"`, `"#ff0000"`, `"Color(1,0,0)"` 自动转换
- **纯 Rust 实现**: 利用 Godot 4 GDExtension 的 Rust 绑定，高性能、低资源占用
- **完善的错误处理**: 错误响应包含下一步操作建议

## Competitive Comparison

### Tool Count

| Category | Godot MCP Pro | GDAI MCP ($19) | tomyud1 (free) | Dokujaa (free) | Coding-Solo (free) | ee0pdt (free) | bradypp (free) |
|----------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| Project | 7 | 5 | 4 | 0 | 2 | 2 | 2 |
| Scene | 9 | 8 | 11 | 9 | 3 | 4 | 5 |
| Node | **14** | 8 | 0 | 8 | 2 | 3 | 0 |
| Script | **8** | 5 | 6 | 4 | 0 | 5 | 0 |
| Editor | **9** | 5 | 1 | 5 | 1 | 3 | 2 |
| Input | **7** | 2 | 0 | 0 | 0 | 0 | 0 |
| Runtime | **19** | 0 | 0 | 0 | 0 | 0 | 0 |
| Animation | **6** | 0 | 0 | 0 | 0 | 0 | 0 |
| TileMap | **6** | 0 | 0 | 0 | 0 | 0 | 0 |
| Theme/UI | **6** | 0 | 0 | 0 | 0 | 0 | 0 |
| Profiling | **2** | 0 | 0 | 0 | 0 | 0 | 0 |
| Batch/Refactor | **8** | 0 | 0 | 0 | 0 | 0 | 0 |
| Shader | **6** | 0 | 0 | 0 | 0 | 0 | 0 |
| Export | **3** | 0 | 0 | 0 | 0 | 0 | 0 |
| Resource | **6** | 0 | 0 | 0 | 0 | 0 | 0 |
| Physics | **6** | 0 | 0 | 0 | 0 | 0 | 0 |
| 3D Scene | **6** | 0 | 0 | 0 | 0 | 0 | 0 |
| Particle | **5** | 0 | 0 | 0 | 0 | 0 | 0 |
| Navigation | **6** | 0 | 0 | 0 | 0 | 0 | 0 |
| Audio | **6** | 0 | 0 | 0 | 0 | 0 | 0 |
| AnimationTree | **4** | 0 | 0 | 0 | 0 | 0 | 0 |
| State Machine | **3** | 0 | 0 | 0 | 0 | 0 | 0 |
| Blend Tree | **1** | 0 | 0 | 0 | 0 | 0 | 0 |
| Analysis | **4** | 0 | 0 | 0 | 0 | 0 | 0 |
| Testing/QA | **6** | 0 | 0 | 0 | 0 | 0 | 0 |
| Asset/AI | 0 | 0 | 1 | 6 | 0 | 0 | 0 |
| Material | 0 | 0 | 0 | 2 | 0 | 0 | 0 |
| Other | 0 | 0 | 9 | 5 | 5 | 2 | 1 |
| Android Deploy | **3** | 0 | 0 | 0 | 0 | 0 | 0 |
| **Total** | **175** | ~30 | **32** | **39** | **13** | **19** | **10** |

### Feature Matrix

| Feature | Godot MCP Pro (Rust) | GDAI MCP ($19) | tomyud1 (free) | Dokujaa (free) | Coding-Solo (free) |
|---------|:---:|:---:|:---:|:---:|:---:|
| **Connection** | HTTP (零中介, 内置服务器) | stdio (Python) | WebSocket | TCP Socket | Headless CLI |
| **Implementation** | Rust GDExtension | GDScript + Python | GDScript + WebSocket | GDScript | GDScript |
| **Undo/Redo** | Yes | Yes | No | No | No |
| **JSON-RPC 2.0** | Yes | Custom | Custom | Custom | N/A |
| **Error suggestions** | Yes (contextual hints) | No | No | No | No |
| **Screenshot capture** | Yes (editor + game) | Yes | No | No | No |
| **Game input simulation** | Yes (key/mouse/action/sequence) | Yes (basic) | No | No | No |
| **Runtime inspection** | Yes (scene tree + properties + monitor) | No | No | No | No |
| **Signal management** | Yes (connect/disconnect/inspect) | No | No | No | No |
| **Browser visualizer** | No | No | Yes | No | No |
| **AI 3D mesh generation** | No | No | No | Yes (Meshy API) | No |

### Exclusive Categories (No Competitor Has These)

| Category | Tools | Why It Matters |
|----------|-------|----------------|
| **Animation** | 6 tools | Create animations, add tracks, set keyframes — all programmatically |
| **TileMap** | 6 tools | Set cells, fill rects, query tile data — essential for 2D level design |
| **Theme/UI** | 6 tools | StyleBox, colors, fonts — build UI themes without manual editor work |
| **Profiling** | 2 tools | FPS, memory, draw calls, physics — performance monitoring |
| **Batch/Refactor** | 8 tools | Find by type, batch property changes, cross-scene updates, dependency analysis |
| **Shader** | 6 tools | Create/edit shaders, assign materials, set parameters |
| **Export** | 3 tools | List presets, get export commands, check templates |
| **Physics** | 6 tools | Set up collision shapes, bodies, raycasts, and layer management |
| **3D Scene** | 6 tools | Add meshes, cameras, lights, environment, GridMap support |
| **Particle** | 5 tools | Create particles with custom materials, presets, and gradients |
| **Navigation** | 6 tools | Configure navigation regions, agents, pathfinding, baking |
| **Audio** | 6 tools | Complete audio bus system, effects, players, live management |
| **AnimationTree** | 4 tools | Build state machines with transitions and blend trees |
| **State Machine** | 3 tools | Advanced state machine management for complex animations |
| **Testing/QA** | 6 tools | Automated testing, assertions, stress testing, screenshot comparison |
| **Runtime** | 19 tools | Inspect and control game at runtime: inspect, record, replay, navigate |

### 架构优势

| Aspect | Godot MCP Pro (Rust) | Typical Competitor |
|--------|--------------|-------------------|
| **Protocol** | JSON-RPC 2.0 over HTTP | Custom JSON or CLI-based |
| **Connection** | 零中介 HTTP (内置 GDExtension) | Per-command subprocess or raw TCP |
| **Implementation** | Rust (GDExtension 原生) | GDScript + Node.js/Python |
| **Type Safety** | Smart type parsing (Vector2, Color, Rect2, hex colors) | String-only or limited types |
| **Error Handling** | Structured errors with codes + suggestions | Generic error messages |
| **Undo Support** | All mutations go through UndoRedo system | Direct modifications (no undo) |

## License

本项目为 **Godot MCP Pro** 的 Rust 重实现，保留原作者版权声明。

- 原始项目 [Godot MCP Pro](https://buymeacoffee.com/y1uda/extras) (作者 y1uda) — 付费许可
- Rust 重实现部分遵循原始项目的许可条款

---

> 特别感谢 [y1uda](https://buymeacoffee.com/y1uda/extras) 创建了 Godot MCP Pro 这一优秀的工具，本 Rust 移植版本正是建立在其开创性工作之上。
