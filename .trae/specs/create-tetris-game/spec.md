# 俄罗斯方块游戏 Spec

## Why

通过 Godot MCP 工具在 Godot 编辑器中实现一个完整的俄罗斯方块游戏，验证 MCP 工具集在游戏开发中的可用性、稳定性与性能，同时记录各工具的使用体验以指导后续优化。

## What Changes

- 创建俄罗斯方块游戏主场景 (`res://tetris/tetris.tscn`)
- 创建游戏主脚本 (`res://tetris/tetris.gd`) — 包含核心游戏逻辑
- 创建方块脚本 (`res://tetris/piece.gd`) — 管理单个方块的行为
- 创建 UI 场景和脚本 (`res://tetris/ui/`) — 显示分数、游戏状态
- 使用 TileMap 构建游戏面板（10x20 网格）
- 实现 7 种标准方块（I, O, T, S, Z, J, L）
- 实现方块移动、旋转、碰撞检测、消行、计分、游戏结束

## Impact

- 创建新文件: 场景、脚本、资源
- 验证 MCP 工具: `create_scene`, `add_node`, `create_script`, `edit_script`, `attach_script`, `update_property`, `open_scene`, `save_scene`, `play_scene` 等

## 工具评估指标

在实现过程中，记录以下指标：
1. **调用成功率** — 每次工具调用是否成功返回
2. **响应时间** — 从调用到收到响应的大致耗时
3. **易用性** — 参数设计是否直观、文档是否清晰
4. **稳定性** — 是否存在超时、异常、不一致等问题
5. **功能完整性** — 工具能否覆盖开发需求

## Requirements

### Requirement: 游戏场景搭建
系统 SHALL 创建俄罗斯方块的主游戏场景，包含：
- 10x20 的游戏面板（TileMap）
- 下一个方块预览区域
- 分数显示
- 游戏状态显示（运行中/暂停/结束）

#### Scenario: 创建主场景
- **WHEN** 调用 create_scene 创建 tetris.tscn
- **THEN** 场景文件被成功创建并可打开

### Requirement: 游戏逻辑
系统 SHALL 实现标准的俄罗斯方块游戏逻辑：
- 7 种方块的生成与旋转
- 方块左右移动和加速下落
- 碰撞检测（边界、已放置方块）
- 满行消除并计分
- 游戏结束判定

### Requirement: 用户交互
系统 SHALL 支持键盘操作：
- 方向键左右：水平移动
- 方向键上：旋转
- 方向键下：加速下落

## 工具使用记录

在 tasks.md 中详细记录每个 MCP 工具调用的体验数据。
