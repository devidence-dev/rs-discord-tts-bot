//! Módulo del proveedor Amazon TTS (port de Go `internal/provider`).
//!
//! Se conecta con la API externa de TTS Tool para generar URLs de audio a partir de textos.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Estructura interna de una voz de Amazon TTS.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Voice {
    pub name: String,
    pub emoji: String,
    pub id: String,
}

/// Estructura de un idioma soportado por Amazon TTS.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Language {
    pub name: String,
    pub emoji: String,
    pub id: String, // Código locale BCP-47 para xml:lang en SSML
    pub voices: Vec<Voice>,
}

/// Mapa estático que contiene los idiomas y voces soportados, cargado desde el JSON embebido.
pub static LANGUAGES: LazyLock<HashMap<String, Language>> = LazyLock::new(|| {
    let data = include_str!("amazon_languages.json");
    serde_json::from_str(data).expect("failed to parse Amazon language data")
});

/// Opciones permitidas de volumen.
pub fn volume_choices() -> &'static [(&'static str, &'static str)] {
    &[
        ("Default Volume", "default"),
        ("Silent", "silent"),
        ("Extra Soft", "x-soft"),
        ("Soft", "soft"),
        ("Medium", "medium"),
        ("Loud", "loud"),
        ("Extra Loud", "x-loud"),
    ]
}

/// Opciones permitidas de velocidad de habla.
pub fn rate_choices() -> &'static [(&'static str, &'static str)] {
    &[
        ("Extra Slow", "x-slow"),
        ("Slow", "slow"),
        ("Medium", "medium"),
        ("Fast", "fast"),
        ("Extra Fast", "x-fast"),
    ]
}

/// Opciones permitidas de tono.
pub fn pitch_choices() -> &'static [(&'static str, &'static str)] {
    &[
        ("Default Pitch", "default"),
        ("Extra Low", "x-low"),
        ("Low", "low"),
        ("Medium", "medium"),
        ("High", "high"),
        ("Extra High", "x-high"),
    ]
}

/// Errores posibles del proveedor TTS.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("provider: unsupported language {0:?}")]
    UnsupportedLanguage(String),

    #[error("provider: createParts request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("provider: createParts returned status {0}")]
    NonOkStatus(reqwest::StatusCode),

    #[error("provider: unexpected createParts response: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("provider: createParts returned empty parts list")]
    EmptyPartsList,

    #[error("provider: invalid URL: {0}")]
    InvalidUrl(String),
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatePartsRequest<'a> {
    voice_id: &'a str,
    ssml: &'a str,
}

/// Proveedor de servicios Amazon TTS.
#[derive(Debug, Clone)]
pub struct AmazonProvider {
    client: reqwest::Client,
    create_parts_url: String,
    get_parts_url: String,
}

impl Default for AmazonProvider {
    fn default() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build default reqwest client");

        Self {
            client,
            create_parts_url: "https://support.readaloud.app/ttstool/createParts".to_string(),
            get_parts_url: "https://support.readaloud.app/ttstool/getParts".to_string(),
        }
    }
}

impl AmazonProvider {
    /// Crea una nueva instancia de `AmazonProvider` con URLs e HttpClient configurados a medida.
    pub fn new(create_parts_url: String, get_parts_url: String, client: reqwest::Client) -> Self {
        Self {
            client,
            create_parts_url,
            get_parts_url,
        }
    }

    /// Obtiene el cliente HTTP `reqwest::Client` subyacente.
    pub fn client(&self) -> reqwest::Client {
        self.client.clone()
    }


