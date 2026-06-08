//! Entry point del bot TTS (port Go → Rust).
//!
//! Inicializa los componentes, establece los manejadores de eventos y
//! se conecta a la pasarela de Discord.

use std::env;
use std::sync::Arc;
use std::path::Path;
use serenity::async_trait;
use serenity::prelude::*;
use serenity::gateway::ActivityData;
use serenity::model::gateway::{Ready, GatewayIntents};
use serenity::model::application::Interaction;
use serenity::model::channel::Message;
use serenity::model::guild::UnavailableGuild;
use serenity::model::voice::VoiceState;
use serenity::all::Guild;
use songbird::SerenityInit;

use tts_bot::data::RedbProvider;
use tts_bot::settings::Manager as SettingsManager;
use tts_bot::provider::AmazonProvider;
use tts_bot::player::PlayerManager;

struct BotHandler {
    player_manager: Arc<PlayerManager>,
    settings_manager: Arc<SettingsManager>,
    provider: Arc<AmazonProvider>,
    songbird_manager: Arc<songbird::Songbird>,
    testing_guild_id: Option<String>,
    enable_tts_channels: bool,
}

#[async_trait]
impl EventHandler for BotHandler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        if let Some(discriminator) = ready.user.discriminator {
            tracing::info!("Logged in as {}#{}", ready.user.name, discriminator);
        } else {
            tracing::info!("Logged in as {}", ready.user.name);
        }

        let cmds = tts_bot::commands::all_commands(self.enable_tts_channels);

        if let Some(ref testing_guild) = self.testing_guild_id {
            if let Ok(guild_id_u64) = testing_guild.parse::<u64>() {
                let guild_id = serenity::all::GuildId::new(guild_id_u64);
                if let Err(e) = guild_id.set_commands(&ctx.http, cmds).await {
                    tracing::error!("Failed to register commands in testing guild {}: {:?}", guild_id, e);
                } else {
                    tracing::info!("Registered slash commands in testing guild {}", guild_id);
                }
            } else {
                tracing::error!("Invalid TESTING_GUILD_ID: {}", testing_guild);
            }
        } else {
            if let Err(e) = serenity::all::Command::set_global_commands(&ctx.http, cmds).await {
                tracing::error!("Failed to register global commands: {:?}", e);
            } else {
                tracing::info!("Registered global slash commands");
            }
        }

        ctx.set_activity(Some(ActivityData::playing("/help for help")));
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            if let Err(e) = tts_bot::commands::handle_interaction(
                &ctx,
                &command,
                &self.player_manager,
                &self.settings_manager,
                &self.provider,
                self.songbird_manager.clone(),
                self.enable_tts_channels,
            )
            .await
            {
                tracing::error!("Error handling interaction: {:?}", e);
            }
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if !self.enable_tts_channels {
            return;
        }

        if msg.author.bot || msg.guild_id.is_none() || msg.content.is_empty() {
            return;
        }

        let guild_id = msg.guild_id.unwrap();
        let guild_id_str = guild_id.to_string();
        let channel_id_str = msg.channel_id.to_string();

        if !self.settings_manager.is_tts_channel(&guild_id_str, &channel_id_str) {
            return;
        }

        let user_id_u64 = msg.author.id.get();
        let guild_id_u64 = guild_id.get();

        let member_ch = match tts_bot::commands::voice_channel_of(&ctx, guild_id_u64, user_id_u64) {
            Some(ch) => ch,
            None => return,
        };

        // Sanitización local
        let resolve_user = |id: &str| {
            let id_u64 = id.parse::<u64>().ok()?;
            let guild_obj = ctx.cache.guild(guild_id_u64)?;
            let m = guild_obj.members.get(&id_u64.into())?;
            Some(tts_bot::commands::display_name(m))
        };
        let resolve_channel = |id: &str| {
            let id_u64 = id.parse::<u64>().ok()?;
            let guild_obj = ctx.cache.guild(guild_id_u64)?;
            let ch = guild_obj.channels.get(&id_u64.into())?;
            Some(ch.name.clone())
        };
        let resolve_role = |id: &str| {
            let id_u64 = id.parse::<u64>().ok()?;
            let guild_obj = ctx.cache.guild(guild_id_u64)?;
            let role = guild_obj.roles.get(&id_u64.into())?;
            Some(role.name.clone())
        };

        let sanitized = tts_bot::commands::sanitize_message(&msg.content, resolve_user, resolve_channel, resolve_role);

        let current_ch = {
            if let Some(call_lock) = self.songbird_manager.get(guild_id) {
                let call = call_lock.lock().await;
                call.current_channel().map(|c| c.0.get())
            } else {
                None
            }
        };

        if let Some(bot_ch) = current_ch {
            if bot_ch == member_ch {
                let _ = self.player_manager.say(&guild_id_str, &msg.author.id.to_string(), &sanitized).await;
            }
            return;
        }

        if let Some(_reason) = tts_bot::commands::cant_connect_reason(&ctx, member_ch, guild_id_u64) {
            return;
        }

        match self.songbird_manager.join(guild_id, serenity::all::ChannelId::new(member_ch)).await {
            Ok(_) => {
                // Esperar a que la conexión de voz se establezca por completo y sea audible
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                let _ = self.player_manager.say(&guild_id_str, &msg.author.id.to_string(), &sanitized).await;
            }
            Err(e) => {
                tracing::error!("Failed to join voice channel: {:?}", e);
            }
        }
    }

    async fn guild_delete(&self, _ctx: Context, incomplete: UnavailableGuild, _full: Option<Guild>) {
        let guild_id = incomplete.id.to_string();
        self.player_manager.destroy(&guild_id).await;
        if let Err(e) = self.settings_manager.clear_guild(&guild_id) {
            tracing::error!("clear guild {} settings error: {:?}", guild_id, e);
        }
    }

    async fn voice_state_update(&self, ctx: Context, _old: Option<VoiceState>, new: VoiceState) {
        let current_user_id = ctx.cache.current_user().id;

        if new.user_id == current_user_id {
            return;
        }

        let guild_id = match new.guild_id {
            Some(id) => id,
            None => return,
        };

        let guild_id_str = guild_id.to_string();
        let player = match self.player_manager.get(&guild_id_str) {
            Some(p) => p,
            None => return,
        };

        let call_lock = match self.songbird_manager.get(guild_id) {
            Some(lock) => lock,
            None => return,
        };

        let bot_channel_id = {
            let call = call_lock.lock().await;
            call.current_channel()
        };

        let bot_channel_id = match bot_channel_id {
            Some(ch) => ch.0.get(),
            None => return,
        };

        let mut non_bot_count = 0;
        {
            let guild = match ctx.cache.guild(guild_id) {
                Some(g) => g,
                None => return,
            };

            for (user_id, vs) in &guild.voice_states {
                if vs.channel_id.map(|c| c.get()) == Some(bot_channel_id) && user_id.get() != current_user_id.get() {
                    non_bot_count += 1;
                }
            }
        }

        if non_bot_count == 0 {
            tracing::info!(guild_id = %guild_id_str, "Bot is alone in voice channel, leaving.");
            let _ = player.stop(&self.songbird_manager).await;
        }
    }
}

