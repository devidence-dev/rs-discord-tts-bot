use std::collections::HashMap;
use std::sync::LazyLock;

pub(crate) static EN_STRINGS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    [
        ("error.channel.not_viewable", "I cannot see your voice channel. Do I have enough permissions to view it?"),
        ("error.channel.full", "Your voice channel is full."),
        ("error.channel.not_joinable", "I cannot join your voice channel. Do I have enough permissions to join it?"),
        ("error.channel.not_speakable", "I cannot speak in your voice channel. Do I have enough permissions to speak in it?"),

        ("command.say.no_channel", "You need to be in a voice channel first."),
        ("command.say.different_channel", "You need to be in my same voice channel to say something."),
        ("command.say.success", "Saying \"{request}\"."),
        ("command.say.joined.withrequest", "Joined {channel} - Saying \"{request}\"."),
        ("command.say.joined", "Joined {channel}."),

        ("command.stop.no_connection", "I'm not in a voice channel."),
        ("command.stop.different_channel", "You need to be in my voice channel to stop me."),
        ("command.stop.success", "Successfully left the voice channel {channel}."),

        ("command.help.embed.title", "Text-to-Speech Help Message"),
        ("command.help.description", "This bot uses Amazon TTS to speak messages in your voice channel.\n\n**Commands:**\n`/say` — Say a message.\n`/langs` — List supported languages.\n`/voices` — List voices for a language.\n`/set_my` — Set your personal settings.\n`/set_default` — Set the guild default settings.\n`/stop` — Stop TTS and leave the channel.\n`/set_locale` — Set the bot language (en/es).\n`/set_timeout` — Set the inactivity timeout."),
        ("command.help.tts_channels_extra", "`/set_tts_channel` — Read every message sent in this channel out loud automatically.\n`/unset_tts_channel` — Stop reading messages in this channel out loud automatically."),

        ("command.langs.embed.title", "Supported languages by the Amazon provider:"),
        ("command.langs.embed.description", "Use **/set_my language LANG_CODE** to change your language.\nUse **/set_default language LANG_CODE** to change the guild default."),

        ("command.voices.embed.title", "Voices available for the {language} language:"),
        ("command.voices.embed.description", "Use **/set_my voice VOICE_NAME** to change your voice.\nUse **/set_default voice VOICE_NAME** to change the guild default."),
        ("command.voices.error.unsupported", "Language **{language}** is not supported. Use **/langs** to see available languages."),

        ("command.settings.default.language.unsupported", "Language **{language}** is not supported. Use **/langs** to see available languages."),
        ("command.settings.default.language.success", "Default language changed to **{language}** with **{voice}**'s voice."),
        ("command.settings.default.voice.invalidated", "The default language is invalid. Reset it with **/set_default language LANG_CODE**."),
        ("command.settings.default.voice.unsupported", "Voice **{voice}** is not supported. Use **/voices** to see available voices."),
        ("command.settings.default.voice.success", "Default voice changed to **{voice}**."),
        ("command.settings.default.volume.success", "Default volume changed to **{volume}**."),
        ("command.settings.default.rate.success", "Default rate changed to **{rate}**."),
        ("command.settings.default.pitch.success", "Default pitch changed to **{pitch}**."),

        ("command.settings.my.language.unsupported", "Language **{language}** is not supported. Use **/langs** to see available languages."),
        ("command.settings.my.language.success", "Your language changed to **{language}** with **{voice}**'s voice."),
        ("command.settings.my.voice.invalidated", "Your stored language is invalid. Reset it with **/set_my language LANG_CODE**."),
        ("command.settings.my.voice.unsupported", "Voice **{voice}** is not supported. Use **/voices** to see available voices."),
        ("command.settings.my.voice.success", "Your voice changed to **{voice}**."),
        ("command.settings.my.volume.success", "Your volume changed to **{volume}**."),
        ("command.settings.my.rate.success", "Your rate changed to **{rate}**."),
        ("command.settings.my.pitch.success", "Your pitch changed to **{pitch}**."),

        ("command.settings.default.embed.title", "Default settings for this guild"),
        ("command.settings.default.embed.description", "These settings are used when you have not configured your own."),
        ("command.settings.my.embed.title", "Your current settings, {name}"),
        ("command.settings.my.embed.description", "If you haven't set a value yet, the guild default is shown."),

        ("command.locale.success", "Bot language changed to **{locale}**."),
        ("command.timeout.out_of_range", "Invalid time. Must be between **{min}** and **{max}** minutes."),
        ("command.timeout.success", "I will leave after **{timeout}** minutes of inactivity."),

        ("command.tts_channel.set.success", "Messages sent in this channel will now be read out loud automatically."),
        ("command.tts_channel.unset.success", "Messages sent in this channel will no longer be read out loud automatically."),
    ]
    .into_iter()
    .collect()
});
