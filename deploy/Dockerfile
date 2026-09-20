# syntax=docker/dockerfile:1
# Build: docker build -f deploy/Dockerfile -t urban-monitor .
# Layout-ul de ieșire al `dx bundle` (0.7): target/dx/urban-app/release/web/{server,public/}
# -> de verificat la primul build.

FROM rust:1-bookworm AS builder
RUN rustup target add wasm32-unknown-unknown \
 && cargo install dioxus-cli --locked
WORKDIR /src
COPY . .
RUN dx bundle --release --platform web -p urban-app

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
RUN useradd --system --uid 10001 --create-home urban
WORKDIR /app
COPY --from=builder /src/target/dx/urban-app/release/web/ /app/
ENV IP=0.0.0.0 \
    PORT=8080 \
    URBAN_DATA_DIR=/data \
    RUST_LOG=info
VOLUME ["/data"]
EXPOSE 8080
USER urban
CMD ["/app/server"]
