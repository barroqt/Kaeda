use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use chrono::Utc;
use rusqlite::Connection;

use crate::deck::{Deck, DeckId};
use crate::expression::ExpressionEntry;
use crate::session::Card;
use crate::subtitle::CoreError;

pub struct DeckEntry {
    pub lemma: String,
    pub surface: String,
    pub meaning: String,
    pub source_sentence: String,
    pub source_file: String,
}

pub struct Stats {
    pub total_words: usize,
    pub added_today: usize,
    pub total_known: usize,
}

/// A learned expression as persisted in the `expressions` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredExpression {
    pub id: i64,
    /// Match key: each member token's lemma, in order.
    pub lemma_seq: Vec<String>,
    /// Canonical dictionary form shown to the user (e.g. 마음에 들다).
    pub display_form: String,
    /// RFC 3339 timestamp of when the expression was learned.
    pub added_at: String,
}

impl StoredExpression {
    /// The detection-facing view of this row (see
    /// [`crate::expression::detect_expressions`]).
    pub fn to_entry(&self) -> ExpressionEntry {
        ExpressionEntry {
            lemma_seq: self.lemma_seq.clone(),
            display_form: self.display_form.clone(),
        }
    }
}

/// Separator used to persist a lemma sequence as a single TEXT column.
/// Lemmas cannot contain `|`, so the join is unambiguous.
const LEMMA_SEQ_SEPARATOR: &str = "|";

/// Persistent store for mined vocabulary, known words, card decks and
/// session cards.
///
/// Owns its SQLite connection; the schema is created (and migrated) by the
/// constructors, so a `Store` is always ready to use.
pub struct Store {
    conn: Connection,
}

