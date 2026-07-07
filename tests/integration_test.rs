use kaeda::app::{Action, AppState, handle_key};
use kaeda_core::dictionary::db::build_index;
use kaeda_core::store::{DeckEntry, Store};
use kaeda_core::subtitle::entries_from_srt;
use kaeda_core::tokenizer::korean::KoreanTokenizer;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyCode;

#[test]
fn full_session_smoke_test() {
    let tokenizer = KoreanTokenizer::new().unwrap();
    let subtitles = entries_from_srt("tests/fixtures/sample.srt".as_ref()).unwrap();
    let mut state = AppState::new(subtitles, "sample.srt".to_string(), &tokenizer);

    let store = Store::in_memory().unwrap();
    build_index(store.connection(), "tests/fixtures/dict_sample.tsv").unwrap();

    state.update_definition(store.connection());

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|f| state.draw(f)).unwrap();

    let keys = [KeyCode::Down, KeyCode::Char('a'), KeyCode::Char('q')];

    for key in &keys {
        let action = handle_key(&mut state, *key);
        if *key == KeyCode::Char('a')
            && let Some(token) = state.selected_candidate()
        {
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
                lemma: token.lemma.to_string(),
                surface: token.surface.to_string(),
                meaning,
                source_sentence: source,
                source_file: state.source_file.clone(),
            };
            store.add_to_deck(&entry).unwrap();
            state.deck_count = state.deck_count.saturating_add(1);
            state.next_subtitle();
        }
        state.update_definition(store.connection());
        terminal.draw(|f| state.draw(f)).unwrap();

        if action == Action::Quit {
            break;
        }
    }

    let count: i64 = store
        .connection()
        .query_row("SELECT COUNT(*) FROM deck", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);

    assert!(state.subtitle_cursor > 0);
}
