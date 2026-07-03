use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kaeda_core::deck::DeckId;
use kaeda_core::dictionary;
use kaeda_core::session::{Card, Session};
use kaeda_core::store::{self, KnownLinesStore};
use kaeda_core::subtitle::{
    SubtitleEntry, SubtitleSource, build_translation_span, prepare_session_subtitles,
    srt_timestamp_to_ms,
};
use kaeda_core::tokenizer::KoreanTokenizer;
use rusqlite::Connection;
use tauri::Emitter;
use tauri::Manager;
use tracing::{debug, error, info, warn};

mod dto;
mod translation;
mod video_server;
use dto::{CardDto, DeckDto, SubtitleDto, SubtitleSearchResultDto, TokenDto};
use translation::{AppError, AppSettings, TranslationProvider, TranslationSettings};

#[derive(Clone, serde::Serialize)]
struct TranslationResult {
    lemma: String,
    translation: Option<String>,
}

struct TranslationManager {
    cache: Mutex<HashMap<String, String>>,
}

impl TranslationManager {
    fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn get_cached(&self, lemma: &str) -> Option<String> {
        let cache = self.cache.lock().ok()?;
        cache.get(lemma).cloned()
    }

    fn insert_cache(&self, lemma: String, translation: String) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(lemma, translation);
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct SessionStartError {
    pub code: String,
    pub message: String,
}

struct MiningSessionInner {
    session: Option<Session>,
    subtitles: Vec<SubtitleEntry>,
    subtitle_tokens: Vec<Vec<TokenDto>>,
    current_index: usize,
    known_store: Option<KnownLinesStore>,
    known_ids: HashSet<i64>,
    source_file_id: String,
    video_path: String,
    card_store: Option<Connection>,
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
                known_store: None,
                known_ids: HashSet::new(),
                source_file_id: String::new(),
                video_path: String::new(),
                card_store: None,
            }),
        }
    }

    /// Shared setup: tokenize, load known lines, create session, write state.
    /// Called by all session-start paths after subtitles are obtained.
    fn init_with_subtitles(
        &self,
        subtitles: Vec<SubtitleEntry>,
        deck_id: DeckId,
        source_file_id: String,
        known_store: KnownLinesStore,
        video_path: String,
        card_store: Connection,
    ) -> Result<(), String> {
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
        let known_ids = known_store
            .known_ids(&source_file_id)
            .map_err(|e| e.to_string())?;
        let session = Session::new(deck_id, source_file_id.clone());
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;
        inner.session = Some(session);
        inner.subtitles = subtitles;
        inner.subtitle_tokens = subtitle_tokens;
        inner.current_index = 0;
        inner.known_store = Some(known_store);
        inner.known_ids = known_ids;
        inner.source_file_id = source_file_id;
        inner.video_path = video_path;
        inner.card_store = Some(card_store);
        Ok(())
    }

    pub fn start_session(
        &self,
        srt_path: &Path,
        deck_name: String,
        source_file_id: String,
        known_store: KnownLinesStore,
        video_path: String,
    ) -> Result<(), String> {
        let source = SubtitleSource::ExternalSrt {
            srt_path: srt_path.to_path_buf(),
            video_path: Some(PathBuf::from(&video_path)),
        };
        let subtitles = prepare_session_subtitles(source).map_err(|e| e.to_string())?;
        let card_store = Connection::open_in_memory().map_err(|e| e.to_string())?;
        store::init_store(&card_store).map_err(|e| e.to_string())?;
        let deck_id = store::get_or_create_deck_by_name(&card_store, &deck_name)
            .map_err(|e| e.to_string())?;
        self.init_with_subtitles(
            subtitles,
            deck_id,
            source_file_id,
            known_store,
            video_path,
            card_store,
        )
    }

    pub fn start_embedded_session(
        &self,
        deck_name: String,
        source_file_id: String,
        known_store: KnownLinesStore,
        video_path: String,
    ) -> Result<(), String> {
        let source = SubtitleSource::Embedded {
            video_path: PathBuf::from(&video_path),
        };
        let subtitles = prepare_session_subtitles(source).map_err(|e| e.to_string())?;
        let card_store = Connection::open_in_memory().map_err(|e| e.to_string())?;
        store::init_store(&card_store).map_err(|e| e.to_string())?;
        let deck_id = store::get_or_create_deck_by_name(&card_store, &deck_name)
            .map_err(|e| e.to_string())?;
        self.init_with_subtitles(
            subtitles,
            deck_id,
            source_file_id,
            known_store,
            video_path,
            card_store,
        )
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
                start_ms: srt_timestamp_to_ms(&entry.start_time).unwrap_or(0),
                end_ms: srt_timestamp_to_ms(&entry.end_time).unwrap_or(0),
                text: entry.text.clone(),
                is_known: inner.known_ids.contains(&(entry.id as i64)),
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

    pub fn save_card(
        &self,
        deck_id: DeckId,
        target: String,
        explanation: String,
    ) -> Result<CardDto, String> {
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
            card_id: 0,
            sentence,
            target,
            explanation,
            deck_id,
            tags: vec![],
            file_id: session.source_file_id.clone(),
            subtitle_id,
        };
        let saved = session.add_card(card);
        if let Some(ref card_store) = inner.card_store {
            let _ = store::save_card_to_store(card_store, &saved);
        }
        Ok(CardDto::from(saved))
    }

    pub fn session_cards(&self) -> Result<Vec<CardDto>, String> {
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        let session = inner.session.as_ref().ok_or("no active session")?;
        Ok(session.cards().iter().cloned().map(CardDto::from).collect())
    }

    pub fn export_session(&self, path: &Path, deck_id: DeckId) -> Result<(), String> {
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        let session = inner.session.as_ref().ok_or("no active session")?;
        session.export_tsv(path, deck_id).map_err(|e| e.to_string())
    }

    pub fn edit_card(
        &self,
        card_id: u32,
        sentence: String,
        target: String,
        explanation: String,
    ) -> Result<CardDto, String> {
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;
        let session = inner.session.as_mut().ok_or("no active session")?;
        session
            .edit_card(card_id, sentence, target, explanation)
            .map_err(|e| e.to_string())?;
        let cards = session.cards();
        let card = cards
            .iter()
            .find(|c| c.card_id == card_id)
            .ok_or("card not found after edit")?;
        Ok(CardDto::from(card.clone()))
    }

    pub fn delete_card(&self, card_id: u32) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;
        let session = inner.session.as_mut().ok_or("no active session")?;
        session.remove_card(card_id).map_err(|e| e.to_string())
    }

    pub fn mark_line_known(&self, subtitle_id: u32) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;
        let store = inner.known_store.as_ref().ok_or("no known lines store")?;
        store
            .mark_known(&inner.source_file_id, subtitle_id as i64)
            .map_err(|e| e.to_string())?;
        inner.known_ids.insert(subtitle_id as i64);
        Ok(())
    }

    pub fn deck_name(&self) -> Result<String, String> {
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        let session = inner.session.as_ref().ok_or("no active session")?;
        let store = inner.card_store.as_ref().ok_or("no card store")?;
        let deck = store::get_deck(store, session.deck_id)
            .map_err(|e| e.to_string())?
            .ok_or("deck not found")?;
        Ok(deck.name)
    }

    pub fn is_line_known(&self, subtitle_id: u32) -> Result<bool, String> {
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        Ok(inner.known_ids.contains(&(subtitle_id as i64)))
    }

    pub fn video_path(&self) -> Result<Option<String>, String> {
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        if inner.session.is_none() {
            return Ok(None);
        }
        let path = inner.video_path.clone();
        if path.is_empty() {
            Ok(None)
        } else {
            Ok(Some(path))
        }
    }

    pub fn translation_span(&self) -> Result<String, String> {
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        if inner.session.is_none() {
            return Err("no active session".to_string());
        }
        if inner.subtitles.is_empty() {
            return Err("no subtitles loaded".to_string());
        }
        Ok(build_translation_span(
            &inner.subtitles,
            inner.current_index,
        ))
    }

    pub async fn translate_current_span(
        &self,
        settings: &TranslationSettings,
    ) -> Result<String, AppError> {
        let config = match &settings.provider {
            TranslationProvider::DeepL(config) => config.clone(),
            TranslationProvider::Disabled => return Err(AppError::translation_disabled()),
        };

        let span = self.translation_span().map_err(AppError::session_error)?;

        let preview: String = span.chars().take(80).collect();
        debug!(
            "calling DeepL: source_lang={}, target_lang={}, text_preview={:?}",
            config.source_lang, config.target_lang, preview,
        );

        let result = translation::translate_with_deepl(&span, &config).await;

        match &result {
            Ok(text) => debug!(
                "DeepL success: {:?}",
                text.chars().take(80).collect::<String>()
            ),
            Err(e) => error!("DeepL error: {:?}", e),
        }

        result.map_err(AppError::from)
    }

    #[cfg(test)]
    pub(crate) async fn translate_current_span_at_url(
        &self,
        settings: &TranslationSettings,
        url: &str,
    ) -> Result<String, AppError> {
        let config = match &settings.provider {
            TranslationProvider::DeepL(config) => config.clone(),
            TranslationProvider::Disabled => return Err(AppError::translation_disabled()),
        };

        let span = self.translation_span().map_err(AppError::session_error)?;

        translation::translate_with_deepl_at_url(&span, &config, url)
            .await
            .map_err(AppError::from)
    }

    pub fn search_subtitles(&self, query: &str) -> Result<Vec<SubtitleSearchResultDto>, String> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        let query_lower = query.to_lowercase();
        Ok(inner
            .subtitles
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.text.to_lowercase().contains(&query_lower))
            .map(|(index, entry)| SubtitleSearchResultDto {
                subtitle_id: entry.id,
                index,
                text: entry.text.clone(),
                start_ms: srt_timestamp_to_ms(&entry.start_time).unwrap_or(0),
            })
            .collect())
    }
}

