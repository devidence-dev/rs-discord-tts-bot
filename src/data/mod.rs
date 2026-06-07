//! Persistencia de configuración por guild.
//!
//! Puerto de `internal/data` (bot-go): namespacing de keys `guildID:key` y
//! valores codificados en JSON, para que el esquema lógico sobreviva aunque
//! cambie el backend físico.
//!
//! Decisión de backend (la tabla "Selección de crates" del plan la dejaba
//! abierta): `redb` en vez de bindings a LevelDB en C. No hay datos de
//! producción que migrar — el bot Go nunca llegó a correr en producción por el
//! bloqueo DAVE (ver ../../bot-go/data/, una DB de prueba vacía) — así que no
//! hay razón para cargar con cgo. `sled`, la otra alternativa pure-Rust, está
//! detenida en alpha desde 2024; `redb` es ACID, 100% Rust y mantenida.

mod redb_provider;

pub use redb_provider::RedbProvider;

use serde_json::{Map, Value};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Database(#[from] redb::DatabaseError),
    #[error(transparent)]
    Transaction(#[from] redb::TransactionError),
    #[error(transparent)]
    Table(#[from] redb::TableError),
    #[error(transparent)]
    Storage(#[from] redb::StorageError),
    #[error(transparent)]
    Commit(#[from] redb::CommitError),
    #[error("error codificando/decodificando JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Interfaz de persistencia de configuración, con scoping por guild.
///
/// Puerto de `data.Provider` (bot-go/internal/data/provider.go). `Close` no se
/// porta: en Rust el cierre del recurso lo maneja `Drop`.
pub trait Provider: Send + Sync {
    /// Recupera un valor para un guild y key, o `default` si no existe.
    fn get(&self, guild_id: &str, key: &str, default: Value) -> Result<Value>;

    /// Guarda un valor para un guild y key (codificado como JSON).
    fn set(&self, guild_id: &str, key: &str, value: Value) -> Result<()>;

    /// Elimina un valor puntual.
    fn delete(&self, guild_id: &str, key: &str) -> Result<()>;

    /// Elimina todos los valores almacenados para un guild.
    fn clear(&self, guild_id: &str) -> Result<()>;

    /// Helper para recuperar un valor `string`, con fallback a `default` si no
    /// existe o si el valor almacenado no es un string.
    ///
    /// A diferencia de Go (donde cada `Provider` reimplementa estos helpers),
    /// acá son métodos por defecto del trait construidos sobre `get` — un solo
    /// lugar para la lógica de "tipo equivocado → usar default".
    fn get_string(&self, guild_id: &str, key: &str, default: &str) -> Result<String> {
        match self.get(guild_id, key, Value::String(default.to_string()))? {
            Value::String(s) => Ok(s),
            _ => Ok(default.to_string()),
        }
    }

    /// Helper para recuperar un valor entero. Los números JSON pueden llegar
    /// como flotantes (p. ej. si se guardaron desde otro lenguaje); igual que
    /// el `int(f)` de Go, se truncan hacia cero.
    fn get_int(&self, guild_id: &str, key: &str, default: i64) -> Result<i64> {
        match self.get(guild_id, key, Value::from(default))? {
            Value::Number(n) => Ok(n
                .as_i64()
                .or_else(|| n.as_f64().map(|f| f as i64))
                .unwrap_or(default)),
            _ => Ok(default),
        }
    }

    /// Helper para recuperar un valor `object`, con fallback a `default` si no
    /// existe o si el valor almacenado no es un objeto.
    fn get_map(
        &self,
        guild_id: &str,
        key: &str,
        default: Option<Map<String, Value>>,
    ) -> Result<Option<Map<String, Value>>> {
        let default_value = default.clone().map(Value::Object).unwrap_or(Value::Null);

        match self.get(guild_id, key, default_value)? {
            Value::Object(m) => Ok(Some(m)),
            _ => Ok(default),
        }
    }
}
