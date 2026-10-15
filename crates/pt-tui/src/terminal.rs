//! The crossterm terminal loop around the [`App`].
//!
//! Terminal state (raw mode, alternate screen) is restored by a guard
//! on every path — normal exit, error, and panic — so a crashing TUI
//! never leaves the user's terminal unusable.

use std::io::{self, Stdout};

use crossterm::event::{Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::render;

/// RAII guard for raw mode and the alternate screen. Its `Drop`
/// restores the terminal even when the event loop unwinds on panic.
struct TerminalGuard;

impl TerminalGuard {
    /// Enables raw mode and switches to the alternate screen.
    fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

/// Runs the TUI until the user quits. Restores the terminal on normal
/// exit, on error, and on panic (via the guard's drop during
/// unwinding).
///
/// # Errors
///
/// Returns an I/O error when the terminal cannot be set up or read.
pub fn run_terminal(mut app: App) -> io::Result<()> {
    let _guard = TerminalGuard::new()?;
    let backend = CrosstermBackend::<Stdout>::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let result = event_loop(&mut terminal, &mut app);
    let _ = terminal.show_cursor();
    result
}

/// The main loop: draw, wait for a key, let the app transition.
fn event_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| render::draw(app, frame))?;
        match crossterm::event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                app.handle_key(key.code);
            }
            _ => continue,
        }
        if app.should_quit() {
            return Ok(());
        }
    }
}