#[tauri::command]
fn start_session(
    state: tauri::State<'_, MiningSessionState>,
    app_handle: tauri::AppHandle,
    video_path: String,
    srt_path: String,
    deck_name: String,
) -> Result<(), String> {
    let source_file_id = Path::new(&video_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;
    let store_path = app_data_dir.join("known_lines.db");
    let known_store = KnownLinesStore::open(&store_path).map_err(|e| e.to_string())?;
    state.start_session(
        Path::new(&srt_path),
        deck_name,
        source_file_id,
        known_store,
        video_path,
    )
}

#[tauri::command]
fn start_embedded_session(
    state: tauri::State<'_, MiningSessionState>,
    app_handle: tauri::AppHandle,
    video_path: String,
    deck_name: String,
) -> Result<(), SessionStartError> {
    let source_file_id = Path::new(&video_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| SessionStartError {
            code: "SETUP_FAILED".into(),
            message: e.to_string(),
        })?;
    std::fs::create_dir_all(&app_data_dir).map_err(|e| SessionStartError {
        code: "SETUP_FAILED".into(),
        message: e.to_string(),
    })?;
    let store_path = app_data_dir.join("known_lines.db");
    let known_store = KnownLinesStore::open(&store_path).map_err(|e| SessionStartError {
        code: "SETUP_FAILED".into(),
        message: e.to_string(),
    })?;

    state
        .start_embedded_session(deck_name, source_file_id, known_store, video_path)
        .map_err(|e| SessionStartError {
            code: "INIT_FAILED".into(),
            message: e,
        })
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
    deck_state: tauri::State<'_, DeckState>,
    target: String,
    explanation: String,
) -> Result<CardDto, String> {
    let active_deck_id = {
        let inner = deck_state.inner.lock().map_err(|e| e.to_string())?;
        inner.active_deck_id
    };
    state.save_card(active_deck_id, target, explanation)
}

#[tauri::command]
fn list_decks(deck_state: tauri::State<'_, DeckState>) -> Result<Vec<DeckDto>, AppError> {
    let inner = deck_state
        .inner
        .lock()
        .map_err(|e| AppError::session_error(e.to_string()))?;
    store::list_decks(&inner.store)
        .map(|decks| decks.into_iter().map(DeckDto::from).collect())
        .map_err(|e| AppError::session_error(e.to_string()))
}

#[tauri::command]
fn get_active_deck(deck_state: tauri::State<'_, DeckState>) -> Result<DeckDto, AppError> {
    let inner = deck_state
        .inner
        .lock()
        .map_err(|e| AppError::session_error(e.to_string()))?;
    let deck = store::get_deck(&inner.store, inner.active_deck_id)
        .map_err(|e| AppError::session_error(e.to_string()))?
        .ok_or_else(|| AppError::deck_not_found(inner.active_deck_id.0))?;
    Ok(DeckDto::from(deck))
}

#[tauri::command]
fn set_active_deck(
    app_handle: tauri::AppHandle,
    deck_state: tauri::State<'_, DeckState>,
    deck_id: i64,
) -> Result<(), AppError> {
    let deck_id = DeckId(deck_id);
    {
        let mut inner = deck_state
            .inner
            .lock()
            .map_err(|e| AppError::session_error(e.to_string()))?;
        store::get_deck(&inner.store, deck_id)
            .map_err(|e| AppError::session_error(e.to_string()))?
            .ok_or_else(|| AppError::deck_not_found(deck_id.0))?;
        inner.active_deck_id = deck_id;
    }
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| AppError::session_error(e.to_string()))?;
    let config_path = app_data_dir.join("deck_config.json");
    save_deck_config(&config_path, deck_id).map_err(|e| AppError::session_error(e))
}

