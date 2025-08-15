use crate::model::{Tree, Node, NodeType};
use std::collections::BTreeSet;
use regex::Regex;

pub fn flatten_to_tsv(tree: &Tree, filter: Option<&str>, expanded: &ExpandedState, show_all: bool, collapse_root: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let matcher = filter.map(|q| build_fuzzy(q));
    for root in &tree.roots {
        push_node(&mut lines, root, 0, None, matcher.as_ref(), expanded, show_all, collapse_root);
    }
    lines
}

fn glyph(is_dir_like: bool, expanded: bool) -> &'static str { if is_dir_like { if expanded { "▾" } else { "▸" } } else { " " } }

fn push_node(lines: &mut Vec<String>, node: &Node, depth: usize, parent_id: Option<&str>, matcher: Option<&Regex>, expanded: &ExpandedState, show_all: bool, collapse_root: bool) {
    let is_dir_like = matches!(node.node_type, NodeType::Range | NodeType::Category | NodeType::ItemDir | NodeType::Other);
    let is_root = depth == 0;
    let is_expanded = if show_all { true } else if is_root { !collapse_root } else { expanded.is_expanded(&node.id) };
    let code_prefix = node.code.as_deref().unwrap_or("");
    let space = if code_prefix.is_empty() { "" } else { " " };
    let display = format!("{}{} {}{}{}", "  ".repeat(depth), glyph(is_dir_like, is_expanded), code_prefix, space, node.title);
    let id = &node.id;
    let typ = match node.node_type { NodeType::Range|NodeType::Category|NodeType::ItemDir|NodeType::Other => "dir", NodeType::File => "file", NodeType::Link => "link" };
    let parent_id = parent_id.unwrap_or("");
    let code = node.code.as_deref().unwrap_or("");
    let path_or_url = match node.node_type { NodeType::Link => node.url.as_deref().unwrap_or(&node.path), _ => &node.path };
    let hay = format!("{} {} {}", display, code, path_or_url);
    if matcher.map(|re| re.is_match(&hay)).unwrap_or(true) {
        lines.push(format!("{}\t{}\t{}\t{}\t{}", typ, id, display, path_or_url, parent_id));
    }
    if is_dir_like && is_expanded { for ch in &node.children { push_node(lines, ch, depth+1, Some(id), matcher, expanded, show_all, collapse_root); } }
}

fn build_fuzzy(q: &str) -> Regex {
    // simple subsequence fuzzy match: q chars appear in order, ignoring other chars
    let mut pat = String::from("(?i)");
    for ch in q.chars() {
        let esc = regex::escape(&ch.to_string());
        pat.push_str(&format!(".*{}", esc));
    }
    Regex::new(&pat).unwrap()
}

pub struct ExpandedState { pub expanded: BTreeSet<String> }
impl ExpandedState { pub fn is_expanded(&self, id: &str) -> bool { self.expanded.contains(id) } pub fn toggle(&mut self, id: &str) { if !self.expanded.insert(id.to_string()) { self.expanded.remove(id); } } }


