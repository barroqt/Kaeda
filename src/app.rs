use crate::dictionary::db::{lookup, DictEntry};
use crate::filter::filter_content_tokens;
use crate::parser::srt::Subtitle;
use crate::tokenizer::korean::Token;
use crate::tokenizer::tokenize;
use crate::ui::build_layout;
use crate::ui::candidate_pane::render_candidate_pane;
use crate::ui::definition_pane::render_definition_pane;
use crate::ui::render_status_bar;
use crate::ui::source_pane::render_source_pane;
use anyhow::Context;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::Frame;
use rusqlite::Connection;
use std::time::Duration;

use crate::filter::FilterConfig;
use crate::store::{add_to_deck, init_store, mark_known, DeckEntry};

#[derive(Debug, PartialEq)]
pub enum Action {
    None,
    AddToDeck,
    MarkKnown,
    Skip,
    Quit,
}

pub struct AppState {
    pub subtitles: Vec<Subtitle>,
    pub subtitle_cursor: usize,
    pub candidates: Vec<Token>,
    pub candidate_cursor: usize,
    pub active_pane: Pane,
    pub source_file: String,
    pub deck_count: usize,
    pub current_definition: Option<DictEntry>,
}

pub enum Pane {
    Source,
    Candidates,
    Definition,
}

impl AppState {
    pub fn draw(&self, f: &mut Frame) {
        let (src, cand, def, status) = build_layout(f.area());
        render_source_pane(f, src, self);
        render_candidate_pane(f, cand, self);
        render_definition_pane(f, def, self.current_definition.as_ref());
        render_status_bar(f, status, self);
    }

    pub fn current_subtitle(&self) -> Option<&Subtitle> {
        self.subtitles.get(self.subtitle_cursor)
    }

    pub fn selected_candidate(&self) -> Option<&Token> {
        self.candidates.get(self.candidate_cursor)
    }

    pub fn next_candidate(&mut self) {
        let next = self.candidate_cursor.saturating_add(1);
        if next < self.candidates.len() {
            self.candidate_cursor = next;
        }
    }

    pub fn prev_candidate(&mut self) {
        self.candidate_cursor = self.candidate_cursor.saturating_sub(1);
    }

    pub fn next_subtitle(&mut self) {
        let next = self.subtitle_cursor.saturating_add(1);
        if next < self.subtitles.len() {
            self.subtitle_cursor = next;
            self.recompute_candidates();
        }
    }

    pub fn prev_subtitle(&mut self) {
        if self.subtitle_cursor > 0 {
            self.subtitle_cursor -= 1;
            self.recompute_candidates();
        }
    }

    pub fn switch_pane(&mut self) {
        self.active_pane = match self.active_pane {
            Pane::Source => Pane::Candidates,
            Pane::Candidates => Pane::Definition,
            Pane::Definition => Pane::Source,
        };
    }

    fn recompute_candidates(&mut self) {
        self.candidates = self
            .subtitles
            .get(self.subtitle_cursor)
            .map(|s| tokenize(&s.text).map(filter_content_tokens).unwrap_or_default())
            .unwrap_or_default();
        self.candidate_cursor = 0;
    }

    pub fn update_definition(&mut self, conn: &Connection) {
        self.current_definition = self
            .selected_candidate()
            .and_then(|t| lookup(conn, &t.lemma).ok().flatten());
    }

    pub fn new(subtitles: Vec<Subtitle>, source_file: String) -> Self {
        let mut state = AppState {
            subtitles,
            subtitle_cursor: 0,
            candidates: Vec::new(),
            candidate_cursor: 0,
            active_pane: Pane::Candidates,
            source_file,
            deck_count: 0,
            current_definition: None,
        };
        state.recompute_candidates();
        state
    }
}

pub fn handle_key(state: &mut AppState, key: KeyCode) -> Action {
    match key {
        KeyCode::Up => {
            state.prev_candidate();
            Action::None
        }
        KeyCode::Down => {
            state.next_candidate();
            Action::None
        }
        KeyCode::Left => {
            state.prev_subtitle();
            Action::None
        }
        KeyCode::Right => {
            state.next_subtitle();
            Action::None
        }
        KeyCode::Tab => {
            state.switch_pane();
            Action::None
        }
        KeyCode::Char('a') => Action::AddToDeck,
        KeyCode::Char('k') => Action::MarkKnown,
        KeyCode::Char('s') => Action::Skip,
        KeyCode::Char('q') => Action::Quit,
        _ => Action::None,
    }
}