#[tauri::command]
fn create_deck(
    app_handle: tauri::AppHandle,
    deck_state: tauri::State<'_, DeckState>,
    name: String,
) -> Result<DeckDto, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::invalid_deck_name());
    }
    let mut inner = deck_state
        .inner
        .lock()
        .map_err(|e| AppError::session_error(e.to_string()))?;
    let count =
        store::deck_count(&inner.store).map_err(|e| AppError::session_error(e.to_string()))?;
    if count >= 200 {
        return Err(AppError::deck_limit_reached());
    }
    let deck_id = store::create_deck(&inner.store, &name)
        .map_err(|e| AppError::session_error(e.to_string()))?;
    let deck = store::get_deck(&inner.store, deck_id)
        .map_err(|e| AppError::session_error(e.to_string()))?
        .ok_or_else(|| AppError::deck_not_found(deck_id.0))?;
    inner.active_deck_id = deck_id;
    drop(inner);

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| AppError::session_error(e.to_string()))?;
    let config_path = app_data_dir.join("deck_config.json");
    save_deck_config(&config_path, deck_id).map_err(|e| AppError::session_error(e))?;

    Ok(DeckDto::from(deck))
}

#[tauri::command]
fn rename_deck(
    deck_state: tauri::State<'_, DeckState>,
    deck_id: i64,
    new_name: String,
) -> Result<DeckDto, AppError> {
    if new_name.trim().is_empty() {
        return Err(AppError::invalid_deck_name());
    }
    let deck_id = DeckId(deck_id);
    let inner = deck_state
        .inner
        .lock()
        .map_err(|e| AppError::session_error(e.to_string()))?;
    store::rename_deck(&inner.store, deck_id, &new_name)
        .map_err(|e| AppError::session_error(e.to_string()))?;
    let deck = store::get_deck(&inner.store, deck_id)
        .map_err(|e| AppError::session_error(e.to_string()))?
        .ok_or_else(|| AppError::deck_not_found(deck_id.0))?;
    Ok(DeckDto::from(deck))
}

#[tauri::command]
fn delete_deck(
    app_handle: tauri::AppHandle,
    deck_state: tauri::State<'_, DeckState>,
    deck_id: i64,
) -> Result<(), AppError> {
    let deck_id = DeckId(deck_id);
    let mut needs_save = false;
    {
        let mut inner = deck_state
            .inner
            .lock()
            .map_err(|e| AppError::session_error(e.to_string()))?;

        // Validate deck exists
        store::get_deck(&inner.store, deck_id)
            .map_err(|e| AppError::session_error(e.to_string()))?
            .ok_or_else(|| AppError::deck_not_found(deck_id.0))?;

        let was_active = inner.active_deck_id == deck_id;

        // Delete deck and its cards
        store::delete_deck(&inner.store, deck_id)
            .map_err(|e| AppError::session_error(e.to_string()))?;

        // If we deleted the active deck, reassign to the first remaining
        if was_active {
            let remaining = store::list_decks(&inner.store)
                .map_err(|e| AppError::session_error(e.to_string()))?;
            if let Some(first) = remaining.first() {
                inner.active_deck_id = first.id;
                needs_save = true;
            }
        }
    }

    if needs_save {
        let inner = deck_state
            .inner
            .lock()
            .map_err(|e| AppError::session_error(e.to_string()))?;
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| AppError::session_error(e.to_string()))?;
        let config_path = app_data_dir.join("deck_config.json");
        save_deck_config(&config_path, inner.active_deck_id)
            .map_err(|e| AppError::session_error(e))?;
    }

    Ok(())
}

#[tauri::command]
fn get_session_cards(state: tauri::State<'_, MiningSessionState>) -> Result<Vec<CardDto>, String> {
    state.session_cards()
}

#[tauri::command]
fn export_session(
    state: tauri::State<'_, MiningSessionState>,
    deck_state: tauri::State<'_, DeckState>,
    path: String,
) -> Result<(), String> {
    let deck_inner = deck_state.inner.lock().map_err(|e| e.to_string())?;
    let deck_id = deck_inner.active_deck_id;
    state.export_session(Path::new(&path), deck_id)
}

