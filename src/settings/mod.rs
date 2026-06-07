//! Módulo de gestión de configuración (port de Go `internal/settings`).
//!
//! Lee y escribe configuraciones a nivel de servidor (guild), usuario y canal,
//! y las almacena en caché en memoria ("write-through") para evitar accesos repetidos a la DB.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::data::Provider;

pub const KEY_GUILD_AMAZON: &str = "amazon:settings:default";
pub const KEY_USER_AMAZON_PREFIX: &str = "amazon:settings:user:";
pub const KEY_LOCALE: &str = "locale";
pub const KEY_TIMEOUT: &str = "timeout";
pub const KEY_TTS_CHANNEL_PREFIX: &str = "tts:channel:";

pub const DEFAULT_LOCALE: &str = "en";
pub const DEFAULT_TIMEOUT: i64 = 5; // minutos
pub const MIN_TIMEOUT: i64 = 1;
pub const MAX_TIMEOUT: i64 = 60;

/// Estructura de configuración para el proveedor Amazon TTS.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AmazonSettings {
    pub language: String,
    pub voice: String,
    pub volume: String,
    pub rate: String,
    pub pitch: String,
}

impl Default for AmazonSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            voice: "Amazon US English (Salli)".to_string(),
            volume: "default".to_string(),
            rate: "medium".to_string(),
            pitch: "default".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
enum CacheValue {
    String(String),
    Int(i64),
    Bool(bool),
    Overrides(HashMap<String, String>),
}

/// Gestor de configuraciones del bot.
pub struct Manager {
    db: Arc<dyn Provider>,
    cache: RwLock<HashMap<String, CacheValue>>,
}

fn cache_key(guild_id: &str, key: &str) -> String {
    format!("{guild_id}\x00{key}")
}

impl Manager {
    /// Crea un nuevo `Manager` respaldado por `db`.
    pub fn new(db: Arc<dyn Provider>) -> Self {
        Self {
            db,
            cache: RwLock::new(HashMap::new()),
        }
    }

    fn invalidate_guild(&self, guild_id: &str) {
        let prefix = format!("{guild_id}\x00");
        let mut cache = self.cache.write().unwrap();
        cache.retain(|k, _| !k.starts_with(&prefix));
    }

    /// Obtiene las configuraciones combinadas para una interacción: default -> guild -> user.
    pub fn get_effective(&self, guild_id: &str, user_id: &str) -> AmazonSettings {
        let mut base = AmazonSettings::default();
        if let Ok(guild_overrides) = self.load_overrides(guild_id, KEY_GUILD_AMAZON) {
            apply_overrides(&mut base, &guild_overrides);
        }
        if let Ok(user_overrides) = self.load_overrides(
            guild_id,
            &format!("{KEY_USER_AMAZON_PREFIX}{user_id}"),
        ) {
            apply_overrides(&mut base, &user_overrides);
        }
        base
    }

    /// Obtiene las configuraciones configuradas a nivel de guild fusionadas sobre las por defecto.
    pub fn get_guild(&self, guild_id: &str) -> AmazonSettings {
        let mut base = AmazonSettings::default();
        if let Ok(guild_overrides) = self.load_overrides(guild_id, KEY_GUILD_AMAZON) {
            apply_overrides(&mut base, &guild_overrides);
        }
        base
    }

    /// Actualiza un campo específico de las configuraciones por defecto del servidor.
    pub fn set_guild(&self, guild_id: &str, field: &str, value: &str) -> crate::data::Result<()> {
        let key = KEY_GUILD_AMAZON;
        let mut overrides = self.load_overrides(guild_id, key).unwrap_or_default();
        overrides.insert(field.to_string(), value.to_string());

        let json_val = serde_json::to_value(&overrides)?;
        self.db.set(guild_id, key, json_val)?;

        let ck = cache_key(guild_id, key);
        let mut cache = self.cache.write().unwrap();
        cache.insert(ck, CacheValue::Overrides(overrides));
        Ok(())
    }

