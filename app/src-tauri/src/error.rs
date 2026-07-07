use crate::translation::TranslationError;

#[derive(Debug, serde::Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn translation_disabled() -> Self {
        Self {
            code: "TRANSLATION_DISABLED",
            message: "Translation disabled".into(),
        }
    }

    pub fn session_error(message: String) -> Self {
        Self {
            code: "SESSION_ERROR",
            message,
        }
    }

    fn deepl_unreachable(msg: String) -> Self {
        Self {
            code: "DEEPL_UNREACHABLE",
            message: format!("Unable to reach DeepL: {msg}"),
        }
    }

    fn deepl_rejected(msg: String) -> Self {
        Self {
            code: "DEEPL_REJECTED",
            message: format!("DeepL rejected the request: {msg}"),
        }
    }

    fn deepl_invalid_response() -> Self {
        Self {
            code: "DEEPL_INVALID_RESPONSE",
            message: "DeepL returned an unexpected response".into(),
        }
    }

    pub fn invalid_token_range(start: usize, end: usize, token_count: usize) -> Self {
        Self {
            code: "INVALID_TOKEN_RANGE",
            message: format!("invalid token range {start}..={end} for {token_count} tokens"),
        }
    }

    pub fn deck_not_found(id: i64) -> Self {
        Self {
            code: "DECK_NOT_FOUND",
            message: format!("Deck with id {id} not found"),
        }
    }

    pub fn expression_not_found(id: i64) -> Self {
        Self {
            code: "EXPRESSION_NOT_FOUND",
            message: format!("Expression with id {id} not found"),
        }
    }

    fn invalid_deck_name() -> Self {
        Self {
            code: "INVALID_DECK_NAME",
            message: "Deck name must not be empty".into(),
        }
    }

    fn deck_limit_reached() -> Self {
        Self {
            code: "DECK_LIMIT_REACHED",
            message: format!(
                "Cannot create more than {} decks",
                kaeda_core::deck::MAX_DECKS
            ),
        }
    }
}

impl From<kaeda_core::deck::DeckError> for AppError {
    fn from(err: kaeda_core::deck::DeckError) -> Self {
        use kaeda_core::deck::DeckError;
        match err {
            DeckError::EmptyName => Self::invalid_deck_name(),
            DeckError::LimitReached => Self::deck_limit_reached(),
            DeckError::NotFound(id) => Self::deck_not_found(id.0),
            DeckError::Store(e) => Self::session_error(e.to_string()),
        }
    }
}

impl From<TranslationError> for AppError {
    fn from(err: TranslationError) -> Self {
        match err {
            TranslationError::NotConfigured => Self::translation_disabled(),
            TranslationError::HttpError(msg) => Self::deepl_unreachable(msg),
            TranslationError::ApiError(msg) => Self::deepl_rejected(msg),
            TranslationError::InvalidResponse => Self::deepl_invalid_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeda_core::deck::{DeckError, DeckId};

    #[test]
    fn deck_error_maps_to_app_error_codes() {
        assert_eq!(
            AppError::from(DeckError::EmptyName).code,
            "INVALID_DECK_NAME"
        );
        assert_eq!(
            AppError::from(DeckError::LimitReached).code,
            "DECK_LIMIT_REACHED"
        );
        assert_eq!(
            AppError::from(DeckError::NotFound(DeckId(42))).code,
            "DECK_NOT_FOUND"
        );
    }

    #[test]
    fn translation_error_maps_to_app_error_codes() {
        assert_eq!(
            AppError::from(TranslationError::NotConfigured).code,
            "TRANSLATION_DISABLED"
        );
        assert_eq!(
            AppError::from(TranslationError::HttpError("timeout".into())).code,
            "DEEPL_UNREACHABLE"
        );
        assert_eq!(
            AppError::from(TranslationError::ApiError("403".into())).code,
            "DEEPL_REJECTED"
        );
        assert_eq!(
            AppError::from(TranslationError::InvalidResponse).code,
            "DEEPL_INVALID_RESPONSE"
        );
    }
}
