pub mod deepl;

pub use deepl::{DeepLConfig, TranslationError};

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
