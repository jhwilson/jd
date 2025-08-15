use crate::model::{Tree, Node};
use anyhow::{Result, bail};

pub fn resolve_code_to_path(tree: &Tree, code: &str) -> Result<std::path::PathBuf> {
    fn walk<'a>(node: &'a Node, code: &str) -> Option<&'a Node> {
        if node.code.as_deref() == Some(code) { return Some(node); }
        for ch in &node.children { if let Some(n) = walk(ch, code) { return Some(n); } }
        None
    }
    for r in &tree.roots { if let Some(n) = walk(r, code) { return Ok(std::path::PathBuf::from(&n.path)); } }
    bail!("Code not found: {}", code)
}


