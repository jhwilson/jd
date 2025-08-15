use anyhow::{Result};
use std::fs;
use std::path::Path;
use crate::ignore::is_ignored_path;

pub fn preview_dir(path: &Path) -> Result<String> {
    let mut dated: Vec<(u128, String)> = Vec::new();
    let mut other: Vec<String> = Vec::new();
    for entry in fs::read_dir(path)? {
        if let Ok(e) = entry {
            let name = e.file_name().to_string_lossy().to_string();
            let full = path.join(e.file_name());
            if is_ignored_path(&full) { continue; }
            // Only treat regular files with YYYYMMDDTTTT* prefix as dated
            let file_type = e.file_type().ok();
            if file_type.as_ref().map(|ft| ft.is_file()).unwrap_or(false) {
                if let Some(ts) = parse_datetime_prefix(&name) {
                    dated.push((ts, name));
                    continue;
                }
            }
            other.push(name);
        }
    }
    // Newest first for dated, then others alpha; show up to 50 entries total
    dated.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    other.sort();
    let mut out = String::new();
    out.push_str(&format!("dir: {}\n\n", path.display()));
    let mut shown = 0usize;
    for (_, name) in &dated {
        if shown >= 50 { break; }
        out.push_str(name);
        out.push('\n');
        shown += 1;
    }
    if shown < 50 {
        for name in &other {
            if shown >= 50 { break; }
            out.push_str(name);
            out.push('\n');
            shown += 1;
        }
    }
    Ok(out)
}

fn parse_datetime_prefix(name: &str) -> Option<u128> {
    // Match a leading 12-digit timestamp: YYYYMMDDTTTT (T= time like HHMM)
    let bytes = name.as_bytes();
    if bytes.len() < 12 { return None; }
    if !bytes[..12].iter().all(|b| b.is_ascii_digit()) { return None; }
    // Convert to integer for ordering; 12 digits fit in u64, use u128 for safety
    let s = &name[..12];
    s.parse::<u128>().ok()
}

pub fn preview_file(path: &Path) -> Result<String> {
    let content = fs::read_to_string(path).unwrap_or_default();
    let head = content.lines().take(200).collect::<Vec<_>>().join("\n");
    Ok(head)
}

pub fn preview_link(path: &Path) -> Result<String> {
    // Preview the file path and show the resolved URL if readable
    let mut out = String::new();
    out.push_str(&format!("link file: {}\n\n", path.display()));
    if let Ok(s) = fs::read_to_string(path) { out.push_str(&s); }
    Ok(out)
}


