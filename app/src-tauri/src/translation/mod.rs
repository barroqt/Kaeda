pub mod deepl;

pub use deepl::{DeepLConfig, TranslationError, translate_with_deepl};
#[cfg(test)]
pub(crate) use deepl::translate_with_deepl_at_url;

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

#[derive(Debug)]
pub enum TranslationProvider {
    DeepL(DeepLConfig),
    Disabled,
}

#[derive(Debug)]
pub struct TranslationSettings {
    pub provider: TranslationProvider,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_settings_disabled() {
        let settings = TranslationSettings {
            provider: TranslationProvider::Disabled,
        };
        assert!(matches!(settings.provider, TranslationProvider::Disabled));
    }

    #[test]
    fn translation_settings_deepl() {
        let config = DeepLConfig {
            api_key: "key".into(),
            source_lang: "KO".into(),
            target_lang: "EN".into(),
        };
        let settings = TranslationSettings {
            provider: TranslationProvider::DeepL(config),
        };
        match &settings.provider {
            TranslationProvider::DeepL(c) => {
                assert_eq!(c.api_key, "key");
            }
            _ => panic!("expected DeepL provider"),
        }
    }

    #[test]
    fn translation_provider_debug() {
        let provider = TranslationProvider::Disabled;
        let debug = format!("{provider:?}");
        assert!(debug.contains("Disabled"));
    }
}
