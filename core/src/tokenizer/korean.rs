use std::fmt;
use std::ops::Deref;

use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;

use crate::subtitle::CoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surface(String);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lemma(String);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pos(String);

macro_rules! impl_str_newtype {
    ($name:ident) => {
        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl Deref for $name {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }
        impl<T: Into<String>> From<T> for $name {
            fn from(s: T) -> Self {
                $name(s.into())
            }
        }
        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

impl_str_newtype!(Surface);
impl_str_newtype!(Lemma);
impl_str_newtype!(Pos);

/// A single token produced by tokenizing a Korean sentence.
///
/// Each token represents a meaningful unit (morpheme) and carries its
/// surface form, lemma, POS tag, and byte-offset position within the
/// original sentence.
#[derive(Debug, Clone)]
pub struct Token {
    pub surface: Surface,
    pub lemma: Lemma,
    pub pos: Pos,
    /// Start byte offset in the original sentence.
    pub byte_start: usize,
    /// End byte offset (exclusive) in the original sentence.
    pub byte_end: usize,
}

fn verb_lemma(surface: &str, pos: &str, details: &[&str]) -> String {
    if pos.starts_with('V') {
        // Try to extract the verb stem from the expression field (details[7]).
        // The expression field has format: stem/POS/*+ending/POS/*
        // For irregular verbs this gives the correct stem (e.g. 걸→걷, not 걸).
        if details.len() > 7 {
            let expression = details[7];
            if expression != "*"
                && let Some(stem) = expression.split('/').next()
                && !stem.is_empty()
            {
                return format!("{}다", stem);
            }
        }
        // Fallback: surface + 다 heuristic for regular verbs
        format!("{}다", surface)
    } else {
        surface.to_string()
    }
}

pub struct KoreanTokenizer {
    tokenizer: Tokenizer,
}

impl KoreanTokenizer {
    pub fn new() -> anyhow::Result<Self> {
        let dictionary = lindera_ko_dic::embedded::load()?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        let tokenizer = Tokenizer::new(segmenter);
        Ok(KoreanTokenizer { tokenizer })
    }

    pub fn tokenize(&self, text: &str) -> anyhow::Result<Vec<Token>> {
        let mut tokens = self.tokenizer.tokenize(text)?;

        let result = tokens
            .iter_mut()
            .map(|t| {
                let surface: Surface = t.surface.to_string().into();
                let details = t.details();
                let pos: Pos = details.first().unwrap_or(&"UNKNOWN").to_string().into();
                let lemma = Lemma(verb_lemma(surface.as_str(), pos.as_str(), &details));
                Token {
                    surface,
                    lemma,
                    pos,
                    byte_start: t.byte_start,
                    byte_end: t.byte_end,
                }
            })
            .collect();

        Ok(result)
    }
}

/// Convenience function that tokenizes a single Korean line.
///
/// Creates a `KoreanTokenizer` internally, tokenizes the line, and returns
/// a `Vec<Token>` with surface, lemma, POS, and byte-position information.
///
/// # Errors
///
/// Returns `CoreError::Tokenize` if the Lindera dictionary cannot be loaded
/// or if tokenization itself fails.
pub fn tokenize_korean_line(line: &str) -> Result<Vec<Token>, CoreError> {
    let tokenizer = KoreanTokenizer::new().map_err(|e| CoreError::Tokenize(e.to_string()))?;
    tokenizer
        .tokenize(line)
        .map_err(|e| CoreError::Tokenize(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tokenizer() -> KoreanTokenizer {
        KoreanTokenizer::new().unwrap()
    }

    #[test]
    fn tokenize_basic_sentence() {
        let tokens = test_tokenizer().tokenize("나는 밥을 먹었어요").unwrap();
        assert!(tokens.len() >= 3);
    }

    #[test]
    fn tokenize_extracts_lemma() {
        let tokens = test_tokenizer().tokenize("먹었어요").unwrap();
        let token = tokens.iter().find(|t| t.pos.starts_with("VV")).unwrap();
        assert_eq!(token.lemma.as_str(), "먹다");
    }

    #[test]
    fn tokenize_empty_string() {
        let tokens = test_tokenizer().tokenize("").unwrap();
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokenize_records_byte_positions() {
        let text = "나는 밥을 먹었어요";
        let tokens = test_tokenizer().tokenize(text).unwrap();
        for token in &tokens {
            assert!(
                token.byte_start < token.byte_end,
                "token '{}' has invalid byte range {}..{}",
                token.surface,
                token.byte_start,
                token.byte_end,
            );
            assert!(
                token.byte_end <= text.len(),
                "token '{}' byte_end {} exceeds text length {}",
                token.surface,
                token.byte_end,
                text.len(),
            );
            assert_eq!(
                &text[token.byte_start..token.byte_end],
                token.surface.as_str(),
                "byte range {}..{} does not match surface '{}'",
                token.byte_start,
                token.byte_end,
                token.surface,
            );
        }
    }

    #[test]
    fn tokenize_korean_line_convenience_function() {
        let tokens = tokenize_korean_line("안녕하세요").unwrap();
        assert!(!tokens.is_empty());
    }

    #[test]
    fn tokenize_korean_line_returns_core_error() {
        let result = tokenize_korean_line("테스트");
        assert!(result.is_ok());
    }

    #[test]
    fn tokenize_complex_sentence_with_various_pos() {
        let tokens = tokenize_korean_line("한국어를 공부하고 있습니다").unwrap();
        assert!(tokens.len() >= 4);

        let surfaces: Vec<&str> = tokens.iter().map(|t| t.surface.as_str()).collect();
        assert!(surfaces.contains(&"한국어"));
        assert!(surfaces.contains(&"공부"));

        let verb_tokens: Vec<&Token> = tokens.iter().filter(|t| t.pos.starts_with('V')).collect();
        assert!(!verb_tokens.is_empty(), "expected at least one verb token");
    }

    #[test]
    fn tokenize_adjective_with_copula() {
        let tokens = tokenize_korean_line("그 사람은 정말 좋은 사람이에요").unwrap();
        assert!(tokens.len() >= 5);

        let adjective = tokens.iter().find(|t| t.pos.starts_with("VA"));
        assert!(adjective.is_some(), "expected an adjective (VA) token",);
        if let Some(adj) = adjective {
            assert_eq!(adj.lemma.as_str(), "좋다");
        }
    }

    #[test]
    fn tokenize_handles_particles() {
        let tokens = tokenize_korean_line("사과가 맛있어요").unwrap();
        let particle = tokens.iter().find(|t| t.pos.as_str() == "JKS");
        assert!(
            particle.is_some(),
            "expected a subject particle (JKS) token",
        );
        if let Some(p) = particle {
            assert_eq!(p.surface.as_str(), "가");
        }
    }
}