pub fn run(
    state: &mut AppState,
    conn: &Connection,
    config: &FilterConfig,
) -> anyhow::Result<()> {
    init_store(conn).context("failed to init store")?;

    state.update_definition(conn);

    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))
            .context("failed to create terminal")?;

    crossterm::terminal::enable_raw_mode().context("failed to enable raw mode")?;

    let res = loop {
        terminal.draw(|f| state.draw(f))?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        let action = match event::read()? {
            Event::Key(ke) => handle_key(state, ke.code),
            _ => Action::None,
        };

        match action {
            Action::None => {}
            Action::AddToDeck => {
                if let Some(token) = state.selected_candidate() {
                    let meaning = state
                        .current_definition
                        .as_ref()
                        .map(|d| d.meaning.clone())
                        .unwrap_or_default();
                    let source = state
                        .current_subtitle()
                        .map(|s| s.text.clone())
                        .unwrap_or_default();
                    let entry = DeckEntry {
                        lemma: token.lemma.clone(),
                        surface: token.surface.clone(),
                        meaning,
                        source_sentence: source,
                        source_file: state.source_file.clone(),
                    };
                    add_to_deck(conn, &entry)?;
                    state.deck_count = state.deck_count.saturating_add(1);
                }
                state.next_subtitle();
            }
            Action::MarkKnown => {
                if let Some(token) = state.selected_candidate() {
                    mark_known(conn, &token.lemma)?;
                }
                state.next_subtitle();
            }
            Action::Skip => {
                state.next_subtitle();
            }
            Action::Quit => break Ok(()),
        }

        state.update_definition(conn);
    };

    crossterm::terminal::disable_raw_mode().context("failed to disable raw mode")?;

    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_subtitle_returns_first() {
        let subtitles = vec![
            Subtitle {
                index: 1,
                timestamp: "00:00:01,000 --> 00:00:02,000".to_string(),
                text: "안녕하세요".to_string(),
            },
            Subtitle {
                index: 2,
                timestamp: "00:00:03,000 --> 00:00:04,000".to_string(),
                text: "반갑습니다".to_string(),
            },
        ];
        let state = AppState::new(subtitles, "test.srt".to_string());
        let sub = state.current_subtitle().unwrap();
        assert_eq!(sub.index, 1);
        assert_eq!(sub.text, "안녕하세요");
    }

    #[test]
    fn selected_candidate_returns_first_token() {
        let subtitles = vec![Subtitle {
            index: 1,
            timestamp: "00:00:01,000 --> 00:00:02,000".to_string(),
            text: "책을 읽습니다".to_string(),
        }];
        let state = AppState::new(subtitles, "test.srt".to_string());
        let token = state.selected_candidate().unwrap();
        assert_eq!(token.surface, "책");
    }

    #[test]
    fn next_candidate_increments_cursor() {
        let subtitles = vec![Subtitle {
            index: 1,
            timestamp: "00:00:01,000 --> 00:00:02,000".to_string(),
            text: "책을 읽습니다".to_string(),
        }];
        let mut state = AppState::new(subtitles, "test.srt".to_string());
        assert_eq!(state.candidate_cursor, 0);
        state.next_candidate();
        assert_eq!(state.candidate_cursor, 1);
    }

    #[test]
    fn prev_candidate_at_zero_stays_zero() {
        let subtitles = vec![Subtitle {
            index: 1,
            timestamp: "00:00:01,000 --> 00:00:02,000".to_string(),
            text: "책을 읽습니다".to_string(),
        }];
        let mut state = AppState::new(subtitles, "test.srt".to_string());
        assert_eq!(state.candidate_cursor, 0);
        state.prev_candidate();
        assert_eq!(state.candidate_cursor, 0);
    }

    #[test]
    fn next_subtitle_resets_candidate_cursor() {
        let subtitles = vec![
            Subtitle {
                index: 1,
                timestamp: "00:00:01,000 --> 00:00:02,000".to_string(),
                text: "첫 번째 문장".to_string(),
            },
            Subtitle {
                index: 2,
                timestamp: "00:00:03,000 --> 00:00:04,000".to_string(),
                text: "두 번째 문장".to_string(),
            },
        ];
        let mut state = AppState::new(subtitles, "test.srt".to_string());
        state.next_candidate();
        assert_eq!(state.candidate_cursor, 1);
        state.next_subtitle();
        assert_eq!(state.subtitle_cursor, 1);
        assert_eq!(state.candidate_cursor, 0);
    }

    #[test]
    fn prev_subtitle_at_zero_stays_zero() {
        let subtitles = vec![Subtitle {
            index: 1,
            timestamp: "00:00:01,000 --> 00:00:02,000".to_string(),
            text: "첫 번째 문장".to_string(),
        }];
        let mut state = AppState::new(subtitles, "test.srt".to_string());
        state.prev_subtitle();
        assert_eq!(state.subtitle_cursor, 0);
    }

    #[test]
    fn switch_pane_cycles_correctly() {
        let subtitles = vec![Subtitle {
            index: 1,
            timestamp: "00:00:01,000 --> 00:00:02,000".to_string(),
            text: "안녕하세요".to_string(),
        }];
        let mut state = AppState::new(subtitles, "test.srt".to_string());
        assert!(matches!(state.active_pane, Pane::Candidates));
        state.switch_pane();
        assert!(matches!(state.active_pane, Pane::Definition));
        state.switch_pane();
        assert!(matches!(state.active_pane, Pane::Source));
        state.switch_pane();
        assert!(matches!(state.active_pane, Pane::Candidates));
    }

    #[test]
    fn app_renders_without_panic() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let subtitles = vec![
            Subtitle {
                index: 1,
                timestamp: "00:00:01,000 --> 00:00:02,000".to_string(),
                text: "안녕하세요".to_string(),
            },
            Subtitle {
                index: 2,
                timestamp: "00:00:03,000 --> 00:00:04,000".to_string(),
                text: "반갑습니다".to_string(),
            },
        ];
        let state = AppState::new(subtitles, "test.srt".to_string());
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| state.draw(f)).unwrap();
    }

    #[test]
    fn key_up_moves_candidate() {
        let subtitles = vec![Subtitle {
            index: 1,
            timestamp: "".to_string(),
            text: "책을 읽습니다".to_string(),
        }];
        let mut state = AppState::new(subtitles, "t.srt".to_string());
        state.next_candidate();
        assert_eq!(state.candidate_cursor, 1);
        assert_eq!(handle_key(&mut state, KeyCode::Up), Action::None);
        assert_eq!(state.candidate_cursor, 0);
    }

    #[test]
    fn key_down_moves_candidate() {
        let subtitles = vec![Subtitle {
            index: 1,
            timestamp: "".to_string(),
            text: "책을 읽습니다".to_string(),
        }];
        let mut state = AppState::new(subtitles, "t.srt".to_string());
        assert_eq!(state.candidate_cursor, 0);
        assert_eq!(handle_key(&mut state, KeyCode::Down), Action::None);
        assert_eq!(state.candidate_cursor, 1);
    }

    #[test]
    fn key_left_moves_subtitle() {
        let subtitles = vec![Subtitle {
            index: 1,
            timestamp: "".to_string(),
            text: "안녕".to_string(),
        }];
        let mut state = AppState::new(subtitles, "t.srt".to_string());
        assert_eq!(state.subtitle_cursor, 0);
        assert_eq!(handle_key(&mut state, KeyCode::Left), Action::None);
        assert_eq!(state.subtitle_cursor, 0);
    }

    #[test]
    fn key_right_moves_subtitle() {
        let subtitles = vec![
            Subtitle {
                index: 1,
                timestamp: "".to_string(),
                text: "안녕".to_string(),
            },
            Subtitle {
                index: 2,
                timestamp: "".to_string(),
                text: "반가워".to_string(),
            },
        ];
        let mut state = AppState::new(subtitles, "t.srt".to_string());
        assert_eq!(state.subtitle_cursor, 0);
        assert_eq!(handle_key(&mut state, KeyCode::Right), Action::None);
        assert_eq!(state.subtitle_cursor, 1);
    }

    #[test]
    fn key_tab_switches_pane() {
        let subtitles = vec![Subtitle {
            index: 1,
            timestamp: "".to_string(),
            text: "안녕".to_string(),
        }];
        let mut state = AppState::new(subtitles, "t.srt".to_string());
        assert!(matches!(state.active_pane, Pane::Candidates));
        assert_eq!(handle_key(&mut state, KeyCode::Tab), Action::None);
        assert!(matches!(state.active_pane, Pane::Definition));
    }

    #[test]
    fn key_a_returns_add_to_deck() {
        let mut state = AppState::new(vec![], "t.srt".to_string());
        assert!(matches!(handle_key(&mut state, KeyCode::Char('a')), Action::AddToDeck));
    }

    #[test]
    fn key_k_returns_mark_known() {
        let mut state = AppState::new(vec![], "t.srt".to_string());
        assert!(matches!(handle_key(&mut state, KeyCode::Char('k')), Action::MarkKnown));
    }

    #[test]
    fn key_s_returns_skip() {
        let mut state = AppState::new(vec![], "t.srt".to_string());
        assert!(matches!(handle_key(&mut state, KeyCode::Char('s')), Action::Skip));
    }

    #[test]
    fn key_q_returns_quit() {
        let mut state = AppState::new(vec![], "t.srt".to_string());
        assert!(matches!(handle_key(&mut state, KeyCode::Char('q')), Action::Quit));
    }

    #[test]
    fn key_unknown_returns_none() {
        let mut state = AppState::new(vec![], "t.srt".to_string());
        assert!(matches!(handle_key(&mut state, KeyCode::Char('z')), Action::None));
    }
}
