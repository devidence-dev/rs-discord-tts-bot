use std::{collections::BTreeSet, path::Path};

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde_json::Value;

use super::{Provider, Result};

const TABLE: TableDefinition<&str, &str> = TableDefinition::new("settings");

/// Implementación de [`Provider`] sobre `redb`, un store embebido ACID 100% Rust.
///
/// Puerto de `LevelProvider` (bot-go/internal/data/level.go): mismo esquema de
/// keys `"{guildID}:{key}"` y valores codificados en JSON — la tabla de `redb`
/// es, en esencia, un mapa ordenado de `&str` a `&str` (JSON).
pub struct RedbProvider {
    db: Database,
}

impl RedbProvider {
    /// Abre (o crea) la base de datos en `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = Database::create(path)?;

        // Crea la tabla si todavía no existe — `open_table` la crea de forma
        // implícita en una transacción de escritura.
        let write_txn = db.begin_write()?;
        {
            write_txn.open_table(TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self { db })
    }

    fn build_key(guild_id: &str, key: &str) -> String {
        format!("{guild_id}:{key}")
    }

    /// Devuelve todos los `guildID` que tienen al menos un valor almacenado.
    ///
    /// Puerto de `LevelProvider.ListGuilds` — no forma parte del trait
    /// `Provider` en Go tampoco (solo `LevelProvider` lo expone).
    pub fn list_guilds(&self) -> Result<Vec<String>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TABLE)?;

        let mut guilds = BTreeSet::new();
        for entry in table.iter()? {
            let (db_key, _value) = entry?;
            if let Some((guild_id, _)) = db_key.value().split_once(':') {
                guilds.insert(guild_id.to_string());
            }
        }

        Ok(guilds.into_iter().collect())
    }
}

impl Provider for RedbProvider {
    fn get(&self, guild_id: &str, key: &str, default: Value) -> Result<Value> {
        let db_key = Self::build_key(guild_id, key);

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TABLE)?;

        match table.get(db_key.as_str())? {
            Some(guard) => Ok(serde_json::from_str(guard.value())?),
            None => Ok(default),
        }
    }

    fn set(&self, guild_id: &str, key: &str, value: Value) -> Result<()> {
        let db_key = Self::build_key(guild_id, key);
        let encoded = serde_json::to_string(&value)?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE)?;
            table.insert(db_key.as_str(), encoded.as_str())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    fn delete(&self, guild_id: &str, key: &str) -> Result<()> {
        let db_key = Self::build_key(guild_id, key);

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE)?;
            table.remove(db_key.as_str())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    fn clear(&self, guild_id: &str) -> Result<()> {
        let prefix = format!("{guild_id}:");

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE)?;

            // `range` toma prestado `&table`, así que recolectamos las keys a
            // borrar (como `String` propios) antes de soltar el iterador y
            // pedir el préstamo mutable que necesita `remove`.
            let keys_to_remove = {
                let mut keys = Vec::new();
                for entry in table.range::<&str>(prefix.as_str()..)? {
                    let (db_key, _value) = entry?;
                    let db_key = db_key.value();

                    if !db_key.starts_with(prefix.as_str()) {
                        break;
                    }

                    keys.push(db_key.to_string());
                }
                keys
            };

            for db_key in &keys_to_remove {
                table.remove(db_key.as_str())?;
            }
        }
        write_txn.commit()?;

        Ok(())
    }
}

