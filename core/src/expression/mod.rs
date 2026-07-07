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

/// Detects lexicon expressions in a token list via contiguous
/// lemma-sequence matching (PRD §5.5).
///
/// Scans left to right; at each index the longest matching entry wins and
/// its tokens are consumed, so spans never overlap and are returned sorted
/// by position. When the last lemma-matched token is a verb/adjective
/// (`V*`), the span extends over the ending (`E*`) tokens attached to it,
/// so a match on 마음|에|들다 in 마음에 들었어요 covers 었/어요 too —
/// mirroring how [`lemma_sequence`] folds endings when the entry is built.
pub fn detect_expressions(tokens: &[Token], lexicon: &[ExpressionEntry]) -> Vec<ExpressionSpan> {
    let mut entries: Vec<&ExpressionEntry> = lexicon
        .iter()
        .filter(|entry| !entry.lemma_seq.is_empty())
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.lemma_seq.len()));

    let mut spans = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let matched = entries.iter().find_map(|entry| {
            let candidate = tokens.get(index..index + entry.lemma_seq.len())?;
            let lemmas_match = entry
                .lemma_seq
                .iter()
                .zip(candidate)
                .all(|(lemma, token)| token.lemma.as_str() == lemma);
            lemmas_match.then(|| {
                let last = index + entry.lemma_seq.len() - 1;
                let end = if tokens[last].pos.starts_with('V') {
                    trailing_endings_end(tokens, last)
                } else {
                    last
                };
                (end, entry.display_form.clone())
            })
        });
        match matched {
            Some((end, display_form)) => {
                spans.push(ExpressionSpan {
                    start: index,
                    end,
                    display_form,
                });
                index = end + 1;
            }
            None => index += 1,
        }
    }
    spans
}

/// Index of the last consecutive ending (`E*`) token following `index`,
/// or `index` itself when none follow.
fn trailing_endings_end(tokens: &[Token], index: usize) -> usize {
    let mut end = index;
    while tokens
        .get(end + 1)
        .is_some_and(|token| token.pos.starts_with('E'))
    {
        end += 1;
    }
    end
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

    // -----------------------------------------------------------------------
    // detect_expressions
    // -----------------------------------------------------------------------

    fn entry(lemmas: &[&str], display_form: &str) -> ExpressionEntry {
        ExpressionEntry {
            lemma_seq: lemmas.iter().map(|l| l.to_string()).collect(),
            display_form: display_form.to_string(),
        }
    }

    #[test]
    fn detects_expression_in_conjugated_form() {
        let tokens = tokens_for("정말 마음에 들었어요");
        let lexicon = vec![entry(&["마음", "에", "들다"], "마음에 들다")];

        let spans = detect_expressions(&tokens, &lexicon);

        assert_eq!(spans.len(), 1);
        let span = &spans[0];
        assert_eq!(span.display_form, "마음에 들다");
        let start_of_maeum = tokens
            .iter()
            .position(|t| t.surface.as_str() == "마음")
            .expect("expected a 마음 token");
        assert_eq!(span.start, start_of_maeum, "span must start at 마음");
        assert_eq!(
            span.end,
            tokens.len() - 1,
            "span must cover the ending-bearing tokens of 들었어요"
        );
    }

    #[test]
    fn no_match_when_lemma_sequence_broken() {
        let tokens = tokens_for("마음에 정말 들어");
        let lexicon = vec![entry(&["마음", "에", "들다"], "마음에 들다")];
        assert!(detect_expressions(&tokens, &lexicon).is_empty());
    }

    #[test]
    fn leftmost_longest_wins_on_overlap() {
        let tokens = tokens_for("마음에 들어");
        // Shorter entry listed first: length, not lexicon order, must win.
        let lexicon = vec![
            entry(&["에", "들다"], "에 들다"),
            entry(&["마음", "에", "들다"], "마음에 들다"),
        ];

        let spans = detect_expressions(&tokens, &lexicon);

        assert_eq!(spans.len(), 1, "matched tokens must be consumed");
        assert_eq!(spans[0].start, 0);
        assert_eq!(spans[0].display_form, "마음에 들다");
    }

    #[test]
    fn multiple_disjoint_matches_all_returned() {
        let line = "마음에 들어 어쩔 수 없어";
        let tokens = tokens_for(line);
        let lexicon = vec![
            entry(&["마음", "에", "들다"], "마음에 들다"),
            ExpressionEntry {
                lemma_seq: lemma_sequence(&tokens_for("어쩔 수 없어")),
                display_form: "어쩔 수 없다".to_string(),
            },
        ];

        let spans = detect_expressions(&tokens, &lexicon);

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].display_form, "마음에 들다");
        assert_eq!(spans[1].display_form, "어쩔 수 없다");
        assert!(
            spans[0].end < spans[1].start,
            "spans must be disjoint and sorted by position"
        );
    }

    #[test]
    fn empty_lexicon_returns_no_spans() {
        let tokens = tokens_for("마음에 들어");
        assert!(detect_expressions(&tokens, &[]).is_empty());
    }

    #[test]
    fn match_at_line_start_and_end() {
        let tokens = tokens_for("마음에 들어");
        let lexicon = vec![entry(&["마음", "에", "들다"], "마음에 들다")];

        let spans = detect_expressions(&tokens, &lexicon);

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start, 0, "flush against the line start");
        assert_eq!(
            spans[0].end,
            tokens.len() - 1,
            "flush against the line end (endings included)"
        );
    }
}
