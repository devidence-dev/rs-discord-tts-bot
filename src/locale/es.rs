use std::collections::HashMap;
use std::sync::LazyLock;

pub(crate) static ES_STRINGS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    [
        ("error.channel.not_viewable", "No puedo ver tu canal de voz. ¿Tengo los suficientes permisos para verlo?"),
        ("error.channel.full", "Tu canal de voz está lleno."),
        ("error.channel.not_joinable", "No puedo unirme a tu canal de voz. ¿Tengo los suficientes permisos para unirme?"),
        ("error.channel.not_speakable", "No puedo hablar en tu canal de voz. ¿Tengo los suficientes permisos para hablar en él?"),

        ("command.say.no_channel", "Necesitas estar en un canal de voz."),
        ("command.say.different_channel", "Necesitas estar en mi canal de voz para decir algo."),
        ("command.say.success", "Diré eso ahora."),
        ("command.say.joined.withrequest", "Entré a {channel}."),
        ("command.say.joined", "Entré a {channel}."),

        ("command.stop.no_connection", "No estoy en un canal de voz."),
        ("command.stop.different_channel", "Necesitas estar en mi canal de voz para detenerme."),
        ("command.stop.success", "He salido del canal de voz {channel} con éxito."),

        ("command.help.embed.title", "Mensaje de ayuda de Text-to-Speech"),
        ("command.help.description", "Este bot usa Amazon TTS para hablar mensajes en tu canal de voz.\n\n**Comandos:**\n`/say` — Decir un mensaje.\n`/langs` — Listar idiomas disponibles.\n`/voices` — Listar voces para un idioma.\n`/set_my` — Configurar tus ajustes personales.\n`/set_default` — Configurar los ajustes por defecto del servidor.\n`/stop` — Detener TTS y salir del canal.\n`/set_locale` — Cambiar el idioma del bot (en/es).\n`/set_timeout` — Configurar el tiempo de inactividad."),
        ("command.help.tts_channels_extra", "`/set_tts_channel` — Leer en voz alta automáticamente cada mensaje enviado en este canal.\n`/unset_tts_channel` — Dejar de leer en voz alta los mensajes de este canal."),

        ("command.langs.embed.title", "Idiomas disponibles del proveedor Amazon:"),
        ("command.langs.embed.description", "Usa **/set_my language LANG_CODE** para cambiar tu idioma.\nUsa **/set_default language LANG_CODE** para cambiar el idioma por defecto del servidor."),

        ("command.voices.embed.title", "Voces disponibles para el idioma {language}:"),
        ("command.voices.embed.description", "Usa **/set_my voice VOICE_NAME** para cambiar tu voz.\nUsa **/set_default voice VOICE_NAME** para cambiar la voz por defecto del servidor."),
        ("command.voices.error.unsupported", "El idioma **{language}** no está disponible. Usa **/langs** para ver los idiomas disponibles."),

        ("command.settings.default.language.unsupported", "El idioma **{language}** no está disponible. Usa **/langs** para ver los idiomas disponibles."),
        ("command.settings.default.language.success", "Idioma por defecto cambiado a **{language}** con la voz de **{voice}**."),
        ("command.settings.default.voice.invalidated", "El idioma por defecto guardado es inválido. Reinícialo con **/set_default language LANG_CODE**."),
        ("command.settings.default.voice.unsupported", "La voz **{voice}** no está disponible. Usa **/voices** para ver las voces disponibles."),
        ("command.settings.default.voice.success", "Voz por defecto cambiada a **{voice}**."),
        ("command.settings.default.volume.success", "Volumen por defecto cambiado a **{volume}**."),
        ("command.settings.default.rate.success", "Ritmo por defecto cambiado a **{rate}**."),
        ("command.settings.default.pitch.success", "Tono por defecto cambiado a **{pitch}**."),

        ("command.settings.my.language.unsupported", "El idioma **{language}** no está disponible. Usa **/langs** para ver los idiomas disponibles."),
        ("command.settings.my.language.success", "Tu idioma fue cambiado a **{language}** con la voz de **{voice}**."),
        ("command.settings.my.voice.invalidated", "Tu idioma guardado es inválido. Reinícialo con **/set_my language LANG_CODE**."),
        ("command.settings.my.voice.unsupported", "La voz **{voice}** no está disponible. Usa **/voices** para ver las voces disponibles."),
        ("command.settings.my.voice.success", "Tu voz fue cambiada a **{voice}**."),
        ("command.settings.my.volume.success", "Tu volumen fue cambiado a **{volume}**."),
        ("command.settings.my.rate.success", "Tu ritmo fue cambiado a **{rate}**."),
        ("command.settings.my.pitch.success", "Tu tono fue cambiado a **{pitch}**."),

        ("command.settings.default.embed.title", "Configuración por defecto de este servidor"),
        ("command.settings.default.embed.description", "Esta configuración se usa cuando no has configurado la tuya propia."),
        ("command.settings.my.embed.title", "Tu configuración actual, {name}"),
        ("command.settings.my.embed.description", "Si no has configurado un valor, se muestra el del servidor."),

        ("command.locale.success", "Idioma del bot cambiado a **{locale}**."),
        ("command.timeout.out_of_range", "Tiempo inválido. Debe estar entre **{min}** y **{max}** minutos."),
        ("command.timeout.success", "Me iré del canal de voz después de **{timeout}** minutos de inactividad."),

        ("command.tts_channel.set.success", "Los mensajes enviados en este canal ahora se leerán en voz alta automáticamente."),
        ("command.tts_channel.unset.success", "Los mensajes enviados en este canal ya no se leerán en voz alta automáticamente."),
    ]
    .into_iter()
    .collect()
});
