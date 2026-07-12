//! Headless tests for the TUI state machine: drive `App` with synthetic key
//! events against the T99 fixture and observe filesystem + state effects.

use jd_helper::ui::app::{App, Mode, Outcome, SuspendRequest};
use jd_helper::ui::render;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::Terminal;
use std::fs;
use std::path::{Path, PathBuf};

/// Rebuild the T99 tree (dirs and empty files only — contents don't matter
/// for state-machine tests) from the committed manifest.
fn build_fixture(dest: &Path) {
    for line in include_str!("fixtures/T99_tree.txt").lines() {
        let Some((kind, rel)) = line.split_once('\t') else {
            continue;
        };
        let target = dest.join(rel);
        match kind {
            "D" => fs::create_dir_all(&target).unwrap(),
            "F" => {
                fs::create_dir_all(target.parent().unwrap()).unwrap();
                fs::write(&target, b"").unwrap();
            }
            _ => {}
        }
    }
}

struct Harness {
    _td: tempfile::TempDir,
    root: PathBuf,
    state: PathBuf,
    app: App,
}

fn harness() -> Harness {
    let td = tempfile::tempdir().unwrap();
    let root = td.path().join("T99_Test_Root");
    build_fixture(&root);
    let state = td.path().join("state.json");
    let app = App::new(vec![root.clone()], state.clone()).unwrap();
    Harness {
        _td: td,
        root,
        state,
        app,
    }
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        app.handle_key(KeyCode::Char(c), KeyModifiers::NONE);
    }
}

