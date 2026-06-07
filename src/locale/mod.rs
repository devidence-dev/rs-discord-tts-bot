//! Módulo de localización (port de Go `internal/locale`).
//!
//! Provee el struct [`Localizer`] para resolver y formatear los mensajes
//! de la aplicación en inglés y español.

mod en;
mod es;

use std::collections::HashMap;

/// Devuelve el listado de códigos de idioma soportados.
pub fn supported() -> &'static [&'static str] {
    &["en", "es"]
}

/// Comprueba si el código de idioma indicado está soportado.
pub fn is_supported(locale: &str) -> bool {
    locale == "en" || locale == "es"
}

/// Permite resolver claves de traducción aplicando interpolación de placeholders.
#[derive(Debug, Clone)]
pub struct Localizer {
    locale: String,
    strings: &'static HashMap<&'static str, &'static str>,
}

impl Localizer {
    /// Crea un nuevo `Localizer` para el idioma indicado, con fallback a "en".
    pub fn new(locale: &str) -> Self {
        let (loc, strings) = match locale {
            "es" => ("es", &*es::ES_STRINGS),
            _ => ("en", &*en::EN_STRINGS),
        };
        Self {
            locale: loc.to_string(),
            strings,
        }
    }

    /// Devuelve el idioma del localizador ("en" o "es").
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Busca la traducción correspondiente a `key` y substituye los marcadores
    /// de posición `{variable}` definidos en `vars`.
    pub fn t(&self, key: &str, vars: &[(&str, &str)]) -> String {
        let Some(mut s) = self.strings.get(key).copied().map(String::from) else {
            return key.to_string();
        };

        for &(k, v) in vars {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let cases = [
            ("en", "en"),
            ("es", "es"),
            ("fr", "en"),
            ("", "en"),
        ];

        for (input, expected) in cases {
            let l = Localizer::new(input);
            assert_eq!(
                l.locale(),
                expected,
                "Localizer::new({input:?}).locale() = {l_loc:?}, want {expected:?}",
                l_loc = l.locale()
            );
        }
    }

    #[test]
    fn test_t_looks_up_known_key() {
        let l = Localizer::new("en");
        let got = l.t("command.stop.no_connection", &[]);
        let want = "I'm not in a voice channel.";
        assert_eq!(got, want);
    }

    #[test]
    fn test_t_falls_back_to_key_when_missing() {
        let l = Localizer::new("en");
        let got = l.t("this.key.does.not.exist", &[]);
        let want = "this.key.does.not.exist";
        assert_eq!(got, want);
    }

    #[test]
    fn test_t_substitutes_placeholders() {
        let l = Localizer::new("en");
        let got = l.t("command.say.success", &[("request", "hello world")]);
        let want = "Saying \"hello world\".";
        assert_eq!(got, want);
    }

    #[test]
    fn test_t_substitutes_multiple_placeholders() {
        let l = Localizer::new("en");
        let got = l.t(
            "command.settings.my.language.success",
            &[("language", "Spanish"), ("voice", "Conchita")],
        );
        let want = "Your language changed to **Spanish** with **Conchita**'s voice.";
        assert_eq!(got, want);
    }

    #[test]
    fn test_t_without_vars_leaves_placeholders_intact() {
        let l = Localizer::new("en");
        let got = l.t("command.say.success", &[]);
        let want = "Saying \"{request}\".";
        assert_eq!(got, want);
    }

    #[test]
    fn test_t_spanish_translation() {
        let l = Localizer::new("es");
        let got = l.t("command.stop.no_connection", &[]);
        let want = "No estoy en un canal de voz.";
        assert_eq!(got, want);
    }

    #[test]
    fn test_is_supported() {
        assert!(is_supported("en"));
        assert!(is_supported("es"));
        assert!(!is_supported("fr"));
        assert!(!is_supported(""));
    }

    #[test]
    fn test_supported() {
        assert_eq!(supported(), &["en", "es"]);
    }

    #[test]
    fn test_locales_have_matching_keys() {
        for key in en::EN_STRINGS.keys() {
            assert!(
                es::ES_STRINGS.contains_key(key),
                "key '{}' exists in en but is missing in es",
                key
            );
        }
        for key in es::ES_STRINGS.keys() {
            assert!(
                en::EN_STRINGS.contains_key(key),
                "key '{}' exists in es but is missing in en",
                key
            );
        }
    }
}
