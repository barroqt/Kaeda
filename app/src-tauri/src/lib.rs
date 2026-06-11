use std::path::Path;
use std::sync::Mutex;

use kaeda_core::session::Session;
use kaeda_core::subtitle::{SubtitleEntry, entries_from_srt};

mod dto;
use dto::SubtitleDto;

struct MiningSessionInner {
    session: Option<Session>,
    subtitles: Vec<SubtitleEntry>,
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
        let session = Session::new(deck_name, source_file_id);
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;
        inner.session = Some(session);
        inner.subtitles = subtitles;
        inner.current_index = 0;
        Ok(())
    }

    pub fn subtitles(&self) -> Result<Vec<SubtitleEntry>, String> {
        let inner = self.inner.lock().map_err(|e| e.to_string())?;
        Ok(inner.subtitles.clone())
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
    state
        .subtitles()
        .map(|v| v.into_iter().map(SubtitleDto::from).collect())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
}