#[tauri::command]
fn edit_card(
    state: tauri::State<'_, MiningSessionState>,
    card_id: u32,
    sentence: String,
    target: String,
    explanation: String,
) -> Result<CardDto, String> {
    state.edit_card(card_id, sentence, target, explanation)
}

#[tauri::command]
fn delete_card(state: tauri::State<'_, MiningSessionState>, card_id: u32) -> Result<(), String> {
    state.delete_card(card_id)
}

#[tauri::command]
fn mark_line_known(
    state: tauri::State<'_, MiningSessionState>,
    subtitle_id: u32,
) -> Result<(), String> {
    state.mark_line_known(subtitle_id)
}

#[tauri::command]
fn get_deck_name(state: tauri::State<'_, MiningSessionState>) -> Result<String, String> {
    state.deck_name()
}

#[tauri::command]
fn get_video_path(state: tauri::State<'_, MiningSessionState>) -> Result<Option<String>, String> {
    state.video_path()
}

#[tauri::command]
fn is_line_known(
    state: tauri::State<'_, MiningSessionState>,
    subtitle_id: u32,
) -> Result<bool, String> {
    state.is_line_known(subtitle_id)
}

#[tauri::command]
fn request_translation(
    lemma: String,
    app_handle: tauri::AppHandle,
    translation_manager: tauri::State<'_, TranslationManager>,
) -> Result<Option<String>, String> {
    if lemma.trim().is_empty() {
        return Ok(None);
    }

    if let Some(translation) = translation_manager.get_cached(&lemma) {
        return Ok(Some(translation));
    }

    let app_handle = app_handle.clone();
    let lemma_clone = lemma.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let translation = match dictionary::suggest_explanation(&lemma_clone) {
            Ok(Some(translation)) => {
                if let Some(manager) = app_handle.try_state::<TranslationManager>() {
                    manager.insert_cache(lemma_clone.clone(), translation.clone());
                }
                Some(translation)
            }
            Ok(None) => None,
            Err(e) => {
                warn!("dictionary lookup failed for '{lemma_clone}': {e:?}");
                None
            }
        };

        let _ = app_handle.emit(
            "translation-result",
            TranslationResult {
                lemma: lemma_clone,
                translation,
            },
        );
    });

    Ok(None)
}

#[tauri::command]
fn copy_translation_span(state: tauri::State<'_, MiningSessionState>) -> Result<(), String> {
    let span = state.translation_span()?;
    let mut clip = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clip.set_text(&span).map_err(|e| e.to_string())?;
    Ok(())
}

struct VideoServerState {
    port: u16,
}

fn default_video_server() -> VideoServerState {
    match video_server::VideoServer::start() {
        Ok(srv) => {
            let port = srv.port();
            // Leak the server so it lives for the full app lifetime.
            // Drop is handled via process exit.
            std::mem::forget(srv);
            VideoServerState { port }
        }
        Err(e) => {
            error!("video server failed to start: {e}");
            VideoServerState { port: 0 }
        }
    }
}

#[tauri::command]
fn search_subtitles(
    state: tauri::State<'_, MiningSessionState>,
    query: String,
) -> Result<Vec<SubtitleSearchResultDto>, String> {
    state.search_subtitles(&query)
}

#[tauri::command]
async fn translate_current_span(
    state: tauri::State<'_, MiningSessionState>,
    settings_state: tauri::State<'_, AppSettingsState>,
) -> Result<String, AppError> {
    let ts = {
        let settings = settings_state
            .inner
            .lock()
            .map_err(|e| AppError::session_error(e.to_string()))?;
        let ts = settings.to_translation_settings();
        let provider_name = match &ts.provider {
            TranslationProvider::DeepL(_) => "DeepL",
            TranslationProvider::Disabled => "Disabled",
        };
        debug!(
            "translate_current_span called: provider={}, target_lang={}",
            provider_name, settings.deepl_target_lang,
        );
        ts
    };
    state.translate_current_span(&ts).await
}

#[tauri::command]
fn get_translation_settings(
    state: tauri::State<'_, AppSettingsState>,
) -> Result<translation::TranslationSettingsDto, String> {
    let settings = state.inner.lock().map_err(|e| e.to_string())?;
    Ok(translation::TranslationSettingsDto {
        enabled: settings.deepl_enabled,
        has_api_key: settings.deepl_api_key.is_some(),
        target_lang: settings.deepl_target_lang.clone(),
    })
}

#[tauri::command]
fn update_translation_settings(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppSettingsState>,
    new_settings: translation::UpdateTranslationSettings,
) -> Result<(), String> {
    let mut settings = state.inner.lock().map_err(|e| e.to_string())?;
    info!(
        "update_translation_settings: enabled={}, has_new_key={}, target_lang={}, had_existing_key={}",
        new_settings.enabled,
        !new_settings.api_key.is_empty(),
        new_settings.target_lang,
        settings.deepl_api_key.is_some(),
    );

    if new_settings.enabled {
        if new_settings.api_key.is_empty() && settings.deepl_api_key.is_none() {
            return Err("API key is required to enable DeepL translation".to_string());
        }
        if !new_settings.api_key.is_empty() {
            settings.deepl_api_key = Some(new_settings.api_key);
        }
    }

    settings.deepl_enabled = new_settings.enabled;
    settings.deepl_target_lang = new_settings.target_lang;

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let config_path = app_data_dir.join("config.json");
    info!("saving config to: {}", config_path.display());
    translation::save_settings(&config_path, &settings)
}

pub struct AppSettingsState {
    pub inner: std::sync::Mutex<AppSettings>,
}

pub struct DeckStateInner {
    pub active_deck_id: DeckId,
    pub store: Connection,
}

pub struct DeckState {
    pub inner: std::sync::Mutex<DeckStateInner>,
}

