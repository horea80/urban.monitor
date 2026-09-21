#!/bin/bash
# deploy/deploy.sh — build PE STAȚIE (deploy/build.sh: wasm cu dx, server cross-compilat pentru aarch64 cu
# cargo-zigbuild), apoi artefactele pleacă pe box, sunt probate cu un smoke test (/healthz pe un port
# temporar, fără sincronizare), se instalează atomic în /opt/urban (binarul vechi rămâne ca server.prev)
# și unitatea se repornește. ADR-0010; box-ul nu compilează nimic.
#
# Folosire (Git Bash, din rădăcina repo-ului sau de oriunde):  just deploy-server   sau   ./deploy/deploy.sh
#   DEPLOY_HOST=millionphones ./deploy/deploy.sh    altă țintă (vezi deploy-common.sh)
#
# Prima dată: deploy/setup.sh pe server, apoi just deploy-env de pe stație.
set -euo pipefail

# shellcheck source=deploy-common.sh
source "$(dirname "$0")/deploy-common.sh"
cd "$(dirname "$0")/.."

# ── 1. Build local ───────────────────────────────────────────────────────────────────────────────
./deploy/build.sh

# ── 2. Artefactele → box ─────────────────────────────────────────────────────────────────────────
echo "▶ Shipping target/deploy → $REMOTE:$STAGE_DIR"
$SSH "$REMOTE" "mkdir -p '$STAGE_DIR' && cd '$STAGE_DIR' && rm -rf server public smoke"
# chmod pe box: tar-ul din Git Bash nu păstrează bitul de execuție pentru un ELF fără extensie (MSYS)
tar czf - -C target/deploy server public | $SSH "$REMOTE" "cd '$STAGE_DIR' && tar xzf - && chmod 0755 server"

# ── 3. Smoke test: binarul cross-compilat pornește pe box și răspunde la /healthz ─────────────────
echo "▶ Smoke test on the box (port 8099, temporary data dir, sync disabled)"
$SSH "$REMOTE" "
  set -e
  cd '$STAGE_DIR' && mkdir -p smoke
  URBAN_SYNC_HOURS=0 URBAN_DATA_DIR='$STAGE_DIR/smoke' IP=127.0.0.1 PORT=8099 RUST_LOG=warn \
    nohup '$STAGE_DIR/server' > smoke/log 2>&1 &
  pid=\$!
  for i in 1 2 3 4 5 6 7 8 9 10; do sleep 1; curl -fsS http://127.0.0.1:8099/healthz >/dev/null 2>&1 && break; done
  ok=0; curl -fsS http://127.0.0.1:8099/healthz && ok=1; echo
  # asset!() nerezolvat de dx => <link href> cu textul placeholder și pagină fără stiluri
  if curl -fsS http://127.0.0.1:8099/ | grep -q 'This should be replaced by dx'; then echo '✗ asset placeholders in the SSR page (server not built by dx?)'; ok=0; fi
  kill \$pid 2>/dev/null || true
  rm -rf smoke/*.db*
  if [ \$ok -ne 1 ]; then echo '✗ smoke test failed; server log:'; cat smoke/log; exit 1; fi
"

# ── 4. Instalare atomică: *.new → în loc; binarul vechi rămâne server.prev ────────────────────────
echo "▶ Installing into $REMOTE_DIR"
$SSH "$REMOTE" "
  set -e
  sudo mkdir -p '$REMOTE_DIR/data'
  sudo rm -rf '$REMOTE_DIR/public.new' '$REMOTE_DIR/public.old'
  sudo cp -r '$STAGE_DIR/public' '$REMOTE_DIR/public.new'
  sudo install -m 0755 '$STAGE_DIR/server' '$REMOTE_DIR/server.new'
  if [ -d '$REMOTE_DIR/public' ]; then sudo mv '$REMOTE_DIR/public' '$REMOTE_DIR/public.old'; fi
  sudo mv '$REMOTE_DIR/public.new' '$REMOTE_DIR/public'
  if [ -f '$REMOTE_DIR/server' ]; then sudo mv -f '$REMOTE_DIR/server' '$REMOTE_DIR/server.prev'; fi
  sudo mv -f '$REMOTE_DIR/server.new' '$REMOTE_DIR/server'
  sudo rm -rf '$REMOTE_DIR/public.old'
  sudo chown -R $APP_USER:$APP_USER '$REMOTE_DIR'
"

# ── 5. Repornire + verificare ────────────────────────────────────────────────────────────────────
echo "▶ Restarting $SERVICE"
$SSH "$REMOTE" "sudo systemctl daemon-reload && sudo systemctl restart $SERVICE && sleep 2 \
  && sudo systemctl status $SERVICE --no-pager -l | head -6"

echo "▶ /healthz"
$SSH "$REMOTE" "PORT=\$(sudo grep -E '^PORT=' '$REMOTE_DIR/.env' | cut -d= -f2); curl -fsS \"http://127.0.0.1:\${PORT:-8081}/healthz\"; echo"

echo ""
echo "✓ Deploy complete."
echo "  Logs: just logs   Rollback: on the box, sudo mv $REMOTE_DIR/server.prev $REMOTE_DIR/server && sudo systemctl restart $SERVICE"
