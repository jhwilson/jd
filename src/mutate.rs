use crate::model::Tree;
use crate::fs_walk;
use crate::io::IndexIo;
use anyhow::{Result, bail};
use std::fs;
use std::path::PathBuf;
use crate::model::{self, parse_item, parse_category, parse_range};
use regex::Regex;
use std::io::{self, Write};

fn sanitize_title(input: &str) -> String {
    let mut s = input.trim().replace(' ', "_");
    s.retain(|c| c != '/');
    s
}

#[derive(Copy, Clone, Debug)]
pub enum NewKind { Dir, File, Link }

fn roots_to_tree(roots: &[PathBuf]) -> Result<Tree> { fs_walk::scan_roots(roots) }

fn id_to_path(tree: &Tree, id: &str) -> Option<PathBuf> {
    fn walk<'a>(node: &'a crate::model::Node, id: &str) -> Option<&'a crate::model::Node> {
        if node.id == id { return Some(node); }
        for ch in &node.children { if let Some(n) = walk(ch, id) { return Some(n); } }
        None
    }
    for r in &tree.roots { if let Some(n) = walk(r, id) { return Some(PathBuf::from(&n.path)); } }
    None
}

pub fn create(roots: &[PathBuf], kind: NewKind, parent_id: &str, name: &str, url: Option<&str>, location: Option<&str>) -> Result<()> {
    let tree = roots_to_tree(roots)?;
    let parent = id_to_path(&tree, parent_id).ok_or_else(|| anyhow::anyhow!("parent not found"))?;
    match kind {
        NewKind::Dir => { fs::create_dir(parent.join(name))?; }
        NewKind::File => {
            let p = parent.join(name);
            let loc_owned: String = match location {
                Some(s) => s.to_string(),
                None => parent.to_string_lossy().to_string(),
            };
            let content = format!("LOCATION={}\n", loc_owned);
            fs::write(p, content.as_bytes())?;
        }
        NewKind::Link => {
            let p = parent.join(name);
            let url = url.ok_or_else(|| anyhow::anyhow!("link requires --url"))?;
            // default to .webloc content if extension matches; else write raw URL
            let is_webloc = p.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("webloc")).unwrap_or(false);
            if is_webloc {
                let plist = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>URL</key><string>{}</string></dict></plist>"#, url);
                fs::write(&p, plist.as_bytes())?;
            } else {
                fs::write(&p, url.as_bytes())?;
            }
        }
    }
    let tree = roots_to_tree(roots)?; // rescan authoritative FS
    IndexIo::default().write_index(None, &tree)?;
    Ok(())
}

pub fn rename(roots: &[PathBuf], id: &str, name: &str) -> Result<()> {
    let tree = roots_to_tree(roots)?; let p = id_to_path(&tree, id).ok_or_else(|| anyhow::anyhow!("not found"))?;
    let fname = p.file_name().unwrap().to_string_lossy().to_string();
    let titled = sanitize_title(name);
    let new_name = if let Some((code, _title, ext)) = parse_item(&fname) {
        match ext.as_deref() {
            Some(e) if !e.is_empty() => format!("{}_{}.{}", code, titled, e),
            _ => format!("{}_{}", code, titled),
        }
    } else if let Some((code, _title)) = parse_category(&fname) {
        format!("{}_{}", code, titled)
    } else if let Some((code, _title)) = parse_range(&fname) {
        format!("{}_{}", code, titled)
    } else {
        titled
    };
    let new_path = p.parent().unwrap().join(new_name);
    fs::rename(&p, &new_path)?;
    let tree = roots_to_tree(roots)?; IndexIo::default().write_index(None, &tree)?; Ok(())
}

