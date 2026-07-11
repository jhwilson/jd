jd-helper
=========

Filesystem-first helper for Johnny Decimal navigation.

- Filesystem is the source of truth. JSON is a cached mirror written only after a disk mutation or explicit scan.
- Ships its own TUI (`jd-helper ui`, built on ratatui): tree navigation, fuzzy search, previews, and mutations — no fzf required.

Install
-------

```bash
cargo build --release
./scripts/install.sh
source ~/.zshrc
```

Requirements
------------

- Shell: zsh (the wrapper uses zsh-only features)
- Rust toolchain: stable Rust with cargo (to build `jd-helper`)

That's it — the old fzf/GNU-sed dependencies are gone.

- The zsh wrapper `scripts/jd.zsh` dynamically prepends `target/release` to `PATH` based on its location, so it keeps working if you move the repo.
- Configure your roots inside `scripts/jd.zsh` (or override per-invocation with `JD_ROOTS="/path/one /path/two"`).

Usage
-----

```zsh
# Launch the TUI
jd

# Jump to a code directly
jd 31.01
```

The TUI draws on stderr and prints a single action line on stdout
(`cd`/`edit`/`open` + target); the `jd()` wrapper dispatches it. Enter on a
directory cd's there, on a file opens `$EDITOR`, on a link opens the URL.

Keybindings
-----------

| Key | Action |
|---|---|
| type | filter the whole tree (spaces = AND-ed terms) |
| ↑/↓, PgUp/PgDn, Home/End | move selection |
| Tab | toggle fold (persists to `~/.cache/jd/state.json`) |
| →/← | expand / collapse |
| Ctrl-A / Ctrl-G | expand all / collapse all |
| Enter | dir→cd · file→`$EDITOR` · link→open |
| Ctrl-N | new (one smart prompt, see below) |
| Ctrl-R | rename title (code preserved) |
| Ctrl-V | move (fuzzy destination picker; items moved under a category get the next free code) |
| Ctrl-X | delete (confirmed; soft-deleted to a sibling `.jd_trash/`) |
| Ctrl-Z | undo the last delete |
| Ctrl-L | edit locations & links (`.jdmeta`, see below) |
| Ctrl-U | clear the filter |
| Esc | clear filter, then quit · Ctrl-Q/Ctrl-C quit |
| F1 | help overlay |

Creating things
---------------

Ctrl-N opens a single prompt. Type any of:

- `21.04 Quantum notes` — explicit item code (extended codes `21.041`,
  `21.04.02` work and are filed under their owning category/item)
- `20-29 Admin` — a range; `21 Papers` — a category
- `Reading list` — title only; the next free code under the selected node is
  suggested
- `notes.md` — an extension makes it a file
- `https://notion.so/abc Colloquium page` — a URL (anywhere in the input)
  makes it a `.webloc` link

Nothing touches disk until you confirm a preview line like
`will create DIR 21.04_Quantum_notes under 21 Papers` (warnings, e.g. a
duplicate code, show alongside). `d`/`f`/`l` override the inferred kind;
Esc aborts.

Locations & links (.jdmeta)
---------------------------

A JD number often lives in more places than the filesystem: a reMarkable
notebook, a Notion page, a filing cabinet. Ctrl-L records those in a plain
`.jdmeta` file inside the directory:

```
LOCATION=remarkable: Colloquium notebook
LOCATION=filing cabinet drawer 2
LINK=https://notion.so/abc123 Colloquium page
```

Entries appear at the top of the folder's preview (alongside any `.webloc`
/`.url` link items and `LOCATION=` file items inside it), so the tree works
as a single index of where everything is. The file is hand-editable;
unknown keys and comments survive edits. Scripts can use
`jd-helper meta list|add|remove --id <id> [--value ...] ROOTS...`.

Search behavior
---------------

- With an empty query, the tree respects your fold state.
- While typing, matching covers the entire tree regardless of folds, with
  match highlighting; spaces separate AND-ed fuzzy terms.
- Clearing the query returns to the fold-aware view.

Commands
--------

The TUI is one subcommand among scriptable primitives:

- `ui ROOTS... [--state PATH]` → the interactive TUI; prints `cd|edit|open\t<target>` on stdout
- `scan ROOTS...` → prints the JSON tree (authoritative FS view; includes `.jdmeta` locations/links and scan warnings)
- `tree ROOTS... [--state PATH] [--all] [--search Q]` → TSV listing
- `preview --type dir|file|link --path PATH` → small preview
- `resolve CODE ROOTS...` → absolute path for a JD code
- `parent --id ID [--path|--both] ROOTS...` → parent id/path
- `codes ROOTS...` → list all parsed codes
- `new [dir|file|link] --parent ID --name NAME [--url URL] [--location STR] ROOTS...`
- `new-interactive --parent-id ID --display DISPLAY [--kind k] ROOTS...` → prompt + confirm on the tty
- `rename --id ID --name TITLE ROOTS...` → change title only
- `move --id ID --parent PARENT_ID ROOTS...` → within one root; items under a category are recoded
- `delete --id ID ROOTS...` → soft delete to `.jd_trash/`
- `meta list|add|remove --id ID [--value STR] ROOTS...` → `.jdmeta` entries
- `suggest --parent CODE ROOTS...` → next free code under `NN`
- `toggle | expand-all | reset-state` → fold-state manipulation
- `write-index ROOTS... [--out PATH]` → write `ROOT/.jd_index.json`

Parsing and inclusion rules
---------------------------

- Ranges: `NN-NN_Title`
- Categories: `NN_Title`
- Items (dirs/files/links): `NN.MM_Title`, `NN.MMM_Title`, `NN.MMMM_Title`,
  segmented `NN.MM.KK_Title` (additional segments are two digits)
- Only conforming names are included (the root itself is always included).
  Non-conforming children are skipped.
- Duplicate codes among siblings are reported as warnings in the TUI status
  line (and in `scan` output), not silently accepted.

Ignore rules
------------

- Directory names: `.git`, `.obsidian`, `.auctex-auto`, `tmp`, `temp`,
  `cache`, `.cache`, `.tmp`, `logs`, `.jd_trash`
- File names: `.DS_Store`, `.jdmeta`, `*.log`, `*.bak`, `*.backup`, `*.old`,
  LaTeX aux files (PDFs kept)
- Names are checked per entry during the walk — a tree that lives *under*
  e.g. `/tmp` scans fine.

Preview behavior
----------------

- Directory preview: the locations/links index first, then entries (up to
  50) — files prefixed `YYYYMMDDTTTT...` first (newest first), then others
  alphabetically.
- File preview shows the first ~200 lines; link preview shows the resolved
  URL and file content.

Development & tests
-------------------

```bash
cargo test                 # unit + headless TUI tests + CLI integration
bash scripts/dev_fixture.sh  # build a sandbox tree in /tmp/jd_fixture
JD_ROOTS=/tmp/jd_fixture/T99_Test_Root jd   # try the TUI safely
```

The previous fzf-based wrapper is still available as `jd_fzf` during the
transition and will be removed later.

Uninstall
---------

```bash
./scripts/uninstall.sh
```

License
-------

This project is licensed under the MIT License. See `LICENSE` for details.
