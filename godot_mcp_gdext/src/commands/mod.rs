//! 命令路由器

use std::collections::HashMap;

use crate::mcp::protocol::ToolDefinition;
use crate::utils::error::McpError;

mod project;
mod scene;
mod node;
mod editor;
mod runtime;
mod profiling;
mod script;
mod input;
mod batch;
mod animation;
mod tilemap;
mod resource;
mod export;
mod shader;
mod physics;
mod scene_3d;
mod audio;
mod theme;
mod animation_tree;
mod navigation;
mod particle;
mod analysis;
mod test;
mod android;

/// 控制台输出捕获工具
pub mod console_capture;

type CommandFn = fn(&serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError>;

static COMMAND_REGISTRY: std::sync::OnceLock<HashMap<String, CommandFn>> = std::sync::OnceLock::new();

fn get_registry() -> &'static HashMap<String, CommandFn> {
    COMMAND_REGISTRY.get_or_init(|| {
        let mut m = HashMap::new();
        register_all(&mut m);
        m
    })
}

pub fn collect_all_tools() -> Vec<ToolDefinition> {
    let mut tools = Vec::new();
    tools.extend(project::collect_tools());
    tools.extend(scene::collect_tools());
    tools.extend(node::collect_tools());
    tools.extend(editor::collect_tools());
    tools.extend(runtime::collect_tools());
    tools.extend(profiling::collect_tools());
    tools.extend(script::collect_tools());
    tools.extend(input::collect_tools());
    tools.extend(batch::collect_tools());
    tools.extend(animation::collect_tools());
    tools.extend(tilemap::collect_tools());
    tools.extend(resource::collect_tools());
    tools.extend(export::collect_tools());
    tools.extend(shader::collect_tools());
    tools.extend(physics::collect_tools());
    tools.extend(scene_3d::collect_tools());
    tools.extend(audio::collect_tools());
    tools.extend(theme::collect_tools());
    tools.extend(animation_tree::collect_tools());
    tools.extend(navigation::collect_tools());
    tools.extend(particle::collect_tools());
    tools.extend(analysis::collect_tools());
    tools.extend(test::collect_tools());
    tools.extend(android::collect_tools());
    tools
}

fn register_all(registry: &mut HashMap<String, CommandFn>) {
    project::register(registry);
    scene::register(registry);
    node::register(registry);
    editor::register(registry);
    runtime::register(registry);
    profiling::register(registry);
    script::register(registry);
    input::register(registry);
    batch::register(registry);
    animation::register(registry);
    tilemap::register(registry);
    resource::register(registry);
    export::register(registry);
    shader::register(registry);
    physics::register(registry);
    scene_3d::register(registry);
    audio::register(registry);
    theme::register(registry);
    animation_tree::register(registry);
    navigation::register(registry);
    particle::register(registry);
    analysis::register(registry);
    test::register(registry);
    android::register(registry);
}

pub fn execute_tool(name: &str, args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let registry = get_registry();
    match registry.get(name) {
        Some(func) => func(args),
        None => Err(McpError::method_not_found(name)),
    }
}
