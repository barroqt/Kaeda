pub mod api;
pub mod db;

use crate::subtitle::CoreError;

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
        Ok(Some(entry)) => {
            let explanation = if entry.pos.is_empty() || entry.pos == "—" {
                entry.meaning
            } else {
                format!("({}) {}", entry.pos, entry.meaning)
            };
            Ok(Some(explanation))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(CoreError::Network(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
