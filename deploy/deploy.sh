#!/bin/bash
# deploy/deploy.sh — trimite sursa pe VPS, rulează `dx bundle --release` ACOLO (box-ul e aarch64, un
# binar compilat pe Windows nu rulează pe el), instalează atomic în /opt/urban și repornește unitatea.
#
# Folosire (Git Bash, din rădăcina repo-ului sau de oriunde):
#   ./deploy/deploy.sh
#   DEPLOY_HOST=1.2.3.4 ./deploy/deploy.sh
#
# Prima dată: deploy/setup.sh pe server, apoi deploy/deploy-env.sh de pe stație.
set -euo pipefail

# shellcheck source=deploy-common.sh
source "$(dirname "$0")/deploy-common.sh"
cd "$(dirname "$0")/.."

# ── 1. Sursa → server (fără target/, data/, .git; target/ de pe server rămâne, pentru build incremental)
echo "▶ Sincronizez sursa → $REMOTE:$BUILD_DIR"
$SSH "$REMOTE" "mkdir -p '$BUILD_DIR' && cd '$BUILD_DIR' && find . -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +"
tar czf - --exclude=./target --exclude=./data --exclude=./.git --exclude='./.env*' . \
  | $SSH "$REMOTE" "cd '$BUILD_DIR' && tar xzf -"

# ── 2. Build pe server ───────────────────────────────────────────────────────────────────────────
echo "▶ Build pe server: dx bundle --release (câteva minute pe ARM)"
$SSH "$REMOTE" "source ~/.cargo/env && cd '$BUILD_DIR' && dx bundle --release --platform web -p urban-app"

# ── 3. Instalare atomică: server.new/public.new → server/public ─────────────────────────────────
echo "▶ Instalez în $REMOTE_DIR"
$SSH "$REMOTE" "
  set -e
  OUT='$BUILD_DIR/target/dx/urban-app/release/web'
  test -x \"\$OUT/server\" && test -d \"\$OUT/public\"
  sudo mkdir -p '$REMOTE_DIR/data'
  sudo rm -rf '$REMOTE_DIR/public.new' '$REMOTE_DIR/public.old'
  sudo cp -r \"\$OUT/public\" '$REMOTE_DIR/public.new'
  sudo install -m 0755 \"\$OUT/server\" '$REMOTE_DIR/server.new'
  if [ -d '$REMOTE_DIR/public' ]; then sudo mv '$REMOTE_DIR/public' '$REMOTE_DIR/public.old'; fi
  sudo mv '$REMOTE_DIR/public.new' '$REMOTE_DIR/public'
  sudo mv -f '$REMOTE_DIR/server.new' '$REMOTE_DIR/server'
  sudo rm -rf '$REMOTE_DIR/public.old'
  sudo chown -R $APP_USER:$APP_USER '$REMOTE_DIR'
"

# ── 4. Repornire + verificare ────────────────────────────────────────────────────────────────────
echo "▶ Repornesc $SERVICE"
$SSH "$REMOTE" "sudo systemctl daemon-reload && sudo systemctl restart $SERVICE && sleep 2 \
  && sudo systemctl status $SERVICE --no-pager -l | head -12"

echo "▶ /healthz"
$SSH "$REMOTE" "set -a; . '$REMOTE_DIR/.env'; set +a; curl -fsS \"http://127.0.0.1:\${PORT:-8081}/healthz\"; echo"

echo ""
echo "✓ Deploy terminat."
echo "  Loguri: $SSH $REMOTE 'journalctl -u $SERVICE -f'"
