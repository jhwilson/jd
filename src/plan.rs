use crate::model::{self, Node, NodeType, Tree};
use anyhow::{bail, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlanKind {
    Dir,
    File,
    Link,
}
pub struct CreateContext<'a> {
    pub tree: &'a Tree,
    pub selected: &'a Node,
}
#[derive(Clone, Debug)]
pub struct CreatePlan {
    pub kind: PlanKind,
    pub parent_id: String,
    pub parent_display: String,
    pub final_name: String,
    pub dest_path: PathBuf,
    pub url: Option<String>,
    pub location: Option<String>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct RenamePlan {
    pub id: String,
    pub old_name: String,
    pub new_name: String,
    pub dest_path: PathBuf,
}
#[derive(Clone, Debug)]
pub struct MovePlan {
    pub id: String,
    pub src_path: PathBuf,
    pub dest_parent_id: String,
    pub final_name: String,
    pub dest_path: PathBuf,
}

pub fn sanitize_title(s: &str) -> String {
    s.trim().replace(' ', "_").replace('/', "")
}
fn display(n: &Node) -> String {
    n.code
        .as_ref()
        .map(|c| format!("{} {}", c, n.title))
        .unwrap_or_else(|| n.title.clone())
}
fn anchor<'a>(tree: &'a Tree, n: &'a Node) -> Result<&'a Node> {
    if matches!(n.node_type, NodeType::File | NodeType::Link) {
        model::find_node(
            tree,
            &model::find_parent_id(tree, &n.id)
                .ok_or_else(|| anyhow::anyhow!("parent not found"))?,
        )
        .ok_or_else(|| anyhow::anyhow!("parent not found"))
    } else {
        Ok(n)
    }
}
pub fn parse_new_input(input: &str, ctx: &CreateContext) -> Result<CreatePlan> {
    parse_new_input_forced(input, ctx, None)
}

/// Like `parse_new_input`, but with the kind forced (the d/f/l override in the
/// confirm step, and `new-interactive --kind`). Forcing File without an
/// extension appends `.txt`; forcing Link without a URL leaves `url` as None
/// so the caller can prompt for it; forcing Dir keeps the title verbatim
/// (dots and all) and drops any URL with a warning.
pub fn parse_new_input_forced(
    input: &str,
    ctx: &CreateContext,
    force: Option<PlanKind>,
) -> Result<CreatePlan> {
    static URL: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.-]*://\S+$").unwrap());
    static RANGE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\d{2}-\d{2})[ _-](.+)$").unwrap());
    static ITEM: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^(\d{2}\.\d{2,4}(?:\.\d{2})*)[ _-](.+)$").unwrap());
    static CAT: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\d{2})[ _-](.+)$").unwrap());
    static EXT: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^(.+)\.([A-Za-z][A-Za-z0-9]{0,4})$").unwrap());
    let input = input.trim();
    if input.is_empty() {
        bail!("name is empty")
    }
    // A URL token anywhere in the input (typically pasted first or last)
    // marks the entry as a link; the remaining tokens are the title.
    let mut parts: Vec<&str> = input.split_whitespace().collect();
    let url = parts
        .iter()
        .position(|x| URL.is_match(x))
        .map(|i| parts.remove(i).to_string());
    let mut warnings = vec![];
    let mut rest = parts.join(" ");
    if rest.is_empty() {
        let raw = url.as_ref().unwrap().split("://").nth(1).unwrap_or("link");
        let mut seg = raw.split('/');
        rest = format!(
            "{} {}",
            seg.next().unwrap_or("link"),
            seg.next().unwrap_or("")
        )
        .trim()
        .to_string();
        warnings.push("title derived from URL".into());
    }
    let mut parent = anchor(ctx.tree, ctx.selected)?;
    let (code, mut title) = if let Some(c) = RANGE.captures(&rest) {
        (Some(c[1].to_string()), c[2].to_string())
    } else if let Some(c) = ITEM.captures(&rest) {
        let code = c[1].to_string();
        let segs: Vec<_> = code.split('.').collect();
        let target = if segs.len() >= 3 {
            format!("{}.{}", segs[0], segs[1])
        } else {
            segs[0].to_string()
        };
        parent = model::find_by_code(ctx.tree, &target)
            .ok_or_else(|| anyhow::anyhow!("parent {} not found", target))?;
        (Some(code), c[2].to_string())
    } else if let Some(c) = CAT.captures(&rest) {
        (Some(c[1].to_string()), c[2].to_string())
    } else {
        let c = model::suggest_child_code(ctx.tree, parent)?;
        (if c.is_empty() { None } else { Some(c) }, rest)
    };
    let inferred = if url.is_some() {
        PlanKind::Link
    } else if EXT.is_match(&title) {
        PlanKind::File
    } else {
        PlanKind::Dir
    };
    let kind = force.unwrap_or(inferred);
    let mut url = url;
    let mut ext = None;
    match kind {
        PlanKind::File => {
            if let Some(c) = EXT.captures(&title) {
                let (t, e) = (c[1].to_string(), c[2].to_string());
                title = t;
                ext = Some(e);
            } else {
                ext = Some("txt".into());
            }
        }
        PlanKind::Dir => {
            if url.take().is_some() {
                warnings.push("URL ignored for directory".into());
            }
        }
        PlanKind::Link => {}
    }
    let base = match &code {
        Some(c) => format!("{}_{}", c, sanitize_title(&title)),
        None => sanitize_title(&title),
    };
    let final_name = match kind {
        PlanKind::File => format!("{}.{}", base, ext.unwrap()),
        PlanKind::Link
            if !base.to_lowercase().ends_with(".webloc")
                && !base.to_lowercase().ends_with(".url") =>
        {
            format!("{}.webloc", base)
        }
        _ => base,
    };
    if let Some(c) = &code {
        if model::all_codes(ctx.tree).contains(c) {
            warnings.push(format!("code {} already in use", c));
        }
    }
    let dest_path = PathBuf::from(&parent.path).join(&final_name);
    if dest_path.exists() {
        bail!("destination already exists: {}", dest_path.display())
    }
    Ok(CreatePlan {
        kind,
        parent_id: parent.id.clone(),
        parent_display: display(parent),
        final_name,
        dest_path,
        url,
        location: None,
        warnings,
    })
}
pub fn plan_rename(tree: &Tree, id: &str, new_title: &str) -> Result<RenamePlan> {
    let n = model::find_node(tree, id).ok_or_else(|| anyhow::anyhow!("not found"))?;
    let p = PathBuf::from(&n.path);
    let old = p.file_name().unwrap().to_string_lossy().to_string();
    let title = sanitize_title(new_title);
    let new = if let Some((c, _, e)) = model::parse_item(&old) {
        e.map(|e| format!("{}_{}.{}", c, title, e))
            .unwrap_or_else(|| format!("{}_{}", c, title))
    } else if let Some((c, _)) = model::parse_category(&old) {
        format!("{}_{}", c, title)
    } else if let Some((c, _)) = model::parse_range(&old) {
        format!("{}_{}", c, title)
    } else {
        title
    };
    let dest = p.parent().unwrap().join(&new);
    if dest.exists() && dest != p {
        bail!("destination already exists")
    };
    Ok(RenamePlan {
        id: id.into(),
        old_name: old,
        new_name: new,
        dest_path: dest,
    })
}
pub fn plan_move(tree: &Tree, id: &str, new_parent_id: &str) -> Result<MovePlan> {
    let n = model::find_node(tree, id).ok_or_else(|| anyhow::anyhow!("not found"))?;
    let parent =
        model::find_node(tree, new_parent_id).ok_or_else(|| anyhow::anyhow!("parent not found"))?;
    let src = PathBuf::from(&n.path);
    let pp = PathBuf::from(&parent.path);
    if id == new_parent_id || pp.starts_with(&src) {
        bail!("cannot move into self or descendant")
    }
    let root_for = |p: &PathBuf| tree.roots.iter().position(|r| p.starts_with(&r.path));
    if root_for(&src) != root_for(&pp) {
        bail!("move across roots is not allowed")
    }
    let mut name = src.file_name().unwrap().to_string_lossy().to_string();
    if matches!(parent.node_type, NodeType::Category) {
        if let Some((_, t, e)) = model::parse_item(&name) {
            let c = model::suggest_next_code(tree, parent.code.as_deref().unwrap())?;
            name = e
                .map(|e| format!("{}_{}.{}", c, sanitize_title(&t), e))
                .unwrap_or_else(|| format!("{}_{}", c, sanitize_title(&t)));
        }
    }
    let dest = pp.join(&name);
    if dest.exists() {
        bail!("destination already exists")
    };
    Ok(MovePlan {
        id: id.into(),
        src_path: src,
        dest_parent_id: new_parent_id.into(),
        final_name: name,
        dest_path: dest,
    })
}
#[derive(Clone, Debug)]
pub struct RenumberPlan {
    pub id: String,
    pub old_code: String,
    pub new_code: String,
    pub src_path: PathBuf,
    pub new_name: String,
    pub dest_path: PathBuf,
    /// Descendants whose filenames embed the old code and get renamed too.
    pub child_renames: usize,
}

