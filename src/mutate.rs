use crate::{
    fs_walk,
    io::IndexIo,
    model,
    plan::{self, CreatePlan, MovePlan, PlanKind, RenamePlan},
};
use anyhow::Result;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub use crate::plan::PlanKind as NewKind;
fn index(roots: &[PathBuf]) -> Result<()> {
    IndexIo::default().write_index(None, &fs_walk::scan_roots(roots)?)?;
    Ok(())
}
pub fn create(
    roots: &[PathBuf],
    kind: NewKind,
    parent_id: &str,
    name: &str,
    url: Option<&str>,
    location: Option<&str>,
) -> Result<()> {
    let tree = fs_walk::scan_roots(roots)?;
    let parent =
        model::find_node(&tree, parent_id).ok_or_else(|| anyhow::anyhow!("parent not found"))?;
    let p = PathBuf::from(&parent.path).join(name);
    match kind {
        PlanKind::Dir => fs::create_dir(&p)?,
        PlanKind::File => fs::write(
            &p,
            format!("LOCATION={}\n", location.unwrap_or(&parent.path)),
        )?,
        PlanKind::Link => {
            let u = url.ok_or_else(|| anyhow::anyhow!("link requires --url"))?;
            if p.extension()
                .and_then(|x| x.to_str())
                .map(|x| x.eq_ignore_ascii_case("webloc"))
                .unwrap_or(false)
            {
                fs::write(
                    &p,
                    format!(
                        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>URL</key><string>{}</string></dict></plist>"#,
                        u
                    ),
                )?
            } else {
                fs::write(&p, u)?
            }
        }
    };
    index(roots)
}
pub fn execute_create(roots: &[PathBuf], p: &CreatePlan) -> Result<()> {
    create(
        roots,
        p.kind,
        &p.parent_id,
        &p.final_name,
        p.url.as_deref(),
        p.location.as_deref(),
    )
}
pub fn execute_rename(roots: &[PathBuf], p: &RenamePlan) -> Result<()> {
    let tree = fs_walk::scan_roots(roots)?;
    let n = model::find_node(&tree, &p.id).ok_or_else(|| anyhow::anyhow!("not found"))?;
    fs::rename(&n.path, &p.dest_path)?;
    index(roots)
}
pub fn rename(roots: &[PathBuf], id: &str, name: &str) -> Result<()> {
    let t = fs_walk::scan_roots(roots)?;
    execute_rename(roots, &plan::plan_rename(&t, id, name)?)
}
pub fn execute_move(roots: &[PathBuf], p: &MovePlan) -> Result<()> {
    fs::rename(&p.src_path, &p.dest_path)?;
    index(roots)
}
pub fn move_node(roots: &[PathBuf], id: &str, parent: &str) -> Result<()> {
    let t = fs_walk::scan_roots(roots)?;
    execute_move(roots, &plan::plan_move(&t, id, parent)?)
}
pub fn delete_node(roots: &[PathBuf], id: &str) -> Result<PathBuf> {
    let t = fs_walk::scan_roots(roots)?;
    let n = model::find_node(&t, id).ok_or_else(|| anyhow::anyhow!("not found"))?;
    let p = PathBuf::from(&n.path);
    let trash = p.parent().unwrap().join(".jd_trash");
    fs::create_dir_all(&trash)?;
    let target = trash.join(p.file_name().unwrap());
    if target.exists() {
        anyhow::bail!("trash destination already exists")
    };
    fs::rename(&p, &target)?;
    index(roots)?;
    Ok(target)
}
pub fn undo_delete(roots: &[PathBuf], trash: &Path, original: &Path) -> Result<()> {
    if original.exists() {
        anyhow::bail!("original path is occupied")
    };
    fs::rename(trash, original)?;
    index(roots)
}
pub fn new_interactive_any(
    roots: &[PathBuf],
    parent_id: &str,
    _display: &str,
    forced: Option<NewKind>,
) -> Result<()> {
    let tree = fs_walk::scan_roots(roots)?;
    let selected = model::find_node(&tree, parent_id)
        .ok_or_else(|| anyhow::anyhow!("selected parent id not found"))?;
    print!("New (code title | name.ext | URL): ");
    io::stdout().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    if s.trim().is_empty() {
        return Ok(());
    }
    let p = plan::parse_new_input_forced(
        &s,
        &plan::CreateContext {
            tree: &tree,
            selected,
        },
        forced,
    )?;
    if p.kind == PlanKind::Link && p.url.is_none() {
        anyhow::bail!("link requires a URL in the input")
    }
    println!("{}", plan::create_summary(&p));
    for w in &p.warnings {
        println!("warning: {}", w)
    }
    print!("Create? [y/N] ");
    io::stdout().flush()?;
    let mut y = String::new();
    io::stdin().read_line(&mut y)?;
    if matches!(y.trim(), "y" | "Y") {
        execute_create(roots, &p)?
    }
    Ok(())
}
