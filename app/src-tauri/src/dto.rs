use serde::{Deserialize, Serialize};

use kaeda_core::deck::Deck;
use kaeda_core::session::Card;
use kaeda_core::subtitle::{SubtitleEntry, srt_timestamp_to_ms};
use kaeda_core::tokenizer::Token;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDto {
    pub surface: String,
    pub lemma: String,
    pub pos: String,
    pub byte_start: usize,
    pub byte_end: usize,
}

impl From<&Token> for TokenDto {
    fn from(t: &Token) -> Self {
        Self {
            surface: t.surface.to_string(),
            lemma: t.lemma.to_string(),
            pos: t.pos.to_string(),
            byte_start: t.byte_start,
            byte_end: t.byte_end,
        }
    }
}

impl From<&TokenDto> for Token {
    fn from(dto: &TokenDto) -> Self {
        Self {
            surface: dto.surface.as_str().into(),
            lemma: dto.lemma.as_str().into(),
            pos: dto.pos.as_str().into(),
            byte_start: dto.byte_start,
            byte_end: dto.byte_end,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleDto {
    pub id: u32,
    pub start_time: String,
    pub end_time: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub is_known: bool,
    pub tokens: Vec<TokenDto>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDto {
    pub card_id: u32,
    pub id: u32,
    pub sentence: String,
    pub target: String,
    pub explanation: String,
    pub deck_id: i64,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleSearchResultDto {
    pub subtitle_id: u32,
    pub index: usize,
    pub text: String,
    pub start_ms: u64,
}

impl From<SubtitleEntry> for SubtitleDto {
    fn from(entry: SubtitleEntry) -> Self {
        Self {
            id: entry.id,
            start_ms: srt_timestamp_to_ms(&entry.start_time).unwrap_or(0),
            end_ms: srt_timestamp_to_ms(&entry.end_time).unwrap_or(0),
            start_time: entry.start_time,
            end_time: entry.end_time,
            text: entry.text,
            is_known: false,
            tokens: Vec::new(),
        }
    }
}

impl From<Card> for CardDto {
    fn from(card: Card) -> Self {
        Self {
            card_id: card.card_id,
            id: card.subtitle_id,
            sentence: card.sentence,
            target: card.target,
            explanation: card.explanation,
            deck_id: card.deck_id.0,
            tags: card.tags,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckDto {
    pub id: i64,
    pub name: String,
}

impl From<Deck> for DeckDto {
    fn from(deck: Deck) -> Self {
        Self {
            id: deck.id.0,
            name: deck.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeda_core::deck::DeckId;

    #[test]
    fn subtitle_entry_to_dto_maps_all_fields() {
        let entry = SubtitleEntry {
            id: 42,
            start_time: "00:01:00,000".into(),
            end_time: "00:01:05,000".into(),
            text: "안녕하세요".into(),
        };
        let dto = SubtitleDto::from(entry);
        assert_eq!(dto.id, 42);
        assert_eq!(dto.start_time, "00:01:00,000");
        assert_eq!(dto.end_time, "00:01:05,000");
        assert_eq!(dto.start_ms, 60_000);
        assert_eq!(dto.end_ms, 65_000);
        assert_eq!(dto.text, "안녕하세요");
        assert!(!dto.is_known);
        assert!(dto.tokens.is_empty());
    }

    #[test]
    fn card_to_dto_maps_all_fields() {
        let card = Card {
            card_id: 0,
            sentence: "안녕하세요".into(),
            target: "안녕".into(),
            explanation: "Hello".into(),
            deck_id: DeckId(42),
            tags: vec!["korean".into()],
            file_id: "video-1".into(),
            subtitle_id: 1,
        };
        let dto = CardDto::from(card);
        assert_eq!(dto.card_id, 0);
        assert_eq!(dto.id, 1);
        assert_eq!(dto.sentence, "안녕하세요");
        assert_eq!(dto.target, "안녕");
        assert_eq!(dto.explanation, "Hello");
        assert_eq!(dto.deck_id, 42);
        assert_eq!(dto.tags, vec!["korean"]);
    }

    #[test]
    fn token_to_dto_maps_all_fields() {
        let token = Token {
            surface: "먹".into(),
            lemma: "먹다".into(),
            pos: "VV".into(),
            byte_start: 0,
            byte_end: 3,
        };
        let dto = TokenDto::from(&token);
        assert_eq!(dto.surface, "먹");
        assert_eq!(dto.lemma, "먹다");
        assert_eq!(dto.pos, "VV");
        assert_eq!(dto.byte_start, 0);
        assert_eq!(dto.byte_end, 3);
    }
}
