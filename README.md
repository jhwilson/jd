jd-helper
=========

Filesystem-first helper for Johnny Decimal navigation.

- Filesystem is the source of truth. JSON is a cached mirror written only after a disk mutation or explicit scan.
- Integrates with fzf for interactive navigation, previews, and mutations.

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
- fzf: required for interactive navigation
- sed: recommended GNU sed as default `sed` (see macOS note below)

macOS (Homebrew):

```bash
brew install fzf gnu-sed
# Make GNU sed the default "sed"
echo 'export PATH="$(brew --prefix gnu-sed)/libexec/gnubin:$PATH"' >> ~/.zshrc
# (optional) install fzf keybindings (not required by jd)
$(brew --prefix)/opt/fzf/install --no-update-rc --no-bash --no-fish --key-bindings --completion
```

Linux:

```bash
# Debian/Ubuntu
sudo apt-get install -y fzf sed
# If your distro ships BusyBox sed, install GNU sed per distro docs
```

Optional tools (enhanced UX):

- bat: nicer file preview; falls back to vim/less/more/cat if absent
- vim: used first if present when viewing files from fzf
- less/more/cat: used as fallbacks for file preview

- The zsh wrapper `scripts/jd.zsh` dynamically prepends `target/release` to `PATH` based on its location, so it keeps working if you move the repo.
- Configure your roots inside `scripts/jd.zsh`:

```zsh
local ROOTS=("/Users/you/R50_Research" "/path/to/another_root")
```

Usage
-----

```zsh
# Launch the fzf tree
jd

# Jump to a code directly
jd 31.01
```

Fzf keybindings
---------------

- Tab: expand/collapse the selected node (persists to `~/.cache/jd/state.json`)
- Enter: dir→cd; file→view (vim/bat/less/more/cat); link→open
- Ctrl-n: create (dir/file/link) via an interactive Rust prompt
- Ctrl-r: rename title only (code preserved)
- Ctrl-m: move within the same root (item moved under a category auto-gets next free `NN.MM`)
- Ctrl-d: delete with confirmation (soft-deleted to `.jd_trash/`)
- Ctrl-a: expand-all (persists state)
- Ctrl-g: collapse-to-roots (persists state)

Search behavior
---------------

- With an empty query, the tree respects your fold state.
- When typing a query, the list reloads with `--all` so fuzzy search covers the entire tree.
- Clearing the query returns to the fold-aware view.

Commands
--------

- `scan ROOTS...` → prints the JSON tree (authoritative FS view)
- `tree ROOTS... [--state PATH] [--all]` → TSV for fzf; uses saved fold state unless `--all`
- `preview --type dir|file|link --path PATH` → small preview for fzf
- `resolve CODE ROOTS...` → prints absolute path for a JD code
- `parent --id ID [--path|--both] ROOTS...` → parent id/path
- `codes ROOTS...` → list all parsed codes
- `new [dir|file|link] --parent ID --name NAME [--url URL] [--location STR] ROOTS...` → mutate FS then rescan
- `new-interactive --parent-id ID --display DISPLAY ROOTS...` → interactive creation; suggests codes contextually
- `rename --id ID --name TITLE ROOTS...` → change title only
- `move --id ID --parent PARENT_ID ROOTS...` → within same root; reassign item codes under categories
- `delete --id ID ROOTS...` → soft delete to `.jd_trash/`
- `suggest --parent CODE ROOTS...` → suggest next free code under `NN`
- `toggle --state STATE.json --id ID` → flip expanded state for a node
- `write-index ROOTS... [--out PATH]` → write `ROOT/.jd_index.json` per root (or combined with `--out`)
- `reset-state --state STATE.json` → clear fold state (roots only)
- `expand-all --state STATE.json ROOTS...` → mark all dir-like nodes expanded

Parsing and inclusion rules
---------------------------

- Ranges: `NN-NN_Title`
- Categories: `NN_Title`
- Items (dirs/files/links):
  - `NN.MM_Title`
  - `NN.MMM_Title`, `NN.MMMM_Title`
  - segmented: `NN.MM.KK_Title` (additional segments are two digits)
- Only conforming names are included (root itself is always included). Non-conforming children are skipped.

Ignore rules
------------

- Directories anywhere in the path: `.git`, `.obsidian`, `.auctex-auto`, `tmp`, `temp`, `cache`, `.cache`, `.tmp`, `logs`
- Files: `.DS_Store`, `*.log`, `*.bak`, `*.backup`, `*.old`
- LaTeX aux files are ignored (PDFs kept)

Preview behavior
----------------

- Directory preview lists entries (up to 50). Files prefixed with `YYYYMMDDTTTT...` are shown first (newest first), then others alphabetically.
- File preview shows the first ~200 lines; link preview shows file content (and URL if readable).

Tests
-----

```bash
cargo test
```

Uninstall
---------

```bash
./scripts/uninstall.sh
```

