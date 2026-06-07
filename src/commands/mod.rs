//! Módulo de comandos de Discord (port de Go `internal/commands`).
//!
//! Define la estructura y el registro de los slash commands, y despacha
//! las interacciones a las acciones de negocio correspondientes en el reproductor y base de datos.

use std::sync::{Arc, LazyLock};
use regex::{Regex, Captures};

use serenity::builder::{
    CreateCommand, CreateCommandOption, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage,
    EditInteractionResponse,
};
use serenity::model::application::{CommandInteraction, ResolvedValue, CommandOptionType};
use serenity::client::Context;

use crate::settings::Manager as SettingsManager;
use crate::player::PlayerManager;
use crate::provider::AmazonProvider;

static USER_MENTION_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<@!?(\d{17,19})>").unwrap());
static CHANNEL_MENTION_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<#(\d{17,19})>").unwrap());
static ROLE_MENTION_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<@&(\d{17,19})>").unwrap());
static CUSTOM_EMOJI_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<:(.*?):(\d{17,19})>").unwrap());
static URL_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"https?://\S+").unwrap());

/// Sanitiza menciones convirtiéndolas a nombres legibles por voz y oculta URLs.
pub fn sanitize_message<F, G, H>(
    msg: &str,
    resolve_user: F,
    resolve_channel: G,
    resolve_role: H,
) -> String
where
    F: Fn(&str) -> Option<String>,
    G: Fn(&str) -> Option<String>,
    H: Fn(&str) -> Option<String>,
{
    let s = USER_MENTION_REGEX.replace_all(msg, |caps: &Captures| {
        let id = &caps[1];
        resolve_user(id).unwrap_or_else(|| caps[0].to_string())
    });
    let s = CHANNEL_MENTION_REGEX.replace_all(&s, |caps: &Captures| {
        let id = &caps[1];
        resolve_channel(id).unwrap_or_else(|| caps[0].to_string())
    });
    let s = ROLE_MENTION_REGEX.replace_all(&s, |caps: &Captures| {
        let id = &caps[1];
        resolve_role(id).unwrap_or_else(|| caps[0].to_string())
    });
    let s = CUSTOM_EMOJI_REGEX.replace_all(&s, |caps: &Captures| {
        let name = &caps[1];
        name.to_string()
    });
    let s = URL_REGEX.replace_all(&s, "ඞ");
    s.into_owned()
}

/// Genera la lista de comandos para registrar en Discord.
pub fn all_commands(tts_channels_enabled: bool) -> Vec<CreateCommand> {
    let mut cmds = vec![
        CreateCommand::new("say")
            .description("Say a message using Amazon TTS in your voice channel.")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "message",
                    "The message to say in your voice channel.",
                )
                .required(true),
            ),
        CreateCommand::new("langs")
            .description("List the languages supported by the Amazon provider."),
        CreateCommand::new("voices")
            .description("List voices available for a given Amazon language.")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "language",
                    "Language code (e.g. en, es). Use /langs for the full list.",
                )
                .required(true),
            ),
        CreateCommand::new("set_my")
            .description("Set your personal Amazon TTS settings.")
            .set_options(setting_subcommands()),
        CreateCommand::new("set_default")
            .description("Set the guild default Amazon TTS settings.")
            .set_options(setting_subcommands()),
        CreateCommand::new("stop")
            .description("Stop TTS and leave the voice channel."),
        CreateCommand::new("help")
            .description("Show bot help information."),
        CreateCommand::new("set_locale")
            .description("Set the bot language for this server.")
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "locale", "Language code.")
                    .required(true)
                    .add_string_choice("English", "en")
                    .add_string_choice("Español", "es"),
            ),
        CreateCommand::new("set_timeout")
            .description("Set the inactivity disconnect timeout.")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "minutes",
                    "Minutes of inactivity before leaving (1–60).",
                )
                .required(true)
                .min_int_value(1)
                .max_int_value(60),
            ),
        CreateCommand::new("my_settings")
            .description("Show your current Amazon TTS settings."),
        CreateCommand::new("default_settings")
            .description("Show the guild default Amazon TTS settings."),
    ];

    if tts_channels_enabled {
        cmds.push(CreateCommand::new("set_tts_channel")
            .description("Read every message sent in this channel out loud automatically."));
        cmds.push(CreateCommand::new("unset_tts_channel")
            .description("Stop reading messages in this channel out loud automatically."));
    }

    cmds
}

