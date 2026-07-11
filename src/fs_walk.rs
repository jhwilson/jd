use crate::ignore::is_ignored_entry;
use crate::meta::{self, Entry};
use crate::model::{
    make_id, parse_category, parse_item, parse_range, validate_unique_codes_among_siblings, Node,
    NodeType, Tree,
};
use anyhow::Result;
use plist::Value as PlistValue;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

fn parse_webloc_url(path: &Path) -> Option<String> {
    if let Ok(mut f) = fs::File::open(path) {
        if let Ok(v) = PlistValue::from_reader_xml(&mut f) {
            if let Some(url) = v.as_dictionary()?.get("URL").and_then(|u| u.as_string()) {
                return Some(url.to_string());
            }
            if let Some(url) = v
                .as_dictionary()?
                .get("URLString")
                .and_then(|u| u.as_string())
            {
                return Some(url.to_string());
            }
        }
    }
    None
}

fn parse_url_file(path: &Path) -> Option<String> {
    // INI-like .url files: [InternetShortcut]\nURL=...
    if let Ok(s) = fs::read_to_string(path) {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("URL=") {
                return Some(rest.trim().to_string());
            }
        }
        // fallback: first URL-looking token
        static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
            Regex::new(r"(?i)\b(https?://\S+|obsidian://\S+|file://\S+)").unwrap()
        });
        if let Some(c) = RE.captures(&s) {
            return Some(c[1].to_string());
        }
    }
    None
}

fn parse_location_from_file(path: &Path) -> Option<String> {
    if let Ok(s) = fs::read_to_string(path) {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("LOCATION=") {
                return Some(rest.trim().to_string());
            }
        }
    }
    None
}

pub fn scan_roots(roots: &[PathBuf]) -> Result<Tree> {
    let mut tree = Tree::default();
    for root in roots {
        let root = root.canonicalize()?;
        let node = scan_dir(&root, true, &mut tree.warnings)?;
        tree.roots.push(node);
    }
    Ok(tree)
}

fn scan_dir(path: &Path, is_root: bool, warnings: &mut Vec<String>) -> Result<Node> {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    // classify by name
    let (code, title, node_type) = if let Some((code, title)) = parse_range(&name) {
        (Some(code), title, NodeType::Range)
    } else if let Some((code, title)) = parse_category(&name) {
        (Some(code), title, NodeType::Category)
    } else if let Some((code, title, _ext)) = parse_item(&name) {
        (Some(code), title, NodeType::ItemDir)
    } else {
        (None, name.clone(), NodeType::Other)
    };

    let mut children: Vec<Node> = Vec::new();
    let mut locations: Vec<String> = Vec::new();
    let mut links: Vec<crate::meta::MetaLink> = Vec::new();
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = Vec::new();
        let mut has_meta = false;
        for e in fs::read_dir(path)?.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.file_name().and_then(|n| n.to_str()) == Some(meta::META_FILE) {
                has_meta = true;
                continue;
            }
            if !is_ignored_entry(&p) {
                entries.push(p);
            }
        }
        entries.sort();
        if has_meta {
            for entry in meta::entries(path) {
                match entry {
                    Entry::Location(s) => locations.push(s),
                    Entry::Link(l) => links.push(l),
                }
            }
        }
        for child in entries {
            if child.is_dir() {
                let cname = child.file_name().unwrap().to_string_lossy().to_string();
                let is_jd_dir = parse_range(&cname).is_some()
                    || parse_category(&cname).is_some()
                    || parse_item(&cname).is_some();
                if is_jd_dir {
                    children.push(scan_dir(&child, false, warnings)?);
                } else {
                    // skip non-conforming directories
                    continue;
                }
            } else {
                let fname = child.file_name().unwrap().to_string_lossy().to_string();
                if let Some((code, title, ext)) = parse_item(&fname) {
                    let (nt, url_opt, location) = match ext.as_deref() {
                        Some("webloc") => {
                            let url = parse_webloc_url(&child);
                            (NodeType::Link, url, None)
                        }
                        Some("url") => {
                            let url = parse_url_file(&child);
                            (NodeType::Link, url, None)
                        }
                        _ => {
                            let loc = parse_location_from_file(&child);
                            (NodeType::File, None, loc)
                        }
                    };
                    children.push(Node {
                        id: make_id(&child),
                        code: Some(code),
                        title,
                        path: child.to_string_lossy().to_string(),
                        node_type: nt,
                        location,
                        url: url_opt,
                        locations: vec![],
                        links: vec![],
                        children: vec![],
                    });
                }
            }
        }
        if let Err(e) = validate_unique_codes_among_siblings(&children, code.as_deref()) {
            warnings.push(format!("{} in {}", e, path.display()));
        }
    }

    let id = make_id(path);
    // Only include non-conforming directory nodes at the root level
    let node_type_final = if !is_root && matches!(node_type, NodeType::Other) {
        NodeType::Other
    } else {
        node_type
    };
    Ok(Node {
        id,
        code,
        title,
        path: path.to_string_lossy().to_string(),
        node_type: node_type_final,
        location: None,
        url: None,
        locations,
        links,
        children,
    })
}
