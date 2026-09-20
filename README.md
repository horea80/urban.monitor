# urban.monitor

Serviciu care urmărește ședințele Comisiei Tehnice de Amenajare a Teritoriului și
Urbanism (CTATU) ale Primăriei Cluj-Napoca, începând cu 2026, și permite căutarea
după stradă a inițiativelor de urbanism: **PUZ**, **PUD** și **avize de oportunitate PUZ**.

Sursa datelor: [primariaclujnapoca.ro › Ședințe comisie](https://primariaclujnapoca.ro/strategii-urbane/comisia-tehnica-de-amenajare-a-teritoriului-si-urbanism/sedinte-comisie/)

Documentație: cerințe în [`docs/spec/requirements.md`](docs/spec/requirements.md), design în [`docs/spec/design.md`](docs/spec/design.md), decizii (ADR, format MADR) în [`docs/adr/`](docs/adr/README.md), harta codului în [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Stack

Rust (edition 2024) · Dioxus 0.7 fullstack · axum 0.8 · SQLite (rusqlite, FTS5) ·
reqwest · scraper · pdf-extract · utoipa (OpenAPI) · Caddy + Docker pentru deploy.

```
crates/shared   tipuri + normalizare text (compilează și la wasm)
crates/core     scraper, PDF, SQLite, sync, CLI `urban`
crates/app      UI Dioxus + API JSON /api/v1 + OpenAPI
deploy/         Dockerfile, Caddyfile, docker-compose.yml
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
cargo run -p urban-core --bin urban -- sync  # sincronizare manuală (după ce există comanda)
dx serve -p urban-app                        # UI + server, cu hot reload
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

## Deploy (VPS + Caddy)

```bash
cd deploy
# editează domeniul în Caddyfile
docker compose up -d --build
```

## API

Read-only, public în v1, documentat OpenAPI: `GET /api/v1/openapi.json`, UI la `/api/docs`.
Planul pentru chei API și cote: [ADR-0004](docs/adr/0004-api-public-openapi-auth-ulterior.md).

## Stare

Schelet: workspace, manifeste, fixture-uri, deploy, CI. Codul de scraping, DB, UI și API urmează.