fn setting_subcommands() -> Vec<CreateCommandOption> {
    vec![
        CreateCommandOption::new(CommandOptionType::SubCommand, "language", "Set the language.")
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "value",
                    "Language code. Use /langs for the full list.",
                )
                .required(true),
            ),
        CreateCommandOption::new(CommandOptionType::SubCommand, "voice", "Set the voice.")
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "value",
                    "Voice name. Use /voices to see available voices.",
                )
                .required(true),
            ),
        CreateCommandOption::new(CommandOptionType::SubCommand, "volume", "Set the volume.")
            .add_sub_option(add_choices(
                CreateCommandOption::new(CommandOptionType::String, "value", "Volume level.").required(true),
                crate::provider::volume_choices(),
            )),
        CreateCommandOption::new(CommandOptionType::SubCommand, "rate", "Set the speech rate.")
            .add_sub_option(add_choices(
                CreateCommandOption::new(CommandOptionType::String, "value", "Speech rate.").required(true),
                crate::provider::rate_choices(),
            )),
        CreateCommandOption::new(CommandOptionType::SubCommand, "pitch", "Set the pitch.")
            .add_sub_option(add_choices(
                CreateCommandOption::new(CommandOptionType::String, "value", "Pitch level.").required(true),
                crate::provider::pitch_choices(),
            )),
    ]
}

fn add_choices(mut opt: CreateCommandOption, choices: &[(&str, &str)]) -> CreateCommandOption {
    for &(name, val) in choices {
        opt = opt.add_string_choice(name, val);
    }
    opt
}

pub fn display_name(member: &serenity::model::guild::Member) -> String {
    if let Some(ref nick) = member.nick {
        nick.clone()
    } else {
        member.user.name.clone()
    }
}

fn settings_embed(title: &str, description: &str, s: &crate::settings::AmazonSettings) -> CreateEmbed {
    CreateEmbed::new()
        .title(title)
        .description(description)
        .color(0x9b59b6)
        .field("Language", &s.language, true)
        .field("Voice", &s.voice, true)
        .field("Volume", &s.volume, true)
        .field("Rate", &s.rate, true)
        .field("Pitch", &s.pitch, true)
}

/// Busca el canal de voz actual de un usuario en un guild.
pub fn voice_channel_of(ctx: &Context, guild_id: u64, user_id: u64) -> Option<u64> {
    let guild = ctx.cache.guild(guild_id)?;
    guild.voice_states.get(&user_id.into()).and_then(|vs| vs.channel_id.map(|c| c.get()))
}

/// Comprueba si el bot tiene los permisos necesarios para entrar y hablar.
pub fn cant_connect_reason(ctx: &Context, channel_id: u64, guild_id: u64) -> Option<&'static str> {
    let guild = ctx.cache.guild(guild_id)?;
    let bot_id = ctx.cache.current_user().id;

    let channel = guild.channels.get(&channel_id.into())?;
    let member = guild.members.get(&bot_id)?;
    let perms = guild.user_permissions_in(channel, member);

    if !perms.contains(serenity::model::Permissions::VIEW_CHANNEL) {
        return Some("error.channel.not_viewable");
    }

    if channel.user_limit.unwrap_or(0) > 0 {
        let count = guild
            .voice_states
            .values()
            .filter(|vs| vs.channel_id.map(|c| c.get()) == Some(channel_id))
            .count();
        if count >= channel.user_limit.unwrap() as usize {
            return Some("error.channel.full");
        }
    }

    if !perms.contains(serenity::model::Permissions::CONNECT) {
        return Some("error.channel.not_joinable");
    }
    if !perms.contains(serenity::model::Permissions::SPEAK) {
        return Some("error.channel.not_speakable");
    }

    None
}

