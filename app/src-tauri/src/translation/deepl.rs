const DEEPL_API_URL: &str = "https://api-free.deepl.com/v2/translate";

#[derive(Clone, Debug)]
pub struct DeepLConfig {
    pub api_key: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(thiserror::Error, Debug)]
pub enum TranslationError {
    #[error("Translation is not configured")]
    NotConfigured,
    #[error("HTTP request failed: {0}")]
    HttpError(String),
    #[error("DeepL API error: {0}")]
    ApiError(String),
    #[error("Invalid response from DeepL")]
    InvalidResponse,
}

pub async fn translate_with_deepl(
    text: &str,
    config: &DeepLConfig,
) -> Result<String, TranslationError> {
    translate_with_deepl_at_url(text, config, DEEPL_API_URL).await
}

pub(crate) async fn translate_with_deepl_at_url(
    text: &str,
    config: &DeepLConfig,
    url: &str,
) -> Result<String, TranslationError> {
    if text.is_empty() {
        return Err(TranslationError::ApiError("empty text not allowed".into()));
    }

    let client = reqwest::Client::new();
    let params = [
        ("auth_key", config.api_key.as_str()),
        ("text", text),
        ("source_lang", config.source_lang.as_str()),
        ("target_lang", config.target_lang.as_str()),
    ];

    let response = client
        .post(url)
        .form(&params)
        .send()
        .await
        .map_err(|e| TranslationError::HttpError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(TranslationError::ApiError(format!("HTTP {status}: {body}")));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|_| TranslationError::InvalidResponse)?;

    let translation = json["translations"][0]["text"]
        .as_str()
        .ok_or(TranslationError::InvalidResponse)?
        .to_string();

    Ok(translation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepl_config_construction() {
        let config = DeepLConfig {
            api_key: "test-key-123".into(),
            source_lang: "KO".into(),
            target_lang: "EN".into(),
        };
        assert_eq!(config.api_key, "test-key-123");
        assert_eq!(config.source_lang, "KO");
        assert_eq!(config.target_lang, "EN");
    }

    #[test]
    fn translation_error_display() {
        assert_eq!(
            TranslationError::NotConfigured.to_string(),
            "Translation is not configured"
        );
        assert_eq!(
            TranslationError::HttpError("timeout".into()).to_string(),
            "HTTP request failed: timeout"
        );
        assert_eq!(
            TranslationError::ApiError("invalid target lang".into()).to_string(),
            "DeepL API error: invalid target lang"
        );
        assert_eq!(
            TranslationError::InvalidResponse.to_string(),
            "Invalid response from DeepL"
        );
    }

    #[test]
    fn translation_error_debug() {
        let err = TranslationError::ApiError("bad request".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("ApiError"));
        assert!(debug.contains("bad request"));
    }

    #[tokio::test]
    async fn translate_empty_text_returns_error() {
        let config = DeepLConfig {
            api_key: "dummy".into(),
            source_lang: "KO".into(),
            target_lang: "EN".into(),
        };
        let result = translate_with_deepl("", &config).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TranslationError::ApiError(msg) => assert_eq!(msg, "empty text not allowed"),
            other => panic!("expected ApiError, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn translate_success() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", "/translate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"translations":[{"detected_source_language":"KO","text":"Hello"}]}"#)
            .create();

        let config = DeepLConfig {
            api_key: "test-key".into(),
            source_lang: "KO".into(),
            target_lang: "EN".into(),
        };

        let url = format!("{}/translate", server.url());
        let result = translate_with_deepl_at_url("안녕하세요", &config, &url).await;
        assert_eq!(result.unwrap(), "Hello");
        mock.assert();
    }

    #[tokio::test]
    async fn translate_api_error_response() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", "/translate")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"Invalid target language"}"#)
            .create();

        let config = DeepLConfig {
            api_key: "test-key".into(),
            source_lang: "KO".into(),
            target_lang: "INVALID".into(),
        };

        let url = format!("{}/translate", server.url());
        let result = translate_with_deepl_at_url("hello", &config, &url).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TranslationError::ApiError(msg) => {
                assert!(msg.contains("400"));
                assert!(msg.contains("Invalid target language"));
            }
            other => panic!("expected ApiError, got {other:?}"),
        }
        mock.assert();
    }

    #[tokio::test]
    async fn translate_malformed_json() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", "/translate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("not json at all")
            .create();

        let config = DeepLConfig {
            api_key: "test-key".into(),
            source_lang: "KO".into(),
            target_lang: "EN".into(),
        };

        let url = format!("{}/translate", server.url());
        let result = translate_with_deepl_at_url("hello", &config, &url).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TranslationError::InvalidResponse => {}
            other => panic!("expected InvalidResponse, got {other:?}"),
        }
        mock.assert();
    }

    #[tokio::test]
    async fn translate_missing_translation_field() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", "/translate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"translations":[{"detected_source_language":"KO"}]}"#)
            .create();

        let config = DeepLConfig {
            api_key: "test-key".into(),
            source_lang: "KO".into(),
            target_lang: "EN".into(),
        };

        let url = format!("{}/translate", server.url());
        let result = translate_with_deepl_at_url("hello", &config, &url).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TranslationError::InvalidResponse => {}
            other => panic!("expected InvalidResponse, got {other:?}"),
        }
        mock.assert();
    }
}