pub fn move_node(roots: &[PathBuf], id: &str, parent_id: &str) -> Result<()> {
    let tree = roots_to_tree(roots)?;
    let p = id_to_path(&tree, id).ok_or_else(|| anyhow::anyhow!("not found"))?;
    let parent_path = id_to_path(&tree, parent_id).ok_or_else(|| anyhow::anyhow!("parent not found"))?;

    // Ensure within same root
    let mut src_root_idx = None; let mut dst_root_idx = None;
    for (i, r) in roots.iter().enumerate() {
        let rc = r.canonicalize()?;
        if src_root_idx.is_none() && p.starts_with(&rc) { src_root_idx = Some(i); }
        if dst_root_idx.is_none() && parent_path.starts_with(&rc) { dst_root_idx = Some(i); }
    }
    if src_root_idx != dst_root_idx { bail!("move across roots is not allowed"); }

    // If parent is an item directory, just move inside without renaming
    let parent_is_item_dir = parent_path.file_name().and_then(|n| n.to_str()).and_then(|n| parse_item(n).map(|_| true)).unwrap_or(false);
    if parent_is_item_dir {
        let new_path = parent_path.join(p.file_name().unwrap());
        fs::rename(&p, &new_path)?;
        let tree = roots_to_tree(roots)?; IndexIo::default().write_index(None, &tree)?; return Ok(());
    }

    // If parent is a category, and we're moving an item, adjust code to next free under that category
    let parent_is_category = parent_path.file_name().and_then(|n| n.to_str()).and_then(|n| parse_category(n).map(|x| x.0)).is_some();
    let item_parsed = p.file_name().and_then(|n| n.to_str()).and_then(|n| parse_item(n));
    if parent_is_category && item_parsed.is_some() {
        // find category code from parent
        let parent_code = parent_path.file_name().and_then(|n| n.to_str()).and_then(|n| parse_category(n).map(|x| x.0)).unwrap();
        let next = crate::model::suggest_next_code(&tree, &parent_code)?; // NN.MM
        let (_old_code, title, ext) = item_parsed.unwrap();
        let new_name = match ext.as_deref() { Some(e) if !e.is_empty() => format!("{}_{}.{}", next, title, e), _ => format!("{}_{}", next, title) };
        let new_path = parent_path.join(new_name);
        fs::rename(&p, &new_path)?;
        let tree = roots_to_tree(roots)?; IndexIo::default().write_index(None, &tree)?; return Ok(());
    }

    // Fallback: simple move
    let new_path = parent_path.join(p.file_name().unwrap());
    fs::rename(&p, &new_path)?;
    let tree = roots_to_tree(roots)?; IndexIo::default().write_index(None, &tree)?; Ok(())
}

pub fn delete_node(roots: &[PathBuf], id: &str) -> Result<()> {
    let tree = roots_to_tree(roots)?; let p = id_to_path(&tree, id).ok_or_else(|| anyhow::anyhow!("not found"))?;
    let trash = p.parent().unwrap().join(".jd_trash");
    fs::create_dir_all(&trash)?;
    let target = trash.join(p.file_name().unwrap());
    fs::rename(&p, &target)?; // soft delete to trash
    let tree = roots_to_tree(roots)?; IndexIo::default().write_index(None, &tree)?; Ok(())
}


