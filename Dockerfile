# Stage 1: Build
FROM rust:1.80-slim-bookworm AS builder

# Instalar dependencias necesarias para compilar opus2/libopus
RUN apt-get update && apt-get install -y --no-install-recommends \
    libopus-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY . .

RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

ARG DATE_CREATED
ARG VERSION

LABEL org.opencontainers.image.created=$DATE_CREATED
LABEL org.opencontainers.image.version=$VERSION
LABEL org.opencontainers.image.title="Discord TTS Bot (Rust)"
LABEL org.opencontainers.image.description="A Text-to-Speech bot for Discord. Ported to Rust."
LABEL org.opencontainers.image.source="https://github.com/devidence-dev/discord-tts-bot"

# En runtime se necesita libopus0 y ca-certificates para peticiones HTTPS externas.
# Ya no necesitamos ffmpeg, ya que symphonia realiza el decode de MP3 a PCM en Rust de forma nativa.
RUN apt-get update && apt-get install -y --no-install-recommends \
    libopus0 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /opt/app

COPY --from=builder /build/target/release/tts-bot ./tts-bot

ENV DATA_PATH=/opt/app/data
VOLUME ["/opt/app/data"]

CMD ["./tts-bot"]
