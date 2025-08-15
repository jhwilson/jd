use crate::model::{Node, NodeType, Tree, make_id, parse_range, parse_category, parse_item, validate_unique_codes_among_siblings};
use crate::ignore::is_ignored_path;
use anyhow::{Result};
use std::fs;
use std::path::{Path, PathBuf};
use plist::Value as PlistValue;
use regex::Regex;

fn parse_webloc_url(path: &Path) -> Option<String> {
    if let Ok(mut f) = fs::File::open(path) {
        if let Ok(v) = PlistValue::from_reader_xml(&mut f) {
            if let Some(url) = v.as_dictionary()?.get("URL").and_then(|u| u.as_string()) {
                return Some(url.to_string());
            }
            if let Some(url) = v.as_dictionary()?.get("URLString").and_then(|u| u.as_string()) {
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
            if let Some(rest) = line.strip_prefix("URL=") { return Some(rest.trim().to_string()); }
        }
        // fallback: first URL-looking token
        static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| Regex::new(r"(?i)\b(https?://\S+|obsidian://\S+|file://\S+)").unwrap());
        if let Some(c) = RE.captures(&s) { return Some(c[1].to_string()); }
    }
    None
}

fn parse_location_from_file(path: &Path) -> Option<String> {
    if let Ok(s) = fs::read_to_string(path) {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("LOCATION=") { return Some(rest.trim().to_string()); }
        }
    }
    None
}

pub fn scan_roots(roots: &[PathBuf]) -> Result<Tree> {
    let mut tree = Tree { roots: Vec::new() };
    for root in roots {
        let root = root.canonicalize()?;
        let node = scan_dir(&root, true)?;
        tree.roots.push(node);
    }
    Ok(tree)
}

fn scan_dir(path: &Path, is_root: bool) -> Result<Node> {
    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| path.display().to_string());
    // classify by name
    let (code, title, node_type) = if let Some((code, title)) = parse_range(&name) { (Some(code), title, NodeType::Range) } 
        else if let Some((code, title)) = parse_category(&name) { (Some(code), title, NodeType::Category) }
        else if let Some((code, title, _ext)) = parse_item(&name) { (Some(code), title, NodeType::ItemDir) }
        else { (None, name.clone(), NodeType::Other) };

    let mut children: Vec<Node> = Vec::new();
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = fs::read_dir(path)?
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| !is_ignored_path(p))
            .collect();
        entries.sort();
        for child in entries {
            if child.is_dir() {
                let cname = child.file_name().unwrap().to_string_lossy().to_string();
                let is_jd_dir = parse_range(&cname).is_some() || parse_category(&cname).is_some() || parse_item(&cname).is_some();
                if is_jd_dir {
                    children.push(scan_dir(&child, false)?);
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
                    children.push(Node { id: make_id(&child), code: Some(code), title, path: child.to_string_lossy().to_string(), node_type: nt, location, url: url_opt, children: vec![] });
                }
            }
        }
        let _ = validate_unique_codes_among_siblings(&children);
    }

    let id = make_id(path);
    // Only include non-conforming directory nodes at the root level
    let node_type_final = if !is_root && matches!(node_type, NodeType::Other) { NodeType::Other } else { node_type };
    Ok(Node { id, code, title, path: path.to_string_lossy().to_string(), node_type: node_type_final, location: None, url: None, children })
}


