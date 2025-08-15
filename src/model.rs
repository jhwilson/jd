use serde::{Serialize, Deserialize};
use anyhow::{Result, bail};
use regex::Regex;
use std::collections::{BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeType { Range, Category, ItemDir, File, Link, Other }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,           // stable id derived from path
    pub code: Option<String>, // JD code parsed from name
    pub title: String,
    pub path: String,         // absolute filesystem path for all node types
    pub node_type: NodeType,
    pub location: Option<String>, // parsed from file contents (LOCATION=...)
    pub url: Option<String>,  // for link nodes only
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Tree { pub roots: Vec<Node> }

pub fn make_id(path: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = path.metadata() {
            return format!("ino:{}:{}", meta.dev(), meta.ino());
        }
    }
    // Fallback: hash of canonical path
    use sha1::{Digest, Sha1};
    let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let s = canon.to_string_lossy();
    let mut hasher = Sha1::new();
    hasher.update(s.as_bytes());
    let h = hasher.finalize();
    format!("sha1:{:x}", h)
}

pub fn parse_range(name: &str) -> Option<(String, String)> {
    static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| Regex::new(r"^(\d{2}-\d{2})_(.+)$").unwrap());
    RE.captures(name).map(|c| (c[1].to_string(), c[2].replace('_', " ")))
}

pub fn parse_category(name: &str) -> Option<(String, String)> {
    static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| Regex::new(r"^(\d{2})_(.+)$").unwrap());
    RE.captures(name).map(|c| (c[1].to_string(), c[2].replace('_', " ")))
}

pub fn parse_item(name: &str) -> Option<(String, String, Option<String>)> {
    // Accept codes like:
    // - NN.MM_Title (classic)
    // - NN.MMM_Title or NN.MMMM_Title (longer item codes)
    // - NN.MM.KK_Title (deeper segments; additional segments are two digits)
    // Files may have an extension captured in group 3
    static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
        Regex::new(r"^(\d{2}\.\d{2,4}(?:\.\d{2})*)_(.+?)(?:\.(.+))?$").unwrap()
    });
    RE.captures(name).map(|c| (c[1].to_string(), c[2].replace('_', " "), c.get(3).map(|m| m.as_str().to_string())))
}

pub fn validate_unique_codes_among_siblings(children: &[Node]) -> Result<()> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for ch in children {
        if let Some(code) = &ch.code {
            if !seen.insert(code.clone()) { bail!("Duplicate code among siblings: {}", code); }
        }
    }
    Ok(())
}

pub fn find_parent_id(tree: &Tree, id: &str) -> Option<String> {
    fn walk(parent: Option<&Node>, node: &Node, id: &str) -> Option<String> {
        if node.id == id { return parent.map(|p| p.id.clone()); }
        for c in &node.children { if let Some(x) = walk(Some(node), c, id) { return Some(x); } }
        None
    }
    for r in &tree.roots { if let Some(x) = walk(None, r, id) { return Some(x); } }
    None
}

pub fn all_codes(tree: &Tree) -> Vec<String> {
    fn walk(node: &Node, acc: &mut Vec<String>) {
        if let Some(c) = &node.code { acc.push(c.clone()); }
        for ch in &node.children { walk(ch, acc); }
    }
    let mut v = Vec::new();
    for r in &tree.roots { walk(r, &mut v); }
    v
}

pub fn suggest_next_code(tree: &Tree, parent_code: &str) -> Result<String> {
    // For a category NN, suggest NN.MM with next free MM among 01..99
    let mut used = BTreeSet::new();
    fn collect(node: &Node, parent_code: &str, used: &mut BTreeSet<String>) {
        if let Some(c) = &node.code { if c.starts_with(parent_code) && c.len() == 5 { used.insert(c.clone()); } }
        for ch in &node.children { collect(ch, parent_code, used); }
    }
    for r in &tree.roots { collect(r, parent_code, &mut used); }
    for i in 1..=99u32 {
        let cand = format!("{}.{:02}", parent_code, i);
        if !used.contains(&cand) { return Ok(cand); }
    }
    bail!("No free item code under {}", parent_code)
}

// end


