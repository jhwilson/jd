use super::{
    actions::FinalAction,
    prompt::LineEditor,
    rows::{self, Row},
    search::Search,
};
use crate::{
    fs_walk, meta,
    model::{self, NodeType},
    mutate,
    plan::{self, CreatePlan, MovePlan, PlanKind},
    state,
    tsv::ExpandedState,
};
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

/// What a key press means for the event loop.
pub enum Outcome {
    Quit,
    Act(FinalAction),
}

pub enum PromptKind {
    /// Smart create prompt; `anchor_id` is the selected node (files/links are
    /// re-anchored to their parent dir at parse time).
    Create { anchor_id: String },
    Rename { id: String },
    /// Follow-up URL prompt after forcing kind=Link on input without a URL.
    LinkUrl {
        plan: CreatePlan,
        input: String,
        anchor_id: String,
    },
    /// Add a location ("remarkable: notebook 3") or link (anything with ://)
    /// to the node's .jdmeta.
    MetaAdd { id: String },
}

pub enum PendingOp {
    /// `input`/`anchor_id` are kept so d/f/l can re-derive the plan.
    Create {
        plan: CreatePlan,
        input: String,
        anchor_id: String,
    },
    Move(MovePlan),
    Delete {
        id: String,
        path: PathBuf,
        display: String,
    },
    MetaRemove {
        id: String,
        dir: PathBuf,
        entry: meta::Entry,
    },
}

pub enum Mode {
    Browse,
    Prompt {
        kind: PromptKind,
        editor: LineEditor,
    },
    Confirm {
        pending: PendingOp,
    },
    MovePicker {
        src_id: String,
        query: String,
        cursor: usize,
        candidates: Vec<usize>,
    },
    Message {
        text: String,
        error: bool,
    },
    /// Edit the selected node's locations/links (.jdmeta).
    MetaEdit {
        id: String,
        cursor: usize,
    },
    Help,
}

pub struct App {
    pub roots: Vec<PathBuf>,
    pub state_path: PathBuf,
    pub tree: crate::model::Tree,
    pub rows: Vec<Row>,
    pub visible: Vec<usize>,
    pub expanded: ExpandedState,
    pub query: String,
    pub cursor: usize,
    pub mode: Mode,
    pub last_delete: Option<(PathBuf, PathBuf)>, // (trash, original)
    pub status: Option<String>,
    pub search: Search,
}

impl App {
    pub fn new(roots: Vec<PathBuf>, state_path: PathBuf) -> Result<Self> {
        let tree = fs_walk::scan_roots(&roots)?;
        let expanded = state::load_state_or_default(Some(&state_path))?;
        let rows = rows::flatten(&tree);
        let visible = rows::visible(&rows, &expanded);
        Ok(Self {
            roots,
            state_path,
            tree,
            rows,
            visible,
            expanded,
            query: String::new(),
            cursor: 0,
            mode: Mode::Browse,
            last_delete: None,
            status: None,
            search: Search::default(),
        })
    }

    pub fn selected(&self) -> Option<&Row> {
        self.visible
            .get(self.cursor)
            .and_then(|i| self.rows.get(*i))
    }

    /// Recompute visible rows: fold-aware tree when the query is empty,
    /// full-tree fuzzy match while a query is active.
    fn filter(&mut self) {
        self.visible = if self.query.is_empty() {
            rows::visible(&self.rows, &self.expanded)
        } else {
            self.search.matched(&self.rows, &self.query)
        };
        self.cursor = self.cursor.min(self.visible.len().saturating_sub(1));
    }

    /// Rescan the filesystem and, if `select` matches a row id or path, expand
    /// its ancestors so it is visible and put the cursor on it.
    fn rescan(&mut self, select: Option<&str>) -> Result<()> {
        self.tree = fs_walk::scan_roots(&self.roots)?;
        self.rows = rows::flatten(&self.tree);
        if let Some(key) = select {
            if let Some(ri) = self
                .rows
                .iter()
                .position(|r| r.id == key || r.path == key)
            {
                let mut p = self.rows[ri].parent_idx;
                let mut changed = false;
                while let Some(i) = p {
                    let r = &self.rows[i];
                    if r.depth > 0 {
                        changed |= self.expanded.expanded.insert(r.id.clone());
                    }
                    p = r.parent_idx;
                }
                if changed {
                    let _ = state::save_state(&self.state_path, &self.expanded);
                }
            }
        }
        self.filter();
        if let Some(key) = select {
            if let Some(pos) = self
                .visible
                .iter()
                .position(|i| self.rows[*i].id == key || self.rows[*i].path == key)
            {
                self.cursor = pos;
            }
        }
        Ok(())
    }