/// Puerto de `level_test.go` (bot-go/internal/data) — la especificación
/// ejecutable del comportamiento esperado de `Provider`/`LevelProvider`.
#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::*;

    fn test_provider() -> (TempDir, RedbProvider) {
        let dir = TempDir::new().expect("no se pudo crear el tempdir");
        let provider = RedbProvider::open(dir.path().join("test.redb")).expect("open falló");
        (dir, provider)
    }

    #[test]
    fn open_creates_a_usable_database() {
        let (_dir, provider) = test_provider();

        assert_eq!(
            provider.get_string("guild123", "missing", "default").unwrap(),
            "default"
        );
    }

    #[test]
    fn set_and_get_string() {
        let (_dir, provider) = test_provider();

        provider.set("guild123", "provider", Value::from("Google")).unwrap();

        assert_eq!(
            provider.get_string("guild123", "provider", "default").unwrap(),
            "Google"
        );
    }

    #[test]
    fn get_returns_default_for_missing_key() {
        let (_dir, provider) = test_provider();

        assert_eq!(
            provider.get_string("guild123", "nonexistent", "mydefault").unwrap(),
            "mydefault"
        );
    }

    #[test]
    fn set_and_get_int() {
        let (_dir, provider) = test_provider();

        provider.set("guild123", "timeout", Value::from(30)).unwrap();

        assert_eq!(provider.get_int("guild123", "timeout", 5).unwrap(), 30);
    }

    #[test]
    fn set_and_get_bool_via_get() {
        let (_dir, provider) = test_provider();

        provider.set("guild123", "enabled", Value::from(true)).unwrap();

        let value = provider.get("guild123", "enabled", Value::from(false)).unwrap();
        assert_eq!(value, Value::from(true));
    }

    #[test]
    fn delete_removes_a_value() {
        let (_dir, provider) = test_provider();

        provider.set("guild123", "key", Value::from("value")).unwrap();
        provider.delete("guild123", "key").unwrap();

        assert_eq!(
            provider.get_string("guild123", "key", "default").unwrap(),
            "default",
            "debería volver a dar el default después del delete"
        );
    }

    #[test]
    fn set_and_get_map() {
        let (_dir, provider) = test_provider();

        let settings = json!({
            "provider": "Amazon",
            "language": "es",
            "speed": "fast",
        });
        provider.set("guild123", "settings", settings).unwrap();

        let value = provider
            .get_map("guild123", "settings", None)
            .unwrap()
            .expect("debería encontrar el mapa guardado");

        assert_eq!(value.get("provider"), Some(&Value::from("Amazon")));
    }

    #[test]
    fn data_persists_across_reopens() {
        let dir = TempDir::new().expect("no se pudo crear el tempdir");
        let db_path = dir.path().join("test.redb");

        {
            let provider = RedbProvider::open(&db_path).expect("open falló");
            provider.set("guild123", "persistent", Value::from("data")).unwrap();
        }

        let provider = RedbProvider::open(&db_path).expect("reopen falló");
        assert_eq!(
            provider.get_string("guild123", "persistent", "default").unwrap(),
            "data",
            "los datos deberían sobrevivir a un cierre y reapertura"
        );
    }

    #[test]
    fn concurrent_writes_do_not_panic_or_error() {
        let (_dir, provider) = test_provider();

        std::thread::scope(|scope| {
            for id in 0..10i64 {
                let provider = &provider;
                scope.spawn(move || {
                    for j in 0..100i64 {
                        provider.set("guild", "key", Value::from(id * 100 + j)).expect("set falló");
                    }
                });
            }
        });

        // El objetivo es que termine sin panics ni errores — igual que el test Go.
        provider.get_int("guild", "key", -1).expect("get falló");
    }

    #[test]
    fn clear_only_removes_values_for_the_given_guild() {
        let (_dir, provider) = test_provider();

        provider.set("guild123", "key1", Value::from("value1")).unwrap();
        provider.set("guild123", "key2", Value::from("value2")).unwrap();
        provider.set("guild456", "key1", Value::from("value1")).unwrap();

        provider.clear("guild123").unwrap();

        assert_eq!(provider.get_string("guild123", "key1", "default").unwrap(), "default");
        assert_eq!(provider.get_string("guild123", "key2", "default").unwrap(), "default");
        assert_eq!(
            provider.get_string("guild456", "key1", "default").unwrap(),
            "value1",
            "Clear no debería tocar otros guilds"
        );
    }

    #[test]
    fn list_guilds_returns_distinct_sorted_guild_ids() {
        let (_dir, provider) = test_provider();

        provider.set("guild3", "key", Value::from("value")).unwrap();
        provider.set("guild1", "key", Value::from("value")).unwrap();
        provider.set("guild2", "key", Value::from("value")).unwrap();
        // Una segunda key en el mismo guild no debería duplicar la entrada.
        provider.set("guild1", "other", Value::from("value")).unwrap();

        let guilds = provider.list_guilds().unwrap();
        assert_eq!(guilds, vec!["guild1", "guild2", "guild3"]);
    }
}