fn load_deck_config(path: &std::path::Path) -> Option<DeckId> {
    let content = std::fs::read_to_string(path).ok()?;
    #[derive(serde::Deserialize)]
    struct Config {
        active_deck_id: i64,
    }
    serde_json::from_str::<Config>(&content)
        .ok()
        .map(|c| DeckId(c.active_deck_id))
}

fn save_deck_config(path: &std::path::Path, active_deck_id: DeckId) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Config {
        active_deck_id: i64,
    }
    let content = serde_json::to_string_pretty(&Config {
        active_deck_id: active_deck_id.0,
    })
    .map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_video_server_port(state: tauri::State<'_, VideoServerState>) -> u16 {
    state.port
}

#[derive(serde::Serialize)]
pub struct AppVersionInfo {
    pub version: String,
    pub name: String,
    pub platform: String,
    pub arch: String,
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    webbrowser::open(&url).map_err(|e| format!("failed to open URL: {e}"))
}

#[tauri::command]
fn get_app_version() -> AppVersionInfo {
    AppVersionInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        name: env!("CARGO_PKG_NAME").to_string(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(MiningSessionState::new())
        .manage(TranslationManager::new())
        .manage(default_video_server())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");

            // Translation settings
            let config_path = app_data_dir.join("config.json");
            let settings = translation::load_settings(&config_path);
            info!(
                "loaded config: enabled={}, has_key={}, target={}",
                settings.deepl_enabled,
                settings.deepl_api_key.is_some(),
                settings.deepl_target_lang,
            );
            app.manage(AppSettingsState {
                inner: std::sync::Mutex::new(settings),
            });

            // Deck store
            let deck_store_path = app_data_dir.join("decks.db");
            let deck_conn = Connection::open(&deck_store_path).expect("failed to open deck store");
            store::init_store(&deck_conn).expect("failed to init deck store");
            let default_deck_id =
                store::ensure_default_deck(&deck_conn).expect("failed to ensure default deck");

            let deck_config_path = app_data_dir.join("deck_config.json");
            let active_deck_id = load_deck_config(&deck_config_path)
                .filter(|id| store::get_deck(&deck_conn, *id).ok().flatten().is_some())
                .unwrap_or(default_deck_id);

            info!("active deck: id={}", active_deck_id.0);

            app.manage(DeckState {
                inner: std::sync::Mutex::new(DeckStateInner {
                    active_deck_id,
                    store: deck_conn,
                }),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_session,
            start_embedded_session,
            get_subtitles,
            get_current_index,
            next_subtitle,
            previous_subtitle,
            set_current_index,
            save_card,
            edit_card,
            delete_card,
            get_session_cards,
            get_deck_name,
            export_session,
            request_translation,
            mark_line_known,
            is_line_known,
            get_video_path,
            get_video_server_port,
            search_subtitles,
            copy_translation_span,
            translate_current_span,
            get_translation_settings,
            update_translation_settings,
            list_decks,
            get_active_deck,
            set_active_deck,
            create_deck,
            rename_deck,
            delete_deck,
            get_app_version,
            open_url,
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|err| {
            error!("{err}");
            std::process::exit(1);
        });

    app.run(|_app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            // allow exit
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use translation::DeepLConfig;

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
            .start_session(
                &srt_path,
                "test-deck".into(),
                "video-1".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let subtitles = state.subtitles().unwrap();
        assert_eq!(subtitles.len(), 5);
        assert_eq!(subtitles[0].text, "안녕하세요 반갑습니다.");
        assert_eq!(subtitles[0].id, 1);
    }

    #[test]
    fn start_session_stores_video_path() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        assert_eq!(state.video_path().unwrap(), None);

        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        assert_eq!(state.video_path().unwrap(), Some("/videos/test.mp4".into()));
    }

    #[test]
    fn subtitle_timings_are_exposed_as_milliseconds() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let subtitles = state.subtitles().unwrap();
        assert_eq!(subtitles[0].id, 1);
        assert_eq!(subtitles[0].start_ms, 1000);
        assert_eq!(subtitles[0].end_ms, 4000);
        assert_eq!(subtitles[1].start_ms, 5000);
        assert_eq!(subtitles[1].end_ms, 8000);
        assert_eq!(subtitles[2].start_ms, 9500);
        assert_eq!(subtitles[2].end_ms, 12300);
        assert_eq!(subtitles[3].start_ms, 13000);
        assert_eq!(subtitles[3].end_ms, 16500);
        assert_eq!(subtitles[4].start_ms, 17000);
        assert_eq!(subtitles[4].end_ms, 20000);
    }

    #[test]
    fn video_path_is_none_before_session_starts() {
        let state = MiningSessionState::new();
        assert_eq!(state.video_path().unwrap(), None);
    }

    #[test]
    fn current_index_initialized_to_zero() {
        let state = MiningSessionState::new();
        assert_eq!(state.current_index().unwrap(), 0);

        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
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
            KnownLinesStore::in_memory().unwrap(),
            "/videos/test.mp4".into(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn start_session_returns_error_on_empty_file() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("empty.srt");
        let result = state.start_session(
            &srt_path,
            "deck".into(),
            "file".into(),
            KnownLinesStore::in_memory().unwrap(),
            "/videos/test.mp4".into(),
        );
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
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        assert_eq!(state.next_subtitle().unwrap(), 1);
        assert_eq!(state.current_index().unwrap(), 1);
    }

    #[test]
    fn next_subtitle_clamps_at_last_entry() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
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
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
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
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
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
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        assert_eq!(state.set_current_index(3).unwrap(), 3);
        assert_eq!(state.current_index().unwrap(), 3);
    }

    #[test]
    fn set_current_index_rejects_out_of_range() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
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
            .start_session(
                &srt_path,
                "test-deck".into(),
                "video-1".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let card = state
            .save_card(DeckId(1), "안녕".into(), "Hello".into())
            .unwrap();
        assert_eq!(card.sentence, "안녕하세요 반갑습니다.");
        assert_eq!(card.target, "안녕");
        assert_eq!(card.explanation, "Hello");
        assert_eq!(card.deck_id, 1);

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
        let result = state.save_card(DeckId(1), "test".into(), "explanation".into());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "no active session");
    }

    #[test]
    fn save_card_multiple_cards() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        state
            .save_card(DeckId(1), "target1".into(), "exp1".into())
            .unwrap();
        state.set_current_index(1).unwrap();
        state
            .save_card(DeckId(1), "target2".into(), "exp2".into())
            .unwrap();

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
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let cards = state.session_cards().unwrap();
        assert!(cards.is_empty());
    }

    #[test]
    fn get_session_cards_returns_cards_after_save() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "test-deck".into(),
                "video-1".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        state
            .save_card(DeckId(1), "안녕".into(), "Hello".into())
            .unwrap();

        let cards = state.session_cards().unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].target, "안녕");
        assert_eq!(cards[0].explanation, "Hello");
        assert_eq!(cards[0].sentence, "안녕하세요 반갑습니다.");
        assert_eq!(cards[0].deck_id, 1);
    }

    #[test]
    fn export_session_returns_error_without_session() {
        let state = MiningSessionState::new();
        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_export_no_session.tsv");
        let result = state.export_session(&path, DeckId(1));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "no active session");
    }

    #[test]
    fn export_session_writes_tsv_file() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "test-deck".into(),
                "video-1".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        state
            .save_card(DeckId(1), "안녕".into(), "Hello".into())
            .unwrap();
        state.set_current_index(1).unwrap();
        state
            .save_card(DeckId(1), "날씨".into(), "Weather".into())
            .unwrap();

        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_export_test.tsv");
        state.export_session(&path, DeckId(1)).unwrap();

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
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        state
            .save_card(DeckId(1), "target1".into(), "exp1".into())
            .unwrap();
        state.set_current_index(1).unwrap();
        state
            .save_card(DeckId(1), "target2".into(), "exp2".into())
            .unwrap();
        state.set_current_index(2).unwrap();
        state
            .save_card(DeckId(1), "target3".into(), "exp3".into())
            .unwrap();

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
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
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
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
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
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
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
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let subtitles = state.subtitles().unwrap();
        for sub in &subtitles {
            for token in &sub.tokens {
                assert!(!token.lemma.is_empty(), "empty lemma in \"{}\"", sub.text);
                assert!(!token.pos.is_empty(), "empty POS in \"{}\"", sub.text);
            }
        }
    }

    #[test]
    fn translation_manager_cache_hit() {
        let manager = TranslationManager::new();
        manager.insert_cache("사랑".into(), "love".into());
        assert_eq!(manager.get_cached("사랑"), Some("love".into()));
    }

    #[test]
    fn translation_manager_cache_miss() {
        let manager = TranslationManager::new();
        assert_eq!(manager.get_cached("없는단어"), None);
    }

    #[test]
    fn translation_manager_cache_empty() {
        let manager = TranslationManager::new();
        assert_eq!(manager.get_cached(""), None);
    }

    #[test]
    fn translation_manager_cache_overwrites() {
        let manager = TranslationManager::new();
        manager.insert_cache("사랑".into(), "love".into());
        manager.insert_cache("사랑".into(), "affection".into());
        assert_eq!(manager.get_cached("사랑"), Some("affection".into()));
    }

    #[test]
    fn translation_manager_cache_multi_entry() {
        let manager = TranslationManager::new();
        manager.insert_cache("사랑".into(), "love".into());
        manager.insert_cache("우정".into(), "friendship".into());
        assert_eq!(manager.get_cached("사랑"), Some("love".into()));
        assert_eq!(manager.get_cached("우정"), Some("friendship".into()));
    }

    #[test]
    fn edit_card_updates_session_card() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let saved = state
            .save_card(DeckId(1), "target1".into(), "exp1".into())
            .unwrap();
        let card_id = saved.card_id;

        let updated = state
            .edit_card(
                card_id,
                "new sentence".into(),
                "new target".into(),
                "new expl".into(),
            )
            .unwrap();
        assert_eq!(updated.sentence, "new sentence");
        assert_eq!(updated.target, "new target");
        assert_eq!(updated.explanation, "new expl");
        assert_eq!(updated.card_id, card_id);

        let cards = state.session_cards().unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].sentence, "new sentence");
        assert_eq!(cards[0].target, "new target");
    }

    #[test]
    fn edit_card_returns_error_without_session() {
        let state = MiningSessionState::new();
        let result = state.edit_card(1, "s".into(), "t".into(), "e".into());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "no active session");
    }

    #[test]
    fn edit_card_returns_error_for_invalid_id() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let result = state.edit_card(999, "s".into(), "t".into(), "e".into());
        assert!(result.is_err());
    }

    #[test]
    fn delete_card_removes_card_from_session() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let saved = state
            .save_card(DeckId(1), "target1".into(), "exp1".into())
            .unwrap();
        let card_id = saved.card_id;

        assert_eq!(state.session_cards().unwrap().len(), 1);

        state.delete_card(card_id).unwrap();
        assert_eq!(state.session_cards().unwrap().len(), 0);
    }

    #[test]
    fn delete_card_returns_error_without_session() {
        let state = MiningSessionState::new();
        let result = state.delete_card(1);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "no active session");
    }

    #[test]
    fn delete_card_returns_error_for_invalid_id() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let result = state.delete_card(999);
        assert!(result.is_err());
    }

    #[test]
    fn deleted_cards_not_in_get_session_cards() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        state
            .save_card(DeckId(1), "keep".into(), "keep".into())
            .unwrap();
        let card2 = state
            .save_card(DeckId(1), "delete".into(), "delete".into())
            .unwrap();
        state
            .save_card(DeckId(1), "keep2".into(), "keep2".into())
            .unwrap();

        state.delete_card(card2.card_id).unwrap();

        let cards = state.session_cards().unwrap();
        assert_eq!(cards.len(), 2);
        for card in &cards {
            assert_ne!(card.target, "delete");
        }
    }

    #[test]
    fn deleted_cards_not_in_export() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        state
            .save_card(DeckId(1), "keep".into(), "keep".into())
            .unwrap();
        let to_delete = state
            .save_card(DeckId(1), "delete".into(), "delete".into())
            .unwrap();
        state.delete_card(to_delete.card_id).unwrap();

        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_edit_delete_export.tsv");
        state.export_session(&path, DeckId(1)).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "keep\t안녕하세요 반갑습니다.\tkeep\n");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn known_lines_are_reflected_in_subtitles_after_mark() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        let store = KnownLinesStore::in_memory().unwrap();
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                store,
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let subtitles = state.subtitles().unwrap();
        assert!(!subtitles[0].is_known);

        state.mark_line_known(subtitles[0].id).unwrap();

        let subtitles = state.subtitles().unwrap();
        assert!(subtitles[0].is_known);
    }

    #[test]
    fn known_lines_persist_between_sessions() {
        let srt_path = fixture_path("sample.srt");
        let dir = std::env::temp_dir();
        let db_path = dir.join("kaeda_test_known_lines.db");
        let _ = std::fs::remove_file(&db_path);

        // Session 1: mark line 1 as known via file-backed store
        {
            let store = KnownLinesStore::open(&db_path).unwrap();
            let state = MiningSessionState::new();
            state
                .start_session(
                    &srt_path,
                    "deck".into(),
                    "file".into(),
                    store,
                    "/videos/test.mp4".into(),
                )
                .unwrap();
            state.mark_line_known(1).unwrap();

            let subtitles = state.subtitles().unwrap();
            assert!(subtitles[0].is_known);
        }

        // Session 2: reopen the same file-backed store
        {
            let store = KnownLinesStore::open(&db_path).unwrap();
            let state = MiningSessionState::new();
            state
                .start_session(
                    &srt_path,
                    "deck".into(),
                    "file".into(),
                    store,
                    "/videos/test.mp4".into(),
                )
                .unwrap();

            assert!(state.is_line_known(1).unwrap());
        }

        // Also verify raw store access
        let store = KnownLinesStore::open(&db_path).unwrap();
        assert!(store.is_known("file", 1).unwrap());

        std::fs::remove_file(&db_path).unwrap();
    }

    #[test]
    fn known_lines_are_empty_for_new_file() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let subtitles = state.subtitles().unwrap();
        for sub in &subtitles {
            assert!(!sub.is_known, "subtitle {} should not be known", sub.id);
        }
    }

    #[test]
    fn mark_line_known_is_idempotent() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        state.mark_line_known(1).unwrap();
        state.mark_line_known(1).unwrap();
        state.mark_line_known(1).unwrap();

        let subtitles = state.subtitles().unwrap();
        assert!(subtitles[0].is_known);
    }

    #[test]
    fn start_embedded_session_missing_file_returns_error() {
        let state = MiningSessionState::new();
        let result = state.start_embedded_session(
            "deck".into(),
            "file".into(),
            KnownLinesStore::in_memory().unwrap(),
            "/videos/test.mp4".into(),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("extraction") || err.contains("ffmpeg") || err.contains("video file"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn is_line_known_returns_correct_state() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        assert!(!state.is_line_known(1).unwrap());
        state.mark_line_known(1).unwrap();
        assert!(state.is_line_known(1).unwrap());
    }

    #[test]
    fn search_subtitles_returns_matching_subset() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let results = state.search_subtitles("날씨").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subtitle_id, 2);
        assert_eq!(results[0].index, 1);
        assert_eq!(results[0].text, "오늘은 날씨가 좋네요.");
        assert_eq!(results[0].start_ms, 5000);
    }

    #[test]
    fn search_subtitles_empty_query_returns_empty_vec() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let results = state.search_subtitles("").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_subtitles_before_session_returns_empty() {
        let state = MiningSessionState::new();
        let results = state.search_subtitles("날씨").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn translation_span_returns_non_empty_with_active_session() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let span = state.translation_span().unwrap();
        assert_eq!(span, "안녕하세요 반갑습니다.");
    }

    #[test]
    fn translation_span_returns_error_without_session() {
        let state = MiningSessionState::new();
        let result = state.translation_span();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "no active session");
    }

    #[test]
    fn translation_span_middle_index_includes_neighbors() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        state.set_current_index(2).unwrap();
        let span = state.translation_span().unwrap();
        assert_eq!(span, "저는 공부를 하고 있어요.");
    }

    #[test]
    fn translation_span_first_index_excludes_prev() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let span = state.translation_span().unwrap();
        assert_eq!(span, "안녕하세요 반갑습니다.");
    }

    #[test]
    fn translation_span_last_index_excludes_next() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        state.set_current_index(4).unwrap();
        let span = state.translation_span().unwrap();
        assert_eq!(span, "한국어 단어를 배웁시다.");
    }

    #[test]
    fn search_subtitles_case_insensitive_noop_for_hangul() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let results = state.search_subtitles("날씨").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subtitle_id, 2);
    }

    #[tokio::test]
    async fn translate_current_span_disabled_provider() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let settings = TranslationSettings {
            provider: TranslationProvider::Disabled,
        };

        let result = state.translate_current_span(&settings).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "TRANSLATION_DISABLED");
    }

    #[tokio::test]
    async fn translate_current_span_no_session() {
        let state = MiningSessionState::new();
        let settings = TranslationSettings {
            provider: TranslationProvider::DeepL(DeepLConfig {
                api_key: "dummy".into(),
                source_lang: "KO".into(),
                target_lang: "EN".into(),
            }),
        };

        let result = state.translate_current_span(&settings).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "SESSION_ERROR");
    }

    #[tokio::test]
    async fn translate_current_span_with_mocked_deepl() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/translate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"translations":[{"detected_source_language":"KO","text":"Hello everyone"}]}"#,
            )
            .create();

        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "deck".into(),
                "file".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let settings = TranslationSettings {
            provider: TranslationProvider::DeepL(DeepLConfig {
                api_key: "test-key".into(),
                source_lang: "KO".into(),
                target_lang: "EN".into(),
            }),
        };

        let url = format!("{}/translate", server.url());
        let result = state.translate_current_span_at_url(&settings, &url).await;
        assert_eq!(result.unwrap(), "Hello everyone");
        mock.assert();
    }

    // -----------------------------------------------------------------------
    // Active Deck / DeckState tests
    // -----------------------------------------------------------------------

    #[test]
    fn save_card_uses_provided_deck_id() {
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "test-deck".into(),
                "video-1".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let card = state
            .save_card(DeckId(42), "안녕".into(), "Hello".into())
            .unwrap();
        assert_eq!(card.deck_id, 42);
    }

    #[test]
    fn fresh_deck_store_creates_default_deck() {
        let dir = std::env::temp_dir();
        let db_path = dir.join("kaeda_test_fresh_decks.db");
        let config_path = dir.join("kaeda_test_fresh_deck_config.json");
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(&config_path);

        let conn = Connection::open(&db_path).unwrap();
        store::init_store(&conn).unwrap();
        let default_id = store::ensure_default_deck(&conn).unwrap();

        let decks = store::list_decks(&conn).unwrap();
        assert_eq!(decks.len(), 1);
        assert_eq!(decks[0].name, "Korean – General");
        assert_eq!(default_id, decks[0].id);

        let active_from_config = load_deck_config(&config_path);
        assert!(active_from_config.is_none(), "no config file yet");

        std::fs::remove_file(&db_path).unwrap();
    }

    #[test]
    fn active_deck_persists_and_used_for_new_cards() {
        let dir = std::env::temp_dir();
        let db_path = dir.join("kaeda_test_active_deck.db");
        let config_path = dir.join("kaeda_test_active_deck_config.json");
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(&config_path);

        // Set up deck store with two decks
        let conn = Connection::open(&db_path).unwrap();
        store::init_store(&conn).unwrap();
        let deck_a = store::create_deck(&conn, "Deck A").unwrap();
        let deck_b = store::create_deck(&conn, "Deck B").unwrap();
        assert_ne!(deck_a, deck_b);

        // Save deck_b as active
        save_deck_config(&config_path, deck_b).unwrap();

        // Reload: verify config loads correctly
        let loaded = load_deck_config(&config_path).unwrap();
        assert_eq!(loaded, deck_b);

        // Now simulate using active deck for a card
        let state = MiningSessionState::new();
        let srt_path = fixture_path("sample.srt");
        state
            .start_session(
                &srt_path,
                "session-deck".into(),
                "video-1".into(),
                KnownLinesStore::in_memory().unwrap(),
                "/videos/test.mp4".into(),
            )
            .unwrap();

        let card = state
            .save_card(deck_b, "테스트".into(), "test".into())
            .unwrap();
        assert_eq!(card.deck_id, deck_b.0, "card should use active deck_id");

        // Switch active deck and verify new card uses new deck
        save_deck_config(&config_path, deck_a).unwrap();
        let loaded_a = load_deck_config(&config_path).unwrap();
        assert_eq!(loaded_a, deck_a);

        state.set_current_index(1).unwrap();
        let card2 = state
            .save_card(deck_a, "테스트2".into(), "test2".into())
            .unwrap();
        assert_eq!(
            card2.deck_id, deck_a.0,
            "card should use new active deck_id"
        );

        std::fs::remove_file(&db_path).unwrap();
        std::fs::remove_file(&config_path).unwrap();
    }

    #[test]
    fn create_deck_increases_deck_count() {
        let conn = Connection::open_in_memory().unwrap();
        store::init_store(&conn).unwrap();
        let _default = store::ensure_default_deck(&conn).unwrap();

        let count_before = store::deck_count(&conn).unwrap();
        store::create_deck(&conn, "New Deck").unwrap();
        let count_after = store::deck_count(&conn).unwrap();
        assert_eq!(count_after, count_before + 1);
    }

    #[test]
    fn set_active_deck_persists_through_state() {
        let conn = Connection::open_in_memory().unwrap();
        store::init_store(&conn).unwrap();
        let default_id = store::ensure_default_deck(&conn).unwrap();
        let deck_id = store::create_deck(&conn, "Custom Deck").unwrap();

        let state = DeckState {
            inner: std::sync::Mutex::new(DeckStateInner {
                active_deck_id: default_id,
                store: conn,
            }),
        };

        {
            let mut inner = state.inner.lock().unwrap();
            inner.active_deck_id = deck_id;
        }

        let inner = state.inner.lock().unwrap();
        assert_eq!(inner.active_deck_id, deck_id);
    }

    #[test]
    fn deleting_active_deck_reassigns_to_another_deck() {
        let conn = Connection::open_in_memory().unwrap();
        store::init_store(&conn).unwrap();
        let default_id = store::ensure_default_deck(&conn).unwrap();
        let extra_id = store::create_deck(&conn, "Extra Deck").unwrap();

        let state = DeckState {
            inner: std::sync::Mutex::new(DeckStateInner {
                active_deck_id: extra_id,
                store: conn,
            }),
        };

        {
            let mut inner = state.inner.lock().unwrap();
            store::delete_deck(&inner.store, extra_id).unwrap();
            let remaining = store::list_decks(&inner.store).unwrap();
            assert_eq!(remaining.len(), 1);
            inner.active_deck_id = remaining[0].id;
        }

        let inner = state.inner.lock().unwrap();
        assert_eq!(inner.active_deck_id, default_id);
        assert_ne!(inner.active_deck_id, extra_id);
    }
}