    fn save_folds(&self) {
        let _ = state::save_state(&self.state_path, &self.expanded);
    }

    fn message(&mut self, text: impl Into<String>) {
        self.mode = Mode::Message {
            text: text.into(),
            error: true,
        };
    }

    /// Move-destination candidates: dir-like rows that are not the source or
    /// inside it, fuzzy-filtered by the picker query.
    fn move_candidates(&mut self, src_id: &str, query: &str) -> Vec<usize> {
        let src_path = self
            .rows
            .iter()
            .find(|r| r.id == src_id)
            .map(|r| format!("{}/", r.path))
            .unwrap_or_default();
        let matched = self.search.matched(&self.rows, query);
        matched
            .into_iter()
            .filter(|i| {
                let r = &self.rows[*i];
                r.dir_like && r.id != src_id && !format!("{}/", r.path).starts_with(&src_path)
            })
            .collect()
    }

    pub fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) -> Option<Outcome> {
        self.update(KeyEvent::new(code, mods))
    }

    pub fn update(&mut self, k: KeyEvent) -> Option<Outcome> {
        // Take the mode out so handlers can own its state and set the next
        // mode without fighting the borrow checker.
        let mode = std::mem::replace(&mut self.mode, Mode::Browse);
        match mode {
            Mode::Browse => self.on_browse(k),
            Mode::Prompt { kind, editor } => {
                self.on_prompt(kind, editor, k);
                None
            }
            Mode::Confirm { pending } => {
                self.on_confirm(pending, k);
                None
            }
            Mode::MovePicker {
                src_id,
                query,
                cursor,
                candidates,
            } => {
                self.on_move_picker(src_id, query, cursor, candidates, k);
                None
            }
            Mode::MetaEdit { id, cursor } => {
                self.on_meta_edit(id, cursor, k);
                None
            }
            // Message and Help are dismissed by any key.
            Mode::Message { .. } | Mode::Help => None,
        }
    }

    /// The node's own .jdmeta entries (locations first, then links) and the
    /// directory they live in.
    pub fn meta_entries(&self, id: &str) -> Option<(PathBuf, Vec<meta::Entry>)> {
        let n = model::find_node(&self.tree, id)?;
        let mut entries: Vec<meta::Entry> = n
            .locations
            .iter()
            .cloned()
            .map(meta::Entry::Location)
            .collect();
        entries.extend(n.links.iter().cloned().map(meta::Entry::Link));
        Some((PathBuf::from(&n.path), entries))
    }

    fn on_meta_edit(&mut self, id: String, mut cursor: usize, k: KeyEvent) {
        let Some((dir, entries)) = self.meta_entries(&id) else {
            self.mode = Mode::Browse;
            return;
        };
        match k.code {
            KeyCode::Esc => {
                self.mode = Mode::Browse;
                return;
            }
            KeyCode::Char('a') => {
                self.mode = Mode::Prompt {
                    kind: PromptKind::MetaAdd { id },
                    editor: LineEditor::default(),
                };
                return;
            }
            KeyCode::Char('x') => {
                if let Some(entry) = entries.get(cursor) {
                    self.mode = Mode::Confirm {
                        pending: PendingOp::MetaRemove {
                            id,
                            dir,
                            entry: entry.clone(),
                        },
                    };
                    return;
                }
            }
            KeyCode::Up => cursor = cursor.saturating_sub(1),
            KeyCode::Down => cursor = (cursor + 1).min(entries.len().saturating_sub(1)),
            _ => {}
        }
        self.mode = Mode::MetaEdit { id, cursor };
    }

    fn on_browse(&mut self, k: KeyEvent) -> Option<Outcome> {
        self.status = None;
        if k.modifiers == KeyModifiers::CONTROL {
            match k.code {
                KeyCode::Char('q') | KeyCode::Char('c') => return Some(Outcome::Quit),
                KeyCode::Char('u') => {
                    self.query.clear();
                    self.filter();
                }
                KeyCode::Char('a') => {
                    self.expanded.expanded = self
                        .rows
                        .iter()
                        .filter(|r| r.dir_like)
                        .map(|r| r.id.clone())
                        .collect();
                    self.save_folds();
                    self.filter();
                }
                KeyCode::Char('g') => {
                    self.expanded.expanded.clear();
                    self.save_folds();
                    self.filter();
                }
                KeyCode::Char('n') => {
                    if let Some(r) = self.selected() {
                        self.mode = Mode::Prompt {
                            kind: PromptKind::Create {
                                anchor_id: r.id.clone(),
                            },
                            editor: LineEditor::default(),
                        };
                    }
                }
                KeyCode::Char('r') => {
                    if let Some(r) = self.selected() {
                        if r.depth == 0 {
                            self.message("cannot rename a root");
                        } else {
                            self.mode = Mode::Prompt {
                                kind: PromptKind::Rename { id: r.id.clone() },
                                editor: LineEditor::new(&r.title),
                            };
                        }
                    }
                }
                KeyCode::Char('v') => {
                    if let Some(r) = self.selected() {
                        if r.depth == 0 {
                            self.message("cannot move a root");
                        } else {
                            let src_id = r.id.clone();
                            let candidates = self.move_candidates(&src_id, "");
                            self.mode = Mode::MovePicker {
                                src_id,
                                query: String::new(),
                                cursor: 0,
                                candidates,
                            };
                        }
                    }
                }
                KeyCode::Char('x') => {
                    if let Some(r) = self.selected() {
                        if r.depth == 0 {
                            self.message("cannot delete a root");
                        } else {
                            self.mode = Mode::Confirm {
                                pending: PendingOp::Delete {
                                    id: r.id.clone(),
                                    path: r.path.clone().into(),
                                    display: r.display.clone(),
                                },
                            };
                        }
                    }
                }
                KeyCode::Char('k') => self.mode = Mode::Help,
                KeyCode::Char('l') => {
                    if let Some(r) = self.selected() {
                        if r.dir_like {
                            self.mode = Mode::MetaEdit {
                                id: r.id.clone(),
                                cursor: 0,
                            };
                        } else {
                            self.message("locations live on directories — select a folder");
                        }
                    }
                }
                KeyCode::Char('z') => match self.last_delete.take() {
                    Some((trash, orig)) => {
                        match mutate::undo_delete(&self.roots, &trash, &orig) {
                            Ok(()) => {
                                self.query.clear();
                                let key = orig.to_string_lossy().to_string();
                                let _ = self.rescan(Some(&key));
                                self.status = Some(format!("restored {}", key));
                            }
                            Err(e) => self.message(e.to_string()),
                        }
                    }
                    None => self.status = Some("nothing to undo".into()),
                },
                _ => {}
            }
            return None;
        }
        match k.code {
            KeyCode::Esc => {
                if self.query.is_empty() {
                    return Some(Outcome::Quit);
                }
                self.query.clear();
                self.filter();
            }
            KeyCode::Char(c) => {
                self.query.push(c);
                self.filter();
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.filter();
            }
            KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(self.visible.len().saturating_sub(1))
            }
            KeyCode::PageUp => self.cursor = self.cursor.saturating_sub(10),
            KeyCode::PageDown => {
                self.cursor = (self.cursor + 10).min(self.visible.len().saturating_sub(1))
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.visible.len().saturating_sub(1),
            KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                if let Some(r) = self.selected() {
                    if r.dir_like && r.depth > 0 {
                        let id = r.id.clone();
                        match k.code {
                            KeyCode::Right => {
                                self.expanded.expanded.insert(id);
                            }
                            KeyCode::Left => {
                                self.expanded.expanded.remove(&id);
                            }
                            _ => self.expanded.toggle(&id),
                        }
                        self.save_folds();
                        self.filter();
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(r) = self.selected() {
                    return Some(Outcome::Act(match r.node_type {
                        NodeType::File => FinalAction::Edit(r.path.clone().into()),
                        NodeType::Link => {
                            FinalAction::Open(r.url.clone().unwrap_or_else(|| r.path.clone()))
                        }
                        _ => FinalAction::Cd(r.path.clone().into()),
                    }));
                }
            }
            KeyCode::F(1) => self.mode = Mode::Help,
            _ => {}
        }
        None
    }

    fn on_prompt(&mut self, kind: PromptKind, mut editor: LineEditor, k: KeyEvent) {
        match k.code {
            KeyCode::Esc => self.mode = Mode::Browse,
            KeyCode::Enter => {
                let input = editor.buffer.trim().to_string();
                match kind {
                    PromptKind::Create { anchor_id } => {
                        if input.is_empty() {
                            self.mode = Mode::Browse;
                            return;
                        }
                        self.plan_create(&input, &anchor_id, None);
                    }
                    PromptKind::Rename { id } => {
                        if input.is_empty() {
                            self.mode = Mode::Browse;
                            return;
                        }
                        let result = plan::plan_rename(&self.tree, &id, &input)
                            .and_then(|p| mutate::execute_rename(&self.roots, &p).map(|_| p));
                        match result {
                            Ok(p) => {
                                let _ = self.rescan(Some(&id));
                                self.status = Some(format!("renamed to {}", p.new_name));
                                self.mode = Mode::Browse;
                            }
                            Err(e) => self.message(e.to_string()),
                        }
                    }
                    PromptKind::LinkUrl {
                        mut plan,
                        input: orig_input,
                        anchor_id,
                    } => {
                        if input.is_empty() {
                            self.mode = Mode::Browse;
                            return;
                        }
                        plan.url = Some(input);
                        self.mode = Mode::Confirm {
                            pending: PendingOp::Create {
                                plan,
                                input: orig_input,
                                anchor_id,
                            },
                        };
                    }
                    PromptKind::MetaAdd { id } => {
                        let Some(entry) = meta::Entry::from_input(&input) else {
                            self.mode = Mode::MetaEdit { id, cursor: 0 };
                            return;
                        };
                        let Some((dir, _)) = self.meta_entries(&id) else {
                            self.mode = Mode::Browse;
                            return;
                        };
                        match meta::add_entry(&dir, &entry) {
                            Ok(()) => {
                                let _ = self.rescan(Some(&id));
                                self.mode = Mode::MetaEdit { id, cursor: 0 };
                            }
                            Err(e) => self.message(e.to_string()),
                        }
                    }
                }
            }
            _ => {
                editor.key(k.code, k.modifiers);
                self.mode = Mode::Prompt { kind, editor };
            }
        }
    }

    /// Parse create input (optionally with a forced kind) and advance to
    /// Confirm — or to the URL prompt when a link has no URL yet.
    fn plan_create(&mut self, input: &str, anchor_id: &str, force: Option<PlanKind>) {
        let plan = model::find_node(&self.tree, anchor_id)
            .ok_or_else(|| anyhow::anyhow!("selected node no longer exists"))
            .and_then(|n| {
                plan::parse_new_input_forced(
                    input,
                    &plan::CreateContext {
                        tree: &self.tree,
                        selected: n,
                    },
                    force,
                )
            });
        match plan {
            Ok(plan) if plan.kind == PlanKind::Link && plan.url.is_none() => {
                self.mode = Mode::Prompt {
                    kind: PromptKind::LinkUrl {
                        plan,
                        input: input.to_string(),
                        anchor_id: anchor_id.to_string(),
                    },
                    editor: LineEditor::default(),
                };
            }
            Ok(plan) => {
                self.mode = Mode::Confirm {
                    pending: PendingOp::Create {
                        plan,
                        input: input.to_string(),
                        anchor_id: anchor_id.to_string(),
                    },
                };
            }
            Err(e) => self.message(e.to_string()),
        }
    }

    fn on_confirm(&mut self, pending: PendingOp, k: KeyEvent) {
        // Meta removals return to the meta editor, not Browse.
        if let PendingOp::MetaRemove { id, dir, entry } = &pending {
            match k.code {
                KeyCode::Esc | KeyCode::Char('n') => {
                    self.mode = Mode::MetaEdit {
                        id: id.clone(),
                        cursor: 0,
                    };
                }
                KeyCode::Enter | KeyCode::Char('y') => match meta::remove_entry(dir, entry) {
                    Ok(()) => {
                        let id = id.clone();
                        let _ = self.rescan(Some(&id));
                        self.mode = Mode::MetaEdit { id, cursor: 0 };
                    }
                    Err(e) => self.message(e.to_string()),
                },
                _ => self.mode = Mode::Confirm { pending },
            }
            return;
        }
        // d/f/l override the inferred kind for pending creates.
        if let PendingOp::Create {
            input, anchor_id, ..
        } = &pending
        {
            let force = match k.code {
                KeyCode::Char('d') => Some(PlanKind::Dir),
                KeyCode::Char('f') => Some(PlanKind::File),
                KeyCode::Char('l') => Some(PlanKind::Link),
                _ => None,
            };
            if let Some(force) = force {
                let (input, anchor_id) = (input.clone(), anchor_id.clone());
                self.plan_create(&input, &anchor_id, Some(force));
                return;
            }
        }
        match k.code {
            KeyCode::Esc | KeyCode::Char('n') => self.mode = Mode::Browse,
            KeyCode::Enter | KeyCode::Char('y') => {
                let result = match &pending {
                    PendingOp::Create { plan, .. } => {
                        let key = plan.dest_path.to_string_lossy().to_string();
                        mutate::execute_create(&self.roots, plan).map(|_| {
                            (Some(key), format!("created {}", plan.final_name))
                        })
                    }
                    PendingOp::Move(p) => mutate::execute_move(&self.roots, p)
                        .map(|_| (Some(p.id.clone()), format!("moved to {}", p.final_name))),
                    PendingOp::Delete { id, path, display } => {
                        mutate::delete_node(&self.roots, id).map(|trash| {
                            self.last_delete = Some((trash, path.clone()));
                            (None, format!("trashed {} · ctrl-z to undo", display))
                        })
                    }
                    PendingOp::MetaRemove { .. } => unreachable!("handled above"),
                };
                match result {
                    Ok((select, status)) => {
                        if select.is_some() {
                            self.query.clear();
                        }
                        let _ = self.rescan(select.as_deref());
                        self.status = Some(status);
                        self.mode = Mode::Browse;
                    }
                    Err(e) => self.message(e.to_string()),
                }
            }
            _ => self.mode = Mode::Confirm { pending },
        }
    }

    fn on_move_picker(
        &mut self,
        src_id: String,
        mut query: String,
        mut cursor: usize,
        candidates: Vec<usize>,
        k: KeyEvent,
    ) {
        match k.code {
            KeyCode::Esc => {
                self.mode = Mode::Browse;
                return;
            }
            KeyCode::Enter => {
                if let Some(i) = candidates.get(cursor) {
                    let dest_id = self.rows[*i].id.clone();
                    match plan::plan_move(&self.tree, &src_id, &dest_id) {
                        Ok(p) => {
                            self.mode = Mode::Confirm {
                                pending: PendingOp::Move(p),
                            }
                        }
                        Err(e) => self.message(e.to_string()),
                    }
                } else {
                    self.mode = Mode::Browse;
                }
                return;
            }
            KeyCode::Char(c) if k.modifiers != KeyModifiers::CONTROL => {
                query.push(c);
                cursor = 0;
            }
            KeyCode::Backspace => {
                query.pop();
                cursor = 0;
            }
            KeyCode::Up => cursor = cursor.saturating_sub(1),
            KeyCode::Down => cursor += 1,
            _ => {}
        }
        let candidates = self.move_candidates(&src_id, &query);
        cursor = cursor.min(candidates.len().saturating_sub(1));
        self.mode = Mode::MovePicker {
            src_id,
            query,
            cursor,
            candidates,
        };
    }
}
