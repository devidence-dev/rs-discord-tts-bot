//! Módulo del reproductor de voz y colas (port de Go `internal/player`).
//!
//! Se encarga de manejar la conexión de voz de Discord por servidor, encolar los
//! textos leídos con TTS, y gestionar el temporizador de desconexión por inactividad.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;
use songbird::tracks::TrackQueue;
use crate::settings::Manager as SettingsManager;
use serenity::all::GuildId;

use crate::provider::AmazonProvider;

/// Errores del reproductor.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("player: not connected to a voice channel")]
    NotConnected,

    #[error("provider error: {0}")]
    Provider(#[from] crate::provider::Error),

    #[error("songbird error: {0}")]
    Songbird(String),
}

struct PlayerState {
    timeout: std::time::Duration,
    timeout_handle: Option<tokio::task::JoinHandle<()>>,
}

/// Reproductor de TTS por servidor (guild).
pub struct Player {
    guild_id: String,
    queue: TrackQueue,
    state: Arc<Mutex<PlayerState>>,
}

impl Player {
    /// Envía un texto al canal de voz usando el proveedor TTS y restablece el timeout.
    pub async fn say(
        &self,
        sentence: &str,
        user_id: &str,
        settings: &SettingsManager,
        provider: &AmazonProvider,
        songbird_manager: Arc<songbird::Songbird>,
    ) -> Result<(), Error> {
        let guild_id_u64 = self.guild_id.parse::<u64>().map_err(|e| Error::Songbird(e.to_string()))?;
        let guild_id = GuildId::new(guild_id_u64);
        let call_lock = songbird_manager
            .get(guild_id)
            .ok_or(Error::NotConnected)?;

        let user_settings = settings.get_effective(&self.guild_id, user_id);

        let url = provider
            .get_audio_url(
                sentence,
                &user_settings.language,
                &user_settings.voice,
                &user_settings.volume,
                &user_settings.rate,
                &user_settings.pitch,
            )
            .await?;

        let client = provider.client();
        let source = songbird::input::HttpRequest::new(client, url);
        let input: songbird::input::Input = source.into();

        let mut call = call_lock.lock().await;
        self.queue.add_source(input, &mut *call).await;

        self.reset_timeout(songbird_manager).await;
        Ok(())
    }

    /// Detiene la reproducción, vacía la cola, aborta el temporizador de desconexión
    /// y saca al bot del canal de voz.
    pub async fn stop(&self, songbird_manager: &songbird::Songbird) -> Result<(), Error> {
        self.stop_timeout().await;

        let guild_id_u64 = self.guild_id.parse::<u64>().map_err(|e| Error::Songbird(e.to_string()))?;
        let guild_id = GuildId::new(guild_id_u64);
        if let Some(call_lock) = songbird_manager.get(guild_id) {
            let mut call = call_lock.lock().await;
            let _ = call.leave().await;
        }

        self.queue.stop();
        Ok(())
    }

    /// Actualiza el timeout de inactividad de forma dinámica y reinicia el temporizador si estaba corriendo.
    pub async fn update_timeout(&self, minutes: i64, songbird_manager: Arc<songbird::Songbird>) {
        let mut state = self.state.lock().await;
        state.timeout = std::time::Duration::from_secs((minutes * 60) as u64);

        if state.timeout_handle.is_some() {
            if let Some(handle) = state.timeout_handle.take() {
                handle.abort();
            }
            let guild_id = self.guild_id.clone();
            let timeout = state.timeout;
            let queue = self.queue.clone();
            let handle_clone = songbird_manager.clone();
            let handle = tokio::spawn(async move {
                tokio::time::sleep(timeout).await;
                tracing::info!(guild_id = %guild_id, "Inactivity timeout — leaving voice channel");
                let guild_id_u64 = guild_id.parse::<u64>().unwrap();
                let guild_id_obj = GuildId::new(guild_id_u64);
                if let Some(call_lock) = handle_clone.get(guild_id_obj) {
                    let mut call = call_lock.lock().await;
                    let _ = call.leave().await;
                }
                queue.stop();
            });
            state.timeout_handle = Some(handle);
        }
    }

