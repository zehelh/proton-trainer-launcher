//! Ratatui rendering of the [`App`] screens.
//!
//! Rendering is a pure function of the app state and works at any
//! terminal size: lists scroll, long text wraps, and no layout has a
//! fixed minimum that could panic on small terminals.

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Screen};
use crate::backend::format_plan_row;

/// Draws the current screen. Called by the terminal loop once per
/// key event.
pub fn draw(app: &App, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let (body, footer) = match app.screen() {
        Screen::Games => (games_widget(app), "Up/Down select  Enter attach  q quit"),
        Screen::Entries => (
            entries_widget(app),
            "Space toggle  Enter plan  a plan+apply  Esc back  q quit",
        ),
        Screen::Plan => (
            plan_widget(app),
            "a apply  d/Esc back (this view is a dry run)  q quit",
        ),
        Screen::Result => (result_widget(app), "Esc back  q quit"),
        Screen::Message => (message_widget(app), "Enter/Esc back  q quit"),
    };
    frame.render_widget(body, chunks[0]);
    frame.render_widget(Paragraph::new(footer), chunks[1]);
}

fn games_widget(app: &App) -> Paragraph<'static> {
    let title = "proton-trainer - installed games with trainers";
    let lines: Vec<Line> = app
        .games()
        .iter()
        .enumerate()
        .map(|(index, game)| {
            let pointer = if index == app.game_index() {
                "> "
            } else {
                "  "
            };
            Line::from(format!("{}{} ({})", pointer, game.name, game.app_id))
        })
        .collect();
    Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
}

fn entries_widget(app: &App) -> Paragraph<'static> {
    let Some((pid, exe_name)) = app.target() else {
        return Paragraph::new("not attached");
    };
    let trainer = app.trainer_id().unwrap_or("?");
    let title = format!("trainer {trainer} - {exe_name} (pid {pid})");
    let lines: Vec<Line> = app
        .entry_rows()
        .iter()
        .enumerate()
        .map(|(index, (id, label, freeze, selected))| {
            let mark = if *selected { "[x]" } else { "[ ]" };
            let pointer = if index == app.entry_index() {
                "> "
            } else {
                "  "
            };
            let freeze = if *freeze { " [freeze]" } else { "" };
            Line::from(format!("{pointer}{mark} {id}: {label}{freeze}"))
        })
        .collect();
    let mut text = vec![
        Line::from(format!("target: {exe_name} (pid {pid})")),
        Line::from(""),
    ];
    text.extend(lines);
    Paragraph::new(Text::from(text))
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
}

fn plan_widget(app: &App) -> Paragraph<'static> {
    let mut text = String::from("Dry-run plan (nothing has been written):\n\n");
    for row in app.plans() {
        text.push_str(&format_plan_row(row));
        text.push('\n');
    }
    Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("plan (dry run)"),
        )
        .wrap(Wrap { trim: false })
}

fn result_widget(app: &App) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    for result in app.results() {
        let line = match &result.outcome {
            Ok(()) => format!(
                "{} ({}): applied and verified",
                result.entry_id, result.label
            ),
            Err(error) => format!("{} ({}): FAILED - {error}", result.entry_id, result.label),
        };
        lines.push(Line::from(line));
    }
    if app.freeze_active() {
        lines.push(Line::from(""));
        lines.push(Line::from("freeze: active for the entries marked [freeze]"));
    }
    Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("apply results"),
        )
        .wrap(Wrap { trim: false })
}

