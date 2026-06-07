# 🎙️ tts-bot

Lightweight Discord Text-to-Speech bot written in Rust (serenity + songbird).

## ✨ Overview

- Play TTS in voice channels using Amazon or other providers.
- Built with `serenity`, `songbird`, `tokio` and `redb` for storage.
- Focused on reliability and modern Discord voice (DAVE) support.

## 🔗 Inspired by

This project was inspired by and builds on the ideas from [moonstar-x/discord-tts-bot](https://github.com/moonstar-x/discord-tts-bot). ✨

## 🚀 Quick start

### Prerequisites

- Rust (recommended stable toolchain)
- A Discord bot token with appropriate intents
- (Optional) Docker

### Environment variables

- `DISCORD_TOKEN` (required) — your bot token.
- `TESTING_GUILD_ID` (optional) — register commands in a test guild.
- `DATA_PATH` (optional) — defaults to `./data`.
- `ENABLE_TTS_CHANNELS` (optional) — set to `true` or `1` to enable TTS channels behavior.

### Run locally (Cargo)

```bash
# Build
cargo build --release

# Run (example)
DISCORD_TOKEN="<your_token>" DATA_PATH="./data" ./target/release/tts-bot
```

### Run with Docker

```bash
# Build image
docker build -t rs-discord-tts-bot:latest .

# Run container (example)
docker run -e DISCORD_TOKEN="$DISCORD_TOKEN" -v $PWD/data:/opt/app/data rs-discord-tts-bot:latest
```

## 🧩 Features

- Slash commands and message-driven TTS (configurable).
- Persistent settings stored under `DATA_PATH` (`redb` DB file: `settings.redb`).
- English (`en`) and Spanish (`es`) locales included.
- Uses native `libopus` at runtime (see Dockerfile).

## 🛠️ Configuration & Notes

- The app creates `DATA_PATH` automatically if missing.
- `ENABLE_TTS_CHANNELS=true` enables automatic TTS channels behavior (see code).
- The bot requires the following gateway intents: `GUILDS`, `GUILD_VOICE_STATES`, `GUILD_MESSAGES`. If `ENABLE_TTS_CHANNELS` is enabled, the bot also requests `MESSAGE_CONTENT`.

## 📁 Important files

- `src/main.rs` — bot entrypoint and env vars.
- `Cargo.toml` — project metadata and dependencies.
- `Dockerfile` — multi-stage build & runtime image (libopus required at runtime).
- `plan-go-rust.md` — migration notes and design decisions.