    /// Reinicia el temporizador de inactividad.
    pub async fn reset_timeout(&self, songbird_manager: Arc<songbird::Songbird>) {
        let mut state = self.state.lock().await;
        if let Some(handle) = state.timeout_handle.take() {
            handle.abort();
        }

        let guild_id = self.guild_id.clone();
        let timeout = state.timeout;
        let queue = self.queue.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            tracing::info!(guild_id = %guild_id, "Inactivity timeout — leaving voice channel");
            let guild_id_u64 = guild_id.parse::<u64>().unwrap();
            let guild_id_obj = GuildId::new(guild_id_u64);
            if let Some(call_lock) = songbird_manager.get(guild_id_obj) {
                let mut call = call_lock.lock().await;
                let _ = call.leave().await;
            }
            queue.stop();
        });
        state.timeout_handle = Some(handle);
    }

    /// Detiene el temporizador de inactividad actual.
    pub async fn stop_timeout(&self) {
        let mut state = self.state.lock().await;
        if let Some(handle) = state.timeout_handle.take() {
            handle.abort();
        }
    }

    /// Retorna la cola de reproducción actual.
    pub fn queue(&self) -> &TrackQueue {
        &self.queue
    }
}

/// Administrador centralizado de reproductores por servidor.
pub struct PlayerManager {
    players: RwLock<HashMap<String, Arc<Player>>>,
    settings: Arc<SettingsManager>,
    provider: Arc<AmazonProvider>,
    songbird_manager: Arc<songbird::Songbird>,
}

impl PlayerManager {
    /// Crea un nuevo `PlayerManager`.
    pub fn new(
        settings: Arc<SettingsManager>,
        provider: Arc<AmazonProvider>,
        songbird_manager: Arc<songbird::Songbird>,
    ) -> Self {
        Self {
            players: RwLock::new(HashMap::new()),
            settings,
            provider,
            songbird_manager,
        }
    }

    /// Obtiene el reproductor existente para el guild, o lo crea con el timeout configurado.
    pub fn get_or_create(&self, guild_id: &str) -> Arc<Player> {
        {
            let players = self.players.read().unwrap();
            if let Some(p) = players.get(guild_id) {
                return p.clone();
            }
        }

        let mut players = self.players.write().unwrap();
        if let Some(p) = players.get(guild_id) {
            return p.clone();
        }

        let timeout_minutes = self.settings.get_timeout(guild_id);
        let timeout = std::time::Duration::from_secs((timeout_minutes * 60) as u64);

        let player = Arc::new(Player {
            guild_id: guild_id.to_string(),
            queue: TrackQueue::new(),
            state: Arc::new(Mutex::new(PlayerState {
                timeout,
                timeout_handle: None,
            })),
        });

        players.insert(guild_id.to_string(), player.clone());
        player
    }

    /// Elimina el reproductor del servidor indicado y detiene su reproducción.
    pub async fn destroy(&self, guild_id: &str) -> Option<Arc<Player>> {
        let player = {
            let mut players = self.players.write().unwrap();
            players.remove(guild_id)
        };

        if let Some(ref p) = player {
            let _ = p.stop(&self.songbird_manager).await;
        }
        player
    }

    /// Encola y reproduce una frase en el reproductor del guild correspondiente.
    pub async fn say(&self, guild_id: &str, user_id: &str, sentence: &str) -> Result<(), Error> {
        let player = self.get_or_create(guild_id);
        player.say(
            sentence,
            user_id,
            &self.settings,
            &self.provider,
            self.songbird_manager.clone()
        ).await
    }

    /// Detiene la reproducción en el guild correspondiente.
    pub async fn stop(&self, guild_id: &str) -> Result<(), Error> {
        let player = self.get_or_create(guild_id);
        player.stop(&self.songbird_manager).await
    }

    /// Obtiene el reproductor existente para el guild sin crearlo.
    pub fn get(&self, guild_id: &str) -> Option<Arc<Player>> {
        let players = self.players.read().unwrap();
        players.get(guild_id).cloned()
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::RedbProvider;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_player_manager_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let db = Arc::new(RedbProvider::open(db_path).unwrap());
        let settings = Arc::new(SettingsManager::new(db));
        let provider = Arc::new(AmazonProvider::default());
        let songbird_manager = songbird::Songbird::serenity();

        let manager = PlayerManager::new(settings, provider, songbird_manager);

        let player1 = manager.get_or_create("guild123");
        assert_eq!(player1.guild_id, "guild123");
        
        let player2 = manager.get_or_create("guild123");
        assert!(Arc::ptr_eq(&player1, &player2), "debería retornar el mismo player existente");

        let state = player1.state.lock().await;
        assert_eq!(state.timeout, std::time::Duration::from_secs(300)); // 5 minutos por defecto
        drop(state);

        player1.update_timeout(10, manager.songbird_manager.clone()).await;
        let state = player1.state.lock().await;
        assert_eq!(state.timeout, std::time::Duration::from_secs(600)); // 10 minutos
        drop(state);

        let destroyed = manager.destroy("guild123").await;
        assert!(destroyed.is_some());
        assert!(Arc::ptr_eq(&player1, &destroyed.unwrap()));
    }
}