    /// Actualiza un campo de las configuraciones personales de un usuario en un servidor.
    pub fn set_user(
        &self,
        guild_id: &str,
        user_id: &str,
        field: &str,
        value: &str,
    ) -> crate::data::Result<()> {
        let key = format!("{KEY_USER_AMAZON_PREFIX}{user_id}");
        let mut overrides = self.load_overrides(guild_id, &key).unwrap_or_default();
        overrides.insert(field.to_string(), value.to_string());

        let json_val = serde_json::to_value(&overrides)?;
        self.db.set(guild_id, &key, json_val)?;

        let ck = cache_key(guild_id, &key);
        let mut cache = self.cache.write().unwrap();
        cache.insert(ck, CacheValue::Overrides(overrides));
        Ok(())
    }

    /// Obtiene el idioma configurado para el bot en un servidor (por defecto "en").
    pub fn get_locale(&self, guild_id: &str) -> String {
        let ck = cache_key(guild_id, KEY_LOCALE);
        {
            let cache = self.cache.read().unwrap();
            if let Some(CacheValue::String(s)) = cache.get(&ck) {
                return s.clone();
            }
        }

        let val = self
            .db
            .get_string(guild_id, KEY_LOCALE, DEFAULT_LOCALE)
            .unwrap_or_else(|_| DEFAULT_LOCALE.to_string());

        let mut cache = self.cache.write().unwrap();
        cache.insert(ck, CacheValue::String(val.clone()));
        val
    }

    /// Guarda el idioma configurado para el bot en un servidor.
    pub fn set_locale(&self, guild_id: &str, locale: &str) -> crate::data::Result<()> {
        self.db
            .set(guild_id, KEY_LOCALE, serde_json::Value::from(locale))?;

        let ck = cache_key(guild_id, KEY_LOCALE);
        let mut cache = self.cache.write().unwrap();
        cache.insert(ck, CacheValue::String(locale.to_string()));
        Ok(())
    }

    /// Obtiene el tiempo límite de inactividad (en minutos) para un servidor.
    pub fn get_timeout(&self, guild_id: &str) -> i64 {
        let ck = cache_key(guild_id, KEY_TIMEOUT);
        {
            let cache = self.cache.read().unwrap();
            if let Some(CacheValue::Int(i)) = cache.get(&ck) {
                return *i;
            }
        }

        let val = self
            .db
            .get_int(guild_id, KEY_TIMEOUT, DEFAULT_TIMEOUT)
            .unwrap_or(DEFAULT_TIMEOUT);

        let mut cache = self.cache.write().unwrap();
        cache.insert(ck, CacheValue::Int(val));
        val
    }

    /// Guarda el tiempo límite de inactividad (en minutos) para un servidor.
    pub fn set_timeout(&self, guild_id: &str, minutes: i64) -> crate::data::Result<()> {
        self.db
            .set(guild_id, KEY_TIMEOUT, serde_json::Value::from(minutes))?;

        let ck = cache_key(guild_id, KEY_TIMEOUT);
        let mut cache = self.cache.write().unwrap();
        cache.insert(ck, CacheValue::Int(minutes));
        Ok(())
    }

    /// Comprueba si un canal específico tiene habilitado el TTS automático.
    pub fn is_tts_channel(&self, guild_id: &str, channel_id: &str) -> bool {
        let key = format!("{KEY_TTS_CHANNEL_PREFIX}{channel_id}");
        let ck = cache_key(guild_id, &key);
        {
            let cache = self.cache.read().unwrap();
            if let Some(CacheValue::Bool(b)) = cache.get(&ck) {
                return *b;
            }
        }

        let val = self
            .db
            .get(guild_id, &key, serde_json::Value::from(false))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut cache = self.cache.write().unwrap();
        cache.insert(ck, CacheValue::Bool(val));
        val
    }

    /// Habilita o deshabilita el TTS automático para un canal específico.
    pub fn set_tts_channel(
        &self,
        guild_id: &str,
        channel_id: &str,
        enabled: bool,
    ) -> crate::data::Result<()> {
        let key = format!("{KEY_TTS_CHANNEL_PREFIX}{channel_id}");
        self.db
            .set(guild_id, &key, serde_json::Value::from(enabled))?;

        let ck = cache_key(guild_id, &key);
        let mut cache = self.cache.write().unwrap();
        cache.insert(ck, CacheValue::Bool(enabled));
        Ok(())
    }

    /// Elimina toda la información guardada para un servidor (cuando el bot abandona el guild).
    pub fn clear_guild(&self, guild_id: &str) -> crate::data::Result<()> {
        self.db.clear(guild_id)?;
        self.invalidate_guild(guild_id);
        Ok(())
    }

