pub mod api;
pub mod db;

use rusqlite::Connection;

use crate::subtitle::CoreError;

/// Formats a dictionary entry as `"(POS) definition"` when a part of
/// speech is available, or just the definition text otherwise.
fn format_explanation(entry: &db::DictEntry) -> String {
    if entry.pos.is_empty() || entry.pos == "—" {
        entry.meaning.clone()
    } else {
        format!("({}) {}", entry.pos, entry.meaning)
    }
}

/// Looks up a lemma and returns a human-readable explanation string.
///
/// Queries the Naver dictionary API and formats the result as
/// `"(POS) definition"` when both are available, or just the
/// definition text otherwise.
///
/// # Errors
///
/// Returns `CoreError::Network` if the HTTP request or JSON parsing fails.
pub fn suggest_explanation(lemma: &str) -> Result<Option<String>, CoreError> {
    match api::search_naver(lemma) {
        Ok(Some(entry)) => Ok(Some(format_explanation(&entry))),
        Ok(None) => Ok(None),
        Err(e) => Err(CoreError::Network(e.to_string())),
    }
}

/// Like [`suggest_explanation`], but backed by the persistent SQLite
/// dictionary cache: cached entries are returned without a network
/// request, and successful online lookups are stored for next time.
///
/// Online lookup failures are logged and reported as `Ok(None)`, matching
/// [`db::lookup_or_fetch`].
///
/// # Errors
///
/// Returns an error if the cache database cannot be read.
pub fn suggest_explanation_cached(
    conn: &Connection,
    lemma: &str,
) -> anyhow::Result<Option<String>> {
    Ok(db::lookup_or_fetch(conn, lemma)?.map(|entry| format_explanation(&entry)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn seeded_conn(pos: &str) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::ensure_dict_table(&conn).unwrap();
        db::cache_entry(
            &conn,
            &db::DictEntry {
                lemma: "가짜시험단어".into(),
                meaning: "seeded meaning".into(),
                pos: pos.into(),
                examples: vec![],
            },
        )
        .unwrap();
        conn
    }

    #[test]
    fn cached_lookup_returns_seeded_entry_without_network() {
        let conn = seeded_conn("명사");
        let result = suggest_explanation_cached(&conn, "가짜시험단어").unwrap();
        assert_eq!(result, Some("(명사) seeded meaning".into()));
    }

    #[test]
    fn cached_lookup_formats_plain_when_pos_empty() {
        let conn = seeded_conn("");
        let result = suggest_explanation_cached(&conn, "가짜시험단어").unwrap();
        assert_eq!(result, Some("seeded meaning".into()));
    }

    #[test]
    fn cached_lookup_formats_plain_when_pos_is_dash() {
        let conn = seeded_conn("—");
        let result = suggest_explanation_cached(&conn, "가짜시험단어").unwrap();
        assert_eq!(result, Some("seeded meaning".into()));
    }

    #[test]
    fn suggest_known_word_returns_some() {
        let result = suggest_explanation("사랑").unwrap();
        assert!(result.is_some(), "expected Some for known word '사랑'");
        let explanation = result.unwrap();
        assert!(!explanation.is_empty(), "explanation should not be empty");
        assert!(
            explanation.contains("love"),
            "expected 'love' in explanation for 사랑, got: {explanation}"
        );
    }

    #[test]
    fn suggest_unknown_word_returns_none() {
        let result = suggest_explanation("zzznonsense123").unwrap();
        assert!(result.is_none(), "expected None for nonsense word");
    }

    #[test]
    fn suggest_verb_returns_formatted_explanation() {
        let result = suggest_explanation("먹다").unwrap();
        assert!(result.is_some(), "expected Some for known verb '먹다'");
        let explanation = result.unwrap();
        assert!(
            explanation.starts_with('(') && explanation.contains(')'),
            "expected formatted explanation starting with '(POS)', got: {explanation}"
        );
    }

    #[test]
    fn suggest_empty_lemma_returns_none() {
        let result = suggest_explanation("").unwrap();
        assert!(result.is_none(), "expected None for empty lemma");
    }
}
