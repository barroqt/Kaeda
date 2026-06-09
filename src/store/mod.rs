use std::fs;
use std::io::Write;

use chrono::Utc;
use rusqlite::Connection;

pub struct DeckEntry {
    pub lemma: String,
    pub surface: String,
    pub meaning: String,
    pub source_sentence: String,
    pub source_file: String,
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
        );",
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
}