/// Plan giving a node the next free code under its parent (duplicate
/// resolution). Children whose names start with the old code are renamed in
/// cascade. Ranges are refused — renumbering a decade invalidates its
/// category codes and needs human judgment.
pub fn plan_renumber(tree: &Tree, id: &str) -> Result<RenumberPlan> {
    let n = model::find_node(tree, id).ok_or_else(|| anyhow::anyhow!("not found"))?;
    let old_code = n
        .code
        .clone()
        .ok_or_else(|| anyhow::anyhow!("node has no JD code"))?;
    if matches!(n.node_type, NodeType::Range) {
        bail!("renumbering a range moves whole decades — do that manually");
    }
    let parent_id = model::find_parent_id(tree, id)
        .ok_or_else(|| anyhow::anyhow!("cannot renumber a root"))?;
    let parent =
        model::find_node(tree, &parent_id).ok_or_else(|| anyhow::anyhow!("parent not found"))?;
    let new_code = model::suggest_child_code(tree, parent)?;
    if new_code.is_empty() {
        bail!(
            "no code scheme under {} — move the entry into a range/category first",
            parent.title
        );
    }
    let src_path = PathBuf::from(&n.path);
    let name = src_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let new_name = format!("{}{}", new_code, &name[old_code.len()..]);
    let dest_path = src_path.parent().unwrap().join(&new_name);
    if dest_path.exists() {
        bail!("destination already exists: {}", dest_path.display());
    }
    fn count_embedded(n: &Node, prefix: &str) -> usize {
        n.children
            .iter()
            .map(|c| {
                let hit = Path::new(&c.path)
                    .file_name()
                    .map(|f| f.to_string_lossy().starts_with(prefix))
                    .unwrap_or(false) as usize;
                hit + count_embedded(c, prefix)
            })
            .sum()
    }
    let child_renames = count_embedded(n, &format!("{}.", old_code));
    Ok(RenumberPlan {
        id: id.into(),
        old_code,
        new_code,
        src_path,
        new_name,
        dest_path,
        child_renames,
    })
}

