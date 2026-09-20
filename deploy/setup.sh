#!/bin/bash
# deploy/setup.sh — provizionarea VPS-ului (Ubuntu, Oracle Cloud ARM Ampere). Se rulează O DATĂ, pe
# server, din directorul deploy/ (după `scp -r deploy ubuntu@vps:` sau un clone). Idempotent.
#
# Instalează: unelte de build, rustup + target wasm + dx (build-ul se face pe server, ADR-0008),
# Caddy, utilizatorul `urban`, /opt/urban, unitatea systemd și fișierul de site Caddy.
set -euo pipefail

APP_USER="urban"
APP_DIR="/opt/urban"
DX_VERSION="0.7.10"   # aceeași versiune ca pe stație (rust-toolchain.toml fixează canalul stable)

echo "=== urban.monitor: setup VPS ==="

# ── unelte de build (rusqlite bundled și ring au nevoie de un compilator C) ─────────────────────
sudo apt update
sudo apt install -y build-essential pkg-config curl git ca-certificates

# ── Rust + dx, pentru utilizatorul curent (cel care rulează deploy.sh) ───────────────────────────
if ! command -v cargo >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/cargo" ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
# shellcheck disable=SC1091
source "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown
if ! command -v dx >/dev/null 2>&1 || [ "$(dx --version 2>/dev/null | awk '{print $2}')" != "$DX_VERSION" ]; then
  # binstall ia binarul gata compilat pentru aarch64; altfel compilăm dx (10+ minute)
  if ! command -v cargo-binstall >/dev/null 2>&1; then
    curl -L --proto '=https' --tlsv1.2 -sSf \
      https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
  fi
  cargo binstall -y "dioxus-cli@$DX_VERSION" || cargo install dioxus-cli --version "$DX_VERSION" --locked
fi
echo "dx: $(dx --version)"

# ── Caddy (din depozitul oficial), doar dacă lipsește ──────────────────────────────────────────
if ! command -v caddy >/dev/null 2>&1; then
  sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
    | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
    | sudo tee /etc/apt/sources.list.d/caddy-stable.list >/dev/null
  sudo apt update
  sudo apt install -y caddy
else
  echo "Caddy există deja: $(caddy version)"
fi

# ── utilizator și directoare ───────────────────────────────────────────────────────────────────
sudo useradd --system --home "$APP_DIR" --shell /usr/sbin/nologin "$APP_USER" 2>/dev/null || true
sudo mkdir -p "$APP_DIR/data"
sudo chown -R "$APP_USER:$APP_USER" "$APP_DIR"

# ── Caddy: un fișier de site per proiect în /etc/caddy/sites/ (box-ul e împărțit cu alte proiecte;
#    nu suprascriem Caddyfile-ul principal, doar ne asigurăm că importă directorul) ──────────────
sudo mkdir -p /etc/caddy/sites /var/log/caddy
sudo cp urban.caddy /etc/caddy/sites/urban.caddy
if ! sudo grep -qs 'import /etc/caddy/sites/\*' /etc/caddy/Caddyfile; then
  echo 'import /etc/caddy/sites/*' | sudo tee -a /etc/caddy/Caddyfile >/dev/null
fi

# ── systemd ────────────────────────────────────────────────────────────────────────────────────
sudo cp urban.service /etc/systemd/system/urban.service
sudo systemctl daemon-reload
sudo systemctl enable urban

echo ""
echo "=== Setup terminat ==="
echo "Pași următori:"
echo "1. Pune domeniul real în /etc/caddy/sites/urban.caddy, apoi: sudo systemctl reload caddy"
echo "2. De pe stație: ./deploy/deploy-env.sh   (.env.prod → $APP_DIR/.env)"
echo "3. De pe stație: ./deploy/deploy.sh       (build pe server, instalare, restart)"
echo "Unitatea pornește în gol până există binarul și .env; e normal să vezi eșecuri în journal până atunci."
