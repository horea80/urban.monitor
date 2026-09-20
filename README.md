# urban.monitor

Serviciu care urmărește ședințele Comisiei Tehnice de Amenajare a Teritoriului și
Urbanism (CTATU) ale Primăriei Cluj-Napoca, începând cu 2026, și permite căutarea
după stradă a inițiativelor de urbanism: **PUZ**, **PUD** și **avize de oportunitate PUZ**.

Sursa datelor: [primariaclujnapoca.ro › Ședințe comisie](https://primariaclujnapoca.ro/strategii-urbane/comisia-tehnica-de-amenajare-a-teritoriului-si-urbanism/sedinte-comisie/)

Documentație: cerințe în [`docs/spec/requirements.md`](docs/spec/requirements.md), design în [`docs/spec/design.md`](docs/spec/design.md), decizii (ADR, format MADR) în [`docs/adr/`](docs/adr/README.md), harta codului în [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Stack

Rust (edition 2024) · Dioxus 0.7 fullstack · axum 0.8 · SQLite (rusqlite, FTS5) ·
reqwest · scraper · pdf-extract · utoipa (OpenAPI) · systemd + Caddy pe VPS, fără Docker.

```
crates/shared   tipuri + normalizare text (compilează și la wasm)
crates/core     scraper, PDF, SQLite, sync, CLI `urban`
crates/app      UI Dioxus + API JSON /api/v1 + OpenAPI
deploy/         setup.sh, deploy.sh, deploy-env.sh, urban.service, urban.caddy
```

## Prerechizite (Windows)

```powershell
winget install Rustlang.Rustup        # acceptă instalarea Visual Studio Build Tools când întreabă
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --locked     # binarul `dx`
```

`rust-toolchain.toml` fixează canalul stable și adaugă automat target-ul wasm.

## Dezvoltare

```powershell
cargo build --workspace                      # primul build compilează toate dependențele
cargo test --workspace                       # parserele rulează pe fixture-urile din crates/core/tests/fixtures
cargo run -p urban-core --bin urban -- sync  # sincronizare manuală
dx serve -p urban-app                        # UI + server, cu hot reload
cargo run -p urban-app --features server     # doar server + SSR, fără wasm (bun pentru API)
```

Variabile de mediu (toate au valori implicite):

| Variabilă | Implicit | Rol |
|---|---|---|
| `URBAN_DATA_DIR` | `./data` | baza SQLite și snapshot-urile brute |
| `URBAN_START_YEAR` | `2026` | prima ședință indexată |
| `URBAN_REFRESH_DAYS` | `45` | ședințele mai noi de atâtea zile se re-descarcă |
| `URBAN_SYNC_HOURS` | `6` | intervalul task-ului de sync din server |
| `URBAN_REQUEST_DELAY_MS` | `1000` | pauza între cereri către site-ul primăriei |
| `IP` / `PORT` | `127.0.0.1` / `8080` | adresa serverului |

## Deploy (VPS Oracle ARM, systemd + Caddy, fără Docker)

Box-ul e aarch64, deci build-ul de producție se face pe server ([ADR-0008](docs/adr/0008-deploy-fara-docker-systemd-caddy.md)).

O singură dată, pe server, din directorul `deploy/` (după `scp -r deploy ubuntu@vps:` sau un clone):
`./setup.sh` instalează rustup, `dx`, Caddy, utilizatorul `urban`, unitatea systemd și fișierul
de site. Apoi pune domeniul real în `/etc/caddy/sites/urban.caddy` și `sudo systemctl reload caddy`.

De pe stație (Git Bash), cu aliasul ssh din `~/.ssh/config` (implicit `millionphones`, vezi
`deploy/deploy-common.sh`):

```bash
cp .env.example .env.prod      # ajustează dacă e nevoie; .env.prod e ignorat de git
./deploy/deploy-env.sh         # .env.prod → /opt/urban/.env
./deploy/deploy.sh             # sursa → server, dx bundle --release acolo, instalare atomică, restart, /healthz
```

Loguri: `ssh millionphones 'journalctl -u urban -f'`. Backup: copia fișierului `/opt/urban/data/urban.db`.

## API

Read-only, public în v1, documentat OpenAPI: `GET /api/v1/openapi.json`, UI la `/api/docs`.
Planul pentru chei API și cote: [ADR-0004](docs/adr/0004-api-public-openapi-auth-ulterior.md).

## Stare

Funcțional local: scraping, ordine de zi din PDF, SQLite/FTS5, CLI, UI (căutare, ședințe) și API
OpenAPI. Urmează: primul deploy pe VPS cu `deploy/deploy.sh`.