fn ctrl(app: &mut App, c: char) -> Option<Outcome> {
    app.handle_key(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn move_cursor_to(app: &mut App, path_suffix: &str) {
    let pos = app
        .visible
        .iter()
        .position(|i| app.rows[*i].path.ends_with(path_suffix))
        .unwrap_or_else(|| panic!("row not visible: {}", path_suffix));
    app.cursor = pos;
}

fn selected_path(app: &App) -> String {
    app.selected().map(|r| r.path.clone()).unwrap_or_default()
}

#[test]
fn query_with_space_filters_across_folds() {
    let mut h = harness();
    // Nothing expanded: the category row is folded away.
    assert!(!h
        .app
        .visible
        .iter()
        .any(|i| h.app.rows[*i].path.ends_with("99_TestCat")));
    // A space-containing query matches "99-99 Test Range" (the fzf pipeline
    // used to crash on exactly this).
    type_str(&mut h.app, "test range");
    assert!(h
        .app
        .visible
        .iter()
        .any(|i| h.app.rows[*i].display == "99-99 Test Range"));
    // Query search ignores fold state entirely.
    type_str(&mut h.app, " "); // trailing atom separator must not error
    assert!(!h.app.visible.is_empty());
}

#[test]
fn esc_clears_query_then_quits() {
    let mut h = harness();
    type_str(&mut h.app, "xyz");
    assert!(h.app.handle_key(KeyCode::Esc, KeyModifiers::NONE).is_none());
    assert!(h.app.query.is_empty());
    assert!(matches!(
        h.app.handle_key(KeyCode::Esc, KeyModifiers::NONE),
        Some(Outcome::Quit)
    ));
}

#[test]
fn tab_toggles_fold_and_persists() {
    let mut h = harness();
    move_cursor_to(&mut h.app, "99-99_Test_Range");
    let id = h.app.selected().unwrap().id.clone();
    h.app.handle_key(KeyCode::Tab, KeyModifiers::NONE);
    assert!(h.app.expanded.expanded.contains(&id));
    assert!(fs::read_to_string(&h.state).unwrap().contains(&id));
    // Category is now visible; toggle back.
    assert!(h
        .app
        .visible
        .iter()
        .any(|i| h.app.rows[*i].path.ends_with("99_TestCat")));
    h.app.handle_key(KeyCode::Tab, KeyModifiers::NONE);
    assert!(!h.app.expanded.expanded.contains(&id));
}

#[test]
fn create_flow_with_confirm() {
    let mut h = harness();
    move_cursor_to(&mut h.app, "90-98_Second_Range");
    ctrl(&mut h.app, 'n');
    assert!(matches!(h.app.mode, Mode::Prompt { .. }));
    type_str(&mut h.app, "95 New Cat");
    h.app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(matches!(h.app.mode, Mode::Confirm { .. }));
    // Nothing on disk until confirmed.
    let dest = h.root.join("90-98_Second_Range/95_New_Cat");
    assert!(!dest.exists());
    h.app.handle_key(KeyCode::Char('y'), KeyModifiers::NONE);
    assert!(dest.is_dir());
    // Cursor lands on the new node.
    assert!(selected_path(&h.app).ends_with("95_New_Cat"));
}

#[test]
fn create_confirm_kind_override() {
    let mut h = harness();
    move_cursor_to(&mut h.app, "90-98_Second_Range");
    ctrl(&mut h.app, 'n');
    type_str(&mut h.app, "95 Notes");
    h.app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    // Inferred Dir; force File -> gets .txt appended.
    h.app.handle_key(KeyCode::Char('f'), KeyModifiers::NONE);
    assert!(matches!(h.app.mode, Mode::Confirm { .. }));
    h.app.handle_key(KeyCode::Char('y'), KeyModifiers::NONE);
    assert!(h.root.join("90-98_Second_Range/95_Notes.txt").is_file());
}

#[test]
fn create_abort_leaves_disk_untouched() {
    let mut h = harness();
    move_cursor_to(&mut h.app, "90-98_Second_Range");
    ctrl(&mut h.app, 'n');
    type_str(&mut h.app, "95 Aborted");
    h.app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    h.app.handle_key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(matches!(h.app.mode, Mode::Browse));
    assert!(!h.root.join("90-98_Second_Range/95_Aborted").exists());
}

#[test]
fn delete_then_undo_restores() {
    let mut h = harness();
    // Reveal the item: expand range then category.
    move_cursor_to(&mut h.app, "90-98_Second_Range");
    h.app.handle_key(KeyCode::Tab, KeyModifiers::NONE);
    move_cursor_to(&mut h.app, "90_Another_Cat");
    h.app.handle_key(KeyCode::Tab, KeyModifiers::NONE);
    move_cursor_to(&mut h.app, "90.01_Alpha_Item");
    let orig = h
        .root
        .join("90-98_Second_Range/90_Another_Cat/90.01_Alpha_Item");
    ctrl(&mut h.app, 'x');
    h.app.handle_key(KeyCode::Char('y'), KeyModifiers::NONE);
    assert!(!orig.exists());
    assert!(orig
        .parent()
        .unwrap()
        .join(".jd_trash/90.01_Alpha_Item")
        .is_dir());
    ctrl(&mut h.app, 'z');
    assert!(orig.is_dir());
    assert!(selected_path(&h.app).ends_with("90.01_Alpha_Item"));
}

#[test]
fn enter_emits_cd_action_for_dirs() {
    let mut h = harness();
    move_cursor_to(&mut h.app, "99-99_Test_Range");
    match h.app.handle_key(KeyCode::Enter, KeyModifiers::NONE) {
        Some(Outcome::Act(jd_helper::ui::FinalAction::Cd(p))) => {
            assert!(p.ends_with("99-99_Test_Range"))
        }
        other => panic!(
            "expected Cd action, got {:?}",
            other.is_some().then_some("some other outcome")
        ),
    }
}

#[test]
fn meta_add_and_remove_via_editor() {
    let mut h = harness();
    move_cursor_to(&mut h.app, "99-99_Test_Range");
    // ctrl-l opens the editor; 'a' prompts; classified as a location
    ctrl(&mut h.app, 'l');
    assert!(matches!(h.app.mode, Mode::MetaEdit { .. }));
    h.app.handle_key(KeyCode::Char('a'), KeyModifiers::NONE);
    type_str(&mut h.app, "remarkable: notebook 3");
    h.app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    let meta_path = h.root.join("99-99_Test_Range/.jdmeta");
    assert_eq!(
        fs::read_to_string(&meta_path).unwrap(),
        "LOCATION=remarkable: notebook 3\n"
    );
    // a URL becomes a LINK entry
    h.app.handle_key(KeyCode::Char('a'), KeyModifiers::NONE);
    type_str(&mut h.app, "https://notion.so/abc Colloquium page");
    h.app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(fs::read_to_string(&meta_path)
        .unwrap()
        .contains("LINK=https://notion.so/abc Colloquium page"));
    // the .jdmeta file itself never appears in the tree
    assert!(!h.app.rows.iter().any(|r| r.path.ends_with(".jdmeta")));
    // aggregated into the row's preview section
    move_cursor_to(&mut h.app, "99-99_Test_Range");
    let row = h.app.selected().unwrap();
    assert!(row.meta_lines.iter().any(|l| l.contains("notebook 3")));
    // remove the location (cursor 0 = first entry), confirmed
    ctrl(&mut h.app, 'l');
    h.app.handle_key(KeyCode::Char('x'), KeyModifiers::NONE);
    h.app.handle_key(KeyCode::Char('y'), KeyModifiers::NONE);
    assert!(matches!(h.app.mode, Mode::MetaEdit { .. }));
    assert!(!fs::read_to_string(&meta_path).unwrap().contains("LOCATION"));
}

/// Create a duplicate of 99.01 and give the original a .jdmeta drawer.
fn make_duplicate(h: &mut Harness) {
    let cat = h.root.join("99-99_Test_Range/99_TestCat");
    fs::create_dir(cat.join("99.01_Second")).unwrap();
    fs::write(
        cat.join("99.01_TestItem/.jdmeta"),
        "LOCATION=remarkable: 99.01 notebook\n",
    )
    .unwrap();
    // reload the app against the changed tree
    h.app = App::new(vec![h.root.clone()], h.state.clone()).unwrap();
}

#[test]
fn duplicate_wizard_renumbers_recommended_entry() {
    let mut h = harness();
    make_duplicate(&mut h);
    assert!(!h.app.tree.warnings.is_empty());
    ctrl(&mut h.app, 'f');
    let Mode::Duplicates { groups, cursor, .. } = &h.app.mode else {
        panic!("expected duplicates mode");
    };
    // recommended = fewest drawers = the twin without .jdmeta
    let rec = &groups[0].entries[groups[0].recommended];
    assert!(h.app.rows[rec.row_idx].path.ends_with("99.01_Second"));
    assert_eq!(*cursor, groups[0].recommended);

    h.app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(matches!(h.app.mode, Mode::Confirm { .. }));
    h.app.handle_key(KeyCode::Char('y'), KeyModifiers::NONE);
    // next free under 99: 99.01..99.04 used -> 99.05; no drawers -> Browse
    let cat = h.root.join("99-99_Test_Range/99_TestCat");
    assert!(cat.join("99.05_Second").is_dir());
    assert!(!cat.join("99.01_Second").exists());
    assert!(matches!(h.app.mode, Mode::Browse));
    assert!(h
        .app
        .status
        .as_deref()
        .unwrap_or("")
        .contains("99.01 → 99.05"));
    assert!(h.app.tree.warnings.is_empty());
}

#[test]
fn duplicate_wizard_cascades_and_reminds_about_drawers() {
    let mut h = harness();
    make_duplicate(&mut h);
    ctrl(&mut h.app, 'f');
    // pick the entry WITH drawers (the original, below the recommended twin)
    h.app.handle_key(KeyCode::Down, KeyModifiers::NONE);
    h.app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    h.app.handle_key(KeyCode::Char('y'), KeyModifiers::NONE);

    let cat = h.root.join("99-99_Test_Range/99_TestCat");
    // renamed with its embedded child recoded
    assert!(cat.join("99.05_TestItem").is_dir());
    assert!(cat
        .join("99.05_TestItem/99.05.01_Nested_Note.txt")
        .is_file());
    // its own .jdmeta had the old code inside -> rewritten
    assert_eq!(
        fs::read_to_string(cat.join("99.05_TestItem/.jdmeta")).unwrap(),
        "LOCATION=remarkable: 99.05 notebook\n"
    );
    // drawers > 0 -> lands in the meta editor with the update reminder
    assert!(matches!(h.app.mode, Mode::MetaEdit { .. }));
    assert!(h.app.status.as_deref().unwrap_or("").contains("update"));
}

#[test]
fn duplicate_wizard_merges_pointer_file_into_folder() {
    let mut h = harness();
    // the 53.02 shape: LOCATION pointer file next to the same-numbered folder
    let cat = h.root.join("99-99_Test_Range/99_TestCat");
    fs::write(cat.join("99.01_TestItem_remote"), b"LOCATION=Box\n").unwrap();
    h.app = App::new(vec![h.root.clone()], h.state.clone()).unwrap();

    ctrl(&mut h.app, 'f');
    // cursor sits on the recommended entry; 'm' merges the pointer into the
    // folder regardless of which of the two is selected
    h.app.handle_key(KeyCode::Char('m'), KeyModifiers::NONE);
    assert!(matches!(h.app.mode, Mode::Confirm { .. }));
    h.app.handle_key(KeyCode::Char('y'), KeyModifiers::NONE);

    // pointer absorbed into .jdmeta, file soft-deleted, duplicates gone
    assert_eq!(
        fs::read_to_string(cat.join("99.01_TestItem/.jdmeta")).unwrap(),
        "LOCATION=Box\n"
    );
    assert!(!cat.join("99.01_TestItem_remote").exists());
    assert!(cat.join(".jd_trash/99.01_TestItem_remote").is_file());
    assert!(h.app.duplicate_groups().is_empty());
    assert!(matches!(h.app.mode, Mode::Browse));
    // undoable: ctrl-z restores the pointer file
    ctrl(&mut h.app, 'z');
    assert!(cat.join("99.01_TestItem_remote").is_file());
}

#[test]
fn render_smoke() {
    let mut h = harness();
    ctrl(&mut h.app, 'a'); // expand all so category rows are on screen
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render::draw(f, &mut h.app)).unwrap();
    let text = format!("{:?}", terminal.backend().buffer());
    assert!(text.contains("Test Range"));
    assert!(text.contains("TestCat"));
    assert!(text.contains("JOHNNY.DECIMAL"));
    assert!(text.contains("99-99 Test Range"));
    assert!(!text.contains('┌'));
    assert!(!text.contains('│'));
    assert!(text.contains("^n new"));
}

fn suspend(outcome: Option<Outcome>) -> SuspendRequest {
    match outcome {
        Some(Outcome::Suspend(req)) => req,
        _ => panic!("expected suspend outcome"),
    }
}

#[test]
fn notes_seed_and_preserve_existing_content() {
    let mut h = harness();
    move_cursor_to(&mut h.app, "90-98_Second_Range");
    let req = suspend(ctrl(&mut h.app, 'e'));
    assert_eq!(fs::read_to_string(&req.file).unwrap(), "# Second Range\n\n");

    fs::write(&req.file, "existing notes\n").unwrap();
    h.app = App::new(vec![h.root.clone()], h.state.clone()).unwrap();
    move_cursor_to(&mut h.app, "90-98_Second_Range");
    let req = suspend(ctrl(&mut h.app, 'e'));
    assert_eq!(fs::read_to_string(req.file).unwrap(), "existing notes\n");
}

#[test]
fn notes_are_hidden_from_rows_and_directory_preview() {
    let h = harness();
    assert!(!h.app.rows.iter().any(|r| r.path.ends_with(".jdmeta.md")));
    let dir = h.root.join("99-99_Test_Range/99_TestCat/99.01_TestItem");
    assert!(!jd_helper::preview::preview_dir(&dir)
        .unwrap()
        .contains(".jdmeta.md"));
}

#[test]
fn after_editor_rescans_and_renders_notes() {
    let mut h = harness();
    move_cursor_to(&mut h.app, "90-98_Second_Range");
    let req = suspend(ctrl(&mut h.app, 'e'));
    fs::write(&req.file, "# Edited notes\n\nUseful **detail**.\n").unwrap();
    h.app
        .after_editor(req, std::process::Command::new("true").status());
    assert!(h.app.selected().unwrap().has_notes);
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render::draw(f, &mut h.app)).unwrap();
    assert!(format!("{:?}", terminal.backend().buffer()).contains("Edited notes"));
}

