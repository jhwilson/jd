use crate::tsv::ExpandedState;
use anyhow::Result;
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default)]
struct StateSerde { expanded: Vec<String> }

pub fn default_state_path() -> PathBuf {
    let cache = home::home_dir().unwrap_or_else(|| PathBuf::from("."));
    cache.join(".cache").join("jd").join("state.json")
}

pub fn load_state_or_default(path: Option<&PathBuf>) -> Result<ExpandedState> {
    let path = path.cloned().unwrap_or_else(default_state_path);
    if let Ok(bytes) = fs::read(&path) {
        if let Ok(s) = serde_json::from_slice::<StateSerde>(&bytes) {
            return Ok(ExpandedState { expanded: s.expanded.into_iter().collect() });
        }
    }
    Ok(ExpandedState { expanded: Default::default() })
}

pub fn save_state(path: &Path, st: &ExpandedState) -> Result<()> {
    let ser = StateSerde { expanded: st.expanded.iter().cloned().collect() };
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    fs::write(path, serde_json::to_vec_pretty(&ser)?)?;
    Ok(())
}