    /// Llama a la API externa para obtener la URL del recurso de audio generado por TTS.
    pub async fn get_audio_url(
        &self,
        sentence: &str,
        lang_code: &str,
        voice_id: &str,
        volume: &str,
        rate: &str,
        pitch: &str,
    ) -> Result<String, Error> {
        let lang = LANGUAGES
            .get(lang_code)
            .ok_or_else(|| Error::UnsupportedLanguage(lang_code.to_string()))?;

        let ssml = format!(
            r#"<speak version="1.0" xml:lang="{}"><prosody volume="{}" rate="{}" pitch="{}">{}</prosody></speak>"#,
            lang.id, volume, rate, pitch, sentence
        );

        let req_body = vec![CreatePartsRequest {
            voice_id,
            ssml: &ssml,
        }];

        let resp = self
            .client
            .post(&self.create_parts_url)
            .header("content-type", "application/json")
            .json(&req_body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(Error::NonOkStatus(resp.status()));
        }

        let text = resp.text().await?;
        let parts: Vec<String> = serde_json::from_str(&text)?;
        if parts.is_empty() {
            return Err(Error::EmptyPartsList);
        }

        let mut url = reqwest::Url::parse(&self.get_parts_url)
            .map_err(|e| Error::InvalidUrl(e.to_string()))?;
        url.query_pairs_mut().append_pair("q", &parts[0]);

        Ok(url.to_string())

    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_audio_url_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/createParts")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"["part-abc123"]"#)
            .create_async()
            .await;

        let provider = AmazonProvider::new(
            format!("{}/createParts", server.url()),
            format!("{}/getParts", server.url()),
            reqwest::Client::new(),
        );

        let got = provider
            .get_audio_url(
                "hello world",
                "en",
                "Amazon US English (Salli)",
                "default",
                "medium",
                "default",
            )
            .await
            .unwrap();

        let want = format!("{}/getParts?q=part-abc123", server.url());
        assert_eq!(got, want);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_audio_url_request_body_contains_expected_ssml() {
        let mut server = mockito::Server::new_async().await;

        let expected_json = serde_json::json!([
            {
                "voiceId": "Amazon Castilian Spanish (Conchita)",
                "ssml": "<speak version=\"1.0\" xml:lang=\"es-ES\"><prosody volume=\"loud\" rate=\"fast\" pitch=\"high\">hi <there></prosody></speak>"
            }
        ]);

        let mock = server
            .mock("POST", "/createParts")
            .match_body(mockito::Matcher::Json(expected_json))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"["part-id"]"#)
            .create_async()
            .await;

        let provider = AmazonProvider::new(
            format!("{}/createParts", server.url()),
            format!("{}/getParts", server.url()),
            reqwest::Client::new(),
        );

        let _ = provider
            .get_audio_url(
                "hi <there>",
                "es",
                "Amazon Castilian Spanish (Conchita)",
                "loud",
                "fast",
                "high",
            )
            .await
            .unwrap();

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_audio_url_unsupported_language() {
        let provider = AmazonProvider::default();
        let err = provider
            .get_audio_url(
                "hello",
                "xx-not-a-lang",
                "voice",
                "default",
                "medium",
                "default",
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("unsupported language"),
            "unexpected error msg: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_get_audio_url_non_ok_status() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/createParts")
            .with_status(500)
            .create_async()
            .await;

        let provider = AmazonProvider::new(
            format!("{}/createParts", server.url()),
            format!("{}/getParts", server.url()),
            reqwest::Client::new(),
        );

        let err = provider
            .get_audio_url(
                "hello",
                "en",
                "voice",
                "default",
                "medium",
                "default",
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("500"),
            "unexpected error msg: {}",
            err
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_audio_url_empty_parts_list() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/createParts")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create_async()
            .await;

        let provider = AmazonProvider::new(
            format!("{}/createParts", server.url()),
            format!("{}/getParts", server.url()),
            reqwest::Client::new(),
        );

        let err = provider
            .get_audio_url(
                "hello",
                "en",
                "voice",
                "default",
                "medium",
                "default",
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("empty parts list"),
            "unexpected error msg: {}",
            err
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_audio_url_malformed_response() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/createParts")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"not": "an array"}"#)
            .create_async()
            .await;

        let provider = AmazonProvider::new(
            format!("{}/createParts", server.url()),
            format!("{}/getParts", server.url()),
            reqwest::Client::new(),
        );

        let err = provider
            .get_audio_url(
                "hello",
                "en",
                "voice",
                "default",
                "medium",
                "default",
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("unexpected createParts response"),
            "unexpected error msg: {}",
            err
        );
        mock.assert_async().await;
    }

    #[test]
    fn test_languages_embedded() {
        for code in &["en", "es"] {
            let lang = LANGUAGES.get(*code).expect("language missing");
            assert!(!lang.id.is_empty(), "id is empty");
            assert!(!lang.voices.is_empty(), "voices list is empty");
            for voice in &lang.voices {
                assert!(!voice.id.is_empty(), "voice id is empty");
            }
        }
    }

    #[test]
    fn test_volume_choices() {
        let choices = volume_choices();
        assert!(!choices.is_empty());
        assert!(choices.iter().any(|c| c.1 == "default"));
        assert!(choices.iter().any(|c| c.1 == "silent"));
    }

    #[test]
    fn test_rate_choices() {
        let choices = rate_choices();
        assert!(!choices.is_empty());
        assert!(choices.iter().any(|c| c.1 == "medium"));
        assert!(choices.iter().any(|c| c.1 == "x-slow"));
    }

    #[test]
    fn test_pitch_choices() {
        let choices = pitch_choices();
        assert!(!choices.is_empty());
        assert!(choices.iter().any(|c| c.1 == "default"));
        assert!(choices.iter().any(|c| c.1 == "x-high"));
    }
}