#[test]
fn markdown_file_is_rendered_in_preview() {
    let mut h = harness();
    let path = h
        .root
        .join("90-98_Second_Range/90_Another_Cat/90.02_Two_Word_Notes.md");
    fs::write(&path, "# Markdown title\n\nSome `code`.\n").unwrap();
    h.app = App::new(vec![h.root.clone()], h.state.clone()).unwrap();
    type_str(&mut h.app, "two word notes");
    move_cursor_to(&mut h.app, "90.02_Two_Word_Notes.md");
    let text = render::preview_content(h.app.selected().unwrap());
    assert!(text.to_string().contains("Markdown title"));
    assert!(text.to_string().contains("Some code."));
}

#[test]
fn notes_on_file_show_a_message() {
    let mut h = harness();
    type_str(&mut h.app, "two word notes");
    move_cursor_to(&mut h.app, "90.02_Two_Word_Notes.md");
    assert!(ctrl(&mut h.app, 'e').is_none());
    assert!(matches!(h.app.mode, Mode::Message { .. }));
}

#[test]
#[serial_test::serial]
fn editor_command_resolution_order() {
    let old = ["JD_EDITOR", "VISUAL", "EDITOR"].map(|k| (k, std::env::var_os(k)));
    std::env::remove_var("JD_EDITOR");
    std::env::set_var("VISUAL", "code -w");
    std::env::set_var("EDITOR", "nano");
    assert_eq!(jd_helper::ui::editor_command(), ["code", "-w"]);
    std::env::set_var("JD_EDITOR", "vim -u NONE");
    assert_eq!(jd_helper::ui::editor_command(), ["vim", "-u", "NONE"]);
    for (key, value) in old {
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}
