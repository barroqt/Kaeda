use serde::{Deserialize, Serialize};

use kaeda_core::session::Card;
use kaeda_core::subtitle::SubtitleEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleDto {
    pub id: u32,
    pub start_time: String,
    pub end_time: String,
    pub text: String,
    pub is_known: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDto {
    pub id: u32,
    pub sentence: String,
    pub target: String,
    pub explanation: String,
    pub deck: String,
    pub tags: Vec<String>,
}

impl From<SubtitleEntry> for SubtitleDto {
    fn from(entry: SubtitleEntry) -> Self {
        Self {
            id: entry.id,
            start_time: entry.start_time,
            end_time: entry.end_time,
            text: entry.text,
            is_known: false,
        }
    }
}

impl From<Card> for CardDto {
    fn from(card: Card) -> Self {
        Self {
            id: card.subtitle_id,
            sentence: card.sentence,
            target: card.target,
            explanation: card.explanation,
            deck: card.deck,
            tags: card.tags,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(dto.text, "안녕하세요");
        assert!(!dto.is_known);
    }

    #[test]
    fn card_to_dto_maps_all_fields() {
        let card = Card {
            sentence: "안녕하세요".into(),
            target: "안녕".into(),
            explanation: "Hello".into(),
            deck: "my-deck".into(),
            tags: vec!["korean".into()],
            file_id: "video-1".into(),
            subtitle_id: 1,
        };
        let dto = CardDto::from(card);
        assert_eq!(dto.id, 1);
        assert_eq!(dto.sentence, "안녕하세요");
        assert_eq!(dto.target, "안녕");
        assert_eq!(dto.explanation, "Hello");
        assert_eq!(dto.deck, "my-deck");
        assert_eq!(dto.tags, vec!["korean"]);
    }
}
