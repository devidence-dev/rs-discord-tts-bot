# Stage 1: Build
FROM rust:1.96.0-slim-trixie AS builder

# Instalar dependencias necesarias para compilar opus2/libopus
RUN apt-get update && apt-get install -y --no-install-recommends \
    libopus-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY . .

RUN cargo build --release

# Stage 2: Runtime
# distroless/base-nossl: incluye glibc + ca-certificates (rustls no usa OpenSSL,
# así que no necesitamos la variante con libssl). Sin shell ni gestor de paquetes,
# por lo que las librerías nativas que el binario enlaza dinámicamente (libopus,
# libgcc_s para el unwinding de pánico de Rust) se copian del stage de build.
FROM gcr.io/distroless/base-nossl-debian13

ARG DATE_CREATED
ARG VERSION

LABEL org.opencontainers.image.created=$DATE_CREATED
LABEL org.opencontainers.image.version=$VERSION
LABEL org.opencontainers.image.title="Discord TTS Bot (Rust)"
LABEL org.opencontainers.image.description="A Text-to-Speech bot for Discord. Ported to Rust."
LABEL org.opencontainers.image.source="https://github.com/devidence-dev/discord-tts-bot"

WORKDIR /opt/app

COPY --from=builder /usr/lib/x86_64-linux-gnu/libopus.so* /usr/lib/x86_64-linux-gnu/
COPY --from=builder /usr/lib/x86_64-linux-gnu/libgcc_s.so* /usr/lib/x86_64-linux-gnu/
COPY --from=builder /build/target/release/tts-bot ./tts-bot

ENV DATA_PATH=/opt/app/data
VOLUME ["/opt/app/data"]

CMD ["./tts-bot"]
