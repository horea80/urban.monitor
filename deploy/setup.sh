#!/bin/bash
# deploy/setup.sh — provizionarea serverului (Ubuntu aarch64, Oracle Cloud ARM Ampere). Se rulează O DATĂ,
# pe server, din directorul deploy/ (după `scp -r deploy emailbox:` sau `just setup`). Idempotent.
#
# Instalează: utilizatorul `urban`, /opt/urban, unitatea systemd și site-ul din reverse proxy:
#   - box cu nginx deja pe 80/443 (cel împrumutat, ADR-0009): site în /etc/nginx/sites-available/, TLS prin certbot
#   - altfel: Caddy din depozitul oficial, site în /etc/caddy/sites/
# PROXY=nginx sau PROXY=caddy forțează alegerea.
# Box-ul nu compilează nimic: binarul vine gata construit de pe stație (deploy/build.sh, ADR-0010).
set -euo pipefail

APP_USER="urban"
APP_DIR="/opt/urban"
DOMAIN="horea.hopartean.com"   # același ca în urban.nginx
PROXY="${PROXY:-auto}"

echo "=== urban.monitor: server setup ==="

# ── utilizator și directoare ───────────────────────────────────────────────────────────────────
sudo useradd --system --home "$APP_DIR" --shell /usr/sbin/nologin "$APP_USER" 2>/dev/null || true
sudo mkdir -p "$APP_DIR/data"
sudo chown -R "$APP_USER:$APP_USER" "$APP_DIR"

# ── reverse proxy ──────────────────────────────────────────────────────────────────────────────
if [ "$PROXY" = auto ]; then
  if command -v nginx >/dev/null 2>&1; then PROXY=nginx; else PROXY=caddy; fi
fi
case "$PROXY" in
  nginx)
    # 80/443 sunt ale nginx-ului proprietarului: nu instalăm nimic, doar adăugăm un site. Certificatul îl
    # obține certbot (există pe box, cu plugin nginx) când DNS-ul arată aici — pasul 1 de la final.
    echo "proxy: nginx ($(nginx -v 2>&1))"
    sudo cp urban.nginx /etc/nginx/sites-available/urban
    sudo ln -sfn /etc/nginx/sites-available/urban /etc/nginx/sites-enabled/urban
    sudo nginx -t
    sudo systemctl reload nginx
    ;;
  caddy)
    # Caddy (din depozitul oficial), doar dacă lipsește
    if ! command -v caddy >/dev/null 2>&1; then
      sudo apt update
      sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https curl
      curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
        | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
      curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
        | sudo tee /etc/apt/sources.list.d/caddy-stable.list >/dev/null
      sudo apt update
      sudo apt install -y caddy
    else
      echo "Caddy already installed: $(caddy version)"
    fi
    # un fișier de site per proiect în /etc/caddy/sites/ (box-ul e împărțit cu alte proiecte;
    # nu suprascriem Caddyfile-ul principal, doar ne asigurăm că importă directorul)
    sudo mkdir -p /etc/caddy/sites /var/log/caddy
    sudo cp urban.caddy /etc/caddy/sites/urban.caddy
    if ! sudo grep -qs 'import /etc/caddy/sites/\*' /etc/caddy/Caddyfile; then
      echo 'import /etc/caddy/sites/*' | sudo tee -a /etc/caddy/Caddyfile >/dev/null
    fi
    ;;
  *)
    echo "unknown PROXY=$PROXY; use nginx or caddy" >&2
    exit 1
    ;;
esac

# ── systemd ────────────────────────────────────────────────────────────────────────────────────
sudo cp urban.service /etc/systemd/system/urban.service
sudo systemctl daemon-reload
sudo systemctl enable urban

echo ""
echo "=== Setup complete ==="
echo "Next steps:"
if [ "$PROXY" = nginx ]; then
  echo "1. Once DNS for $DOMAIN points at this box: sudo certbot --nginx -d $DOMAIN --redirect"
else
  echo "1. Put the real domain in /etc/caddy/sites/urban.caddy, then: sudo systemctl reload caddy"
fi
echo "2. From your workstation: just deploy-env      (.env.prod → $APP_DIR/.env)"
echo "3. From your workstation: just deploy-server   (build locally, ship, smoke test, install, restart)"
echo "The unit has nothing to run until the binary and .env exist; failures in the journal are expected until then."
