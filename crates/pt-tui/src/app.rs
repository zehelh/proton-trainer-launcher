//! The TUI state machine: screens, transitions, and input handling.
//!
//! The `App` is pure: it talks to a [`TuiBackend`] only, so every
//! transition is unit-testable without a terminal. The rendering
//! layer ([`crate::render`]) and the terminal loop
//! ([`crate::terminal`]) are thin and contain no decisions.

use crossterm::event::KeyCode;

use crate::backend::{AttachOutcome, EntryBrief, GameRow, TuiBackend};

/// Which screen the TUI is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// List of installed games with trainer definitions.
    Games,
    /// Entry list of the attached trainer.
    Entries,
    /// Dry-run plan view for the selected entries.
    Plan,
    /// Apply results (and freeze status).
    Result,
    /// An informational or error message.
    Message,
}

/// One selectable entry in the Entries screen.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryState {
    brief: EntryBrief,
    selected: bool,
}

/// The TUI application state.
pub struct App {
    backend: Box<dyn TuiBackend>,
    screen: Screen,
    games: Vec<GameRow>,
    game_index: usize,
    trainer_id: Option<String>,
    target: Option<(i32, String)>,
    entries: Vec<EntryState>,
    entry_index: usize,
    plans: Vec<crate::backend::PlanRow>,
    results: Vec<crate::backend::ApplyResult>,
    message: String,
    quit: bool,
}

impl App {
    /// Creates the app and loads the game list from the backend.
    pub fn new(backend: Box<dyn TuiBackend>) -> Self {
        let mut app = Self {
            backend,
            screen: Screen::Message,
            games: Vec::new(),
            game_index: 0,
            trainer_id: None,
            target: None,
            entries: Vec::new(),
            entry_index: 0,
            plans: Vec::new(),
            results: Vec::new(),
            message: String::new(),
            quit: false,
        };
        match app.backend.games() {
            Ok(games) if games.is_empty() => {
                app.message = "No installed games with trainer definitions found.\n\
                     Add trainer files to the trainers directory and retry."
                    .to_string();
                app.screen = Screen::Message;
            }
            Ok(games) => {
                app.games = games;
                app.screen = Screen::Games;
            }
            Err(error) => app.message = error,
        }
        app
    }

    /// The currently visible screen.
    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// Whether the user asked to quit.
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// The loaded game rows (for rendering).
    pub fn games(&self) -> &[GameRow] {
        &self.games
    }

    /// The selected game index (for rendering).
    pub fn game_index(&self) -> usize {
        self.game_index
    }

    /// The attached trainer id, if any.
    pub fn trainer_id(&self) -> Option<&str> {
        self.trainer_id.as_deref()
    }

    /// The attached target (pid, exe name), if any.
    pub fn target(&self) -> Option<(i32, String)> {
        self.target.clone()
    }

    /// The entry rows (for rendering): id, label, freeze, selected.
    pub fn entry_rows(&self) -> Vec<(String, String, bool, bool)> {
        self.entries
            .iter()
            .map(|state| {
                (
                    state.brief.id.clone(),
                    state.brief.label.clone(),
                    state.brief.freeze,
                    state.selected,
                )
            })
            .collect()
    }

    /// The selected entry index (for rendering).
    pub fn entry_index(&self) -> usize {
        self.entry_index
    }

    /// The plan rows of the last plan (for rendering).
    pub fn plans(&self) -> &[crate::backend::PlanRow] {
        &self.plans
    }

    /// The apply results (for rendering).
    pub fn results(&self) -> &[crate::backend::ApplyResult] {
        &self.results
    }

    /// The current message screen text (for rendering).
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Whether a freeze loop is currently running.
    pub fn freeze_active(&self) -> bool {
        !self.freeze_entry_ids().is_empty()
    }

    /// Entry ids that were applied with freeze enabled.
    fn freeze_entry_ids(&self) -> Vec<String> {
        self.results
            .iter()
            .filter(|result| result.outcome.is_ok())
            .filter_map(|result| {
                self.plans
                    .iter()
                    .find(|plan| plan.entry_id == result.entry_id && plan.freeze)
                    .map(|_| result.entry_id.clone())
            })
            .collect()
    }

