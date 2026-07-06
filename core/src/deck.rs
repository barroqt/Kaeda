use std::fmt;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::store;
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
pub fn create_deck(conn: &Connection, name: &str) -> Result<Deck, DeckError> {
    if name.trim().is_empty() {
        return Err(DeckError::EmptyName);
    }
    if store::deck_count(conn)? >= MAX_DECKS {
        return Err(DeckError::LimitReached);
    }
    let id = store::create_deck(conn, name)?;
    store::get_deck(conn, id)?.ok_or(DeckError::NotFound(id))
}

/// Rename `deck_id` and return the updated deck. Rejects blank names.
pub fn rename_deck(conn: &Connection, deck_id: DeckId, new_name: &str) -> Result<Deck, DeckError> {
    if new_name.trim().is_empty() {
        return Err(DeckError::EmptyName);
    }
    store::rename_deck(conn, deck_id, new_name)?;
    store::get_deck(conn, deck_id)?.ok_or(DeckError::NotFound(deck_id))
}

/// Delete `deck_id` and its cards. If the deleted deck was `active_deck_id`
/// and other decks remain, the first remaining deck becomes the new active
/// deck and its id is returned so the caller can persist it.
pub fn delete_deck(
    conn: &Connection,
    deck_id: DeckId,
    active_deck_id: DeckId,
) -> Result<Option<DeckId>, DeckError> {
    store::get_deck(conn, deck_id)?.ok_or(DeckError::NotFound(deck_id))?;
    store::delete_deck(conn, deck_id)?;
    if deck_id == active_deck_id {
        let remaining = store::list_decks(conn)?;
        return Ok(remaining.first().map(|deck| deck.id));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        store::init_store(&conn).expect("init store");
        conn
    }

    #[test]
    fn create_deck_returns_new_deck() {
        let conn = test_conn();

        let deck = create_deck(&conn, "New Deck").unwrap();

        assert_eq!(deck.name, "New Deck");
        assert_eq!(store::deck_count(&conn).unwrap(), 1);
    }

    #[test]
    fn create_deck_rejects_blank_name() {
        let conn = test_conn();

        let err = create_deck(&conn, "   ").unwrap_err();

        assert!(matches!(err, DeckError::EmptyName));
        assert_eq!(store::deck_count(&conn).unwrap(), 0);
    }

    #[test]
    fn create_deck_enforces_max_decks_cap() {
        let conn = test_conn();
        for i in 0..MAX_DECKS {
            store::create_deck(&conn, &format!("Deck {i}")).unwrap();
        }

        let err = create_deck(&conn, "One Too Many").unwrap_err();

        assert!(matches!(err, DeckError::LimitReached));
        assert_eq!(store::deck_count(&conn).unwrap(), MAX_DECKS);
    }

    #[test]
    fn rename_deck_returns_updated_deck() {
        let conn = test_conn();
        let deck = create_deck(&conn, "Old Name").unwrap();

        let renamed = rename_deck(&conn, deck.id, "New Name").unwrap();

        assert_eq!(renamed.id, deck.id);
        assert_eq!(renamed.name, "New Name");
    }

    #[test]
    fn rename_deck_rejects_blank_name() {
        let conn = test_conn();
        let deck = create_deck(&conn, "Old Name").unwrap();

        let err = rename_deck(&conn, deck.id, " ").unwrap_err();

        assert!(matches!(err, DeckError::EmptyName));
        let unchanged = store::get_deck(&conn, deck.id).unwrap().unwrap();
        assert_eq!(unchanged.name, "Old Name");
    }

    #[test]
    fn rename_deck_rejects_unknown_deck() {
        let conn = test_conn();

        let err = rename_deck(&conn, DeckId(9999), "Name").unwrap_err();

        assert!(matches!(err, DeckError::NotFound(DeckId(9999))));
    }

    #[test]
    fn delete_active_deck_reassigns_to_first_remaining_deck() {
        let conn = test_conn();
        let first = create_deck(&conn, "First").unwrap();
        let second = create_deck(&conn, "Second").unwrap();

        let reassigned = delete_deck(&conn, second.id, second.id).unwrap();

        assert_eq!(reassigned, Some(first.id));
        assert_eq!(store::deck_count(&conn).unwrap(), 1);
    }

    #[test]
    fn delete_last_deck_returns_no_replacement() {
        let conn = test_conn();
        let only = create_deck(&conn, "Only").unwrap();

        let reassigned = delete_deck(&conn, only.id, only.id).unwrap();

        assert_eq!(reassigned, None);
        assert_eq!(store::deck_count(&conn).unwrap(), 0);
    }

    #[test]
    fn delete_inactive_deck_keeps_active_deck() {
        let conn = test_conn();
        let active = create_deck(&conn, "Active").unwrap();
        let extra = create_deck(&conn, "Extra").unwrap();

        let reassigned = delete_deck(&conn, extra.id, active.id).unwrap();

        assert_eq!(reassigned, None);
        assert_eq!(store::deck_count(&conn).unwrap(), 1);
    }

    #[test]
    fn delete_deck_rejects_unknown_deck() {
        let conn = test_conn();
        let active = create_deck(&conn, "Active").unwrap();

        let err = delete_deck(&conn, DeckId(9999), active.id).unwrap_err();

        assert!(matches!(err, DeckError::NotFound(DeckId(9999))));
    }
}
