use std::path::Path;
use std::sync::Mutex;

use kaeda_core::session::{Card, Session};
use kaeda_core::subtitle::{SubtitleEntry, entries_from_srt};
use kaeda_core::tokenizer::KoreanTokenizer;

mod dto;
use dto::{CardDto, SubtitleDto, TokenDto};

struct MiningSessionInner {
    session: Option<Session>,
    subtitles: Vec<SubtitleEntry>,
    subtitle_tokens: Vec<Vec<TokenDto>>,
    current_index: usize,
}

pub struct MiningSessionState {
    inner: Mutex<MiningSessionInner>,
}

impl Default for MiningSessionState {
    fn default() -> Self {
        Self::new()
    }
}

impl MiningSessionState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MiningSessionInner {
                session: None,
                subtitles: Vec::new(),
                subtitle_tokens: Vec::new(),
                current_index: 0,
            }),
        }
    }

    pub fn start_session(
        &self,
        srt_path: &Path,
        deck_name: String,
        source_file_id: String,
    ) -> Result<(), String> {
        let subtitles = entries_from_srt(srt_path).map_err(|e| e.to_string())?;
        if subtitles.is_empty() {
            return Err("no subtitles found in file".to_string());
        }
        let tokenizer = KoreanTokenizer::new().map_err(|e| e.to_string())?;
        let subtitle_tokens: Vec<Vec<TokenDto>> = subtitles
            .iter()
            .map(|sub| {
                tokenizer
                    .tokenize(&sub.text)
                    .map(|tokens| tokens.iter().map(TokenDto::from).collect())
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        let session = Session::new(deck_name, source_file_id);
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;
        inner.session = Some(session);
        inner.subtitles = subtitles;
        inner.subtitle_tokens = subtitle_tokens;
        inner.current_index = 0;
        Ok(())
    }

    pub fn subtitles(&self) -> Result<Vec<SubtitleDto>, String> {
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        Ok(inner
            .subtitles
            .iter()
            .zip(inner.subtitle_tokens.iter())
            .map(|(entry, tokens)| SubtitleDto {
                id: entry.id,
                start_time: entry.start_time.clone(),
                end_time: entry.end_time.clone(),
                text: entry.text.clone(),
                is_known: false,
                tokens: tokens.clone(),
            })
            .collect())
    }

    pub fn current_index(&self) -> Result<usize, String> {
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        Ok(inner.current_index)
    }

    pub fn next_subtitle(&self) -> Result<usize, String> {
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;
        if inner.session.is_none() {
            return Err("no active session".to_string());
        }
        if inner.current_index + 1 < inner.subtitles.len() {
            inner.current_index += 1;
        }
        Ok(inner.current_index)
    }

    pub fn previous_subtitle(&self) -> Result<usize, String> {
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;
        if inner.session.is_none() {
            return Err("no active session".to_string());
        }
        if inner.current_index > 0 {
            inner.current_index -= 1;
        }
        Ok(inner.current_index)
    }

    pub fn set_current_index(&self, index: usize) -> Result<usize, String> {
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;
        if inner.session.is_none() {
            return Err("no active session".to_string());
        }
        if index >= inner.subtitles.len() {
            return Err(format!(
                "index {} out of range (0..{})",
                index,
                inner.subtitles.len()
            ));
        }
        inner.current_index = index;
        Ok(inner.current_index)
    }

    pub fn save_card(&self, target: String, explanation: String) -> Result<CardDto, String> {
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;
        if inner.session.is_none() {
            return Err("no active session".to_string());
        }
        let (sentence, subtitle_id) = {
            let entry = inner
                .subtitles
                .get(inner.current_index)
                .ok_or("no current subtitle")?;
            (entry.text.clone(), entry.id)
        };

        let session = inner.session.as_mut().ok_or("no active session")?;

        let card = Card {
            sentence,
            target,
            explanation,
            deck: session.deck_name.clone(),
            tags: vec![],
            file_id: session.source_file_id.clone(),
            subtitle_id,
        };
        let dto = CardDto::from(card.clone());
        session.add_card(card);
        Ok(dto)
    }

    pub fn session_cards(&self) -> Result<Vec<CardDto>, String> {
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        let session = inner.session.as_ref().ok_or("no active session")?;
        Ok(session.cards().iter().cloned().map(CardDto::from).collect())
    }

    pub fn export_session(&self, path: &Path) -> Result<(), String> {
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        let session = inner.session.as_ref().ok_or("no active session")?;
        session.export_tsv(path).map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn start_session(
    state: tauri::State<'_, MiningSessionState>,
    video_path: String,
    srt_path: String,
    deck_name: String,
) -> Result<(), String> {
    let source_file_id = Path::new(&video_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    state.start_session(Path::new(&srt_path), deck_name, source_file_id)
}

#[tauri::command]
fn get_subtitles(state: tauri::State<'_, MiningSessionState>) -> Result<Vec<SubtitleDto>, String> {
    state.subtitles()
}

#[tauri::command]
fn get_current_index(state: tauri::State<'_, MiningSessionState>) -> Result<usize, String> {
    state.current_index()
}

#[tauri::command]
fn next_subtitle(state: tauri::State<'_, MiningSessionState>) -> Result<usize, String> {
    state.next_subtitle()
}

#[tauri::command]
fn previous_subtitle(state: tauri::State<'_, MiningSessionState>) -> Result<usize, String> {
    state.previous_subtitle()
}

#[tauri::command]
fn set_current_index(
    state: tauri::State<'_, MiningSessionState>,
    index: usize,
) -> Result<usize, String> {
    state.set_current_index(index)
}

#[tauri::command]
fn save_card(
    state: tauri::State<'_, MiningSessionState>,
    target: String,
    explanation: String,
) -> Result<CardDto, String> {
    state.save_card(target, explanation)
}

#[tauri::command]
fn get_session_cards(state: tauri::State<'_, MiningSessionState>) -> Result<Vec<CardDto>, String> {
    state.session_cards()
}

#[tauri::command]
fn export_session(state: tauri::State<'_, MiningSessionState>, path: String) -> Result<(), String> {
    state.export_session(Path::new(&path))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(MiningSessionState::new())
        .invoke_handler(tauri::generate_handler![
            start_session,
            get_subtitles,
            get_current_index,
            next_subtitle,
            previous_subtitle,
            set_current_index,
            save_card,
            get_session_cards,
            export_session,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|err| {
            eprintln!("error: {err}");
            std::process::exit(1);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn session_starts_with_parsed_subtitles() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "test-deck".into(), "video-1".into())
            .unwrap();

        let subtitles = state.subtitles().unwrap();
        assert_eq!(subtitles.len(), 5);
        assert_eq!(subtitles[0].text, "안녕하세요 반갑습니다.");
        assert_eq!(subtitles[0].id, 1);
    }

    #[test]
    fn current_index_initialized_to_zero() {
        let state = MiningSessionState::new();
        assert_eq!(state.current_index().unwrap(), 0);

        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "test-deck".into(), "video-1".into())
            .unwrap();

        assert_eq!(state.current_index().unwrap(), 0);
    }

    #[test]
    fn start_session_returns_error_on_missing_file() {
        let state = MiningSessionState::new();
        let result = state.start_session(
            Path::new("/nonexistent/file.srt"),
            "deck".into(),
            "file".into(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn start_session_returns_error_on_empty_file() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("empty.srt");
        let result = state.start_session(&srt_path, "deck".into(), "file".into());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "no subtitles found in file");
    }

    #[test]
    fn subtitles_are_empty_before_session_starts() {
        let state = MiningSessionState::new();
        let subtitles = state.subtitles().unwrap();
        assert!(subtitles.is_empty());
    }

    #[test]
    fn next_subtitle_increments_index() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        assert_eq!(state.next_subtitle().unwrap(), 1);
        assert_eq!(state.current_index().unwrap(), 1);
    }

    #[test]
    fn next_subtitle_clamps_at_last_entry() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        for _ in 0..10 {
            state.next_subtitle().unwrap();
        }
        assert_eq!(state.current_index().unwrap(), 4);
    }

    #[test]
    fn previous_subtitle_decrements_index() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        state.next_subtitle().unwrap();
        state.next_subtitle().unwrap();
        assert_eq!(state.current_index().unwrap(), 2);

        assert_eq!(state.previous_subtitle().unwrap(), 1);
    }

    #[test]
    fn previous_subtitle_clamps_at_zero() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        assert_eq!(state.previous_subtitle().unwrap(), 0);
        assert_eq!(state.previous_subtitle().unwrap(), 0);
    }

    #[test]
    fn navigation_returns_error_without_session() {
        let state = MiningSessionState::new();
        assert!(state.next_subtitle().is_err());
        assert!(state.previous_subtitle().is_err());
    }

    #[test]
    fn set_current_index_jumps_to_target() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        assert_eq!(state.set_current_index(3).unwrap(), 3);
        assert_eq!(state.current_index().unwrap(), 3);
    }

    #[test]
    fn set_current_index_rejects_out_of_range() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        let result = state.set_current_index(100);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of range"));
    }

    #[test]
    fn set_current_index_rejects_without_session() {
        let state = MiningSessionState::new();
        assert!(state.set_current_index(0).is_err());
    }

    #[test]
    fn save_card_appends_to_session() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "test-deck".into(), "video-1".into())
            .unwrap();

        let card = state.save_card("안녕".into(), "Hello".into()).unwrap();
        assert_eq!(card.sentence, "안녕하세요 반갑습니다.");
        assert_eq!(card.target, "안녕");
        assert_eq!(card.explanation, "Hello");
        assert_eq!(card.deck, "test-deck");

        let inner = state.inner.lock().unwrap();
        let cards = inner.session.as_ref().unwrap().cards();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].target, "안녕");
        assert_eq!(cards[0].explanation, "Hello");
        assert_eq!(cards[0].subtitle_id, 1);
    }

    #[test]
    fn save_card_returns_error_without_session() {
        let state = MiningSessionState::new();
        let result = state.save_card("test".into(), "explanation".into());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "no active session");
    }

    #[test]
    fn save_card_multiple_cards() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        state.save_card("target1".into(), "exp1".into()).unwrap();
        state.set_current_index(1).unwrap();
        state.save_card("target2".into(), "exp2".into()).unwrap();

        let inner = state.inner.lock().unwrap();
        assert_eq!(inner.session.as_ref().unwrap().card_count(), 2);
        assert_eq!(
            inner.session.as_ref().unwrap().cards()[0].sentence,
            "안녕하세요 반갑습니다."
        );
        assert_eq!(
            inner.session.as_ref().unwrap().cards()[1].sentence,
            "오늘은 날씨가 좋네요."
        );
    }

    #[test]
    fn get_session_cards_returns_empty_list_before_session() {
        let state = MiningSessionState::new();
        assert!(state.session_cards().is_err());
    }

    #[test]
    fn get_session_cards_returns_empty_list_in_active_session() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        let cards = state.session_cards().unwrap();
        assert!(cards.is_empty());
    }

    #[test]
    fn get_session_cards_returns_cards_after_save() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "test-deck".into(), "video-1".into())
            .unwrap();

        state.save_card("안녕".into(), "Hello".into()).unwrap();

        let cards = state.session_cards().unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].target, "안녕");
        assert_eq!(cards[0].explanation, "Hello");
        assert_eq!(cards[0].sentence, "안녕하세요 반갑습니다.");
        assert_eq!(cards[0].deck, "test-deck");
    }

    #[test]
    fn export_session_returns_error_without_session() {
        let state = MiningSessionState::new();
        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_export_no_session.tsv");
        let result = state.export_session(&path);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "no active session");
    }

    #[test]
    fn export_session_writes_tsv_file() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "test-deck".into(), "video-1".into())
            .unwrap();

        state.save_card("안녕".into(), "Hello".into()).unwrap();
        state.set_current_index(1).unwrap();
        state.save_card("날씨".into(), "Weather".into()).unwrap();

        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_export_test.tsv");
        state.export_session(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let expected =
            "안녕\t안녕하세요 반갑습니다.\tHello\n날씨\t오늘은 날씨가 좋네요.\tWeather\n";
        assert_eq!(content, expected);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn get_session_cards_reflects_multiple_saves() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        state.save_card("target1".into(), "exp1".into()).unwrap();
        state.set_current_index(1).unwrap();
        state.save_card("target2".into(), "exp2".into()).unwrap();
        state.set_current_index(2).unwrap();
        state.save_card("target3".into(), "exp3".into()).unwrap();

        let cards = state.session_cards().unwrap();
        assert_eq!(cards.len(), 3);
        assert_eq!(cards[0].target, "target1");
        assert_eq!(cards[1].target, "target2");
        assert_eq!(cards[2].target, "target3");
        assert_eq!(cards[0].sentence, "안녕하세요 반갑습니다.");
        assert_eq!(cards[1].sentence, "오늘은 날씨가 좋네요.");
        assert_eq!(cards[2].sentence, "저는 공부를 하고 있어요.");
    }

    #[test]
    fn each_subtitle_has_tokens_after_session_start() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        let subtitles = state.subtitles().unwrap();
        assert_eq!(subtitles.len(), 5);
        for (i, sub) in subtitles.iter().enumerate() {
            assert!(
                !sub.tokens.is_empty(),
                "subtitle {} \"{}\" has no tokens",
                i,
                sub.text
            );
        }
    }

    #[test]
    fn token_byte_positions_map_to_subtitle_text() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        let subtitles = state.subtitles().unwrap();
        for sub in &subtitles {
            for token in &sub.tokens {
                let slice = &sub.text[token.byte_start..token.byte_end];
                assert_eq!(
                    slice, token.surface,
                    "byte range {}..{} does not match surface '{}' in text \"{}\"",
                    token.byte_start, token.byte_end, token.surface, sub.text
                );
            }
        }
    }

    #[test]
    fn token_surfaces_are_contiguous_in_byte_order() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        let subtitles = state.subtitles().unwrap();
        for sub in &subtitles {
            let mut prev_end: Option<usize> = None;
            for token in &sub.tokens {
                if let Some(end) = prev_end {
                    assert!(
                        token.byte_start >= end,
                        "token '{}' start {} < previous end {} in \"{}\"",
                        token.surface,
                        token.byte_start,
                        end,
                        sub.text
                    );
                }
                prev_end = Some(token.byte_end);
            }
        }
    }

    #[test]
    fn every_token_has_lemma_and_pos() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(&srt_path, "deck".into(), "file".into())
            .unwrap();

        let subtitles = state.subtitles().unwrap();
        for sub in &subtitles {
            for token in &sub.tokens {
                assert!(!token.lemma.is_empty(), "empty lemma in \"{}\"", sub.text);
                assert!(!token.pos.is_empty(), "empty POS in \"{}\"", sub.text);
            }
        }
    }
}
