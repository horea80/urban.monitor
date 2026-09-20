#!/bin/bash
# deploy/deploy-env.sh — trimite configurația pe server: .env.prod (sau .env.example, dacă nu există
# .env.prod) → /opt/urban/.env, apoi repornește unitatea dacă rulează.
#
# Folosire: ./deploy/deploy-env.sh
set -euo pipefail

# shellcheck source=deploy-common.sh
source "$(dirname "$0")/deploy-common.sh"
cd "$(dirname "$0")/.."

SRC=".env.prod"
if [ ! -f "$SRC" ]; then
  echo "▶ Nu există .env.prod; folosesc .env.example (valorile implicite)"
  SRC=".env.example"
fi

echo "▶ $SRC → $REMOTE:$REMOTE_DIR/.env"
$SSH "$REMOTE" "sudo mkdir -p '$REMOTE_DIR' && sudo tee '$REMOTE_DIR/.env' >/dev/null \
  && sudo chown $APP_USER:$APP_USER '$REMOTE_DIR/.env' && sudo chmod 0640 '$REMOTE_DIR/.env'" < "$SRC"

if $SSH "$REMOTE" "systemctl is-active --quiet $SERVICE"; then
  echo "▶ Repornesc $SERVICE ca să citească noul .env"
  $SSH "$REMOTE" "sudo systemctl restart $SERVICE"
fi
echo "✓ Configurație trimisă."