    fn load_overrides(
        &self,
        guild_id: &str,
        key: &str,
    ) -> Result<HashMap<String, String>, crate::data::Error> {
        let ck = cache_key(guild_id, key);
        {
            let cache = self.cache.read().unwrap();
            if let Some(CacheValue::Overrides(map)) = cache.get(&ck) {
                return Ok(map.clone());
            }
        }

        let raw = self.db.get_map(guild_id, key, None)?;
        let mut result = HashMap::new();
        if let Some(map) = raw {
            for (k, v) in map {
                if let serde_json::Value::String(s) = v {
                    result.insert(k, s);
                }
            }
        }

        let mut cache = self.cache.write().unwrap();
        cache.insert(ck, CacheValue::Overrides(result.clone()));
        Ok(result)
    }
}

fn apply_overrides(base: &mut AmazonSettings, overrides: &HashMap<String, String>) {
    if let Some(v) = overrides.get("language") {
        if !v.is_empty() {
            base.language = v.clone();
        }
    }
    if let Some(v) = overrides.get("voice") {
        if !v.is_empty() {
            base.voice = v.clone();
        }
    }
    if let Some(v) = overrides.get("volume") {
        if !v.is_empty() {
            base.volume = v.clone();
        }
    }
    if let Some(v) = overrides.get("rate") {
        if !v.is_empty() {
            base.rate = v.clone();
        }
    }
    if let Some(v) = overrides.get("pitch") {
        if !v.is_empty() {
            base.pitch = v.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::RedbProvider;
    use tempfile::TempDir;

    fn new_manager(temp_dir: &TempDir) -> (Manager, Arc<RedbProvider>) {
        let db_path = temp_dir.path().join("test.redb");
        let provider = Arc::new(RedbProvider::open(db_path).expect("failed to open database"));
        let manager = Manager::new(provider.clone());
        (manager, provider)
    }

    #[test]
    fn test_get_effective_defaults_when_nothing_stored() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        let got = m.get_effective("guild1", "user1");
        assert_eq!(got, AmazonSettings::default());
    }

    #[test]
    fn test_get_effective_guild_override_applies_over_defaults() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        m.set_guild("guild1", "language", "es").unwrap();

        let got = m.get_effective("guild1", "user1");
        assert_eq!(got.language, "es");
        assert_eq!(got.voice, AmazonSettings::default().voice);
    }

    #[test]
    fn test_get_effective_user_override_applies_over_guild() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        m.set_guild("guild1", "language", "es").unwrap();
        m.set_user("guild1", "user1", "language", "en").unwrap();

        let got = m.get_effective("guild1", "user1");
        assert_eq!(got.language, "en");

        // Otro usuario del mismo guild debería ver el valor del guild
        let other = m.get_effective("guild1", "user2");
        assert_eq!(other.language, "es");
    }

    #[test]
    fn test_get_effective_overrides_are_scoped_per_guild() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        m.set_guild("guild1", "voice", "Amazon Castilian Spanish (Conchita)").unwrap();

        let got = m.get_effective("guild2", "user1");
        assert_eq!(got.voice, AmazonSettings::default().voice);
    }

    #[test]
    fn test_get_guild_returns_override_merged_with_defaults() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        m.set_guild("guild1", "rate", "fast").unwrap();

        let got = m.get_guild("guild1");
        assert_eq!(got.rate, "fast");
        assert_eq!(got.pitch, AmazonSettings::default().pitch);
    }

    #[test]
    fn test_set_guild_persists_multiple_fields_independently() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        m.set_guild("guild1", "language", "es").unwrap();
        m.set_guild("guild1", "volume", "loud").unwrap();

        let got = m.get_guild("guild1");
        assert_eq!(got.language, "es");
        assert_eq!(got.volume, "loud");
    }

    #[test]
    fn test_set_user_does_not_affect_other_users() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        m.set_user("guild1", "user1", "pitch", "high").unwrap();

        let got1 = m.get_effective("guild1", "user1");
        assert_eq!(got1.pitch, "high");

        let got2 = m.get_effective("guild1", "user2");
        assert_eq!(got2.pitch, AmazonSettings::default().pitch);
    }

    #[test]
    fn test_locale_default_and_round_trip() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        assert_eq!(m.get_locale("guild1"), DEFAULT_LOCALE);

        m.set_locale("guild1", "es").unwrap();
        assert_eq!(m.get_locale("guild1"), "es");
    }

    #[test]
    fn test_timeout_default_and_round_trip() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        assert_eq!(m.get_timeout("guild1"), DEFAULT_TIMEOUT);

        m.set_timeout("guild1", 15).unwrap();
        assert_eq!(m.get_timeout("guild1"), 15);
    }

    #[test]
    fn test_clear_guild_removes_stored_overrides() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        m.set_guild("guild1", "language", "es").unwrap();
        m.set_locale("guild1", "es").unwrap();

        m.clear_guild("guild1").unwrap();

        assert_eq!(m.get_effective("guild1", "user1"), AmazonSettings::default());
        assert_eq!(m.get_locale("guild1"), DEFAULT_LOCALE);
    }

    #[test]
    fn test_apply_overrides_empty_values_do_not_override_defaults() {
        let mut base = AmazonSettings::default();
        let mut overrides = HashMap::new();
        overrides.insert("language".to_string(), "".to_string());
        overrides.insert("voice".to_string(), "Amazon Castilian Spanish (Conchita)".to_string());

        apply_overrides(&mut base, &overrides);

        assert_eq!(base.language, AmazonSettings::default().language);
        assert_eq!(base.voice, "Amazon Castilian Spanish (Conchita)");
    }

    #[test]
    fn test_apply_overrides_does_not_mutate_base_unless_called_on_it() {
        let base = AmazonSettings::default();
        let mut copy = base.clone();
        let mut overrides = HashMap::new();
        overrides.insert("language".to_string(), "es".to_string());

        apply_overrides(&mut copy, &overrides);

        assert_eq!(base.language, AmazonSettings::default().language);
        assert_eq!(copy.language, "es");
    }

    #[test]
    fn test_cache_get_locale_serves_from_cache_without_hitting_db() {
        let dir = TempDir::new().unwrap();
        let (m, db) = new_manager(&dir);

        assert_eq!(m.get_locale("guild1"), DEFAULT_LOCALE);

        // Modificamos directamente en la DB subyacente.
        // Si el manager volviese a consultar la DB, vería "es".
        // Pero al estar cacheado, debería devolver el valor por defecto en caché ("en").
        db.set("guild1", KEY_LOCALE, serde_json::Value::from("es")).unwrap();
        assert_eq!(m.get_locale("guild1"), DEFAULT_LOCALE);
    }

    #[test]
    fn test_cache_set_guild_serves_from_cache_after_underlying_delete() {
        let dir = TempDir::new().unwrap();
        let (m, db) = new_manager(&dir);

        m.set_guild("guild1", "voice", "Amazon Castilian Spanish (Conchita)").unwrap();

        // Eliminamos el valor de la base de datos subyacente.
        // Un hit a la caché debería seguir retornando lo guardado en caché.
        db.delete("guild1", KEY_GUILD_AMAZON).unwrap();

        let got = m.get_guild("guild1");
        assert_eq!(got.voice, "Amazon Castilian Spanish (Conchita)");
    }

    #[test]
    fn test_cache_clear_guild_invalidates_only_that_guild() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        m.set_locale("guild1", "es").unwrap();
        m.set_locale("guild2", "es").unwrap();

        m.clear_guild("guild1").unwrap();

        assert_eq!(m.get_locale("guild1"), DEFAULT_LOCALE);
        assert_eq!(m.get_locale("guild2"), "es");
    }

    #[test]
    fn test_tts_channel_defaults_to_disabled_and_round_trips() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        assert!(!m.is_tts_channel("guild1", "chan1"));

        m.set_tts_channel("guild1", "chan1", true).unwrap();
        assert!(m.is_tts_channel("guild1", "chan1"));

        m.set_tts_channel("guild1", "chan1", false).unwrap();
        assert!(!m.is_tts_channel("guild1", "chan1"));
    }

    #[test]
    fn test_tts_channel_scoped_per_guild_and_channel() {
        let dir = TempDir::new().unwrap();
        let (m, _) = new_manager(&dir);

        m.set_tts_channel("guild1", "chan1", true).unwrap();

        assert!(!m.is_tts_channel("guild1", "chan2"));
        assert!(!m.is_tts_channel("guild2", "chan1"));
    }
}