pub fn new_interactive_dir(roots: &[PathBuf], parent_id: &str, _display: &str) -> Result<()> {
    let tree = roots_to_tree(roots)?;

    // locate selected node by id
    fn find_node<'a>(n: &'a model::Node, id: &str) -> Option<&'a model::Node> {
        if n.id == id { return Some(n); }
        for c in &n.children { if let Some(x)=find_node(c,id){ return Some(x);} }
        None
    }
    let mut selected: Option<&model::Node> = None;
    'outer: for r in &tree.roots { if let Some(x)=find_node(r, parent_id){ selected=Some(x); break 'outer; } }
    let selected = selected.ok_or_else(|| anyhow::anyhow!("selected parent id not found"))?;

    // prepare prompt
    print!("New directory (code + title or title): ");
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();
    if input.is_empty() { return Ok(()); }

    let re_item = Regex::new(r"^(\d{2}\.\d{2})[ _-](.+)$").unwrap();
    let re_cat = Regex::new(r"^(\d{2})[ _-](.+)$").unwrap();
    let re_range = Regex::new(r"^(\d{2}-\d{2})[ _-](.+)$").unwrap();

    // Suggest next category NN within range NN-NN
    fn suggest_next_category_in_range(tree: &model::Tree, range_code: &str) -> Result<String> {
        let parts: Vec<&str> = range_code.split('-').collect();
        if parts.len()!=2 { anyhow::bail!("invalid range code: {}", range_code); }
        let start: u32 = parts[0].parse()?; let end: u32 = parts[1].parse()?;
        // find range node
        fn find_by_code<'a>(n: &'a model::Node, code: &str) -> Option<&'a model::Node> {
            if n.code.as_deref()==Some(code) { return Some(n); }
            for c in &n.children { if let Some(x)=find_by_code(c, code){ return Some(x);} }
            None
        }
        let mut range_node: Option<&model::Node> = None;
        'seek: for r in &tree.roots { if let Some(x)=find_by_code(r, range_code){ range_node=Some(x); break 'seek; } }
        let range_node = range_node.ok_or_else(|| anyhow::anyhow!("range not found: {}", range_code))?;
        let mut used = std::collections::BTreeSet::new();
        for ch in &range_node.children {
            if matches!(ch.node_type, model::NodeType::Category) {
                if let Some(code) = &ch.code { used.insert(code.clone()); }
            }
        }
        for n in start..=end {
            let cand = format!("{:02}", n);
            if !used.contains(&cand) { return Ok(cand); }
        }
        anyhow::bail!("No free category code in {}", range_code)
    }

    // compute parent id (pid) and final name (nmf)
    let mut pid = parent_id.to_string();
    let nmf = if let Some(c) = re_item.captures(input) {
        let code = &c[1];
        let title = sanitize_title(&c[2]);
        let nn = &code[..2];
        // find category id for NN in current tree
        // helper
        fn find_cat_id<'a>(n: &'a model::Node, nn: &str) -> Option<String> {
            if matches!(n.node_type, model::NodeType::Category) && n.code.as_deref()==Some(nn) { return Some(n.id.clone()); }
            for c in &n.children { if let Some(x)=find_cat_id(c, nn){ return Some(x);} }
            None
        }
        let mut found: Option<String> = None;
        for r in &tree.roots { if let Some(x)=find_cat_id(r, nn){ found=Some(x); break; } }
        pid = found.ok_or_else(|| anyhow::anyhow!("category {} not found", nn))?;
        format!("{}_{}", code, title)
    } else if let Some(c) = re_cat.captures(input) {
        let code = &c[1];
        let title = sanitize_title(&c[2]);
        format!("{}_{}", code, title)
    } else if let Some(c) = re_range.captures(input) {
        let code = &c[1];
        let title = sanitize_title(&c[2]);
        format!("{}_{}", code, title)
    } else {
        // Title only: suggest based on selected node type
        let title = sanitize_title(input);
        match selected.node_type {
            model::NodeType::Category => {
                let nn = selected.code.as_deref().ok_or_else(|| anyhow::anyhow!("selected category missing code"))?;
                let sug = model::suggest_next_code(&tree, nn)?; // NN.MM
                format!("{}_{}", sug, title)
            }
            model::NodeType::Range => {
                let range_code = selected.code.as_deref().ok_or_else(|| anyhow::anyhow!("selected range missing code"))?; // NN-NN
                let nn = suggest_next_category_in_range(&tree, range_code)?; // NN
                format!("{}_{}", nn, title)
            }
            _ => title,
        }
    };

    create(roots, NewKind::Dir, &pid, &nmf, None, None)
}

