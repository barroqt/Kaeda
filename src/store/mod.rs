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
    pub total_words: i64,
    pub added_today: i64,
    pub total_known: i64,
}

pub fn get_stats(conn: &Connection) -> anyhow::Result<Stats> {
    let total_words: i64 = conn
        .query_row("SELECT COUNT(*) FROM deck", [], |row| row.get(0))
        .unwrap_or(0);
    let today_prefix = Utc::now().format("%Y-%m-%d").to_string();
    let added_today: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM deck WHERE added_at LIKE ?1",
            rusqlite::params![format!("{}%", today_prefix)],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let total_known: i64 = conn
        .query_row("SELECT COUNT(*) FROM known_words", [], |row| row.get(0))
        .unwrap_or(0);
    Ok(Stats {
        total_words,
        added_today,
        total_known,
    })
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
    fn get_stats_returns_zero_for_empty_db() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let stats = get_stats(&conn).unwrap();
        assert_eq!(stats.total_words, 0);
        assert_eq!(stats.added_today, 0);
        assert_eq!(stats.total_known, 0);
    }

    #[test]
    fn stats_output_contains_total_count() {
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

        let stats = get_stats(&conn).unwrap();
        assert_eq!(stats.total_words, 2);
        assert_eq!(stats.total_known, 1);
    }

    #[test]
    fn get_stats_counts_added_today() {
        let conn = Connection::open_in_memory().unwrap();
        init_store(&conn).unwrap();
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let yesterday = "2000-01-01"; // far in the past

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

        let stats = get_stats(&conn).unwrap();
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
}