/// Manejador de llamadas de comandos slash.
pub async fn handle_interaction(
    ctx: &Context,
    interaction: &CommandInteraction,
    player_manager: &PlayerManager,
    settings_manager: &SettingsManager,
    provider: &AmazonProvider,
    songbird_manager: Arc<songbird::Songbird>,
    tts_channels_enabled: bool,
) -> Result<(), serenity::Error> {
    let name = interaction.data.name.as_str();
    let guild_id = match interaction.guild_id {
        Some(id) => id.to_string(),
        None => return Ok(()),
    };
    let guild_id_u64 = interaction.guild_id.unwrap().get();

    let locale_code = settings_manager.get_locale(&guild_id);
    let loc = crate::locale::Localizer::new(&locale_code);

    let reply = |text: String, ephemeral: bool| async move {
        let _ = interaction
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().content(text).ephemeral(ephemeral),
                ),
            )
            .await;
    };

    let reply_embed = |embed: CreateEmbed| async move {
        let _ = interaction
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().embed(embed)),
            )
            .await;
    };

    let options = interaction.data.options();

    match name {
        "say" => {
            let msg = match options.first().and_then(|o| match o.value {
                ResolvedValue::String(s) => Some(s),
                _ => None,
            }) {
                Some(val) => val,
                None => "",
            };

            let member = match &interaction.member {
                Some(m) => m,
                None => return Ok(()),
            };
            let user_id_u64 = member.user.id.get();

            let member_ch = match voice_channel_of(ctx, guild_id_u64, user_id_u64) {
                Some(ch) => ch,
                None => {
                    reply(loc.t("command.say.no_channel", &[]), true).await;
                    return Ok(());
                }
            };

            let player = player_manager.get_or_create(&guild_id);
            let current_ch = {
                if let Some(call_lock) = songbird_manager.get(interaction.guild_id.unwrap()) {
                    let call = call_lock.lock().await;
                    call.current_channel().map(|c| c.0.get())
                } else {
                    None
                }
            };

            if let Some(bot_ch) = current_ch {
                if bot_ch != member_ch {
                    reply(loc.t("command.say.different_channel", &[]), true).await;
                    return Ok(());
                }

                // Sanitización local
                let resolve_user = |id: &str| {
                    let id_u64 = id.parse::<u64>().ok()?;
                    let guild_obj = ctx.cache.guild(guild_id_u64)?;
                    let m = guild_obj.members.get(&id_u64.into())?;
                    Some(display_name(m))
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

                let sanitized = sanitize_message(msg, resolve_user, resolve_channel, resolve_role);
                let _ = player.say(&sanitized, &member.user.id.to_string(), settings_manager, provider, songbird_manager).await;
                reply(loc.t("command.say.success", &[("request", msg)]), false).await;
                return Ok(());
            }

            if let Some(reason) = cant_connect_reason(ctx, member_ch, guild_id_u64) {
                reply(loc.t(reason, &[]), true).await;
                return Ok(());
            }

            // Defer response publicly to prevent timeout during voice join & handshake
            let _ = interaction.defer(&ctx.http).await;

            // Unirse a voz
            match songbird_manager.join(interaction.guild_id.unwrap(), serenity::all::ChannelId::new(member_ch)).await {
                Ok(_call_lock) => {
                    // Esperar a que la conexión de voz se establezca por completo y sea audible
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

                    let ch_name = ctx.cache.guild(guild_id_u64)
                        .and_then(|g| g.channels.get(&member_ch.into()).map(|c| c.name.clone()))
                        .unwrap_or_else(|| member_ch.to_string());
                    let resolve_user = |id: &str| {
                        let id_u64 = id.parse::<u64>().ok()?;
                        let guild_obj = ctx.cache.guild(guild_id_u64)?;
                        let m = guild_obj.members.get(&id_u64.into())?;
                        Some(display_name(m))
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

                    let sanitized = sanitize_message(msg, resolve_user, resolve_channel, resolve_role);
                    let _ = player.say(&sanitized, &member.user.id.to_string(), settings_manager, provider, songbird_manager).await;
                    
                    let _ = interaction.edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .content(loc.t("command.say.joined.withrequest", &[("channel", &ch_name), ("request", msg)]))
                    ).await;
                }
                Err(e) => {
                    tracing::error!("Failed to join channel: {:?}", e);
                    let _ = interaction.edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .content("Failed to join your voice channel.")
                    ).await;
                }
            }
        }
        "langs" => {
            let mut embed = CreateEmbed::new()
                .title(loc.t("command.langs.embed.title", &[]))
                .description(loc.t("command.langs.embed.description", &[]))
                .color(0x3498db);

            let mut sorted_keys: Vec<String> = crate::provider::LANGUAGES.keys().cloned().collect();
            sorted_keys.sort();

            for k in sorted_keys {
                if let Some(lang) = crate::provider::LANGUAGES.get(&k) {
                    embed = embed.field(
                        format!("{} {} (`{}`)", lang.emoji, lang.name, k),
                        format!("Voices count: {}", lang.voices.len()),
                        true,
                    );
                }
            }
            reply_embed(embed).await;
        }
        "voices" => {
            let lang_code = match options.first().and_then(|o| match o.value {
                ResolvedValue::String(s) => Some(s),
                _ => None,
            }) {
                Some(val) => val,
                None => "",
            };

            if let Some(lang) = crate::provider::LANGUAGES.get(lang_code) {
                let mut embed = CreateEmbed::new()
                    .title(loc.t("command.voices.embed.title", &[("language", &lang.name)]))
                    .description(loc.t("command.voices.embed.description", &[]))
                    .color(0x2ecc71);

                for v in &lang.voices {
                    embed = embed.field(format!("{} {}", v.emoji, v.name), format!("`{}`", v.id), true);
                }
                reply_embed(embed).await;
            } else {
                reply(
                    loc.t("command.voices.error.unsupported", &[("language", lang_code)]),
                    true,
                )
                .await;
            }
        }
        "set_my" => {
            let member = match &interaction.member {
                Some(m) => m,
                None => return Ok(()),
            };
            let user_id = member.user.id.to_string();

            if let Some(opt) = options.first() {
                let sub_name = opt.name;
                if let ResolvedValue::SubCommand(sub_opts) = &opt.value {
                    if let Some(val_opt) = sub_opts.first() {
                        if let ResolvedValue::String(val) = &val_opt.value {
                            match sub_name {
                                "language" => {
                                    if let Some(lang) = crate::provider::LANGUAGES.get(*val) {
                                        let default_voice = &lang.voices[0];
                                        let _ = settings_manager.set_user(&guild_id, &user_id, "language", val);
                                        let _ = settings_manager.set_user(&guild_id, &user_id, "voice", &default_voice.id);
                                        reply(
                                            loc.t(
                                                "command.settings.my.language.success",
                                                &[("language", &lang.name), ("voice", &default_voice.name)],
                                            ),
                                            true,
                                        )
                                        .await;
                                    } else {
                                        reply(
                                            loc.t("command.settings.my.language.unsupported", &[("language", val)]),
                                            true,
                                        )
                                        .await;
                                    }
                                }
                                "voice" => {
                                    let voice_lower = val.to_lowercase();
                                    let eff = settings_manager.get_effective(&guild_id, &user_id);
                                    if let Some(lang) = crate::provider::LANGUAGES.get(&eff.language) {
                                        if let Some(found) = lang.voices.iter().find(|v| v.name.to_lowercase() == voice_lower) {
                                            let _ = settings_manager.set_user(&guild_id, &user_id, "voice", &found.id);
                                            reply(
                                                loc.t("command.settings.my.voice.success", &[("voice", &found.name)]),
                                                true,
                                            )
                                            .await;
                                        } else {
                                            reply(
                                                loc.t("command.settings.my.voice.unsupported", &[("voice", val)]),
                                                true,
                                            )
                                            .await;
                                        }
                                    } else {
                                        reply(loc.t("command.settings.my.voice.invalidated", &[]), true).await;
                                    }
                                }
                                "volume" | "rate" | "pitch" => {
                                    let _ = settings_manager.set_user(&guild_id, &user_id, sub_name, val);
                                    let success_key = format!("command.settings.my.{sub_name}.success");
                                    reply(loc.t(&success_key, &[(sub_name, val)]), true).await;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        "set_default" => {
            if let Some(opt) = options.first() {
                let sub_name = opt.name;
                if let ResolvedValue::SubCommand(sub_opts) = &opt.value {
                    if let Some(val_opt) = sub_opts.first() {
                        if let ResolvedValue::String(val) = &val_opt.value {
                            match sub_name {
                                "language" => {
                                    if let Some(lang) = crate::provider::LANGUAGES.get(*val) {
                                        let default_voice = &lang.voices[0];
                                        let _ = settings_manager.set_guild(&guild_id, "language", val);
                                        let _ = settings_manager.set_guild(&guild_id, "voice", &default_voice.id);
                                        reply(
                                            loc.t(
                                                "command.settings.default.language.success",
                                                &[("language", &lang.name), ("voice", &default_voice.name)],
                                            ),
                                            true,
                                        )
                                        .await;
                                    } else {
                                        reply(
                                            loc.t(
                                                "command.settings.default.language.unsupported",
                                                &[("language", val)],
                                            ),
                                            true,
                                        )
                                        .await;
                                    }
                                }
                                "voice" => {
                                    let voice_lower = val.to_lowercase();
                                    let guild_settings = settings_manager.get_guild(&guild_id);
                                    if let Some(lang) = crate::provider::LANGUAGES.get(&guild_settings.language) {
                                        if let Some(found) = lang.voices.iter().find(|v| v.name.to_lowercase() == voice_lower) {
                                            let _ = settings_manager.set_guild(&guild_id, "voice", &found.id);
                                            reply(
                                                loc.t("command.settings.default.voice.success", &[("voice", &found.name)]),
                                                true,
                                            )
                                            .await;
                                        } else {
                                            reply(
                                                loc.t("command.settings.default.voice.unsupported", &[("voice", val)]),
                                                true,
                                            )
                                            .await;
                                        }
                                    } else {
                                        reply(loc.t("command.settings.default.voice.invalidated", &[]), true).await;
                                    }
                                }
                                "volume" | "rate" | "pitch" => {
                                    let _ = settings_manager.set_guild(&guild_id, sub_name, val);
                                    let success_key = format!("command.settings.default.{sub_name}.success");
                                    reply(loc.t(&success_key, &[(sub_name, val)]), true).await;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        "stop" => {
            let member = match &interaction.member {
                Some(m) => m,
                None => return Ok(()),
            };
            let user_id_u64 = member.user.id.get();

            let current_ch = {
                if let Some(call_lock) = songbird_manager.get(interaction.guild_id.unwrap()) {
                    let call = call_lock.lock().await;
                    call.current_channel().map(|c| c.0.get())
                } else {
                    None
                }
            };

            let bot_ch = match current_ch {
                Some(ch) => ch,
                None => {
                    reply(loc.t("command.stop.no_connection", &[]), true).await;
                    return Ok(());
                }
            };

            let player = player_manager.get_or_create(&guild_id);
            let member_ch = voice_channel_of(ctx, guild_id_u64, user_id_u64);
            if member_ch.is_some() && Some(bot_ch) != member_ch {
                reply(loc.t("command.stop.different_channel", &[]), true).await;
                return Ok(());
            }

            let ch_name = current_ch
                .and_then(|c| {
                    let guild_obj = ctx.cache.guild(guild_id_u64)?;
                    let ch = guild_obj.channels.get(&c.into())?;
                    Some(ch.name.clone())
                })
                .unwrap_or_else(|| "".to_string());

            let _ = player.stop(&songbird_manager).await;
            reply(loc.t("command.stop.success", &[("channel", &ch_name)]), false).await;
        }
        "help" => {
            let mut description = loc.t("command.help.description", &[]);
            if tts_channels_enabled {
                description += &format!("\n{}", loc.t("command.help.tts_channels_extra", &[]));
            }

            let embed = CreateEmbed::new()
                .title(loc.t("command.help.embed.title", &[]))
                .description(description)
                .color(0x2ecc71);
            reply_embed(embed).await;
        }
        "set_locale" => {
            let code = match options.first().and_then(|o| match o.value {
                ResolvedValue::String(s) => Some(s),
                _ => None,
            }) {
                Some(val) => val,
                None => "",
            };

            if !crate::locale::is_supported(code) {
                reply("Unsupported locale.".to_string(), true).await;
                return Ok(());
            }

            let _ = settings_manager.set_locale(&guild_id, code);
            let new_loc = crate::locale::Localizer::new(code);
            reply(new_loc.t("command.locale.success", &[("locale", code)]), true).await;
        }
        "set_timeout" => {
            let minutes = match options.first().and_then(|o| match o.value {
                ResolvedValue::Integer(i) => Some(i),
                _ => None,
            }) {
                Some(val) => val,
                None => 0,
            };

            if minutes < crate::settings::MIN_TIMEOUT || minutes > crate::settings::MAX_TIMEOUT {
                reply(
                    loc.t(
                        "command.timeout.out_of_range",
                        &[
                            ("min", &crate::settings::MIN_TIMEOUT.to_string()),
                            ("max", &crate::settings::MAX_TIMEOUT.to_string()),
                        ],
                    ),
                    true,
                )
                .await;
                return Ok(());
            }

            let _ = settings_manager.set_timeout(&guild_id, minutes);
            // Actualizar el player activo si existe
            let player = player_manager.get_or_create(&guild_id);
            player.update_timeout(minutes, songbird_manager.clone()).await;

            reply(loc.t("command.timeout.success", &[("timeout", &minutes.to_string())]), true).await;
        }
        "my_settings" => {
            let member = match &interaction.member {
                Some(m) => m,
                None => return Ok(()),
            };
            let eff = settings_manager.get_effective(&guild_id, &member.user.id.to_string());
            let embed = settings_embed(
                &loc.t("command.settings.my.embed.title", &[("name", &display_name(member))]),
                &loc.t("command.settings.my.embed.description", &[]),
                &eff,
            );
            reply_embed(embed).await;
        }
        "default_settings" => {
            let guild_settings = settings_manager.get_guild(&guild_id);
            let embed = settings_embed(
                &loc.t("command.settings.default.embed.title", &[]),
                &loc.t("command.settings.default.embed.description", &[]),
                &guild_settings,
            );
            reply_embed(embed).await;
        }
        "set_tts_channel" => {
            if tts_channels_enabled {
                let channel_id = interaction.channel_id.to_string();
                let _ = settings_manager.set_tts_channel(&guild_id, &channel_id, true);
                reply(loc.t("command.tts_channel.set.success", &[]), true).await;
            }
        }
        "unset_tts_channel" => {
            if tts_channels_enabled {
                let channel_id = interaction.channel_id.to_string();
                let _ = settings_manager.set_tts_channel(&guild_id, &channel_id, false);
                reply(loc.t("command.tts_channel.unset.success", &[]), true).await;
            }
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_message_replaces_urls() {
        let no_res = |_id: &str| None;
        assert_eq!(
            sanitize_message("check this out http://example.com/page nice", no_res, no_res, no_res),
            "check this out ඞ nice"
        );
        assert_eq!(
            sanitize_message("go to https://example.com/path?x=1 now", no_res, no_res, no_res),
            "go to ඞ now"
        );
        assert_eq!(
            sanitize_message("http://a.com and https://b.com both", no_res, no_res, no_res),
            "ඞ and ඞ both"
        );
        assert_eq!(
            sanitize_message("normal message without links", no_res, no_res, no_res),
            "normal message without links"
        );
        assert_eq!(sanitize_message("", no_res, no_res, no_res), "");
    }

    #[test]
    fn test_sanitize_message_resolves_user_mentions() {
        let resolve_user = |id: &str| {
            if id == "123456789012345678" {
                Some("Alice".to_string())
            } else {
                None
            }
        };
        let no_res = |_id: &str| None;

        let got = sanitize_message(
            "hey <@123456789012345678> and <@!123456789012345678>, you there?",
            resolve_user,
            no_res,
            no_res,
        );
        assert_eq!(got, "hey Alice and Alice, you there?");
    }

    #[test]
    fn test_sanitize_message_resolves_channel_mentions() {
        let resolve_chan = |id: &str| {
            if id == "123456789012345678" {
                Some("general".to_string())
            } else {
                None
            }
        };
        let no_res = |_id: &str| None;

        let got = sanitize_message(
            "see you in <#123456789012345678>",
            no_res,
            resolve_chan,
            no_res,
        );
        assert_eq!(got, "see you in general");
    }

    #[test]
    fn test_sanitize_message_resolves_role_mentions() {
        let resolve_role = |id: &str| {
            if id == "111222333444555666" {
                Some("admin".to_string())
            } else {
                None
            }
        };
        let no_res = |_id: &str| None;

        let got = sanitize_message(
            "alerting the <@&111222333444555666>",
            no_res,
            no_res,
            resolve_role,
        );
        assert_eq!(got, "alerting the admin");
    }

    #[test]
    fn test_sanitize_message_resolves_custom_emojis() {
        let no_res = |_id: &str| None;
        let got = sanitize_message(
            "this is <:cool_emoji:123456789012345678> indeed",
            no_res,
            no_res,
            no_res,
        );
        assert_eq!(got, "this is cool_emoji indeed");
    }

    #[test]
    fn test_sanitize_message_unresolvable_leaves_untouched() {
        let no_res = |_id: &str| None;
        let in_msg = "hi <@123456789012345678> in <#876543210987654321> for <@&111222333444555666>";
        assert_eq!(
            sanitize_message(in_msg, no_res, no_res, no_res),
            in_msg
        );
    }
}
