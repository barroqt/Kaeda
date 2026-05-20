use std::fs;

use anyhow::Context;
use rusqlite::Connection;

pub struct DictEntry {
    pub lemma: String,
    pub meaning: String,
    pub pos: String,
    pub examples: Vec<String>,
}

pub fn build_index(conn: &Connection, tsv_path: &str) -> anyhow::Result<()> {
    let content = fs::read_to_string(tsv_path).context("failed to read TSV")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS dictionary (
            lemma   TEXT PRIMARY KEY,
            meaning TEXT NOT NULL,
            pos     TEXT,
            examples TEXT
        );",
    )?;

    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO dictionary (lemma, meaning, pos, examples) VALUES (?1, ?2, ?3, ?4)",
    )?;

    for line in content.lines().skip(1) {
        let line = line.trim();
        println!("LINE ===> {}", &line);
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 4 {
            continue;
        }
        println!("COL ===> {:?}", cols);
        let lemma = cols[0];
        let meaning = cols[1];
        let pos = cols[2];
        let example = cols[3];
        let examples = serde_json::to_string(&vec![example])?;
        stmt.execute(rusqlite::params![lemma, meaning, pos, examples])?;
    }

    Ok(())
}

pub fn lookup(conn: &Connection, lemma: &str) -> anyhow::Result<Option<DictEntry>> {
    let mut stmt =
        conn.prepare_cached("SELECT lemma, meaning, pos, examples FROM dictionary WHERE lemma = ?1")?;

    let mut rows = stmt.query(rusqlite::params![lemma])?;
    if let Some(row) = rows.next()? {
        let examples_str: String = row.get(3)?;
        let examples: Vec<String> = serde_json::from_str(&examples_str)?;
        Ok(Some(DictEntry {
            lemma: row.get(0)?,
            meaning: row.get(1)?,
            pos: row.get(2)?,
            examples,
        }))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_lemma() {
        let conn = Connection::open_in_memory().unwrap();
        build_index(&conn, "tests/fixtures/dict_sample.tsv").unwrap();
        let entry = lookup(&conn, "먹다").unwrap();
        assert!(entry.is_some());
    }

    #[test]
    fn lookup_unknown_lemma_returns_none() {
        let conn = Connection::open_in_memory().unwrap();
        build_index(&conn, "tests/fixtures/dict_sample.tsv").unwrap();
        let entry = lookup(&conn, "zzznonsense").unwrap();
        assert!(entry.is_none());
    }

    #[test]
    fn build_index_twice_does_not_error() {
        let conn = Connection::open_in_memory().unwrap();
        build_index(&conn, "tests/fixtures/dict_sample.tsv").unwrap();
        build_index(&conn, "tests/fixtures/dict_sample.tsv").unwrap();
    }
}