fn message_widget(app: &App) -> Paragraph<'static> {
    Paragraph::new(app.message().to_string())
        .block(Block::default().borders(Borders::ALL).title("message"))
        .wrap(Wrap { trim: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Screen;
    use crate::backend::{ApplyResult, AttachOutcome, EntryBrief, GameRow, PlanRow, TuiBackend};
    use pt_core::maps::Region;

    struct Fake;

    impl TuiBackend for Fake {
        fn games(&mut self) -> Result<Vec<GameRow>, String> {
            Ok(vec![GameRow {
                app_id: 367520,
                name: "Hollow Knight".to_string(),
                trainer_id: "hollow-knight".to_string(),
            }])
        }

        fn attach(&mut self, _trainer_id: &str) -> Result<AttachOutcome, String> {
            Ok(AttachOutcome::Running {
                pid: 42,
                exe_name: "hollow_knight.exe".to_string(),
                entries: vec![EntryBrief {
                    id: "soul".to_string(),
                    label: "Infinite SOUL".to_string(),
                    freeze: true,
                }],
            })
        }

        fn plan(&mut self, _entry_ids: &[String]) -> Result<Vec<PlanRow>, String> {
            Ok(vec![PlanRow {
                entry_id: "soul".to_string(),
                label: "Infinite SOUL".to_string(),
                freeze: true,
                plan: pt_core::WritePlan {
                    pid: 42,
                    address: 0x1000,
                    old_bytes: vec![0x39, 0x05, 0x00, 0x00],
                    new_bytes: vec![0x63, 0x00, 0x00, 0x00],
                    region: Region {
                        start: 0x1000,
                        end: 0x2000,
                        readable: true,
                        writable: true,
                        executable: false,
                        private: true,
                        pathname: None,
                    },
                },
            }])
        }

        fn apply(&mut self) -> Result<Vec<ApplyResult>, String> {
            Ok(vec![ApplyResult {
                entry_id: "soul".to_string(),
                label: "Infinite SOUL".to_string(),
                outcome: Ok(()),
            }])
        }

        fn start_freeze(&mut self, _entry_ids: &[String]) -> Result<(), String> {
            Ok(())
        }

        fn stop_freeze(&mut self) {}
    }

    fn buffer_text(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect()
    }

    fn draw_and_read(app: &App, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).expect("test terminal");
        terminal.draw(|frame| draw(app, frame)).expect("draw");
        buffer_text(&terminal)
    }

    fn app_with_keys(keys: &[crossterm::event::KeyCode]) -> App {
        let mut app = App::new(Box::new(Fake));
        for key in keys {
            app.handle_key(*key);
        }
        app
    }

    #[test]
    fn games_screen_renders_at_normal_size() {
        let app = app_with_keys(&[]);
        assert_eq!(app.screen(), Screen::Games);
        let text = draw_and_read(&app, 60, 16);
        assert!(text.contains("Hollow Knight"));
        assert!(text.contains("367520"));
    }

    #[test]
    fn games_screen_renders_at_tiny_size() {
        let app = app_with_keys(&[]);
        let text = draw_and_read(&app, 20, 6);
        // The title or game name is clipped, not panicked on.
        assert!(!text.contains("INTERNAL ERROR"));
    }

    #[test]
    fn entries_screen_renders_target_and_entries() {
        let app = app_with_keys(&[crossterm::event::KeyCode::Enter]);
        assert_eq!(app.screen(), Screen::Entries);
        let text = draw_and_read(&app, 60, 16);
        assert!(text.contains("hollow_knight.exe"));
        assert!(text.contains("pid 42"));
        assert!(text.contains("Infinite SOUL"));
        assert!(text.contains("[freeze]"));
    }

    #[test]
    fn plan_screen_renders_addresses_and_bytes() {
        let app = app_with_keys(&[
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyCode::Char('p'),
        ]);
        assert_eq!(app.screen(), Screen::Plan);
        let text = draw_and_read(&app, 60, 16);
        assert!(text.contains("dry run"));
        assert!(text.contains("0x1000"));
        assert!(text.contains("39 05 00 00"));
        assert!(text.contains("63 00 00 00"));
    }

    #[test]
    fn result_screen_renders_apply_status() {
        let app = app_with_keys(&[
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyCode::Char('p'),
            crossterm::event::KeyCode::Char('a'),
        ]);
        assert_eq!(app.screen(), Screen::Result);
        let text = draw_and_read(&app, 60, 16);
        assert!(text.contains("applied and verified"));
        assert!(text.contains("freeze: active"));
    }

    #[test]
    fn long_names_render_without_panicking() {
        let app = app_with_keys(&[crossterm::event::KeyCode::Enter]);
        let text = draw_and_read(&app, 24, 8);
        assert!(!text.contains("INTERNAL ERROR"));
    }
}
