use std::path::Path;

use crate::subtitle::CoreError;

#[derive(Debug, Clone)]
pub struct Card {
    pub sentence: String,
    pub target: String,
    pub explanation: String,
    pub deck: String,
    pub tags: Vec<String>,
    pub file_id: String,
    pub subtitle_id: u32,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub deck_name: String,
    pub source_file_id: String,
    cards: Vec<Card>,
}

impl Session {
    pub fn new(deck_name: String, source_file_id: String) -> Self {
        Self {
            deck_name,
            source_file_id,
            cards: Vec::new(),
        }
    }

    pub fn add_card(&mut self, card: Card) {
        self.cards.push(card);
    }

    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub fn card_count(&self) -> usize {
        self.cards.len()
    }

    pub fn export_tsv(&self, path: &Path) -> Result<(), CoreError> {
        use std::io::Write;

        let mut file = std::fs::File::create(path).map_err(|e| CoreError::Export(e.to_string()))?;
        for card in &self.cards {
            let target = card.target.replace(['\t', '\n'], " ");
            let sentence = card.sentence.replace(['\t', '\n'], " ");
            let explanation = card.explanation.replace(['\t', '\n'], " ");
            writeln!(file, "{target}\t{sentence}\t{explanation}")
                .map_err(|e| CoreError::Export(e.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_tsv_empty_session_writes_nothing() {
        let session = Session::new("deck".to_string(), "file".to_string());
        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_test_empty.tsv");
        session.export_tsv(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_tsv_single_card_writes_correct_row() {
        let mut session = Session::new("deck".to_string(), "file".to_string());
        session.add_card(Card {
            sentence: "안녕하세요".to_string(),
            target: "안녕".to_string(),
            explanation: "Hello".to_string(),
            deck: "deck".to_string(),
            tags: vec![],
            file_id: "file".to_string(),
            subtitle_id: 1,
        });

        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_test_single.tsv");
        session.export_tsv(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "안녕\t안녕하세요\tHello\n");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_tsv_multiple_cards_produces_correct_rows() {
        let mut session = Session::new("deck".to_string(), "file".to_string());
        session.add_card(Card {
            sentence: "책을 읽습니다".to_string(),
            target: "책".to_string(),
            explanation: "book".to_string(),
            deck: "deck".to_string(),
            tags: vec![],
            file_id: "file".to_string(),
            subtitle_id: 1,
        });
        session.add_card(Card {
            sentence: "물을 마십니다".to_string(),
            target: "물".to_string(),
            explanation: "water".to_string(),
            deck: "deck".to_string(),
            tags: vec![],
            file_id: "file".to_string(),
            subtitle_id: 2,
        });

        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_test_multi.tsv");
        session.export_tsv(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let expected = "책\t책을 읽습니다\tbook\n물\t물을 마십니다\twater\n";
        assert_eq!(content, expected);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_tsv_sanitizes_tabs_and_newlines() {
        let mut session = Session::new("deck".to_string(), "file".to_string());
        session.add_card(Card {
            sentence: "line1\nline2".to_string(),
            target: "tar\tget".to_string(),
            explanation: "exp\tlanation\nsecond".to_string(),
            deck: "deck".to_string(),
            tags: vec![],
            file_id: "file".to_string(),
            subtitle_id: 1,
        });

        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_test_sanitize.tsv");
        session.export_tsv(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "tar get\tline1 line2\texp lanation second\n");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn session_new_creates_empty_session() {
        let session = Session::new("test-deck".to_string(), "file-1".to_string());
        assert_eq!(session.deck_name, "test-deck");
        assert_eq!(session.source_file_id, "file-1");
        assert_eq!(session.card_count(), 0);
    }

    #[test]
    fn add_card_increases_count() {
        let mut session = Session::new("deck".to_string(), "f".to_string());
        let card = Card {
            sentence: "안녕하세요".to_string(),
            target: "안녕".to_string(),
            explanation: "Hello".to_string(),
            deck: "deck".to_string(),
            tags: vec!["korean".to_string()],
            file_id: "f".to_string(),
            subtitle_id: 1,
        };
        session.add_card(card);
        assert_eq!(session.card_count(), 1);
    }

    #[test]
    fn add_card_stores_correct_fields() {
        let mut session = Session::new("deck".to_string(), "file".to_string());
        let card = Card {
            sentence: "책을 읽습니다".to_string(),
            target: "책".to_string(),
            explanation: "book".to_string(),
            deck: "deck".to_string(),
            tags: vec!["korean".to_string(), "noun".to_string()],
            file_id: "file".to_string(),
            subtitle_id: 42,
        };
        session.add_card(card);
        let stored = &session.cards()[0];
        assert_eq!(stored.sentence, "책을 읽습니다");
        assert_eq!(stored.target, "책");
        assert_eq!(stored.explanation, "book");
        assert_eq!(stored.tags, vec!["korean", "noun"]);
        assert_eq!(stored.subtitle_id, 42);
    }

    #[test]
    fn cards_returns_all_added_cards() {
        let mut session = Session::new("deck".to_string(), "file".to_string());
        for i in 0..3 {
            session.add_card(Card {
                sentence: format!("sentence {i}"),
                target: format!("target {i}"),
                explanation: String::new(),
                deck: "deck".to_string(),
                tags: vec![],
                file_id: "file".to_string(),
                subtitle_id: i,
            });
        }
        assert_eq!(session.card_count(), 3);
        assert_eq!(session.cards().len(), 3);
    }

    #[test]
    fn cards_getter_returns_immutable_slice() {
        let mut session = Session::new("d".to_string(), "f".to_string());
        session.add_card(Card {
            sentence: "test".to_string(),
            target: "t".to_string(),
            explanation: String::new(),
            deck: "d".to_string(),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 0,
        });
        let cards = session.cards();
        assert_eq!(cards.len(), 1);
    }
}
