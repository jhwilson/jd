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
    // From this directory's .jdmeta: where else this number lives
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<crate::meta::MetaLink>,
    #[serde(default)]
    pub has_notes: bool,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Tree {
    pub roots: Vec<Node>,
    // Non-fatal scan findings, e.g. duplicate sibling codes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
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

/// Duplicate check for one directory's children. Children reusing the
/// directory's own code (`12.02_notes.pdf` inside `12.02_Quenched_Dark_Spot/`)
/// follow the JD convention of stamping an item's contents with its number
/// and are not duplicates.
pub fn validate_unique_codes_among_siblings(
    children: &[Node],
    parent_code: Option<&str>,
) -> Result<()> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for ch in children {
        if let Some(code) = &ch.code {
            if Some(code.as_str()) == parent_code {
                continue;
            }
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

/// How many places outside this folder the number lives in: .jdmeta
/// locations and links, plus child link items and child LOCATION= file
/// items. Used to pick which duplicate is cheaper to renumber.
pub fn drawer_count(n: &Node) -> usize {
    n.locations.len()
        + n.links.len()
        + n.children
            .iter()
            .filter(|c| {
                matches!(c.node_type, NodeType::Link)
                    || (matches!(c.node_type, NodeType::File) && c.location.is_some())
            })
            .count()
}

/// Codes used by more than one node within the same root (siblings or not —
/// a stray category 21 inside 30-39 still collides with the real 21).
/// Nodes reusing an ancestor's code (files stamped with their item's number
/// inside its folder) are the JD convention, not duplicates, and are
/// excluded. Returns (code, ids) groups in code order.
pub fn duplicate_groups(tree: &Tree) -> Vec<(String, Vec<String>)> {
    use std::collections::BTreeMap;
    let mut out = Vec::new();
    for root in &tree.roots {
        let mut by_code: BTreeMap<String, Vec<String>> = BTreeMap::new();
        fn walk(
            n: &Node,
            ancestors: &mut Vec<String>,
            m: &mut std::collections::BTreeMap<String, Vec<String>>,
        ) {
            if let Some(c) = &n.code {
                if !ancestors.contains(c) {
                    m.entry(c.clone()).or_default().push(n.id.clone());
                }
            }
            let pushed = if let Some(c) = &n.code {
                ancestors.push(c.clone());
                true
            } else {
                false
            };
            for ch in &n.children {
                walk(ch, ancestors, m);
            }
            if pushed {
                ancestors.pop();
            }
        }
        walk(root, &mut Vec::new(), &mut by_code);
        out.extend(by_code.into_iter().filter(|(_, ids)| ids.len() > 1));
    }
    out
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
            locations: vec![],
            links: vec![],
            has_notes: false,
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
            warnings: vec![],
        };
        assert_eq!(suggest_next_code(&t, "30").unwrap(), "30.02");
    }
}