pub fn new_interactive_any(roots: &[PathBuf], parent_id: &str, display: &str, preselected: Option<NewKind>) -> Result<()> {
    use std::io::{self, Write};
    let mut kind = preselected;
    if kind.is_none() {
        print!("Type: (d)irectory, (f)ile (location), or (l)ink? [d/f/l]: ");
        io::stdout().flush().ok();
        let mut t = String::new(); io::stdin().read_line(&mut t)?; let c = t.trim().to_lowercase();
        kind = match c.as_str() { "d" => Some(NewKind::Dir), "f" => Some(NewKind::File), "l" => Some(NewKind::Link), _ => Some(NewKind::Dir) };
    }
    match kind.unwrap() {
        NewKind::Dir => new_interactive_dir(roots, parent_id, display),
        NewKind::File => {
            // Name logic identical to dir creation
            // Reuse logic by temporarily creating name only, then prompting for location
            let tree = roots_to_tree(roots)?;
            // helper for range suggestion
            fn suggest_next_category_in_range(tree: &model::Tree, range_code: &str) -> Result<String> {
                let parts: Vec<&str> = range_code.split('-').collect();
                if parts.len()!=2 { anyhow::bail!("invalid range code: {}", range_code); }
                let start: u32 = parts[0].parse()?; let end: u32 = parts[1].parse()?;
                fn find_by_code<'a>(n: &'a model::Node, code: &str) -> Option<&'a model::Node> {
                    if n.code.as_deref()==Some(code) { return Some(n); }
                    for c in &n.children { if let Some(x)=find_by_code(c, code){ return Some(x);} }
                    None
                }
                let mut range_node: Option<&model::Node> = None;
                'seek: for r in &tree.roots { if let Some(x)=find_by_code(r, range_code){ range_node=Some(x); break 'seek; } }
                let range_node = range_node.ok_or_else(|| anyhow::anyhow!("range not found: {}", range_code))?;
                let mut used = std::collections::BTreeSet::new();
                for ch in &range_node.children { if matches!(ch.node_type, model::NodeType::Category) { if let Some(code) = &ch.code { used.insert(code.clone()); } } }
                for n in start..=end { let cand = format!("{:02}", n); if !used.contains(&cand) { return Ok(cand); } }
                anyhow::bail!("No free category code in {}", range_code)
            }
            // find selected node
            fn find_node<'a>(n: &'a model::Node, id: &str) -> Option<&'a model::Node> { if n.id==id {return Some(n);} for c in &n.children { if let Some(x)=find_node(c,id){return Some(x);} } None }
            let mut sel=None; for r in &tree.roots { if let Some(x)=find_node(r, parent_id){ sel=Some(x); break; } }
            let selected = sel.ok_or_else(|| anyhow::anyhow!("selected parent id not found"))?;

            // prompt for name input
            print!("New file (code + title or title): "); io::stdout().flush().ok();
            let mut input = String::new(); io::stdin().read_line(&mut input)?; let input = input.trim(); if input.is_empty(){ return Ok(()); }

            // derive pid and nmf using same patterns as dir
            let re_item = Regex::new(r"^(\d{2}\.\d{2})[ _-](.+)$").unwrap();
            let re_cat = Regex::new(r"^(\d{2})[ _-](.+)$").unwrap();
            let re_range = Regex::new(r"^(\d{2}-\d{2})[ _-](.+)$").unwrap();
            let mut pid = parent_id.to_string();
            let nmf = if let Some(c) = re_item.captures(input) {
                let code=&c[1]; let title=sanitize_title(&c[2]);
                let nn=&code[..2];
                // resolve category by nn
                fn find_cat_id<'a>(n: &'a model::Node, nn: &str) -> Option<String> { if matches!(n.node_type, model::NodeType::Category)&& n.code.as_deref()==Some(nn){return Some(n.id.clone());} for c in &n.children { if let Some(x)=find_cat_id(c, nn){return Some(x);} } None }
                let mut found=None; for r in &tree.roots { if let Some(x)=find_cat_id(r, nn){ found=Some(x); break; } }
                pid = found.ok_or_else(|| anyhow::anyhow!("category {} not found", nn))?;
                format!("{}_{}", code, title)
            } else if let Some(c) = re_cat.captures(input) {
                let code=&c[1]; let title=sanitize_title(&c[2]); format!("{}_{}", code, title)
            } else if let Some(c) = re_range.captures(input) {
                let code=&c[1]; let title=sanitize_title(&c[2]); format!("{}_{}", code, title)
            } else {
                let title=sanitize_title(input);
                match selected.node_type {
                    model::NodeType::Category => { let nn=selected.code.as_deref().ok_or_else(|| anyhow::anyhow!("selected category missing code"))?; let sug=model::suggest_next_code(&tree, nn)?; format!("{}_{}", sug, title) }
                    model::NodeType::Range => { let range=selected.code.as_deref().ok_or_else(|| anyhow::anyhow!("selected range missing code"))?; let nn=suggest_next_category_in_range(&tree, range)?; format!("{}_{}", nn, title) }
                    _ => title,
                }
            };
            print!("Location: "); io::stdout().flush().ok(); let mut loc=String::new(); io::stdin().read_line(&mut loc)?; let loc=loc.trim();
            create(roots, NewKind::File, &pid, &nmf, None, if loc.is_empty(){ None } else { Some(loc) })
        }
        NewKind::Link => {
            let tree = roots_to_tree(roots)?;
            // helper for range suggestion
            fn suggest_next_category_in_range(tree: &model::Tree, range_code: &str) -> Result<String> {
                let parts: Vec<&str> = range_code.split('-').collect();
                if parts.len()!=2 { anyhow::bail!("invalid range code: {}", range_code); }
                let start: u32 = parts[0].parse()?; let end: u32 = parts[1].parse()?;
                fn find_by_code<'a>(n: &'a model::Node, code: &str) -> Option<&'a model::Node> {
                    if n.code.as_deref()==Some(code) { return Some(n); }
                    for c in &n.children { if let Some(x)=find_by_code(c, code){ return Some(x);} }
                    None
                }
                let mut range_node: Option<&model::Node> = None;
                'seek: for r in &tree.roots { if let Some(x)=find_by_code(r, range_code){ range_node=Some(x); break 'seek; } }
                let range_node = range_node.ok_or_else(|| anyhow::anyhow!("range not found: {}", range_code))?;
                let mut used = std::collections::BTreeSet::new();
                for ch in &range_node.children { if matches!(ch.node_type, model::NodeType::Category) { if let Some(code) = &ch.code { used.insert(code.clone()); } } }
                for n in start..=end { let cand = format!("{:02}", n); if !used.contains(&cand) { return Ok(cand); } }
                anyhow::bail!("No free category code in {}", range_code)
            }
            fn find_node<'a>(n: &'a model::Node, id: &str) -> Option<&'a model::Node> { if n.id==id {return Some(n);} for c in &n.children { if let Some(x)=find_node(c,id){return Some(x);} } None }
            let mut sel=None; for r in &tree.roots { if let Some(x)=find_node(r, parent_id){ sel=Some(x); break; } }
            let selected = sel.ok_or_else(|| anyhow::anyhow!("selected parent id not found"))?;
            print!("New link (code + title or title): "); io::stdout().flush().ok(); let mut input=String::new(); io::stdin().read_line(&mut input)?; let input=input.trim(); if input.is_empty(){ return Ok(()); }
            let re_item=Regex::new(r"^(\d{2}\.\d{2})[ _-](.+)$").unwrap(); let re_cat=Regex::new(r"^(\d{2})[ _-](.+)$").unwrap(); let re_range=Regex::new(r"^(\d{2}-\d{2})[ _-](.+)$").unwrap(); let mut pid=parent_id.to_string();
            let nmf = if let Some(c)=re_item.captures(input){ let code=&c[1]; let title=sanitize_title(&c[2]); let nn=&code[..2]; fn find_cat_id<'a>(n:&'a model::Node, nn:&str)->Option<String>{ if matches!(n.node_type, model::NodeType::Category)&& n.code.as_deref()==Some(nn){return Some(n.id.clone());} for ch in &n.children{ if let Some(x)=find_cat_id(ch, nn){return Some(x);} } None } let mut found=None; for r in &tree.roots{ if let Some(x)=find_cat_id(r, nn){ found=Some(x); break;} } pid=found.ok_or_else(|| anyhow::anyhow!("category {} not found", nn))?; format!("{}_{}", code, title) } else if let Some(c)=re_cat.captures(input){ let code=&c[1]; let title=sanitize_title(&c[2]); format!("{}_{}", code, title) } else if let Some(c)=re_range.captures(input){ let code=&c[1]; let title=sanitize_title(&c[2]); format!("{}_{}", code, title) } else { let title=sanitize_title(input); match selected.node_type { model::NodeType::Category => { let nn=selected.code.as_deref().ok_or_else(|| anyhow::anyhow!("selected category missing code"))?; let sug=model::suggest_next_code(&tree, nn)?; format!("{}_{}", sug, title) } model::NodeType::Range => { let range=selected.code.as_deref().ok_or_else(|| anyhow::anyhow!("selected range missing code"))?; let nn=suggest_next_category_in_range(&tree, range)?; format!("{}_{}", nn, title) } _ => title, } };
            print!("URL: "); io::stdout().flush().ok(); let mut url=String::new(); io::stdin().read_line(&mut url)?; let url=url.trim(); if url.is_empty(){ return Ok(()); }
            create(roots, NewKind::Link, &pid, &nmf, Some(url), None)
        }
    }
}