pub fn renumber_summary(p: &RenumberPlan) -> String {
    let cascade = match p.child_renames {
        0 => String::new(),
        n => format!(" (+{} children recoded)", n),
    };
    format!(
        "will renumber {} → {}{}",
        p.old_code, p.new_name, cascade
    )
}

pub fn create_summary(p: &CreatePlan) -> String {
    format!(
        "will create {} {} under {}{}",
        format!("{:?}", p.kind).to_uppercase(),
        p.final_name,
        p.parent_display,
        p.url
            .as_ref()
            .map(|u| format!(" -> {}", u))
            .unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_walk;
    use std::fs;

    /// R/30-39_Research/{31_Papers/{31.01_Existing/, 31.03_Two_Word_Note.txt,
    /// 31.04_Container/}, 32_Empty/} plus an empty range 90-99_Stuff.
    fn fixture() -> (tempfile::TempDir, Tree) {
        let td = tempfile::tempdir().unwrap();
        let r = td.path().join("R");
        for d in [
            "30-39_Research/31_Papers/31.01_Existing",
            "30-39_Research/31_Papers/31.04_Container",
            "30-39_Research/32_Empty",
            "90-99_Stuff",
        ] {
            fs::create_dir_all(r.join(d)).unwrap();
        }
        fs::write(
            r.join("30-39_Research/31_Papers/31.03_Two_Word_Note.txt"),
            b"x",
        )
        .unwrap();
        let tree = fs_walk::scan_roots(&[r]).unwrap();
        (td, tree)
    }

    fn node_by_suffix<'a>(tree: &'a Tree, suffix: &str) -> &'a Node {
        fn walk<'a>(n: &'a Node, s: &str) -> Option<&'a Node> {
            if n.path.ends_with(s) {
                return Some(n);
            }
            n.children.iter().find_map(|c| walk(c, s))
        }
        tree.roots
            .iter()
            .find_map(|r| walk(r, suffix))
            .unwrap_or_else(|| panic!("fixture node not found: {}", suffix))
    }

    fn plan_at(tree: &Tree, anchor_suffix: &str, input: &str) -> Result<CreatePlan> {
        let ctx = CreateContext {
            tree,
            selected: node_by_suffix(tree, anchor_suffix),
        };
        parse_new_input(input, &ctx)
    }

    #[test]
    fn range_input_beats_category_regex() {
        // The old bug's exact input shape: '[ _-]' in the category regex
        // matched the '-' of a range code.
        let (_td, tree) = fixture();
        let p = plan_at(&tree, "R", "20-29 Admin").unwrap();
        assert_eq!(p.kind, PlanKind::Dir);
        assert_eq!(p.final_name, "20-29_Admin");
    }

    #[test]
    fn extended_item_codes_retarget_parent() {
        let (_td, tree) = fixture();
        // NN.MMM anchors to category NN even when invoked elsewhere
        let p = plan_at(&tree, "32_Empty", "31.041 Long item").unwrap();
        assert_eq!(p.final_name, "31.041_Long_item");
        assert!(p.dest_path.ends_with("31_Papers/31.041_Long_item"));
        // NN.MM.KK anchors to the item dir NN.MM
        let p = plan_at(&tree, "32_Empty", "31.04.02 Sub").unwrap();
        assert!(p.dest_path.ends_with("31.04_Container/31.04.02_Sub"));
        // unknown owner errors
        assert!(plan_at(&tree, "32_Empty", "77.01 Nowhere").is_err());
    }

    #[test]
    fn url_inference() {
        let (_td, tree) = fixture();
        let p = plan_at(&tree, "31_Papers", "Notes about stuff https://example.com/x").unwrap();
        assert_eq!(p.kind, PlanKind::Link);
        assert_eq!(p.url.as_deref(), Some("https://example.com/x"));
        assert_eq!(p.final_name, "31.02_Notes_about_stuff.webloc");

        let p = plan_at(&tree, "31_Papers", "obsidian://open?vault=x Note").unwrap();
        assert_eq!(p.kind, PlanKind::Link);

        // bare URL: placeholder title + warning
        let p = plan_at(&tree, "31_Papers", "https://notion.so/abc123").unwrap();
        assert_eq!(p.kind, PlanKind::Link);
        assert!(p.warnings.iter().any(|w| w.contains("derived")));
    }

    #[test]
    fn extension_vs_code() {
        let (_td, tree) = fixture();
        let p = plan_at(&tree, "31_Papers", "notes.md").unwrap();
        assert_eq!(p.kind, PlanKind::File);
        assert_eq!(p.final_name, "31.02_notes.md");
        // a numeric code is not an extension
        let p = plan_at(&tree, "31_Papers", "31.07 Foo").unwrap();
        assert_eq!(p.kind, PlanKind::Dir);
        assert_eq!(p.final_name, "31.07_Foo");
    }

    #[test]
    fn title_only_suggests_by_anchor_kind() {
        let (_td, tree) = fixture();
        // category: next free item code (31.01/31.03/31.04 used -> 31.02)
        let p = plan_at(&tree, "31_Papers", "Title").unwrap();
        assert_eq!(p.final_name, "31.02_Title");
        // range: next free category (31, 32 used -> 30)
        let p = plan_at(&tree, "30-39_Research", "Title").unwrap();
        assert_eq!(p.final_name, "30_Title");
        // item dir: next segmented child code
        let p = plan_at(&tree, "31.04_Container", "Title").unwrap();
        assert_eq!(p.final_name, "31.04.01_Title");
        // file anchors re-anchor to their parent category
        let p = plan_at(&tree, "31.03_Two_Word_Note.txt", "Title").unwrap();
        assert_eq!(p.final_name, "31.02_Title");
    }

    #[test]
    fn duplicate_code_warns_and_existing_dest_errors() {
        let (_td, tree) = fixture();
        let p = plan_at(&tree, "31_Papers", "31.01 Dup").unwrap();
        assert!(p.warnings.iter().any(|w| w.contains("already in use")));
        assert!(plan_at(&tree, "31_Papers", "31.01 Existing").is_err());
    }

    #[test]
    fn forced_kinds() {
        let (_td, tree) = fixture();
        let ctx = CreateContext {
            tree: &tree,
            selected: node_by_suffix(&tree, "31_Papers"),
        };
        // file without extension gets .txt
        let p = parse_new_input_forced("31.07 Plain", &ctx, Some(PlanKind::File)).unwrap();
        assert_eq!(p.final_name, "31.07_Plain.txt");
        // link without URL: url stays None so the UI can prompt for it
        let p = parse_new_input_forced("31.07 Plain", &ctx, Some(PlanKind::Link)).unwrap();
        assert_eq!(p.final_name, "31.07_Plain.webloc");
        assert!(p.url.is_none());
        // dir keeps the dotted title verbatim and drops the URL with a warning
        let p =
            parse_new_input_forced("31.07 Notes.md https://x.io/z", &ctx, Some(PlanKind::Dir))
                .unwrap();
        assert_eq!(p.final_name, "31.07_Notes.md");
        assert!(p.url.is_none());
        assert!(p.warnings.iter().any(|w| w.contains("URL ignored")));
    }

    #[test]
    fn duplicate_groups_and_renumber_planning() {
        let (td, _) = fixture();
        let r = td.path().join("R");
        // a twin of 31.01 plus a segmented child under 31.04
        fs::create_dir(r.join("30-39_Research/31_Papers/31.01_Twin")).unwrap();
        fs::write(
            r.join("30-39_Research/31_Papers/31.04_Container/31.04.01_Sub.txt"),
            b"x",
        )
        .unwrap();
        let tree = fs_walk::scan_roots(&[r]).unwrap();

        let groups = model::duplicate_groups(&tree);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, "31.01");
        assert_eq!(groups[0].1.len(), 2);

        // twin gets the next free code, no children to recode
        let twin = node_by_suffix(&tree, "31.01_Twin").id.clone();
        let p = plan_renumber(&tree, &twin).unwrap();
        assert_eq!(p.new_code, "31.02");
        assert_eq!(p.new_name, "31.02_Twin");
        assert_eq!(p.child_renames, 0);

        // container cascade counts the embedded child code
        let container = node_by_suffix(&tree, "31.04_Container").id.clone();
        let p = plan_renumber(&tree, &container).unwrap();
        assert_eq!(p.new_code, "31.02");
        assert_eq!(p.child_renames, 1);

        // ranges are refused
        let range = node_by_suffix(&tree, "30-39_Research").id.clone();
        assert!(plan_renumber(&tree, &range).is_err());
    }

    #[test]
    fn move_planning() {
        let (_td, tree) = fixture();
        let src = node_by_suffix(&tree, "31.03_Two_Word_Note.txt").id.clone();
        let dest = node_by_suffix(&tree, "32_Empty").id.clone();
        // item under category: recoded, title keeps underscores
        let p = plan_move(&tree, &src, &dest).unwrap();
        assert_eq!(p.final_name, "32.01_Two_Word_Note.txt");
        // into own descendant: rejected
        let papers = node_by_suffix(&tree, "31_Papers").id.clone();
        let container = node_by_suffix(&tree, "31.04_Container").id.clone();
        assert!(plan_move(&tree, &papers, &container).is_err());
        // item into an item dir: name kept as-is
        let existing = node_by_suffix(&tree, "31.01_Existing").id.clone();
        let p = plan_move(&tree, &src, &existing).unwrap();
        assert_eq!(p.final_name, "31.03_Two_Word_Note.txt");
    }
}
