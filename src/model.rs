use anyhow::{bail, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeType {
    Range,
    Category,
    ItemDir,
    File,
    Link,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,           // stable id derived from path
    pub code: Option<String>, // JD code parsed from name
    pub title: String,
    pub path: String, // absolute filesystem path for all node types
    pub node_type: NodeType,
    pub location: Option<String>, // parsed from file contents (LOCATION=...)
    pub url: Option<String>,      // for link nodes only
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Tree {
    pub roots: Vec<Node>,
}

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
    static RE: once_cell::sync::Lazy<Regex> =
        once_cell::sync::Lazy::new(|| Regex::new(r"^(\d{2}-\d{2})_(.+)$").unwrap());
    RE.captures(name)
        .map(|c| (c[1].to_string(), c[2].replace('_', " ")))
}

pub fn parse_category(name: &str) -> Option<(String, String)> {
    static RE: once_cell::sync::Lazy<Regex> =
        once_cell::sync::Lazy::new(|| Regex::new(r"^(\d{2})_(.+)$").unwrap());
    RE.captures(name)
        .map(|c| (c[1].to_string(), c[2].replace('_', " ")))
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
    RE.captures(name).map(|c| {
        (
            c[1].to_string(),
            c[2].replace('_', " "),
            c.get(3).map(|m| m.as_str().to_string()),
        )
    })
}

pub fn validate_unique_codes_among_siblings(children: &[Node]) -> Result<()> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for ch in children {
        if let Some(code) = &ch.code {
            if !seen.insert(code.clone()) {
                bail!("Duplicate code among siblings: {}", code);
            }
        }
    }
    Ok(())
}

pub fn find_parent_id(tree: &Tree, id: &str) -> Option<String> {
    fn walk(parent: Option<&Node>, node: &Node, id: &str) -> Option<String> {
        if node.id == id {
            return parent.map(|p| p.id.clone());
        }
        for c in &node.children {
            if let Some(x) = walk(Some(node), c, id) {
                return Some(x);
            }
        }
        None
    }
    for r in &tree.roots {
        if let Some(x) = walk(None, r, id) {
            return Some(x);
        }
    }
    None
}

pub fn all_codes(tree: &Tree) -> Vec<String> {
    fn walk(node: &Node, acc: &mut Vec<String>) {
        if let Some(c) = &node.code {
            acc.push(c.clone());
        }
        for ch in &node.children {
            walk(ch, acc);
        }
    }
    let mut v = Vec::new();
    for r in &tree.roots {
        walk(r, &mut v);
    }
    v
}

pub fn suggest_next_code(tree: &Tree, parent_code: &str) -> Result<String> {
    // For a category NN, suggest NN.MM with next free MM among 01..99
    let mut used = BTreeSet::new();
    fn collect(node: &Node, parent_code: &str, used: &mut BTreeSet<String>) {
        if let Some(c) = &node.code {
            if c.starts_with(parent_code) && c.len() == 5 {
                used.insert(c.clone());
            }
        }
        for ch in &node.children {
            collect(ch, parent_code, used);
        }
    }
    for r in &tree.roots {
        collect(r, parent_code, &mut used);
    }
    for i in 1..=99u32 {
        let cand = format!("{}.{:02}", parent_code, i);
        if !used.contains(&cand) {
            return Ok(cand);
        }
    }
    bail!("No free item code under {}", parent_code)
}

pub fn find_node<'a>(tree: &'a Tree, id: &str) -> Option<&'a Node> {
    fn walk<'a>(n: &'a Node, id: &str) -> Option<&'a Node> {
        if n.id == id {
            return Some(n);
        }
        n.children.iter().find_map(|c| walk(c, id))
    }
    tree.roots.iter().find_map(|r| walk(r, id))
}

pub fn find_by_code<'a>(tree: &'a Tree, code: &str) -> Option<&'a Node> {
    fn walk<'a>(n: &'a Node, code: &str) -> Option<&'a Node> {
        if n.code.as_deref() == Some(code) {
            return Some(n);
        }
        n.children.iter().find_map(|c| walk(c, code))
    }
    tree.roots.iter().find_map(|r| walk(r, code))
}

pub fn suggest_next_category_in_range(tree: &Tree, range_code: &str) -> Result<String> {
    let (a, b) = range_code
        .split_once('-')
        .ok_or_else(|| anyhow::anyhow!("invalid range code: {}", range_code))?;
    let (start, end): (u32, u32) = (a.parse()?, b.parse()?);
    let range = find_by_code(tree, range_code)
        .ok_or_else(|| anyhow::anyhow!("range not found: {}", range_code))?;
    let used: BTreeSet<_> = range
        .children
        .iter()
        .filter_map(|n| n.code.clone())
        .collect();
    for n in start..=end {
        let c = format!("{:02}", n);
        if !used.contains(&c) {
            return Ok(c);
        }
    }
    bail!("No free category code in {}", range_code)
}

pub fn suggest_child_code(tree: &Tree, parent: &Node) -> Result<String> {
    match parent.node_type {
        NodeType::Category => suggest_next_code(
            tree,
            parent
                .code
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("category missing code"))?,
        ),
        NodeType::Range => suggest_next_category_in_range(
            tree,
            parent
                .code
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("range missing code"))?,
        ),
        NodeType::ItemDir => {
            let base = parent
                .code
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("item missing code"))?;
            let used: BTreeSet<_> = parent
                .children
                .iter()
                .filter_map(|n| n.code.clone())
                .collect();
            for n in 1..=99 {
                let c = format!("{}.{:02}", base, n);
                if !used.contains(&c) {
                    return Ok(c);
                }
            }
            bail!("No free child code under {}", base)
        }
        _ => Ok(String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, code: Option<&str>, node_type: NodeType, children: Vec<Node>) -> Node {
        Node {
            id: id.into(),
            code: code.map(|c| c.to_string()),
            title: id.into(),
            path: String::new(),
            node_type,
            location: None,
            url: None,
            children,
        }
    }

    #[test]
    fn parse_variants() {
        assert_eq!(parse_range("30-39_Research").unwrap().0, "30-39");
        assert_eq!(parse_category("30_Topic").unwrap().0, "30");
        assert_eq!(parse_item("30.01_Something.txt").unwrap().0, "30.01");
        assert_eq!(parse_item("30.011_Longer").unwrap().0, "30.011");
        assert_eq!(parse_item("30.01.02_Deep").unwrap().0, "30.01.02");
    }

    #[test]
    fn suggest_basic() {
        let t = Tree {
            roots: vec![node(
                "r",
                Some("30"),
                NodeType::Category,
                vec![node("a", Some("30.01"), NodeType::ItemDir, vec![])],
            )],
        };
        assert_eq!(suggest_next_code(&t, "30").unwrap(), "30.02");
    }
}
