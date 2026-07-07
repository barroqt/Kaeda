use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::store::Store;
use crate::subtitle::CoreError;

/// Maximum number of decks a user may create.
pub const MAX_DECKS: i64 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeckId(pub i64);

impl fmt::Display for DeckId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeckId({})", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
    pub id: DeckId,
    pub name: String,
}

#[derive(Debug, Error)]
pub enum DeckError {
    #[error("deck name must not be empty")]
    EmptyName,
    #[error("cannot create more than {MAX_DECKS} decks")]
    LimitReached,
    #[error("deck with id {0} not found")]
    NotFound(DeckId),
    #[error(transparent)]
    Store(CoreError),
}

impl From<CoreError> for DeckError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::DeckNotFound(id) => Self::NotFound(id),
            other => Self::Store(other),
        }
    }
}

/// Create a deck named `name` and return it. Rejects blank names and
/// enforces the [`MAX_DECKS`] cap.
pub fn create_deck(store: &Store, name: &str) -> Result<Deck, DeckError> {
    if name.trim().is_empty() {
        return Err(DeckError::EmptyName);
    }
    if store.deck_count()? >= MAX_DECKS {
        return Err(DeckError::LimitReached);
    }
    let id = store.create_deck(name)?;
    store.get_deck(id)?.ok_or(DeckError::NotFound(id))
}

/// Rename `deck_id` and return the updated deck. Rejects blank names.
pub fn rename_deck(store: &Store, deck_id: DeckId, new_name: &str) -> Result<Deck, DeckError> {
    if new_name.trim().is_empty() {
        return Err(DeckError::EmptyName);
    }
    store.rename_deck(deck_id, new_name)?;
    store.get_deck(deck_id)?.ok_or(DeckError::NotFound(deck_id))
}

/// Delete `deck_id` and its cards. If the deleted deck was `active_deck_id`
/// and other decks remain, the first remaining deck becomes the new active
/// deck and its id is returned so the caller can persist it.
pub fn delete_deck(
    store: &Store,
    deck_id: DeckId,
    active_deck_id: DeckId,
) -> Result<Option<DeckId>, DeckError> {
    store
        .get_deck(deck_id)?
        .ok_or(DeckError::NotFound(deck_id))?;
    store.delete_deck(deck_id)?;
    if deck_id == active_deck_id {
        let remaining = store.list_decks()?;
        return Ok(remaining.first().map(|deck| deck.id));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> Store {
        Store::in_memory().expect("open in-memory store")
    }

    #[test]
    fn create_deck_returns_new_deck() {
        let store = test_store();

        let deck = create_deck(&store, "New Deck").unwrap();

        assert_eq!(deck.name, "New Deck");
        assert_eq!(store.deck_count().unwrap(), 1);
    }

    #[test]
    fn create_deck_rejects_blank_name() {
        let store = test_store();

        let err = create_deck(&store, "   ").unwrap_err();

        assert!(matches!(err, DeckError::EmptyName));
        assert_eq!(store.deck_count().unwrap(), 0);
    }

    #[test]
    fn create_deck_enforces_max_decks_cap() {
        let store = test_store();
        for i in 0..MAX_DECKS {
            store.create_deck(&format!("Deck {i}")).unwrap();
        }

        let err = create_deck(&store, "One Too Many").unwrap_err();

        assert!(matches!(err, DeckError::LimitReached));
        assert_eq!(store.deck_count().unwrap(), MAX_DECKS);
    }

    #[test]
    fn rename_deck_returns_updated_deck() {
        let store = test_store();
        let deck = create_deck(&store, "Old Name").unwrap();

        let renamed = rename_deck(&store, deck.id, "New Name").unwrap();

        assert_eq!(renamed.id, deck.id);
        assert_eq!(renamed.name, "New Name");
    }

    #[test]
    fn rename_deck_rejects_blank_name() {
        let store = test_store();
        let deck = create_deck(&store, "Old Name").unwrap();

        let err = rename_deck(&store, deck.id, " ").unwrap_err();

        assert!(matches!(err, DeckError::EmptyName));
        let unchanged = store.get_deck(deck.id).unwrap().unwrap();
        assert_eq!(unchanged.name, "Old Name");
    }

    #[test]
    fn rename_deck_rejects_unknown_deck() {
        let store = test_store();

        let err = rename_deck(&store, DeckId(9999), "Name").unwrap_err();

        assert!(matches!(err, DeckError::NotFound(DeckId(9999))));
    }

    #[test]
    fn delete_active_deck_reassigns_to_first_remaining_deck() {
        let store = test_store();
        let first = create_deck(&store, "First").unwrap();
        let second = create_deck(&store, "Second").unwrap();

        let reassigned = delete_deck(&store, second.id, second.id).unwrap();

        assert_eq!(reassigned, Some(first.id));
        assert_eq!(store.deck_count().unwrap(), 1);
    }

    #[test]
    fn delete_last_deck_returns_no_replacement() {
        let store = test_store();
        let only = create_deck(&store, "Only").unwrap();

        let reassigned = delete_deck(&store, only.id, only.id).unwrap();

        assert_eq!(reassigned, None);
        assert_eq!(store.deck_count().unwrap(), 0);
    }

    #[test]
    fn delete_inactive_deck_keeps_active_deck() {
        let store = test_store();
        let active = create_deck(&store, "Active").unwrap();
        let extra = create_deck(&store, "Extra").unwrap();

        let reassigned = delete_deck(&store, extra.id, active.id).unwrap();

        assert_eq!(reassigned, None);
        assert_eq!(store.deck_count().unwrap(), 1);
    }

    #[test]
    fn delete_deck_rejects_unknown_deck() {
        let store = test_store();
        let active = create_deck(&store, "Active").unwrap();

        let err = delete_deck(&store, DeckId(9999), active.id).unwrap_err();

        assert!(matches!(err, DeckError::NotFound(DeckId(9999))));
    }
}
