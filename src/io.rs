use crate::model::Tree;
use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Default)]
pub struct IndexIo;

impl IndexIo {
    pub fn write_index(&self, out: Option<&PathBuf>, tree: &Tree) -> Result<PathBuf> {
        if let Some(explicit) = out { return self.atomic_write(explicit, tree); }
        // Default: write a .jd_index.json per root top-level directory if there is exactly one root.
        if tree.roots.len() == 1 {
            let root_path = &tree.roots[0].path;
            let out_path = PathBuf::from(root_path).join(".jd_index.json");
            return self.atomic_write(&out_path, tree);
        }
        // Multiple roots: write combined index only with --out
        let combined = default_index_path();
        self.atomic_write(&combined, tree)
    }

    fn atomic_write(&self, out_path: &PathBuf, tree: &Tree) -> Result<PathBuf> {
        if let Some(dir) = out_path.parent() { fs::create_dir_all(dir)?; }
        let tmp = out_path.with_extension("tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            let data = serde_json::to_vec_pretty(tree)?;
            f.write_all(&data)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, out_path)?;
        Ok(out_path.clone())
    }
}

pub fn default_index_path() -> PathBuf { 
    let cfg = home::home_dir().unwrap_or_else(|| PathBuf::from("."));
    cfg.join(".config").join("jd").join(".jd_index.json")
}