#[tokio::main]
async fn main() {
    // Inicializar tracing
    tracing_subscriber::fmt::init();

    // K8s-only: the cluster's pod network is IPv4-only (no IPv6 routes), but the
    // system resolver still returns AAAA records for Discord's voice gateway, so
    // songbird's WS connect picks an unreachable IPv6 address and fails with
    // "Network is unreachable" (ENETUNREACH). Disabling IPv6 on the pod's own
    // interfaces makes getaddrinfo (AI_ADDRCONFIG) skip AAAA entirely. Requires
    // CAP_NET_ADMIN, granted via the pod's securityContext in the Helm values —
    // writes are silently ignored elsewhere (e.g. local runs without the capability).
    let _ = std::fs::write("/proc/sys/net/ipv6/conf/all/disable_ipv6", b"1");
    let _ = std::fs::write("/proc/sys/net/ipv6/conf/default/disable_ipv6", b"1");

    // Cargar variables de entorno
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let testing_guild_id = env::var("TESTING_GUILD_ID").ok();
    
    let data_path = env::var("DATA_PATH").unwrap_or_else(|_| "./data".to_string());
    if let Err(e) = std::fs::create_dir_all(&data_path) {
        tracing::error!("Could not create DATA_PATH directory {}: {:?}", data_path, e);
    }
    let db_filepath = Path::new(&data_path).join("settings.redb");

    let enable_tts_channels = env::var("ENABLE_TTS_CHANNELS")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);

    // Inicializar base de datos y administradores
    let db = Arc::new(RedbProvider::open(db_filepath).expect("Failed to open redb database"));
    let settings_manager = Arc::new(SettingsManager::new(db));
    let provider = Arc::new(AmazonProvider::default());
    let songbird_manager = songbird::Songbird::serenity();
    let player_manager = Arc::new(PlayerManager::new(
        settings_manager.clone(),
        provider.clone(),
        songbird_manager.clone(),
    ));

    // Configurar intents de Gateway
    let mut intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILD_MESSAGES;
    if enable_tts_channels {
        intents |= GatewayIntents::MESSAGE_CONTENT;
    }

    // Configurar cliente Serenity
    let mut client = Client::builder(&token, intents)
        .event_handler(BotHandler {
            player_manager,
            settings_manager,
            provider,
            songbird_manager: songbird_manager.clone(),
            testing_guild_id,
            enable_tts_channels,
        })
        .register_songbird_with(songbird_manager)
        .await
        .expect("Err creating client");

    // Manejar apagado ordenado mediante Ctrl+C / SIGTERM
    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to register Ctrl+C handler");
        tracing::info!("Shutdown signal received. Shutting down...");
        shard_manager.shutdown_all().await;
    });

    if let Err(why) = client.start().await {
        tracing::error!("Client error: {:?}", why);
    }
}
