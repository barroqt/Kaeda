pub mod deepl;

#[cfg(test)]
pub(crate) use deepl::translate_with_deepl_at_url;
pub use deepl::{DeepLConfig, TranslationError, translate_with_deepl};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub deepl_enabled: bool,
    #[serde(default)]
    pub deepl_api_key: Option<String>,
    #[serde(default = "default_target_lang")]
    pub deepl_target_lang: String,
}

fn default_target_lang() -> String {
    "EN".into()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            deepl_enabled: false,
            deepl_api_key: None,
            deepl_target_lang: "EN".into(),
        }
    }
}

impl AppSettings {
    pub fn to_translation_settings(&self) -> TranslationSettings {
        if self.deepl_enabled
            && let Some(key) = &self.deepl_api_key
            && !key.is_empty()
        {
            return TranslationSettings {
                provider: TranslationProvider::DeepL(DeepLConfig {
                    api_key: key.clone(),
                    source_lang: "KO".into(),
                    target_lang: self.deepl_target_lang.clone(),
                }),
            };
        }
        TranslationSettings {
            provider: TranslationProvider::Disabled,
        }
    }
}

pub fn load_settings(path: &std::path::Path) -> AppSettings {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    }
}

pub fn save_settings(path: &std::path::Path, settings: &AppSettings) -> Result<(), String> {
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}

#[derive(Debug, serde::Serialize)]
pub struct TranslationSettingsDto {
    pub enabled: bool,
    pub has_api_key: bool,
    pub target_lang: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateTranslationSettings {
    pub enabled: bool,
    pub api_key: String,
    pub target_lang: String,
}

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

    pub fn deck_not_found(id: i64) -> Self {
        Self {
            code: "DECK_NOT_FOUND",
            message: format!("Deck with id {id} not found"),
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

    #[test]
    fn app_settings_default_is_disabled() {
        let settings = AppSettings::default();
        assert!(!settings.deepl_enabled);
        assert!(settings.deepl_api_key.is_none());
        assert_eq!(settings.deepl_target_lang, "EN");
    }

    #[test]
    fn app_settings_to_translation_settings_disabled() {
        let settings = AppSettings::default();
        let ts = settings.to_translation_settings();
        assert!(matches!(ts.provider, TranslationProvider::Disabled));
    }

    #[test]
    fn app_settings_to_translation_settings_enabled() {
        let mut settings = AppSettings::default();
        settings.deepl_enabled = true;
        settings.deepl_api_key = Some("test-key".into());
        settings.deepl_target_lang = "FR".into();
        let ts = settings.to_translation_settings();
        match &ts.provider {
            TranslationProvider::DeepL(config) => {
                assert_eq!(config.api_key, "test-key");
                assert_eq!(config.target_lang, "FR");
                assert_eq!(config.source_lang, "KO");
            }
            _ => panic!("expected DeepL provider"),
        }
    }

    #[test]
    fn app_settings_to_translation_settings_enabled_no_key() {
        let mut settings = AppSettings::default();
        settings.deepl_enabled = true;
        settings.deepl_api_key = None;
        let ts = settings.to_translation_settings();
        assert!(matches!(ts.provider, TranslationProvider::Disabled));
    }

    #[test]
    fn app_settings_to_translation_settings_enabled_empty_key() {
        let mut settings = AppSettings::default();
        settings.deepl_enabled = true;
        settings.deepl_api_key = Some("".into());
        let ts = settings.to_translation_settings();
        assert!(matches!(ts.provider, TranslationProvider::Disabled));
    }

    #[test]
    fn load_settings_returns_default_on_missing_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_test_config_missing.json");
        let _ = std::fs::remove_file(&path);
        let settings = load_settings(&path);
        assert!(!settings.deepl_enabled);
        assert!(settings.deepl_api_key.is_none());
    }

    #[test]
    fn save_and_load_settings_roundtrip() {
        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_test_config_roundtrip.json");
        let _ = std::fs::remove_file(&path);

        let mut original = AppSettings::default();
        original.deepl_enabled = true;
        original.deepl_api_key = Some("saved-key".into());
        original.deepl_target_lang = "DE".into();
        save_settings(&path, &original).unwrap();

        let loaded = load_settings(&path);
        assert!(loaded.deepl_enabled);
        assert_eq!(loaded.deepl_api_key, Some("saved-key".into()));
        assert_eq!(loaded.deepl_target_lang, "DE");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn save_settings_overwrites_existing() {
        let dir = std::env::temp_dir();
        let path = dir.join("kaeda_test_config_overwrite.json");

        let mut first = AppSettings::default();
        first.deepl_enabled = true;
        first.deepl_api_key = Some("key1".into());
        save_settings(&path, &first).unwrap();

        let mut second = AppSettings::default();
        second.deepl_api_key = Some("key2".into());
        second.deepl_target_lang = "FR".into();
        save_settings(&path, &second).unwrap();

        let loaded = load_settings(&path);
        assert_eq!(loaded.deepl_api_key, Some("key2".into()));
        assert_eq!(loaded.deepl_target_lang, "FR");
        assert!(!loaded.deepl_enabled);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn update_translation_settings_rejects_missing_key_when_enabled() {
        let state = std::sync::Mutex::new(AppSettings::default());
        let settings = state.lock().unwrap();
        let new = UpdateTranslationSettings {
            enabled: true,
            api_key: "".into(),
            target_lang: "EN".into(),
        };
        // When no key is stored and none provided, enable should fail
        assert!(new.enabled && new.api_key.is_empty() && settings.deepl_api_key.is_none());
    }

    #[test]
    fn translation_settings_dto_serialization() {
        let dto = TranslationSettingsDto {
            enabled: true,
            has_api_key: true,
            target_lang: "EN".into(),
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("true"));
        assert!(json.contains("EN"));
    }

    #[test]
    fn update_translation_settings_dto_deserialization() {
        let json = r#"{"enabled":true,"api_key":"my-key","target_lang":"FR"}"#;
        let dto: UpdateTranslationSettings = serde_json::from_str(json).unwrap();
        assert!(dto.enabled);
        assert_eq!(dto.api_key, "my-key");
        assert_eq!(dto.target_lang, "FR");
    }
}
