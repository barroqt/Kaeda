use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;

#[derive(Debug, Clone)]
pub struct Token {
    pub surface: String,
    pub lemma: String,
    pub pos: String,
}

fn verb_lemma(surface: &str, pos: &str) -> String {
    if pos.starts_with('V') {
        format!("{}다", surface)
    } else {
        surface.to_string()
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
            let surface = t.surface.to_string();
            let details = t.details();
            let pos = details.first().unwrap_or(&"UNKNOWN").to_string();
            let lemma = verb_lemma(&surface, &pos);
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
        assert_eq!(token.lemma, "먹다");
    }

    #[test]
    fn tokenize_empty_string() {
        let tokens = tokenize("").unwrap();
        assert!(tokens.is_empty());
    }
}
