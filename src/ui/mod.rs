pub mod actions;
pub mod app;
pub mod keymap;
pub mod prompt;
pub mod render;
pub mod rows;
pub mod search;
pub mod theme;

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
    fs::File,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
};

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stderr(), LeaveAlternateScreen);
}

fn suspend_tui() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stderr(), LeaveAlternateScreen)?;
    Ok(())
}

fn resume_tui(terminal: &mut Terminal<CrosstermBackend<io::Stderr>>) -> Result<()> {
    enable_raw_mode()?;
    execute!(io::stderr(), EnterAlternateScreen)?;
    terminal.clear()?;
    Ok(())
}

pub fn editor_command() -> Vec<String> {
    ["JD_EDITOR", "VISUAL", "EDITOR"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok().filter(|s| !s.trim().is_empty()))
        .unwrap_or_else(|| "vim".into())
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

pub fn spawn_editor(path: &Path) -> io::Result<ExitStatus> {
    let parts = editor_command();
    let mut command = Command::new(&parts[0]);
    command
        .args(&parts[1..])
        .arg(path)
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit());
    command.stdout(match File::options().write(true).open("/dev/tty") {
        Ok(tty) => Stdio::from(tty),
        Err(_) => Stdio::inherit(),
    });
    command.status()
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
                Some(Outcome::Suspend(req)) => {
                    suspend_tui()?;
                    let result = spawn_editor(&req.file);
                    resume_tui(&mut terminal)?;
                    app.after_editor(req, result);
                }
                None => {}
            }
        }
    }
}
