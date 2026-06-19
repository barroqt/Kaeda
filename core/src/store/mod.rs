use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use chrono::Utc;
use rusqlite::Connection;

use crate::deck::{Deck, DeckId};
use crate::session::Card;
use crate::subtitle::CoreError;

pub struct DeckEntry {
    pub lemma: String,
    pub surface: String,
    pub meaning: String,
    pub source_sentence: String,
    pub source_file: String,
}

fn migrate_card_decks(conn: &Connection) -> Result<(), CoreError> {
    let schema: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='card_decks'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();
    if schema.to_uppercase().contains("UNIQUE") {
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             CREATE TABLE card_decks_migrated (
                 id   INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL
             );
             INSERT INTO card_decks_migrated (id, name) SELECT id, name FROM card_decks;
             DROP TABLE card_decks;
             ALTER TABLE card_decks_migrated RENAME TO card_decks;
             PRAGMA foreign_keys = ON;",
        )?;
    }
    Ok(())
}

pub fn init_store(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS deck (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            lemma           TEXT NOT NULL UNIQUE,
            surface         TEXT NOT NULL,
            meaning         TEXT NOT NULL,
            source_sentence TEXT,
            source_file     TEXT,
            added_at        TEXT NOT NULL,
            status          TEXT DEFAULT 'new'
        );
        CREATE TABLE IF NOT EXISTS known_words (
            lemma   TEXT PRIMARY KEY,
            added_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS known_lines (
            file_id     TEXT NOT NULL,
            subtitle_id INTEGER NOT NULL,
            added_at    TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (file_id, subtitle_id)
        );
        CREATE TABLE IF NOT EXISTS card_decks (
            id   INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS session_cards (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            deck_id         INTEGER NOT NULL REFERENCES card_decks(id) ON DELETE CASCADE,
            session_card_id INTEGER NOT NULL,
            sentence        TEXT NOT NULL,
            target          TEXT NOT NULL,
            explanation     TEXT NOT NULL DEFAULT '',
            tags            TEXT NOT NULL DEFAULT '',
            file_id         TEXT NOT NULL,
            subtitle_id     INTEGER NOT NULL
        );",
    )?;
    migrate_card_decks(conn)?;
    Ok(())
}

pub fn deck_count(conn: &Connection) -> Result<i64, CoreError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM card_decks", [], |row| row.get(0))?;
    Ok(count)
}

/// Ensure at least one card deck exists. If the `card_decks` table is empty,
/// inserts a default deck named `"Korean – General"`. Returns the ID of the
/// first deck (existing or newly created).
pub fn ensure_default_deck(conn: &Connection) -> Result<DeckId, CoreError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM card_decks", [], |row| row.get(0))?;
    if count == 0 {
        conn.execute(
            "INSERT INTO card_decks (name) VALUES ('Korean – General')",
            [],
        )?;
    }
    let id: i64 = conn.query_row("SELECT id FROM card_decks ORDER BY id LIMIT 1", [], |row| {
        row.get(0)
    })?;
    Ok(DeckId(id))
}

pub fn create_deck(conn: &Connection, name: &str) -> Result<DeckId, CoreError> {
    conn.execute(
        "INSERT INTO card_decks (name) VALUES (?1)",
        rusqlite::params![name],
    )?;
    let id: i64 = conn.last_insert_rowid();
    Ok(DeckId(id))
}

