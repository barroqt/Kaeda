//! Expression (multi-token phrase) logic: canonical dictionary forms and
//! lemma sequences over token slices (PRD §5.4 / §6.1).

use crate::tokenizer::Token;

/// A learned expression as stored in the lexicon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionEntry {
    /// Match key: each member token's lemma, in order.
    pub lemma_seq: Vec<String>,
    /// Canonical dictionary form shown to the user (e.g. 마음에 들다).
    pub display_form: String,
}

/// A detected expression occurrence within a subtitle's token list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionSpan {
    /// Index of the first member token (inclusive).
    pub start: usize,
    /// Index of the last member token (inclusive).
    pub end: usize,
    /// Canonical dictionary form of the matched entry.
    pub display_form: String,
}

/// Builds the canonical (dictionary) form of a token slice (PRD §5.4).
///
/// Trailing ending tokens (POS `E*`) are folded into the verb/adjective they
/// attach to: when the slice ends in a `V*` token optionally followed only by
/// endings, that token contributes its lemma (e.g. 들/었/어요 → 들다) and the
/// endings are dropped. Otherwise every token contributes its surface.
/// Whitespace between tokens is reproduced from `line` via byte offsets.
pub fn dictionary_form(line: &str, tokens: &[Token]) -> String {
    let Some(verb_index) = final_verb_index(tokens) else {
        return join_surfaces(line, tokens);
    };
    let mut form = join_surfaces(line, &tokens[..verb_index]);
    if let Some(verb) = tokens.get(verb_index) {
        if let Some(prev) = verb_index.checked_sub(1).and_then(|p| tokens.get(p)) {
            form.push_str(gap(line, prev, verb));
        }
        form.push_str(verb.lemma.as_str());
    }
    form
}

/// Index of the final `V*` token when the slice ends in a verb/adjective
/// optionally followed only by ending (`E*`) tokens; `None` otherwise.
fn final_verb_index(tokens: &[Token]) -> Option<usize> {
    let mut end = tokens.len();
    while end > 0 && tokens[end - 1].pos.starts_with('E') {
        end -= 1;
    }
    let last = end.checked_sub(1)?;
    tokens[last].pos.starts_with('V').then_some(last)
}

/// Original-line text between two adjacent tokens (usually "" or a space).
fn gap<'a>(line: &'a str, prev: &Token, next: &Token) -> &'a str {
    line.get(prev.byte_end..next.byte_start).unwrap_or("")
}

/// Joins token surfaces, reproducing the whitespace between them from `line`.
fn join_surfaces(line: &str, tokens: &[Token]) -> String {
    let mut out = String::new();
    for (i, token) in tokens.iter().enumerate() {
        if let Some(prev) = i.checked_sub(1).and_then(|p| tokens.get(p)) {
            out.push_str(gap(line, prev, token));
        }
        out.push_str(token.surface.as_str());
    }
    out
}

/// Returns each token's lemma in order, folding trailing endings into the
/// final verb/adjective lemma the same way as [`dictionary_form`].
pub fn lemma_sequence(tokens: &[Token]) -> Vec<String> {
    let end = match final_verb_index(tokens) {
        Some(verb_index) => verb_index + 1,
        None => tokens.len(),
    };
    tokens[..end]
        .iter()
        .map(|t| t.lemma.as_str().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::tokenize_korean_line;

    fn tokens_for(line: &str) -> Vec<Token> {
        tokenize_korean_line(line).unwrap()
    }

    #[test]
    fn dictionary_form_lemmatizes_final_verb() {
        let line = "마음에 들어";
        let tokens = tokens_for(line);
        assert_eq!(dictionary_form(line, &tokens), "마음에 들다");
    }

    #[test]
    fn dictionary_form_handles_conjugation() {
        let line = "마음에 들었어요";
        let tokens = tokens_for(line);
        assert_eq!(dictionary_form(line, &tokens), "마음에 들다");
    }

    #[test]
    fn dictionary_form_preserves_interior_spacing() {
        let line = "마음에 들어";
        let tokens = tokens_for(line);
        let form = dictionary_form(line, &tokens);
        assert_eq!(
            form.matches(' ').count(),
            1,
            "expected exactly one interior space in '{form}'"
        );
        assert!(
            form.starts_with("마음에"),
            "expected no space between 마음 and 에 in '{form}'"
        );
        assert!(
            form.ends_with(" 들다"),
            "expected a single space before 들다 in '{form}'"
        );
    }

    #[test]
    fn dictionary_form_noun_only_keeps_surfaces() {
        let line = "어쩔 수 없어";
        let tokens = tokens_for(line);
        // Slice covering 어쩔 + 수 only (the verb 없다 and its ending excluded):
        // the slice does not end in a V* token, so surfaces are kept as-is.
        let slice: Vec<Token> = tokens
            .iter()
            .take_while(|t| !t.pos.starts_with("VA"))
            .cloned()
            .collect();
        assert_eq!(slice.len(), 2, "expected the slice to be 어쩔 + 수");
        assert_eq!(dictionary_form(line, &slice), "어쩔 수");
    }

    #[test]
    fn dictionary_form_verb_only_slice_returns_lemma() {
        let line = "마음에 들었어요";
        let tokens = tokens_for(line);
        // Slice starting at the verb: 들 + 었 + 어요 → lemma alone.
        let verb_start = tokens
            .iter()
            .position(|t| t.pos.starts_with('V'))
            .expect("expected a verb token");
        assert_eq!(dictionary_form(line, &tokens[verb_start..]), "들다");
    }

    #[test]
    fn lemma_sequence_returns_each_token_lemma_in_order() {
        let tokens = tokens_for("마음에 들어");
        assert_eq!(lemma_sequence(&tokens), vec!["마음", "에", "들다"]);
    }
}
