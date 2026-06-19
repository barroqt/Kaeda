use std::path::Path;

use crate::deck::DeckId;
use crate::subtitle::CoreError;

#[derive(Debug, Clone)]
pub struct Card {
    pub card_id: u32,
    pub deck_id: DeckId,
    pub sentence: String,
    pub target: String,
    pub explanation: String,
    pub tags: Vec<String>,
    pub file_id: String,
    pub subtitle_id: u32,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub deck_id: DeckId,
    pub source_file_id: String,
    next_card_id: u32,
    cards: Vec<Card>,
}

impl Session {
    pub fn new(deck_id: DeckId, source_file_id: String) -> Self {
        Self {
            deck_id,
            source_file_id,
            next_card_id: 1,
            cards: Vec::new(),
        }
    }

    pub fn add_card(&mut self, mut card: Card) -> Card {
        card.card_id = self.next_card_id;
        self.next_card_id += 1;
        self.cards.push(card.clone());
        card
    }

    pub fn edit_card(
        &mut self,
        card_id: u32,
        sentence: String,
        target: String,
        explanation: String,
    ) -> Result<(), CoreError> {
        let card = self
            .cards
            .iter_mut()
            .find(|c| c.card_id == card_id)
            .ok_or(CoreError::CardNotFound(card_id))?;
        card.sentence = sentence;
        card.target = target;
        card.explanation = explanation;
        Ok(())
    }