pub fn list_decks(conn: &Connection) -> Result<Vec<Deck>, CoreError> {
    let mut stmt = conn.prepare("SELECT id, name FROM card_decks ORDER BY id")?;
    let decks = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            Ok(Deck {
                id: DeckId(id),
                name,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(decks)
}

pub fn get_deck(conn: &Connection, id: DeckId) -> Result<Option<Deck>, CoreError> {
    let mut stmt = conn.prepare("SELECT id, name FROM card_decks WHERE id = ?1")?;
    let mut rows = stmt.query_map(rusqlite::params![id.0], |row| {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        Ok(Deck {
            id: DeckId(id),
            name,
        })
    })?;
    match rows.next() {
        Some(Ok(deck)) => Ok(Some(deck)),
        Some(Err(e)) => Err(CoreError::Database(e)),
        None => Ok(None),
    }
}

pub fn rename_deck(conn: &Connection, id: DeckId, name: &str) -> Result<(), CoreError> {
    let affected = conn.execute(
        "UPDATE card_decks SET name = ?1 WHERE id = ?2",
        rusqlite::params![name, id.0],
    )?;
    if affected == 0 {
        return Err(CoreError::DeckNotFound(id));
    }
    Ok(())
}

pub fn delete_deck(conn: &Connection, id: DeckId) -> Result<(), CoreError> {
    conn.execute(
        "DELETE FROM session_cards WHERE deck_id = ?1",
        rusqlite::params![id.0],
    )?;
    conn.execute(
        "DELETE FROM card_decks WHERE id = ?1",
        rusqlite::params![id.0],
    )?;
    Ok(())
}

pub fn save_card_to_store(conn: &Connection, card: &Card) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO session_cards (deck_id, session_card_id, sentence, target, explanation, tags, file_id, subtitle_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            card.deck_id.0,
            card.card_id,
            card.sentence,
            card.target,
            card.explanation,
            card.tags.join(","),
            card.file_id,
            card.subtitle_id,
        ],
    )?;
    Ok(())
}

pub fn list_cards_by_deck(conn: &Connection, deck_id: DeckId) -> Result<Vec<Card>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT session_card_id, sentence, target, explanation, tags, file_id, subtitle_id
         FROM session_cards WHERE deck_id = ?1 ORDER BY session_card_id",
    )?;
    let cards = stmt
        .query_map(rusqlite::params![deck_id.0], |row| {
            let session_card_id: u32 = row.get(0)?;
            let sentence: String = row.get(1)?;
            let target: String = row.get(2)?;
            let explanation: String = row.get(3)?;
            let tags_str: String = row.get(4)?;
            let file_id: String = row.get(5)?;
            let subtitle_id: u32 = row.get(6)?;
            let tags: Vec<String> = if tags_str.is_empty() {
                Vec::new()
            } else {
                tags_str.split(',').map(|s| s.to_string()).collect()
            };
            Ok(Card {
                card_id: session_card_id,
                deck_id,
                sentence,
                target,
                explanation,
                tags,
                file_id,
                subtitle_id,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cards)
}

pub fn get_or_create_deck_by_name(conn: &Connection, name: &str) -> Result<DeckId, CoreError> {
    let mut stmt = conn.prepare("SELECT id FROM card_decks WHERE name = ?1")?;
    let existing: Option<i64> = stmt
        .query_map(rusqlite::params![name], |row| row.get(0))?
        .next()
        .transpose()?;
    match existing {
        Some(id) => Ok(DeckId(id)),
        None => create_deck(conn, name),
    }
}

pub fn delete_session_card(conn: &Connection, card_id: u32) -> Result<(), CoreError> {
    conn.execute(
        "DELETE FROM session_cards WHERE session_card_id = ?1",
        rusqlite::params![card_id],
    )?;
    Ok(())
}

pub fn mark_known(conn: &Connection, lemma: &str) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO known_words (lemma, added_at) VALUES (?1, ?2)",
        rusqlite::params![lemma, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn list_known_words(conn: &Connection) -> anyhow::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT lemma FROM known_words ORDER BY lemma")?;
    let lemmas: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lemmas)
}

pub fn add_known_word(conn: &Connection, lemma: &str, known_path: &str) -> anyhow::Result<()> {
    mark_known(conn, lemma)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(known_path)?;
    writeln!(file, "{lemma}")?;
    Ok(())
}

pub fn sync_wordlist(conn: &Connection, path: &str) -> anyhow::Result<()> {
    let mut stmt = conn.prepare("SELECT lemma FROM deck WHERE status = 'new'")?;
    let lemmas: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut file = fs::File::create(path)?;
    for lemma in &lemmas {
        writeln!(file, "{lemma}")?;
    }
    Ok(())
}

pub struct Stats {
    pub total_words: usize,
    pub added_today: usize,
    pub total_known: usize,
}

impl Stats {
    pub fn load(conn: &Connection) -> anyhow::Result<Self> {
        let total_words: i64 = conn
            .query_row("SELECT COUNT(*) FROM deck", [], |row| row.get(0))
            .unwrap_or(0);
        let added_today: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM deck WHERE date(added_at) = date('now')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let total_known: i64 = conn
            .query_row("SELECT COUNT(*) FROM known_words", [], |row| row.get(0))
            .unwrap_or(0);
        Ok(Stats {
            total_words: total_words as usize,
            added_today: added_today as usize,
            total_known: total_known as usize,
        })
    }
}

pub fn load_known_list(conn: &Connection) -> anyhow::Result<Vec<String>> {
    let mut stmt = conn.prepare_cached("SELECT lemma FROM known_words ORDER BY lemma")?;
    let lemmas: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lemmas)
}

