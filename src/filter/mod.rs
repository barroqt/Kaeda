use std::collections::HashSet;

use crate::tokenizer::Token;

pub struct FilterConfig {
    pub frequency_set: HashSet<String>,
    pub known_set: HashSet<String>,
}

/// Removes particles (POS starting with J) and punctuation (SF, SP, SS).
pub fn filter_content_tokens(tokens: Vec<Token>) -> Vec<Token> {
    tokens
        .into_iter()
        .filter(|t| {
            let first = t.pos.as_str();
            let starts_s = first.starts_with('S');
            let is_punct = starts_s && matches!(&first[..], "SF" | "SP" | "SS");
            !(first.starts_with('J') || is_punct)
        })
        .collect()
}

pub fn is_candidate(config: &FilterConfig, lemma: &str) -> bool {
    !config.frequency_set.contains(lemma) && !config.known_set.contains(lemma)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use crate::tokenizer::tokenize;

    #[test]
    fn filter_pos_removes_particles() {
        let tokens = tokenize("나는 밥을 먹었어요.").unwrap();
        let filtered = filter_content_tokens(tokens);
        assert!(filtered.iter().all(|t| !t.pos.starts_with('J')));
        assert!(filtered.iter().all(|t| !matches!(t.pos.as_str(), "SF" | "SP" | "SS")));
        assert!(filtered.len() > 0);
    }

    #[test]
    fn is_candidate_rejects_frequent_word() {
        let mut frequency_set = HashSet::new();
        frequency_set.insert("하다".to_string());
        let config = FilterConfig {
            frequency_set,
            known_set: HashSet::new(),
        };
        let result = is_candidate(&config, "하다");
        assert!(!result);
    }
}
