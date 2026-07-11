use crate::model::{Node, NodeType, Tree};
use crate::tsv::ExpandedState;

#[derive(Clone, Debug)]
pub struct Row {
    pub id: String,
    pub parent_idx: Option<usize>,
    pub depth: usize,
    pub code: Option<String>,
    pub title: String,
    pub display: String,
    pub path: String,
    pub node_type: NodeType,
    pub dir_like: bool,
    pub url: Option<String>,
}

pub fn flatten(t: &Tree) -> Vec<Row> {
    fn go(n: &Node, parent_idx: Option<usize>, depth: usize, out: &mut Vec<Row>) {
        let dir_like = matches!(
            n.node_type,
            NodeType::Range | NodeType::Category | NodeType::ItemDir | NodeType::Other
        );
        let display = n
            .code
            .as_ref()
            .map(|c| format!("{} {}", c, n.title))
            .unwrap_or_else(|| n.title.clone());
        out.push(Row {
            id: n.id.clone(),
            parent_idx,
            depth,
            code: n.code.clone(),
            title: n.title.clone(),
            display,
            path: n.path.clone(),
            node_type: n.node_type.clone(),
            dir_like,
            url: n.url.clone(),
        });
        let me = out.len() - 1;
        for c in &n.children {
            go(c, Some(me), depth + 1, out);
        }
    }
    let mut out = Vec::new();
    for r in &t.roots {
        go(r, None, 0, &mut out);
    }
    out
}

/// Fold-aware visible rows: a row is visible iff every ancestor below the root
/// is expanded (roots themselves are always expanded).
pub fn visible(rows: &[Row], expanded: &ExpandedState) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, r)| {
            let mut p = r.parent_idx;
            while let Some(i) = p {
                let pr = &rows[i];
                if pr.depth > 0 && !expanded.expanded.contains(&pr.id) {
                    return false;
                }
                p = pr.parent_idx;
            }
            true
        })
        .map(|(i, _)| i)
        .collect()
}