pub fn add_to_deck(conn: &Connection, entry: &DeckEntry) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO deck (lemma, surface, meaning, source_sentence, source_file, added_at, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'new')",
        rusqlite::params![
            entry.lemma,
            entry.surface,
            entry.meaning,
            entry.source_sentence,
            entry.source_file,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Tracks known subtitle lines per source file.
///
/// Persists in the `known_lines` SQLite table:
/// - `file_id` — identifier for the source subtitle file
/// - `subtitle_id` — unique subtitle entry index within that file
pub struct KnownLinesStore {
    conn: Connection,
}

impl KnownLinesStore {
    fn init(&self) -> Result<(), CoreError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS known_lines (
                file_id     TEXT NOT NULL,
                subtitle_id INTEGER NOT NULL,
                added_at    TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (file_id, subtitle_id)
            );",
        )?;
        Ok(())
    }

    /// Open a file-backed store at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init()?;
        Ok(store)
    }

    /// Create an in-memory store (useful for testing).
    pub fn in_memory() -> Result<Self, CoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init()?;
        Ok(store)
    }

    /// Mark a subtitle line as known for the given file.
    pub fn mark_known(&self, file_id: &str, subtitle_id: i64) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO known_lines (file_id, subtitle_id) VALUES (?1, ?2)",
            rusqlite::params![file_id, subtitle_id],
        )?;
        Ok(())
    }

    /// Check whether a subtitle line has been marked known.
    pub fn is_known(&self, file_id: &str, subtitle_id: i64) -> Result<bool, CoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM known_lines WHERE file_id = ?1 AND subtitle_id = ?2",
            rusqlite::params![file_id, subtitle_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Return the set of all known subtitle IDs for the given file.
    pub fn known_ids(&self, file_id: &str) -> Result<HashSet<i64>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT subtitle_id FROM known_lines WHERE file_id = ?1")?;
        let ids: HashSet<i64> = stmt
            .query_map(rusqlite::params![file_id], |row| row.get(0))?
            .collect::<Result<HashSet<_>, _>>()?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_returns_zero_for_empty_db() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let stats = Stats::load(&conn).unwrap();
        assert_eq!(stats.total_words, 0);
        assert_eq!(stats.added_today, 0);
        assert_eq!(stats.total_known, 0);
    }

    #[test]
    fn stats_counts_correctly_after_inserts() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let entry = DeckEntry {
            lemma: "먹다".to_string(),
            surface: "먹".to_string(),
            meaning: "to eat".to_string(),
            source_sentence: "나는 밥을 먹는다".to_string(),
            source_file: "test.srt".to_string(),
        };
        add_to_deck(&conn, &entry).unwrap();
        let entry2 = DeckEntry {
            lemma: "가다".to_string(),
            surface: "가".to_string(),
            meaning: "to go".to_string(),
            source_sentence: "집에 간다".to_string(),
            source_file: "test.srt".to_string(),
        };
        add_to_deck(&conn, &entry2).unwrap();
        mark_known(&conn, "보다").unwrap();

        let stats = Stats::load(&conn).unwrap();
        assert_eq!(stats.total_words, 2);
        assert_eq!(stats.total_known, 1);
    }

    #[test]
    fn stats_counts_added_today() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let yesterday = "2000-01-01";

        conn.execute(
            "INSERT INTO deck (lemma, surface, meaning, added_at, status)
             VALUES (?1, '', '', ?2, 'new')",
            rusqlite::params!["오늘단어", format!("{}T12:00:00+00:00", today)],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO deck (lemma, surface, meaning, added_at, status)
             VALUES (?1, '', '', ?2, 'new')",
            rusqlite::params!["어제단어", format!("{}T12:00:00+00:00", yesterday)],
        )
        .unwrap();

        let stats = Stats::load(&conn).unwrap();
        assert_eq!(stats.total_words, 2);
        assert_eq!(stats.added_today, 1);
    }

    #[test]
    fn add_to_deck_persists_entry() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let entry = DeckEntry {
            lemma: "먹다".to_string(),
            surface: "먹".to_string(),
            meaning: "to eat".to_string(),
            source_sentence: "나는 밥을 먹는다".to_string(),
            source_file: "test.srt".to_string(),
        };
        add_to_deck(&conn, &entry).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM deck", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let lemma: String = conn
            .query_row("SELECT lemma FROM deck", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lemma, "먹다");
    }

    #[test]
    fn mark_known_persists_to_db() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        mark_known(&conn, "먹다").unwrap();
        let lemma: String = conn
            .query_row("SELECT lemma FROM known_words", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lemma, "먹다");
    }

    #[test]
    fn known_add_persists() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let known_path = std::env::temp_dir().join("test_known_add.txt");
        // Remove if left over from previous run
        let _ = std::fs::remove_file(&known_path);

        add_known_word(&conn, "먹다", known_path.to_str().unwrap()).unwrap();

        let lemma: String = conn
            .query_row("SELECT lemma FROM known_words", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lemma, "먹다");
        let content = std::fs::read_to_string(&known_path).unwrap();
        assert!(content.lines().any(|l| l == "먹다"));
        std::fs::remove_file(&known_path).unwrap();
    }

    #[test]
    fn known_list_returns_added_words() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        mark_known(&conn, "먹다").unwrap();
        mark_known(&conn, "가다").unwrap();

        let words = list_known_words(&conn).unwrap();
        assert_eq!(words.len(), 2);
        assert!(words.contains(&"먹다".to_string()));
        assert!(words.contains(&"가다".to_string()));
    }

    #[test]
    fn sync_wordlist_appends_lemma() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let entry = DeckEntry {
            lemma: "먹다".to_string(),
            surface: "먹".to_string(),
            meaning: "to eat".to_string(),
            source_sentence: "나는 밥을 먹는다".to_string(),
            source_file: "test.srt".to_string(),
        };
        add_to_deck(&conn, &entry).unwrap();
        let path = std::env::temp_dir().join("test_wordlist.txt");
        sync_wordlist(&conn, path.to_str().unwrap()).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(content.lines().any(|l| l == "먹다"));
    }

    #[test]
    fn add_to_deck_twice_does_not_error() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let entry = DeckEntry {
            lemma: "먹다".to_string(),
            surface: "먹".to_string(),
            meaning: "to eat".to_string(),
            source_sentence: "첫 번째 문장".to_string(),
            source_file: "a.srt".to_string(),
        };
        add_to_deck(&conn, &entry).unwrap();
        let entry2 = DeckEntry {
            lemma: "먹다".to_string(),
            surface: "먹".to_string(),
            meaning: "to eat".to_string(),
            source_sentence: "두 번째 문장".to_string(),
            source_file: "b.srt".to_string(),
        };
        add_to_deck(&conn, &entry2).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM deck", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn load_known_list_returns_alphabetical_order() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        mark_known(&conn, "가다").unwrap();
        mark_known(&conn, "나다").unwrap();
        mark_known(&conn, "가마").unwrap();
        let words = load_known_list(&conn).unwrap();
        assert_eq!(words, vec!["가다", "가마", "나다"]);
    }

    // -----------------------------------------------------------------------
    // KnownLinesStore
    // -----------------------------------------------------------------------

    #[test]
    fn known_lines_store_mark_known_adds_entry() {
        let store = KnownLinesStore::in_memory().unwrap();
        store.mark_known("file-1", 1).unwrap();
        assert!(store.is_known("file-1", 1).unwrap());
    }

    #[test]
    fn known_lines_store_mark_known_twice_does_not_error() {
        let store = KnownLinesStore::in_memory().unwrap();
        store.mark_known("file-1", 1).unwrap();
        store.mark_known("file-1", 1).unwrap();
        assert!(store.is_known("file-1", 1).unwrap());
    }

    #[test]
    fn known_lines_store_is_known_returns_false_for_unknown() {
        let store = KnownLinesStore::in_memory().unwrap();
        assert!(!store.is_known("file-1", 99).unwrap());
    }

    #[test]
    fn known_lines_store_is_known_respects_file_id() {
        let store = KnownLinesStore::in_memory().unwrap();
        store.mark_known("file-a", 1).unwrap();
        assert!(store.is_known("file-a", 1).unwrap());
        assert!(!store.is_known("file-b", 1).unwrap());
    }

    #[test]
    fn known_lines_store_known_ids_returns_expected_set() {
        let store = KnownLinesStore::in_memory().unwrap();
        store.mark_known("file-1", 10).unwrap();
        store.mark_known("file-1", 20).unwrap();
        store.mark_known("file-1", 30).unwrap();
        let ids = store.known_ids("file-1").unwrap();
        let expected: HashSet<i64> = [10, 20, 30].into();
        assert_eq!(ids, expected);
    }

    #[test]
    fn known_lines_store_known_ids_returns_empty_set_when_none() {
        let store = KnownLinesStore::in_memory().unwrap();
        let ids = store.known_ids("file-1").unwrap();
        assert!(ids.is_empty());
    }

    #[test]
    fn known_lines_store_known_ids_only_returns_ids_for_specific_file() {
        let store = KnownLinesStore::in_memory().unwrap();
        store.mark_known("file-a", 1).unwrap();
        store.mark_known("file-a", 2).unwrap();
        store.mark_known("file-b", 1).unwrap();
        let ids_a = store.known_ids("file-a").unwrap();
        let ids_b = store.known_ids("file-b").unwrap();
        assert_eq!(ids_a, [1, 2].into_iter().collect::<HashSet<i64>>());
        assert_eq!(ids_b, [1].into_iter().collect::<HashSet<i64>>());
    }

    #[test]
    fn known_lines_store_known_ids_consistent_after_mark_known() {
        let store = KnownLinesStore::in_memory().unwrap();
        store.mark_known("f", 1).unwrap();
        store.mark_known("f", 2).unwrap();
        let before = store.known_ids("f").unwrap();
        store.mark_known("f", 1).unwrap();
        let after = store.known_ids("f").unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn known_lines_store_can_be_file_backed() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_known_lines.db");
        let _ = std::fs::remove_file(&path);

        {
            let store = KnownLinesStore::open(&path).unwrap();
            store.mark_known("f", 42).unwrap();
        }

        {
            let store = KnownLinesStore::open(&path).unwrap();
            assert!(store.is_known("f", 42).unwrap());
            assert_eq!(
                store.known_ids("f").unwrap(),
                [42].into_iter().collect::<HashSet<i64>>()
            );
        }

        std::fs::remove_file(&path).unwrap();
    }

    // -----------------------------------------------------------------------
    // Deck CRUD
    // -----------------------------------------------------------------------

    #[test]
    fn ensure_default_deck_creates_general_deck_when_empty() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let id = ensure_default_deck(&conn).unwrap();
        assert_eq!(id, DeckId(1));
        let decks = list_decks(&conn).unwrap();
        assert_eq!(decks.len(), 1);
        assert_eq!(decks[0].name, "Korean – General");
    }

    #[test]
    fn ensure_default_deck_returns_existing_deck() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let first = ensure_default_deck(&conn).unwrap();
        let second = ensure_default_deck(&conn).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn create_deck_returns_id_and_persists() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let id = create_deck(&conn, "My Deck").unwrap();
        assert_eq!(id, DeckId(1));
        let deck = get_deck(&conn, id).unwrap().expect("deck should exist");
        assert_eq!(deck.name, "My Deck");
        assert_eq!(deck.id, id);
    }

    #[test]
    fn create_deck_multiple_decks_have_unique_ids() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let id1 = create_deck(&conn, "Deck A").unwrap();
        let id2 = create_deck(&conn, "Deck B").unwrap();
        assert_ne!(id1, id2);
        let decks = list_decks(&conn).unwrap();
        assert_eq!(decks.len(), 2);
    }

    #[test]
    fn list_decks_returns_all_decks() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let _ = create_deck(&conn, "First").unwrap();
        let _ = create_deck(&conn, "Second").unwrap();
        let decks = list_decks(&conn).unwrap();
        assert_eq!(decks.len(), 2);
        assert_eq!(decks[0].name, "First");
        assert_eq!(decks[1].name, "Second");
    }

    #[test]
    fn get_deck_returns_none_for_missing_id() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let result = get_deck(&conn, DeckId(99)).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn rename_deck_updates_name_but_not_id() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let id = create_deck(&conn, "Original").unwrap();
        rename_deck(&conn, id, "Renamed").unwrap();
        let deck = get_deck(&conn, id).unwrap().unwrap();
        assert_eq!(deck.id, id);
        assert_eq!(deck.name, "Renamed");
    }

    #[test]
    fn rename_deck_returns_error_for_missing_deck() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let result = rename_deck(&conn, DeckId(99), "Nope");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CoreError::DeckNotFound(_)));
    }

    #[test]
    fn delete_deck_removes_deck_and_its_cards() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let deck_id = create_deck(&conn, "To Delete").unwrap();

        let card = Card {
            card_id: 1,
            deck_id,
            sentence: "test sentence".into(),
            target: "test".into(),
            explanation: "test expl".into(),
            tags: vec![],
            file_id: "f".into(),
            subtitle_id: 1,
        };
        save_card_to_store(&conn, &card).unwrap();

        let cards = list_cards_by_deck(&conn, deck_id).unwrap();
        assert_eq!(cards.len(), 1);

        delete_deck(&conn, deck_id).unwrap();

        assert!(get_deck(&conn, deck_id).unwrap().is_none());
        let cards = list_cards_by_deck(&conn, deck_id).unwrap();
        assert!(cards.is_empty(), "cards should be deleted with deck");
    }

    #[test]
    fn save_card_to_store_persists_card_fields() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let deck_id = create_deck(&conn, "Test Deck").unwrap();

        let card = Card {
            card_id: 1,
            deck_id,
            sentence: "안녕하세요".into(),
            target: "안녕".into(),
            explanation: "Hello".into(),
            tags: vec!["korean".into(), "greeting".into()],
            file_id: "video-1".into(),
            subtitle_id: 5,
        };
        save_card_to_store(&conn, &card).unwrap();

        let cards = list_cards_by_deck(&conn, deck_id).unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].sentence, "안녕하세요");
        assert_eq!(cards[0].target, "안녕");
        assert_eq!(cards[0].explanation, "Hello");
        assert_eq!(cards[0].tags, vec!["korean", "greeting"]);
        assert_eq!(cards[0].file_id, "video-1");
        assert_eq!(cards[0].subtitle_id, 5);
    }

    #[test]
    fn list_cards_by_deck_only_returns_cards_for_that_deck() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let d1 = create_deck(&conn, "Deck 1").unwrap();
        let d2 = create_deck(&conn, "Deck 2").unwrap();

        save_card_to_store(
            &conn,
            &Card {
                card_id: 1,
                deck_id: d1,
                sentence: "s1".into(),
                target: "t1".into(),
                explanation: "e1".into(),
                tags: vec![],
                file_id: "f".into(),
                subtitle_id: 1,
            },
        )
        .unwrap();
        save_card_to_store(
            &conn,
            &Card {
                card_id: 1,
                deck_id: d2,
                sentence: "s2".into(),
                target: "t2".into(),
                explanation: "e2".into(),
                tags: vec![],
                file_id: "f".into(),
                subtitle_id: 2,
            },
        )
        .unwrap();

        let cards_d1 = list_cards_by_deck(&conn, d1).unwrap();
        assert_eq!(cards_d1.len(), 1);
        assert_eq!(cards_d1[0].target, "t1");

        let cards_d2 = list_cards_by_deck(&conn, d2).unwrap();
        assert_eq!(cards_d2.len(), 1);
        assert_eq!(cards_d2[0].target, "t2");
    }

    #[test]
    fn delete_session_card_removes_single_card() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let deck_id = create_deck(&conn, "Test").unwrap();

        save_card_to_store(
            &conn,
            &Card {
                card_id: 1,
                deck_id,
                sentence: "keep".into(),
                target: "keep".into(),
                explanation: "".into(),
                tags: vec![],
                file_id: "f".into(),
                subtitle_id: 1,
            },
        )
        .unwrap();
        save_card_to_store(
            &conn,
            &Card {
                card_id: 2,
                deck_id,
                sentence: "delete".into(),
                target: "delete".into(),
                explanation: "".into(),
                tags: vec![],
                file_id: "f".into(),
                subtitle_id: 2,
            },
        )
        .unwrap();

        delete_session_card(&conn, 2).unwrap();

        let cards = list_cards_by_deck(&conn, deck_id).unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].target, "keep");
    }
}