impl Store {
    fn init(&self) -> Result<(), CoreError> {
        self.conn.execute_batch(
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
            );
            CREATE TABLE IF NOT EXISTS expressions (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                lemma_seq    TEXT NOT NULL UNIQUE,
                display_form TEXT NOT NULL,
                added_at     TEXT NOT NULL
            );",
        )?;
        self.migrate_card_decks()?;
        Ok(())
    }

    /// Drop the legacy UNIQUE constraint on `card_decks.name` by rebuilding
    /// the table when an old schema is detected.
    fn migrate_card_decks(&self) -> Result<(), CoreError> {
        let schema: String = self
            .conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='card_decks'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();
        if schema.to_uppercase().contains("UNIQUE") {
            self.conn.execute_batch(
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

    /// Access the underlying connection, for modules that keep their own
    /// tables in the same database file (e.g. the dictionary cache).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    // -- card decks ---------------------------------------------------------

    pub fn deck_count(&self) -> Result<i64, CoreError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM card_decks", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Ensure at least one card deck exists. If the `card_decks` table is
    /// empty, inserts a default deck named `"Korean – General"`. Returns the
    /// ID of the first deck (existing or newly created).
    pub fn ensure_default_deck(&self) -> Result<DeckId, CoreError> {
        if self.deck_count()? == 0 {
            self.conn.execute(
                "INSERT INTO card_decks (name) VALUES ('Korean – General')",
                [],
            )?;
        }
        let id: i64 =
            self.conn
                .query_row("SELECT id FROM card_decks ORDER BY id LIMIT 1", [], |row| {
                    row.get(0)
                })?;
        Ok(DeckId(id))
    }

    pub fn create_deck(&self, name: &str) -> Result<DeckId, CoreError> {
        self.conn.execute(
            "INSERT INTO card_decks (name) VALUES (?1)",
            rusqlite::params![name],
        )?;
        Ok(DeckId(self.conn.last_insert_rowid()))
    }

    pub fn list_decks(&self) -> Result<Vec<Deck>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM card_decks ORDER BY id")?;
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

    pub fn get_deck(&self, id: DeckId) -> Result<Option<Deck>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM card_decks WHERE id = ?1")?;
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

    pub fn rename_deck(&self, id: DeckId, name: &str) -> Result<(), CoreError> {
        let affected = self.conn.execute(
            "UPDATE card_decks SET name = ?1 WHERE id = ?2",
            rusqlite::params![name, id.0],
        )?;
        if affected == 0 {
            return Err(CoreError::DeckNotFound(id));
        }
        Ok(())
    }

    pub fn delete_deck(&self, id: DeckId) -> Result<(), CoreError> {
        self.conn.execute(
            "DELETE FROM session_cards WHERE deck_id = ?1",
            rusqlite::params![id.0],
        )?;
        self.conn.execute(
            "DELETE FROM card_decks WHERE id = ?1",
            rusqlite::params![id.0],
        )?;
        Ok(())
    }

    pub fn get_or_create_deck_by_name(&self, name: &str) -> Result<DeckId, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM card_decks WHERE name = ?1")?;
        let existing: Option<i64> = stmt
            .query_map(rusqlite::params![name], |row| row.get(0))?
            .next()
            .transpose()?;
        match existing {
            Some(id) => Ok(DeckId(id)),
            None => self.create_deck(name),
        }
    }

    // -- session cards ------------------------------------------------------

    pub fn save_card(&self, card: &Card) -> Result<(), CoreError> {
        self.conn.execute(
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

    pub fn list_cards_by_deck(&self, deck_id: DeckId) -> Result<Vec<Card>, CoreError> {
        let mut stmt = self.conn.prepare(
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

    pub fn delete_session_card(&self, card_id: u32) -> Result<(), CoreError> {
        self.conn.execute(
            "DELETE FROM session_cards WHERE session_card_id = ?1",
            rusqlite::params![card_id],
        )?;
        Ok(())
    }

    // -- known words --------------------------------------------------------

    pub fn mark_known(&self, lemma: &str) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO known_words (lemma, added_at) VALUES (?1, ?2)",
            rusqlite::params![lemma, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn list_known_words(&self) -> Result<Vec<String>, CoreError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT lemma FROM known_words ORDER BY lemma")?;
        let lemmas: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(lemmas)
    }

    /// Mark `lemma` as known and append it to the plain-text word list at
    /// `known_path`.
    pub fn add_known_word(&self, lemma: &str, known_path: &str) -> Result<(), CoreError> {
        self.mark_known(lemma)?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(known_path)
            .map_err(CoreError::WordList)?;
        writeln!(file, "{lemma}").map_err(CoreError::WordList)?;
        Ok(())
    }

    // -- learned expressions ------------------------------------------------

    /// Add an expression to the personal lexicon. Idempotent: re-adding an
    /// existing lemma sequence is a no-op. An empty lemma sequence is
    /// ignored (nothing to match against).
    pub fn add_expression(
        &self,
        lemma_seq: &[String],
        display_form: &str,
    ) -> Result<(), CoreError> {
        if lemma_seq.is_empty() {
            return Ok(());
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO expressions (lemma_seq, display_form, added_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![
                lemma_seq.join(LEMMA_SEQ_SEPARATOR),
                display_form,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_expressions(&self) -> Result<Vec<StoredExpression>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, lemma_seq, display_form, added_at FROM expressions ORDER BY id")?;
        let expressions = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let lemma_seq: String = row.get(1)?;
                let display_form: String = row.get(2)?;
                let added_at: String = row.get(3)?;
                Ok(StoredExpression {
                    id,
                    lemma_seq: lemma_seq
                        .split(LEMMA_SEQ_SEPARATOR)
                        .map(|s| s.to_string())
                        .collect(),
                    display_form,
                    added_at,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(expressions)
    }

    pub fn delete_expression(&self, id: i64) -> Result<(), CoreError> {
        let affected = self.conn.execute(
            "DELETE FROM expressions WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if affected == 0 {
            return Err(CoreError::ExpressionNotFound(id));
        }
        Ok(())
    }

    // -- mined vocabulary ---------------------------------------------------

    pub fn add_to_deck(&self, entry: &DeckEntry) -> Result<(), CoreError> {
        self.conn.execute(
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

    /// Write every lemma with status `'new'` to the plain-text word list at
    /// `path`, replacing its contents.
    pub fn sync_wordlist(&self, path: &str) -> Result<(), CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT lemma FROM deck WHERE status = 'new'")?;
        let lemmas: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut file = fs::File::create(path).map_err(CoreError::WordList)?;
        for lemma in &lemmas {
            writeln!(file, "{lemma}").map_err(CoreError::WordList)?;
        }
        Ok(())
    }

    pub fn stats(&self) -> Result<Stats, CoreError> {
        let total_words: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM deck", [], |row| row.get(0))?;
        let added_today: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM deck WHERE date(added_at) = date('now')",
            [],
            |row| row.get(0),
        )?;
        let total_known: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM known_words", [], |row| row.get(0))?;
        Ok(Stats {
            total_words: total_words as usize,
            added_today: added_today as usize,
            total_known: total_known as usize,
        })
    }
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

    fn test_store() -> Store {
        Store::in_memory().expect("open in-memory store")
    }

    #[test]
    fn stats_returns_zero_for_empty_db() {
        let store = test_store();
        let stats = store.stats().unwrap();
        assert_eq!(stats.total_words, 0);
        assert_eq!(stats.added_today, 0);
        assert_eq!(stats.total_known, 0);
    }

    #[test]
    fn stats_counts_correctly_after_inserts() {
        let store = test_store();
        let entry = DeckEntry {
            lemma: "먹다".to_string(),
            surface: "먹".to_string(),
            meaning: "to eat".to_string(),
            source_sentence: "나는 밥을 먹는다".to_string(),
            source_file: "test.srt".to_string(),
        };
        store.add_to_deck(&entry).unwrap();
        let entry2 = DeckEntry {
            lemma: "가다".to_string(),
            surface: "가".to_string(),
            meaning: "to go".to_string(),
            source_sentence: "집에 간다".to_string(),
            source_file: "test.srt".to_string(),
        };
        store.add_to_deck(&entry2).unwrap();
        store.mark_known("보다").unwrap();

        let stats = store.stats().unwrap();
        assert_eq!(stats.total_words, 2);
        assert_eq!(stats.total_known, 1);
    }

    #[test]
    fn stats_counts_added_today() {
        let store = test_store();
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let yesterday = "2000-01-01";

        store
            .connection()
            .execute(
                "INSERT INTO deck (lemma, surface, meaning, added_at, status)
                 VALUES (?1, '', '', ?2, 'new')",
                rusqlite::params!["오늘단어", format!("{}T12:00:00+00:00", today)],
            )
            .unwrap();
        store
            .connection()
            .execute(
                "INSERT INTO deck (lemma, surface, meaning, added_at, status)
                 VALUES (?1, '', '', ?2, 'new')",
                rusqlite::params!["어제단어", format!("{}T12:00:00+00:00", yesterday)],
            )
            .unwrap();

        let stats = store.stats().unwrap();
        assert_eq!(stats.total_words, 2);
        assert_eq!(stats.added_today, 1);
    }

    #[test]
    fn add_to_deck_persists_entry() {
        let store = test_store();
        let entry = DeckEntry {
            lemma: "먹다".to_string(),
            surface: "먹".to_string(),
            meaning: "to eat".to_string(),
            source_sentence: "나는 밥을 먹는다".to_string(),
            source_file: "test.srt".to_string(),
        };
        store.add_to_deck(&entry).unwrap();
        let count: i64 = store
            .connection()
            .query_row("SELECT COUNT(*) FROM deck", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let lemma: String = store
            .connection()
            .query_row("SELECT lemma FROM deck", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lemma, "먹다");
    }

    #[test]
    fn mark_known_persists_to_db() {
        let store = test_store();
        store.mark_known("먹다").unwrap();
        let lemma: String = store
            .connection()
            .query_row("SELECT lemma FROM known_words", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lemma, "먹다");
    }

    #[test]
    fn known_add_persists() {
        let store = test_store();
        let known_path = std::env::temp_dir().join("test_known_add.txt");
        // Remove if left over from previous run
        let _ = std::fs::remove_file(&known_path);

        store
            .add_known_word("먹다", known_path.to_str().unwrap())
            .unwrap();

        let lemma: String = store
            .connection()
            .query_row("SELECT lemma FROM known_words", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lemma, "먹다");
        let content = std::fs::read_to_string(&known_path).unwrap();
        assert!(content.lines().any(|l| l == "먹다"));
        std::fs::remove_file(&known_path).unwrap();
    }

    #[test]
    fn known_list_returns_added_words() {
        let store = test_store();
        store.mark_known("먹다").unwrap();
        store.mark_known("가다").unwrap();

        let words = store.list_known_words().unwrap();
        assert_eq!(words.len(), 2);
        assert!(words.contains(&"먹다".to_string()));
        assert!(words.contains(&"가다".to_string()));
    }

    #[test]
    fn sync_wordlist_appends_lemma() {
        let store = test_store();
        let entry = DeckEntry {
            lemma: "먹다".to_string(),
            surface: "먹".to_string(),
            meaning: "to eat".to_string(),
            source_sentence: "나는 밥을 먹는다".to_string(),
            source_file: "test.srt".to_string(),
        };
        store.add_to_deck(&entry).unwrap();
        let path = std::env::temp_dir().join("test_wordlist.txt");
        store.sync_wordlist(path.to_str().unwrap()).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(content.lines().any(|l| l == "먹다"));
    }

    #[test]
    fn add_to_deck_twice_does_not_error() {
        let store = test_store();
        let entry = DeckEntry {
            lemma: "먹다".to_string(),
            surface: "먹".to_string(),
            meaning: "to eat".to_string(),
            source_sentence: "첫 번째 문장".to_string(),
            source_file: "a.srt".to_string(),
        };
        store.add_to_deck(&entry).unwrap();
        let entry2 = DeckEntry {
            lemma: "먹다".to_string(),
            surface: "먹".to_string(),
            meaning: "to eat".to_string(),
            source_sentence: "두 번째 문장".to_string(),
            source_file: "b.srt".to_string(),
        };
        store.add_to_deck(&entry2).unwrap();
        let count: i64 = store
            .connection()
            .query_row("SELECT COUNT(*) FROM deck", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn list_known_words_returns_alphabetical_order() {
        let store = test_store();
        store.mark_known("가다").unwrap();
        store.mark_known("나다").unwrap();
        store.mark_known("가마").unwrap();
        let words = store.list_known_words().unwrap();
        assert_eq!(words, vec!["가다", "가마", "나다"]);
    }

    #[test]
    fn store_can_be_file_backed() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_store_file_backed.db");
        let _ = std::fs::remove_file(&path);

        {
            let store = Store::open(&path).unwrap();
            store.mark_known("먹다").unwrap();
        }

        {
            let store = Store::open(&path).unwrap();
            assert_eq!(store.list_known_words().unwrap(), vec!["먹다"]);
        }

        std::fs::remove_file(&path).unwrap();
    }

    // -----------------------------------------------------------------------
    // Expressions lexicon
    // -----------------------------------------------------------------------

    fn sample_lemma_seq() -> Vec<String> {
        vec!["마음".to_string(), "에".to_string(), "들다".to_string()]
    }

    #[test]
    fn add_and_list_expressions_roundtrip() {
        let store = test_store();
        store
            .add_expression(&sample_lemma_seq(), "마음에 들다")
            .unwrap();

        let entries = store.list_expressions().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].lemma_seq, sample_lemma_seq());
        assert_eq!(entries[0].display_form, "마음에 들다");
        assert!(!entries[0].added_at.is_empty());
    }

    #[test]
    fn add_expression_is_idempotent() {
        let store = test_store();
        store
            .add_expression(&sample_lemma_seq(), "마음에 들다")
            .unwrap();
        store
            .add_expression(&sample_lemma_seq(), "마음에 들다")
            .unwrap();

        let entries = store.list_expressions().unwrap();
        assert_eq!(entries.len(), 1, "duplicate insert must not add a row");
    }

    #[test]
    fn delete_expression_removes_entry() {
        let store = test_store();
        store
            .add_expression(&sample_lemma_seq(), "마음에 들다")
            .unwrap();
        let id = store.list_expressions().unwrap()[0].id;

        store.delete_expression(id).unwrap();

        assert!(store.list_expressions().unwrap().is_empty());
    }

    #[test]
    fn delete_expression_missing_id_errors() {
        let store = test_store();
        let result = store.delete_expression(999);
        assert!(matches!(
            result.unwrap_err(),
            CoreError::ExpressionNotFound(999)
        ));
    }

    #[test]
    fn expressions_persist_across_reopen() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_expressions_persist.db");
        let _ = std::fs::remove_file(&path);

        {
            let store = Store::open(&path).unwrap();
            store
                .add_expression(&sample_lemma_seq(), "마음에 들다")
                .unwrap();
        }

        {
            let store = Store::open(&path).unwrap();
            let entries = store.list_expressions().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].lemma_seq, sample_lemma_seq());
            assert_eq!(entries[0].display_form, "마음에 들다");
        }

        std::fs::remove_file(&path).unwrap();
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
        let store = test_store();
        let id = store.ensure_default_deck().unwrap();
        assert_eq!(id, DeckId(1));
        let decks = store.list_decks().unwrap();
        assert_eq!(decks.len(), 1);
        assert_eq!(decks[0].name, "Korean – General");
    }

    #[test]
    fn ensure_default_deck_returns_existing_deck() {
        let store = test_store();
        let first = store.ensure_default_deck().unwrap();
        let second = store.ensure_default_deck().unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn create_deck_returns_id_and_persists() {
        let store = test_store();
        let id = store.create_deck("My Deck").unwrap();
        assert_eq!(id, DeckId(1));
        let deck = store.get_deck(id).unwrap().expect("deck should exist");
        assert_eq!(deck.name, "My Deck");
        assert_eq!(deck.id, id);
    }

    #[test]
    fn create_deck_multiple_decks_have_unique_ids() {
        let store = test_store();
        let id1 = store.create_deck("Deck A").unwrap();
        let id2 = store.create_deck("Deck B").unwrap();
        assert_ne!(id1, id2);
        let decks = store.list_decks().unwrap();
        assert_eq!(decks.len(), 2);
    }

    #[test]
    fn list_decks_returns_all_decks() {
        let store = test_store();
        let _ = store.create_deck("First").unwrap();
        let _ = store.create_deck("Second").unwrap();
        let decks = store.list_decks().unwrap();
        assert_eq!(decks.len(), 2);
        assert_eq!(decks[0].name, "First");
        assert_eq!(decks[1].name, "Second");
    }

    #[test]
    fn get_deck_returns_none_for_missing_id() {
        let store = test_store();
        let result = store.get_deck(DeckId(99)).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn rename_deck_updates_name_but_not_id() {
        let store = test_store();
        let id = store.create_deck("Original").unwrap();
        store.rename_deck(id, "Renamed").unwrap();
        let deck = store.get_deck(id).unwrap().unwrap();
        assert_eq!(deck.id, id);
        assert_eq!(deck.name, "Renamed");
    }

    #[test]
    fn rename_deck_returns_error_for_missing_deck() {
        let store = test_store();
        let result = store.rename_deck(DeckId(99), "Nope");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CoreError::DeckNotFound(_)));
    }

    #[test]
    fn delete_deck_removes_deck_and_its_cards() {
        let store = test_store();
        let deck_id = store.create_deck("To Delete").unwrap();

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
        store.save_card(&card).unwrap();

        let cards = store.list_cards_by_deck(deck_id).unwrap();
        assert_eq!(cards.len(), 1);

        store.delete_deck(deck_id).unwrap();

        assert!(store.get_deck(deck_id).unwrap().is_none());
        let cards = store.list_cards_by_deck(deck_id).unwrap();
        assert!(cards.is_empty(), "cards should be deleted with deck");
    }

    #[test]
    fn save_card_persists_card_fields() {
        let store = test_store();
        let deck_id = store.create_deck("Test Deck").unwrap();

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
        store.save_card(&card).unwrap();

        let cards = store.list_cards_by_deck(deck_id).unwrap();
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
        let store = test_store();
        let d1 = store.create_deck("Deck 1").unwrap();
        let d2 = store.create_deck("Deck 2").unwrap();

        store
            .save_card(&Card {
                card_id: 1,
                deck_id: d1,
                sentence: "s1".into(),
                target: "t1".into(),
                explanation: "e1".into(),
                tags: vec![],
                file_id: "f".into(),
                subtitle_id: 1,
            })
            .unwrap();
        store
            .save_card(&Card {
                card_id: 1,
                deck_id: d2,
                sentence: "s2".into(),
                target: "t2".into(),
                explanation: "e2".into(),
                tags: vec![],
                file_id: "f".into(),
                subtitle_id: 2,
            })
            .unwrap();

        let cards_d1 = store.list_cards_by_deck(d1).unwrap();
        assert_eq!(cards_d1.len(), 1);
        assert_eq!(cards_d1[0].target, "t1");

        let cards_d2 = store.list_cards_by_deck(d2).unwrap();
        assert_eq!(cards_d2.len(), 1);
        assert_eq!(cards_d2[0].target, "t2");
    }

    #[test]
    fn delete_session_card_removes_single_card() {
        let store = test_store();
        let deck_id = store.create_deck("Test").unwrap();

        store
            .save_card(&Card {
                card_id: 1,
                deck_id,
                sentence: "keep".into(),
                target: "keep".into(),
                explanation: "".into(),
                tags: vec![],
                file_id: "f".into(),
                subtitle_id: 1,
            })
            .unwrap();
        store
            .save_card(&Card {
                card_id: 2,
                deck_id,
                sentence: "delete".into(),
                target: "delete".into(),
                explanation: "".into(),
                tags: vec![],
                file_id: "f".into(),
                subtitle_id: 2,
            })
            .unwrap();

        store.delete_session_card(2).unwrap();

        let cards = store.list_cards_by_deck(deck_id).unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].target, "keep");
    }
}