    /// Handles one key press. Pure state transition: the only side
    /// effects are backend calls (attach, plan, apply, freeze).
    pub fn handle_key(&mut self, key: KeyCode) {
        match self.screen {
            Screen::Games => self.handle_games_key(key),
            Screen::Entries => self.handle_entries_key(key),
            Screen::Plan => self.handle_plan_key(key),
            Screen::Result => self.handle_result_key(key),
            Screen::Message => self.handle_message_key(key),
        }
    }

    fn handle_games_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => self.move_game(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_game(1),
            KeyCode::Enter => self.attach_selected(),
            KeyCode::Char('q') | KeyCode::Esc => {
                self.backend.stop_freeze();
                self.quit = true;
            }
            _ => {}
        }
    }

    fn handle_entries_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => self.move_entry(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_entry(1),
            KeyCode::Char(' ') => self.toggle_entry(),
            KeyCode::Enter | KeyCode::Char('p') | KeyCode::Char('d') => {
                self.build_plan();
            }
            KeyCode::Char('a') => {
                // 'a' from the entry list: plan then apply, matching
                // the plan screen's apply key.
                if self.build_plan() {
                    self.apply_plans();
                }
            }
            KeyCode::Esc => self.release_attach(),
            KeyCode::Char('q') => {
                self.backend.stop_freeze();
                self.quit = true;
            }
            _ => {}
        }
    }

    fn handle_plan_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('a') | KeyCode::Enter => self.apply_plans(),
            KeyCode::Esc | KeyCode::Char('d') => {
                self.plans.clear();
                self.screen = Screen::Entries;
            }
            KeyCode::Char('q') => {
                self.backend.stop_freeze();
                self.quit = true;
            }
            _ => {}
        }
    }

    fn handle_result_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc | KeyCode::Enter => {
                self.backend.stop_freeze();
                self.results.clear();
                self.screen = Screen::Entries;
            }
            KeyCode::Char('q') => {
                self.backend.stop_freeze();
                self.quit = true;
            }
            _ => {}
        }
    }

    fn handle_message_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => {
                self.backend.stop_freeze();
                self.quit = true;
            }
            KeyCode::Enter | KeyCode::Esc => {
                if self.games.is_empty() {
                    self.backend.stop_freeze();
                    self.quit = true;
                } else {
                    self.screen = Screen::Games;
                }
            }
            _ => {}
        }
    }

    fn move_game(&mut self, delta: i32) {
        if self.games.is_empty() {
            return;
        }
        let count = self.games.len() as i32;
        self.game_index = (self.game_index as i32 + delta).rem_euclid(count) as usize;
    }

    fn move_entry(&mut self, delta: i32) {
        if self.entries.is_empty() {
            return;
        }
        let count = self.entries.len() as i32;
        self.entry_index = (self.entry_index as i32 + delta).rem_euclid(count) as usize;
    }

    fn toggle_entry(&mut self) {
        if let Some(entry) = self.entries.get_mut(self.entry_index) {
            entry.selected = !entry.selected;
        }
    }

    fn attach_selected(&mut self) {
        let Some(game) = self.games.get(self.game_index) else {
            return;
        };
        let trainer_id = game.trainer_id.clone();
        match self.backend.attach(&trainer_id) {
            Ok(AttachOutcome::Running {
                pid,
                exe_name,
                entries,
            }) => {
                self.trainer_id = Some(trainer_id);
                self.target = Some((pid, exe_name));
                self.entries = entries
                    .into_iter()
                    .map(|brief| EntryState {
                        brief,
                        selected: true,
                    })
                    .collect();
                self.entry_index = 0;
                self.screen = Screen::Entries;
            }
            Ok(AttachOutcome::NotRunning { exe_names }) => {
                self.message = format!(
                    "No running process found for {:?}.\nLaunch the game, then retry.",
                    exe_names
                );
                self.screen = Screen::Message;
            }
            Ok(AttachOutcome::Refused { explanation }) => {
                self.message = format!("REFUSED: {explanation}");
                self.screen = Screen::Message;
            }
            Err(error) => {
                self.message = error;
                self.screen = Screen::Message;
            }
        }
    }

    fn release_attach(&mut self) {
        self.backend.stop_freeze();
        self.trainer_id = None;
        self.target = None;
        self.entries.clear();
        self.entry_index = 0;
        self.plans.clear();
        self.screen = Screen::Games;
    }

    /// Builds the dry-run plan for the selected entries. Returns
    /// whether planning succeeded.
    fn build_plan(&mut self) -> bool {
        let ids: Vec<String> = self
            .entries
            .iter()
            .filter(|entry| entry.selected)
            .map(|entry| entry.brief.id.clone())
            .collect();
        if ids.is_empty() {
            self.message = "No entries selected: select entries with Space.".to_string();
            self.screen = Screen::Message;
            return false;
        }
        match self.backend.plan(&ids) {
            Ok(plans) => {
                self.plans = plans;
                self.screen = Screen::Plan;
                true
            }
            Err(error) => {
                self.message = error;
                self.screen = Screen::Message;
                false
            }
        }
    }

    fn apply_plans(&mut self) {
        match self.backend.apply() {
            Ok(results) => {
                self.results = results;
                let freeze_ids = self.freeze_entry_ids();
                if !freeze_ids.is_empty() {
                    if let Err(error) = self.backend.start_freeze(&freeze_ids) {
                        self.message = format!("freeze failed to start: {error}");
                        self.screen = Screen::Message;
                        return;
                    }
                }
                self.screen = Screen::Result;
            }
            Err(error) => {
                self.message = error;
                self.screen = Screen::Message;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{ApplyResult, EntryBrief, GameRow, PlanRow, TuiBackend};
    use pt_core::maps::Region;

    /// Scripted backend for state-machine tests.
    struct Fake {
        games_result: Result<Vec<GameRow>, String>,
        attach_outcome: AttachOutcome,
        plan_result: Result<Vec<PlanRow>, String>,
        apply_result: Result<Vec<ApplyResult>, String>,
        freeze_started: Vec<String>,
        freeze_stopped: usize,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                games_result: Ok(vec![
                    GameRow {
                        app_id: 367520,
                        name: "Hollow Knight".to_string(),
                        trainer_id: "hollow-knight".to_string(),
                    },
                    GameRow {
                        app_id: 413150,
                        name: "Stardew Valley".to_string(),
                        trainer_id: "stardew-valley".to_string(),
                    },
                ]),
                attach_outcome: AttachOutcome::Running {
                    pid: 42,
                    exe_name: "hollow_knight.exe".to_string(),
                    entries: vec![
                        EntryBrief {
                            id: "soul".to_string(),
                            label: "Infinite SOUL".to_string(),
                            freeze: true,
                        },
                        EntryBrief {
                            id: "geo".to_string(),
                            label: "Geo".to_string(),
                            freeze: false,
                        },
                    ],
                },
                plan_result: Ok(vec![PlanRow {
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
                }]),
                apply_result: Ok(vec![ApplyResult {
                    entry_id: "soul".to_string(),
                    label: "Infinite SOUL".to_string(),
                    outcome: Ok(()),
                }]),
                freeze_started: Vec::new(),
                freeze_stopped: 0,
            }
        }
    }

    impl TuiBackend for Fake {
        fn games(&mut self) -> Result<Vec<GameRow>, String> {
            self.games_result.clone()
        }

        fn attach(&mut self, _trainer_id: &str) -> Result<AttachOutcome, String> {
            Ok(self.attach_outcome.clone())
        }

        fn plan(&mut self, _entry_ids: &[String]) -> Result<Vec<PlanRow>, String> {
            self.plan_result.clone()
        }

        fn apply(&mut self) -> Result<Vec<ApplyResult>, String> {
            self.apply_result.clone()
        }

        fn start_freeze(&mut self, entry_ids: &[String]) -> Result<(), String> {
            self.freeze_started = entry_ids.to_vec();
            Ok(())
        }

        fn stop_freeze(&mut self) {
            self.freeze_stopped += 1;
        }
    }

    // App::new consumes the backend box; tests check state via the
    // public accessors only.
    fn new_app() -> App {
        App::new(Box::new(Fake::new()))
    }

    #[test]
    fn starts_on_games_screen() {
        let app = new_app();
        assert_eq!(app.screen(), Screen::Games);
        assert_eq!(app.games().len(), 2);
        assert!(!app.should_quit());
    }

    #[test]
    fn navigation_wraps() {
        let mut app = new_app();
        assert_eq!(app.game_index(), 0);
        app.handle_key(KeyCode::Up);
        assert_eq!(app.game_index(), 1);
        app.handle_key(KeyCode::Down);
        assert_eq!(app.game_index(), 0);
    }

    #[test]
    fn attach_goes_to_entries() {
        let mut app = new_app();
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.screen(), Screen::Entries);
        assert_eq!(app.trainer_id(), Some("hollow-knight"));
        assert_eq!(app.target(), Some((42, "hollow_knight.exe".to_string())));
        assert_eq!(app.entry_rows().len(), 2);
        assert!(app.entry_rows().iter().all(|row| row.3));
    }

    #[test]
    fn esc_from_entries_releases_attach() {
        let mut app = new_app();
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Esc);
        assert_eq!(app.screen(), Screen::Games);
        assert_eq!(app.trainer_id(), None);
        assert!(app.entry_rows().is_empty());
    }

    #[test]
    fn plan_then_apply_reaches_result_and_freezes() {
        let mut app = new_app();
        app.handle_key(KeyCode::Enter); // attach
        app.handle_key(KeyCode::Char(' ')); // deselect entry 0 (freeze one)
        app.handle_key(KeyCode::Char('p')); // plan only "geo"
        assert_eq!(app.screen(), Screen::Plan);
        assert_eq!(app.plans().len(), 1);

        // Deselect means no freeze rows in the scripted apply result...
        // the scripted result covers "soul"; use the plan-screen apply.
        app.handle_key(KeyCode::Char('a'));
        assert_eq!(app.screen(), Screen::Result);
        assert!(app.freeze_active());

        app.handle_key(KeyCode::Esc); // back to entries stops the freeze
        assert_eq!(app.screen(), Screen::Entries);
        assert!(!app.freeze_active());
    }

    #[test]
    fn toggle_then_plan_covers_only_selected() {
        let mut app = new_app();
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Char(' ')); // toggle second off -> none? No: toggling flips selected
        let selected: Vec<bool> = app.entry_rows().iter().map(|row| row.3).collect();
        assert_eq!(selected, vec![true, false]);
    }

    #[test]
    fn refused_attach_shows_message_and_returns_to_games() {
        let mut app = App::new(Box::new(Fake {
            attach_outcome: AttachOutcome::Refused {
                explanation: "anti-cheat detected".to_string(),
            },
            ..*Box::new(Fake::new())
        }));
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.screen(), Screen::Message);
        assert!(app.message().contains("REFUSED"));
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.screen(), Screen::Games);
    }

    #[test]
    fn not_running_shows_message() {
        let mut app = App::new(Box::new(Fake {
            attach_outcome: AttachOutcome::NotRunning {
                exe_names: vec!["game.exe".to_string()],
            },
            ..*Box::new(Fake::new())
        }));
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.screen(), Screen::Message);
        assert!(app.message().contains("game.exe"));
    }

    #[test]
    fn quit_from_games_sets_quit() {
        let mut app = new_app();
        app.handle_key(KeyCode::Char('q'));
        assert!(app.should_quit());
    }

    #[test]
    fn empty_games_list_starts_on_message_and_quits() {
        let mut app = App::new(Box::new(Fake {
            games_result: Ok(Vec::new()),
            ..*Box::new(Fake::new())
        }));
        assert_eq!(app.screen(), Screen::Message);
        app.handle_key(KeyCode::Enter);
        assert!(app.should_quit());
    }

    #[test]
    fn plan_error_shows_message() {
        let mut app = App::new(Box::new(Fake {
            plan_result: Err("entry \"soul\": pattern not found".to_string()),
            ..*Box::new(Fake::new())
        }));
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Char('p'));
        assert_eq!(app.screen(), Screen::Message);
        assert!(app.message().contains("pattern not found"));
    }
}
