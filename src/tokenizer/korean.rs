use std::fmt;
use std::ops::Deref;

use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;

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

#[derive(Debug, Clone)]
pub struct Token {
    pub surface: Surface,
    pub lemma: Lemma,
    pub pos: Pos,
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
                }
            })
            .collect();

        Ok(result)
    }
}

pub fn tokenize(text: &str) -> anyhow::Result<Vec<Token>> {
    let dictionary = lindera_ko_dic::embedded::load()?;
    let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
    let tokenizer = Tokenizer::new(segmenter);
    let mut tokens = tokenizer.tokenize(text)?;

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
            }
        })
        .collect();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_basic_sentence() {
        let tokens = tokenize("나는 밥을 먹었어요").unwrap();
        assert!(tokens.len() >= 3);
    }

    #[test]
    fn tokenize_extracts_lemma() {
        let tokens = tokenize("먹었어요").unwrap();
        let token = tokens.iter().find(|t| t.pos.starts_with("VV")).unwrap();
        assert_eq!(token.lemma.as_str(), "먹다");
    }

    #[test]
    fn tokenize_empty_string() {
        let tokens = tokenize("").unwrap();
        assert!(tokens.is_empty());
    }
}
