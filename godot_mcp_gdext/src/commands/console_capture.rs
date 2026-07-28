//! 控制台输出捕获模块
//!
//! 通过遍历编辑器场景树，获取 Output 面板中 RichTextLabel 的文本内容。
//! 用于在工具调用前后捕获 Godot 的控制台输出增量。

use godot::classes::{EditorInterface, Node, RichTextLabel};
use godot::obj::Gd;
use godot::prelude::*;

use crate::utils::error::McpError;

/// 最大递归深度，防止无限递归
const MAX_DEPTH: u32 = 10;

/// 捕获编辑器 Output 面板当前的全部文本内容
///
/// 通过 EditorInterface 获取编辑器基控件，递归查找名为 "Output" 的节点，
/// 然后从中找到 RichTextLabel 子节点，获取解析后的文本。
pub fn capture_output_panel() -> Result<String, McpError> {
    let editor = EditorInterface::singleton();
    let base = editor
        .get_base_control()
        .ok_or_else(|| McpError::internal("无法获取编辑器基控件"))?;

    // 递归查找名为 "Output" 的节点（EditorLog）
    // find_child 默认 recursive=true, owned=false
    let output_node = base
        .find_child("Output")
        .ok_or_else(|| McpError::internal("未找到 Output 面板节点"))?;

    // 递归查找 RichTextLabel 子节点
    let rtl = find_rtl_recursive(&output_node, 0)
        .ok_or_else(|| McpError::internal("未在 Output 面板中找到 RichTextLabel"))?;

    Ok(rtl.get_parsed_text().to_string())
}

/// 递归查找 RichTextLabel
///
/// 从指定节点开始，递归遍历子节点，找到第一个 RichTextLabel。
fn find_rtl_recursive(node: &Gd<Node>, depth: u32) -> Option<Gd<RichTextLabel>> {
    if depth > MAX_DEPTH {
        return None;
    }

    // 尝试将当前节点转换为 RichTextLabel（godot 0.5.x 中 try_cast 返回 Result，消耗所有权）
    if let Ok(rtl) = node.clone().try_cast::<RichTextLabel>() {
        return Some(rtl);
    }

    // 递归遍历子节点
    for child in node.get_children().iter_shared() {
        if let Some(found) = find_rtl_recursive(&child, depth + 1) {
            return Some(found);
        }
    }

    None
}

/// 计算输出增量（新输出行）
///
/// 比较当前输出内容与上一次捕获的内容，返回新增的行。
pub fn compute_output_delta(current: &str, last: &str) -> Vec<String> {
    if current == last {
        return Vec::new();
    }

    // 如果当前内容以之前的内容开头，则增量在后面
    if current.starts_with(last) && !last.is_empty() {
        let new_part = &current[last.len()..];
        let lines: Vec<String> = new_part
            .split('\n')
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();
        return lines;
    }

    // 如果无法简单比较，从逐行比较找到差异起始位置
    let current_lines: Vec<&str> = current.split('\n').collect();
    let last_lines: Vec<&str> = last.split('\n').collect();

    // 从头部开始逐行比较，找到第一个不同的位置
    let mut diff_start = 0;
    for i in 0..current_lines.len().min(last_lines.len()) {
        if current_lines[i] != last_lines[i] {
            diff_start = i;
            break;
        }
    }

    // 如果完全遍历都没发现不同，说明 last 比 current 长（清空了？）
    if diff_start == 0 && current == last {
        return Vec::new();
    }

    current_lines[diff_start..]
        .iter()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
