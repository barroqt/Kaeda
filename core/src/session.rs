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
}

#[cfg(test)]
mod tests {
    use super::*;

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
