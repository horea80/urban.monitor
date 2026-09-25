# urban.monitor — project commands, the equivalent of package.json scripts. `just` lists them.
# Install: winget install Casey.Just   (or: cargo install just --locked)

# Git Bash: the deploy/ scripts are bash, and Git's `bash` is on PATH in PowerShell too.
set shell := ["bash", "-cu"]

# deploy target, same default as deploy/deploy-common.sh; override with DEPLOY_HOST=millionphones just logs
host := env("DEPLOY_HOST", "emailbox")

# list the commands
default:
    @'{{just_executable()}}' --list --unsorted

# ── development ───────────────────────────────────────────────────────────────────────────────────

# UI + server with hot reload, http://127.0.0.1:8080
dev:
    dx serve -p urban-app

# server + SSR only, no wasm (good for the API); RUST_LOG=info,tower_http=debug shows requests
server:
    cargo run -p urban-app --features server

# manual sync into ./data; arguments go to the CLI (e.g. just sync --help)
sync *ARGS:
    cargo run -p urban-core --bin urban -- sync {{ARGS}}

# tests for the whole workspace
test:
    cargo test --workspace

# ── release builds ────────────────────────────────────────────────────────────────────────────────

# release bundle for THIS machine (server.exe + public/) → target/dx/urban-app/release/web; run it with `just run-local`; dx's tight profiles (fat LTO), slower than build-cross
build-local:
    dx bundle --release --platform web -p urban-app

# release bundle for the box: wasm client + server cross-compiled for aarch64 Linux → target/deploy/ (ADR-0010)
build-cross:
    ./deploy/build.sh

# the local release bundle, as production runs it (SSR + hydration + assets), http://127.0.0.1:8080
run-local:
    cd target/dx/urban-app/release/web && IP=127.0.0.1 PORT=8080 ./server.exe

# ── deploy (ADR-0009, ADR-0010) ───────────────────────────────────────────────────────────────────

# one-time server provisioning: the urban user, the systemd unit, the nginx/Caddy site
setup:
    scp -r deploy {{host}}: && ssh {{host}} 'cd deploy && ./setup.sh'

# .env.prod → /opt/urban/.env, restarting the service if it is running
deploy-env:
    ./deploy/deploy-env.sh

# build-cross, ship to the box, smoke test, atomic install (old binary kept as server.prev), restart, /healthz
deploy-server:
    ./deploy/deploy.sh

# restart the service, which syncs 15 s after startup; `just resync full` re-downloads everything (slow)
resync MODE="":
    @[ -z "{{MODE}}" ] || [ "{{MODE}}" = full ] || { echo "usage: just resync [full]" >&2; exit 1; }
    ssh {{host}} '{{ if MODE == "full" { "sudo -u urban touch /opt/urban/data/full-sync && " } else { "" } }}sudo systemctl restart urban'
    @echo "sync starts in ~15 s; follow it with: just logs"

# follow the service journal
logs:
    ssh {{host}} 'journalctl -u urban -f'

# unit status and the public /healthz
status:
    ssh {{host}} 'systemctl status urban --no-pager -l | head -8'
    curl -fsS https://urbanism.hopartean.com/healthz; echo
