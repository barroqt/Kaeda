use std::collections::HashSet;
use std::fs;

use anyhow::Context;

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
            let is_punct = starts_s && matches!(first, "SF" | "SP" | "SS");
            !(first.starts_with('J') || is_punct)
        })
        .collect()
}

pub fn is_candidate(config: &FilterConfig, lemma: &str) -> bool {
    !config.frequency_set.contains(lemma) && !config.known_set.contains(lemma)
}

pub fn load_known_set(path: &str) -> anyhow::Result<HashSet<String>> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashSet::new()),
        Err(e) => return Err(e).context("failed to read known list"),
    };
    let mut set = HashSet::new();
    for line in content.lines() {
        let line = line.trim();
        if !line.is_empty() {
            set.insert(line.to_string());
        }
    }
    Ok(set)
}

pub fn load_frequency_list(path: &str) -> anyhow::Result<HashSet<String>> {
    let content = fs::read_to_string(path).context("failed to read frequency list")?;
    let mut set = HashSet::new();
    for line in content.lines() {
        let line = line.trim();
        if !line.is_empty() {
            set.insert(line.to_string());
        }
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use crate::tokenizer::KoreanTokenizer;

    #[test]
    fn filter_pos_removes_particles() {
        let tokenizer = KoreanTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("나는 밥을 먹었어요.").unwrap();
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

    #[test]
    fn is_candidate_passes_rare_word() {
        let config = FilterConfig {
            frequency_set: HashSet::new(),
            known_set: HashSet::new(),
        };
        let result = is_candidate(&config, "먹다");
        assert!(result);
    }

    #[test]
    fn is_candidate_rejects_known_word() {
        let mut known_set = HashSet::new();
        known_set.insert("먹다".to_string());
        let config = FilterConfig {
            frequency_set: HashSet::new(),
            known_set,
        };
        let result = is_candidate(&config, "먹다");
        assert!(!result);
    }

    #[test]
    fn load_frequency_list_from_file() {
        let set = load_frequency_list("tests/fixtures/frequency_top10.txt").unwrap();
        assert!(set.contains("하다"));
    }

    #[test]
    fn load_known_set_from_file() {
        let path = std::env::temp_dir().join("test_known_list.txt");
        std::fs::write(&path, "먹다\n보다\n가다\n").unwrap();
        let set = load_known_set(path.to_str().unwrap()).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(set.contains("먹다"));
        assert!(set.contains("보다"));
        assert!(set.contains("가다"));
        assert_eq!(set.len(), 3);
    }
}