    pub fn remove_card(&mut self, card_id: u32) -> Result<(), CoreError> {
        let idx = self
            .cards
            .iter()
            .position(|c| c.card_id == card_id)
            .ok_or(CoreError::CardNotFound(card_id))?;
        self.cards.remove(idx);
        Ok(())
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
        let session = Session::new(DeckId(1), "file".to_string());
        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_test_empty.tsv");
        session.export_tsv(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_tsv_single_card_writes_correct_row() {
        let mut session = Session::new(DeckId(1), "file".to_string());
        session.add_card(Card {
            card_id: 0,
            sentence: "안녕하세요".to_string(),
            target: "안녕".to_string(),
            explanation: "Hello".to_string(),
            deck_id: DeckId(1),
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
        let mut session = Session::new(DeckId(1), "file".to_string());
        session.add_card(Card {
            card_id: 0,
            sentence: "책을 읽습니다".to_string(),
            target: "책".to_string(),
            explanation: "book".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "file".to_string(),
            subtitle_id: 1,
        });
        session.add_card(Card {
            card_id: 0,
            sentence: "물을 마십니다".to_string(),
            target: "물".to_string(),
            explanation: "water".to_string(),
            deck_id: DeckId(1),
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
        let mut session = Session::new(DeckId(1), "file".to_string());
        session.add_card(Card {
            card_id: 0,
            sentence: "line1\nline2".to_string(),
            target: "tar\tget".to_string(),
            explanation: "exp\tlanation\nsecond".to_string(),
            deck_id: DeckId(1),
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
        let session = Session::new(DeckId(42), "file-1".to_string());
        assert_eq!(session.deck_id, DeckId(42));
        assert_eq!(session.source_file_id, "file-1");
        assert_eq!(session.card_count(), 0);
    }

    #[test]
    fn add_card_increases_count() {
        let mut session = Session::new(DeckId(1), "f".to_string());
        let card = Card {
            card_id: 0,
            sentence: "안녕하세요".to_string(),
            target: "안녕".to_string(),
            explanation: "Hello".to_string(),
            deck_id: DeckId(1),
            tags: vec!["korean".to_string()],
            file_id: "f".to_string(),
            subtitle_id: 1,
        };
        session.add_card(card);
        assert_eq!(session.card_count(), 1);
    }

    #[test]
    fn add_card_stores_correct_fields() {
        let mut session = Session::new(DeckId(1), "file".to_string());
        let card = Card {
            card_id: 0,
            sentence: "책을 읽습니다".to_string(),
            target: "책".to_string(),
            explanation: "book".to_string(),
            deck_id: DeckId(1),
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
        let mut session = Session::new(DeckId(1), "file".to_string());
        for i in 0..3 {
            session.add_card(Card {
                card_id: 0,
                sentence: format!("sentence {i}"),
                target: format!("target {i}"),
                explanation: String::new(),
                deck_id: DeckId(1),
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
        let mut session = Session::new(DeckId(1), "f".to_string());
        session.add_card(Card {
            card_id: 0,
            sentence: "test".to_string(),
            target: "t".to_string(),
            explanation: String::new(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 0,
        });
        let cards = session.cards();
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn add_card_assigns_card_id() {
        let mut session = Session::new(DeckId(1), "file".to_string());
        let card = session.add_card(Card {
            card_id: 0,
            sentence: "test".to_string(),
            target: "t".to_string(),
            explanation: "e".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 1,
        });
        assert_eq!(card.card_id, 1);
        assert_eq!(session.cards()[0].card_id, 1);
    }

    #[test]
    fn add_card_increments_card_id() {
        let mut session = Session::new(DeckId(1), "file".to_string());
        let card1 = session.add_card(Card {
            card_id: 0,
            sentence: "s1".to_string(),
            target: "t1".to_string(),
            explanation: "e1".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 1,
        });
        let card2 = session.add_card(Card {
            card_id: 0,
            sentence: "s2".to_string(),
            target: "t2".to_string(),
            explanation: "e2".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 2,
        });
        assert_eq!(card1.card_id, 1);
        assert_eq!(card2.card_id, 2);
    }

    #[test]
    fn edit_card_updates_fields() {
        let mut session = Session::new(DeckId(1), "file".to_string());
        session.add_card(Card {
            card_id: 0,
            sentence: "original sentence".to_string(),
            target: "original".to_string(),
            explanation: "original expl".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 1,
        });
        session
            .edit_card(
                1,
                "new sentence".to_string(),
                "new target".to_string(),
                "new expl".to_string(),
            )
            .unwrap();
        let card = &session.cards()[0];
        assert_eq!(card.sentence, "new sentence");
        assert_eq!(card.target, "new target");
        assert_eq!(card.explanation, "new expl");
    }

    #[test]
    fn edit_card_returns_error_for_invalid_id() {
        let mut session = Session::new(DeckId(1), "file".to_string());
        let result = session.edit_card(42, "s".to_string(), "t".to_string(), "e".to_string());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CoreError::CardNotFound(42)));
    }

    #[test]
    fn edit_card_preserves_other_cards() {
        let mut session = Session::new(DeckId(1), "file".to_string());
        session.add_card(Card {
            card_id: 0,
            sentence: "first".to_string(),
            target: "t1".to_string(),
            explanation: "e1".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 1,
        });
        session.add_card(Card {
            card_id: 0,
            sentence: "second".to_string(),
            target: "t2".to_string(),
            explanation: "e2".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 2,
        });
        session
            .edit_card(
                1,
                "edited first".to_string(),
                "edited".to_string(),
                "edited".to_string(),
            )
            .unwrap();
        assert_eq!(session.cards()[1].sentence, "second");
        assert_eq!(session.cards()[1].target, "t2");
    }

    #[test]
    fn remove_card_decreases_count() {
        let mut session = Session::new(DeckId(1), "file".to_string());
        session.add_card(Card {
            card_id: 0,
            sentence: "test".to_string(),
            target: "t".to_string(),
            explanation: "e".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 1,
        });
        assert_eq!(session.card_count(), 1);
        session.remove_card(1).unwrap();
        assert_eq!(session.card_count(), 0);
    }

    #[test]
    fn remove_card_removes_specific_card() {
        let mut session = Session::new(DeckId(1), "file".to_string());
        session.add_card(Card {
            card_id: 0,
            sentence: "first".to_string(),
            target: "t1".to_string(),
            explanation: "e1".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 1,
        });
        session.add_card(Card {
            card_id: 0,
            sentence: "second".to_string(),
            target: "t2".to_string(),
            explanation: "e2".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 2,
        });
        session.remove_card(1).unwrap();
        assert_eq!(session.card_count(), 1);
        assert_eq!(session.cards()[0].target, "t2");
    }

    #[test]
    fn remove_card_returns_error_for_invalid_id() {
        let mut session = Session::new(DeckId(1), "file".to_string());
        let result = session.remove_card(99);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CoreError::CardNotFound(99)));
    }

    #[test]
    fn remove_card_excludes_from_export() {
        let mut session = Session::new(DeckId(1), "file".to_string());
        session.add_card(Card {
            card_id: 0,
            sentence: "keep".to_string(),
            target: "keep".to_string(),
            explanation: "keep".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 1,
        });
        session.add_card(Card {
            card_id: 0,
            sentence: "delete".to_string(),
            target: "delete".to_string(),
            explanation: "delete".to_string(),
            deck_id: DeckId(1),
            tags: vec![],
            file_id: "f".to_string(),
            subtitle_id: 2,
        });
        session.remove_card(2).unwrap();

        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_test_after_delete.tsv");
        session.export_tsv(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "keep\tkeep\tkeep\n");

        let _ = std::fs::remove_file(&path);
    }
}
