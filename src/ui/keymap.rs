pub const HINT: &str =
    "enter open · tab fold · ^N new ^R rename ^V move ^X delete ^Z undo ^L links · ^K help";

pub const HELP: &str = "\
Navigate      type to filter (spaces ok) · ↑/↓ PgUp/PgDn Home/End select
Fold          tab toggle · →/← expand/collapse · ^A expand all · ^G collapse all
Open          enter — dir: cd · file: $EDITOR · link: open URL
Create        ^N — one prompt: '21.04 Title' | 'Title' | 'notes.md' | paste a URL
              kind is inferred; d/f/l in the confirm step overrides it
Rename        ^R — edits the title, the code is preserved
Move          ^V — pick a destination; items moved under a category are recoded
Delete        ^X — to .jd_trash/ next to the item · ^Z undoes the last delete
Locations     ^L — a number's other homes (reMarkable, Notion, …) in .jdmeta;
              shown atop the preview · a add ('drawer 2' or a URL) · x remove
Query         ^U clear · esc clears, then quits
Help          ^K (or F1)
Quit          ^Q or ^C · esc (with empty filter)";
