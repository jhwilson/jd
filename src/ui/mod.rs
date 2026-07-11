pub mod actions;
pub mod app;
pub mod keymap;
pub mod prompt;
pub mod render;
pub mod rows;
pub mod search;

pub use actions::FinalAction;
use anyhow::{bail, Result};
use app::Outcome;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use std::{
    io::{self, IsTerminal},
    path::{Path, PathBuf},
};

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stderr(), LeaveAlternateScreen);
}

struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

pub fn run(roots: &[PathBuf], state: &Path) -> Result<Option<FinalAction>> {
    if !io::stderr().is_terminal() {
        bail!("jd-helper ui requires stderr to be a tty")
    }
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        prev_hook(info);
    }));
    enable_raw_mode()?;
    execute!(io::stderr(), EnterAlternateScreen)?;
    let _guard = Guard;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stderr()))?;
    let mut app = app::App::new(roots.to_vec(), state.to_path_buf())?;
    loop {
        terminal.draw(|f| render::draw(f, &mut app))?;
        if let Event::Key(k) = event::read()? {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match app.update(k) {
                Some(Outcome::Quit) => return Ok(None),
                Some(Outcome::Act(a)) => return Ok(Some(a)),
                None => {}
            }
        }
    }
}
