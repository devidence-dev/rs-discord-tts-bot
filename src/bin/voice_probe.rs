//! Sonda de voz independiente del bot real (binario auxiliar `cargo run --bin voice_probe`).
//!
//! Nació como el scaffold de la fase 1 del plan de migración (de-risk): unirse a
//! un canal de voz y reproducir un tono de prueba generado en memoria, para
//! confirmar que `songbird` resuelve el bloqueo DAVE (close code 4017) que frenó
//! el port a Go. Ver ../bot-go/plan-go-rust.md, sección "Fases sugeridas", fase 1.
//!
//! Se mantiene como binario separado (deliberadamente standalone, sin depender
//! de los módulos del bot real) para poder revalidar la conexión de voz en
//! aislamiento si algo cambia en versiones futuras de songbird/serenity.
//!
//! Variables de entorno requeridas:
//!   DISCORD_TOKEN          token del bot
//!   TEST_GUILD_ID          guild donde está el canal de voz de prueba
//!   TEST_VOICE_CHANNEL_ID  canal de voz al que unirse y reproducir el tono

use std::{env, io::Cursor, sync::Arc};

use serenity::{
    all::{ChannelId, GatewayIntents, GuildId, Ready},
    async_trait,
    client::{Client, Context, EventHandler},
};
use songbird::{
    SerenityInit,
    events::{Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent},
    input::{Input, RawAdapter},
};

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u32 = 2;
const TONE_HZ: f32 = 440.0;
const TONE_SECONDS: f32 = 3.0;

/// Genera un tono senoidal como PCM `f32` intercalado — el formato que espera
/// `RawAdapter`. Evita depender de un archivo de audio o de codecs extra de
/// symphonia: para este de-risk solo importa que algo suene en el canal.
fn sine_wave_pcm(freq_hz: f32, duration_secs: f32, sample_rate: u32, channels: u32) -> Vec<u8> {
    let frames = (sample_rate as f32 * duration_secs) as u32;
    let mut pcm = Vec::with_capacity((frames * channels * 4) as usize);

    for frame in 0..frames {
        let t = frame as f32 / sample_rate as f32;
        let sample = (t * freq_hz * std::f32::consts::TAU).sin() * 0.25;
        for _ in 0..channels {
            pcm.extend_from_slice(&sample.to_le_bytes());
        }
    }

    pcm
}

struct VoiceProbe {
    guild_id: GuildId,
    channel_id: ChannelId,
}

#[async_trait]
impl EventHandler for VoiceProbe {
    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!(bot = %ready.user.name, "conectado a Discord; uniendo al canal de voz de prueba");

        let manager = songbird::get(&ctx)
            .await
            .expect("songbird no inicializado (register_songbird)")
            .clone();

        match manager.join(self.guild_id, self.channel_id).await {
            Ok(call_lock) => {
                tracing::info!(channel = %self.channel_id, "unido al canal de voz — esto es lo que fallaba con close code 4017 en discordgo");

                let mut call = call_lock.lock().await;
                call.add_global_event(TrackEvent::End.into(), TrackNotifier("End"));
                call.add_global_event(TrackEvent::Error.into(), TrackNotifier("Error"));

                let pcm = sine_wave_pcm(TONE_HZ, TONE_SECONDS, SAMPLE_RATE, CHANNELS);
                let source = RawAdapter::new(Cursor::new(pcm), SAMPLE_RATE, CHANNELS);
                let input: Input = source.into();
                call.play_input(input);

                tracing::info!(hz = TONE_HZ, seconds = TONE_SECONDS, "reproduciendo tono de prueba");
            },
            Err(why) => {
                tracing::error!(error = ?why, "no se pudo unir al canal de voz");
            },
        }
    }
}

/// Loguea los eventos `End`/`Error` de las pistas para confirmar que la
/// reproducción corre hasta el final sin que la conexión de voz se caiga.
struct TrackNotifier(&'static str);

#[async_trait]
impl VoiceEventHandler for TrackNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            for (state, handle) in *track_list {
                tracing::info!(event = self.0, track = %handle.uuid(), playing = ?state.playing, "evento de pista");
            }
        }

        None
    }
}

fn required_env(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("falta la variable de entorno {key}"))
}

fn required_env_id<T: From<u64>>(key: &str) -> T {
    let raw = required_env(key);
    let id: u64 = raw
        .parse()
        .unwrap_or_else(|_| panic!("{key}={raw:?} no es un ID de Discord (u64) válido"));
    T::from(id)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let token = required_env("DISCORD_TOKEN");
    let guild_id: GuildId = required_env_id("TEST_GUILD_ID");
    let channel_id: ChannelId = required_env_id("TEST_VOICE_CHANNEL_ID");

    let intents = GatewayIntents::GUILD_VOICE_STATES;

    let mut client = Client::builder(&token, intents)
        .event_handler(VoiceProbe { guild_id, channel_id })
        .register_songbird()
        .await
        .expect("error creando el cliente de Discord");

    let shard_manager = Arc::clone(&client.shard_manager);

    tokio::spawn(async move {
        if let Err(why) = client.start().await {
            tracing::error!(error = ?why, "el cliente de Discord terminó con error");
        }
    });

    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("Ctrl-C recibido, cerrando");
    shard_manager.shutdown_all().await;
}
