// Centralized ignore rules for scanning and previewing
use std::path::Path;

fn file_name_lower(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().to_string().to_lowercase())
        .unwrap_or_default()
}

pub fn is_ignored_dir_name(name: &str) -> bool {
    let n = name.to_lowercase();
    matches!(
        n.as_str(),
        ".git"
            | ".obsidian"
            | ".auctex-auto"
            | "tmp"
            | "temp"
            | "cache"
            | ".cache"
            | ".tmp"
            | "logs"
            | ".jd_trash"
    )
}

pub fn is_ignored_file_name(name: &str) -> bool {
    let n = name.to_lowercase();
    // macOS Finder metadata; jd's own per-directory metadata file
    if n == ".ds_store" || n == ".jdmeta" {
        return true;
    }
    // Logs and backups
    if n.ends_with(".log")
        || n == "logs"
        || n.ends_with(".bak")
        || n.ends_with(".backup")
        || n.ends_with(".old")
    {
        return true;
    }
    // LaTeX auxiliary files (allow PDFs)
    if n.ends_with(".aux")
        || n.ends_with(".bbl")
        || n.ends_with(".bcf")
        || n.ends_with(".blg")
        || n.ends_with(".fdb_latexmk")
        || n.ends_with(".fls")
        || n.ends_with(".synctex.gz")
        || n.ends_with(".synctex")
        || n.ends_with(".toc")
        || n.ends_with(".out")
        || n.ends_with(".lof")
        || n.ends_with(".lot")
        || n.ends_with(".nav")
        || n.ends_with(".snm")
        || n.ends_with(".vrb")
        || n.ends_with(".dvi")
        || n.ends_with(".idx")
        || n.ends_with(".ilg")
        || n.ends_with(".ind")
        || n.ends_with(".xdv")
    {
        return true;
    }
    false
}

/// Ignore check for a single directory entry, by its own name only. Scanning
/// and previews recurse top-down, so ancestor filtering happens naturally —
/// checking ancestor components here would wrongly ignore everything under
/// e.g. /tmp or any user directory that happens to be named "cache".
pub fn is_ignored_entry(path: &Path) -> bool {
    let name = file_name_lower(path);
    if name.is_empty() {
        return false;
    }
    if path.is_dir() {
        is_ignored_dir_name(&name)
    } else {
        is_ignored_file_name(&name)
    }
}
