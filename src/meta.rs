//! Per-directory `.jdmeta` files: the multi-location index layer.
//!
//! A JD number can live in several places at once — the folder on disk plus a
//! Notion page, a reMarkable notebook, a filing cabinet. `.jdmeta` records
//! those as plain text inside the directory (filesystem stays the source of
//! truth):
//!
//! ```text
//! # comments and unknown lines are preserved verbatim
//! LOCATION=remarkable: Colloquium notebook
//! LOCATION=filing cabinet drawer 2
//! LINK=https://notion.so/abc123 Colloquium page
//! ```
//!
//! Repeated keys mean multiple values. `LINK` is URL-first with an optional
//! label after the first space (URLs contain no spaces).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const META_FILE: &str = ".jdmeta";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetaLink {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Location(String),
    Link(MetaLink),
}

impl Entry {
    /// Classify free-form input: anything containing `://` is a link (first
    /// token = URL, remainder = label), everything else a location.
    pub fn from_input(input: &str) -> Option<Entry> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }
        if input.contains("://") {
            let (url, label) = match input.split_once(' ') {
                Some((u, l)) => (u.to_string(), Some(l.trim().to_string()).filter(|s| !s.is_empty())),
                None => (input.to_string(), None),
            };
            Some(Entry::Link(MetaLink { url, label }))
        } else {
            Some(Entry::Location(input.to_string()))
        }
    }

    fn to_line(&self) -> String {
        match self {
            Entry::Location(s) => format!("LOCATION={}", s),
            Entry::Link(l) => match &l.label {
                Some(lb) => format!("LINK={} {}", l.url, lb),
                None => format!("LINK={}", l.url),
            },
        }
    }

    pub fn display(&self) -> String {
        match self {
            Entry::Location(s) => format!("⌂ {}", s),
            Entry::Link(l) => match &l.label {
                Some(lb) => format!("↗ {} — {}", lb, l.url),
                None => format!("↗ {}", l.url),
            },
        }
    }

    fn parse_line(line: &str) -> Option<Entry> {
        if let Some(rest) = line.strip_prefix("LOCATION=") {
            let rest = rest.trim();
            (!rest.is_empty()).then(|| Entry::Location(rest.to_string()))
        } else if let Some(rest) = line.strip_prefix("LINK=") {
            let rest = rest.trim();
            if rest.is_empty() {
                return None;
            }
            let (url, label) = match rest.split_once(' ') {
                Some((u, l)) => (
                    u.to_string(),
                    Some(l.trim().to_string()).filter(|s| !s.is_empty()),
                ),
                None => (rest.to_string(), None),
            };
            Some(Entry::Link(MetaLink { url, label }))
        } else {
            None
        }
    }
}

/// Ordered entries of a directory's `.jdmeta` (empty if absent/unreadable).
pub fn entries(dir: &Path) -> Vec<Entry> {
    fs::read_to_string(dir.join(META_FILE))
        .map(|s| s.lines().filter_map(Entry::parse_line).collect())
        .unwrap_or_default()
}

/// Append an entry, creating the file if needed.
pub fn add_entry(dir: &Path, entry: &Entry) -> Result<()> {
    let path = dir.join(META_FILE);
    let mut content = fs::read_to_string(&path).unwrap_or_default();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&entry.to_line());
    content.push('\n');
    atomic_write(&path, &content)
}

/// Remove the first line matching the entry, preserving all other lines
/// (comments, unknown keys) byte-for-byte. Removes the file when nothing but
/// whitespace remains.
pub fn remove_entry(dir: &Path, entry: &Entry) -> Result<()> {
    let path = dir.join(META_FILE);
    let content = fs::read_to_string(&path)?;
    let needle = entry.to_line();
    let mut removed = false;
    let mut out = String::new();
    for line in content.lines() {
        if !removed && line.trim_end() == needle {
            removed = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !removed {
        anyhow::bail!("entry not found in {}", path.display());
    }
    if out.trim().is_empty() {
        fs::remove_file(&path)?;
        Ok(())
    } else {
        atomic_write(&path, &out)
    }
}

fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp = path.with_extension("jdmeta.tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_input() {
        assert_eq!(
            Entry::from_input("remarkable: Colloquium notebook"),
            Some(Entry::Location("remarkable: Colloquium notebook".into()))
        );
        assert_eq!(
            Entry::from_input("https://notion.so/abc Colloquium page"),
            Some(Entry::Link(MetaLink {
                url: "https://notion.so/abc".into(),
                label: Some("Colloquium page".into())
            }))
        );
        assert_eq!(Entry::from_input("  "), None);
    }

    #[test]
    fn round_trip_preserves_unknown_lines() {
        let td = tempfile::tempdir().unwrap();
        let dir = td.path();
        let original = "# my notes\nLOCATION=drawer 2\nFUTUREKEY=whatever\n\nLINK=https://x.io lab\n";
        std::fs::write(dir.join(META_FILE), original).unwrap();

        let e = entries(dir);
        assert_eq!(e.len(), 2); // FUTUREKEY is preserved but is not an entry
        assert_eq!(e[0], Entry::Location("drawer 2".into()));

        // add then remove -> unknown lines and comments untouched
        let extra = Entry::Location("remarkable: notes".into());
        add_entry(dir, &extra).unwrap();
        assert_eq!(entries(dir).len(), 3);
        remove_entry(dir, &extra).unwrap();
        let after = std::fs::read_to_string(dir.join(META_FILE)).unwrap();
        assert_eq!(after, original);
    }

    #[test]
    fn remove_last_entry_deletes_file() {
        let td = tempfile::tempdir().unwrap();
        let dir = td.path();
        let e = Entry::Location("only one".into());
        add_entry(dir, &e).unwrap();
        assert!(dir.join(META_FILE).exists());
        remove_entry(dir, &e).unwrap();
        assert!(!dir.join(META_FILE).exists());
    }
}
