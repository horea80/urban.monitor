# urban.monitor

Serviciu care urmărește ședințele Comisiei Tehnice de Amenajare a Teritoriului și
Urbanism (CTATU) ale Primăriei Cluj-Napoca, începând cu 2026, și permite căutarea
după stradă a inițiativelor de urbanism: **PUZ**, **PUD** și **avize de oportunitate PUZ**.

Sursa datelor: [primariaclujnapoca.ro › Ședințe comisie](https://primariaclujnapoca.ro/strategii-urbane/comisia-tehnica-de-amenajare-a-teritoriului-si-urbanism/sedinte-comisie/)

Documentație: cerințe în [`docs/spec/requirements.md`](docs/spec/requirements.md), design în [`docs/spec/design.md`](docs/spec/design.md), decizii (ADR, format MADR) în [`docs/adr/`](docs/adr/README.md), harta codului în [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Stack

Rust (edition 2024) · Dioxus 0.7 fullstack · axum 0.8 · SQLite (rusqlite, FTS5) ·
reqwest · scraper · pdf-extract · utoipa (OpenAPI) · systemd + nginx (sau Caddy) pe VPS, fără Docker.

```
crates/shared   tipuri + normalizare text (compilează și la wasm)
crates/core     scraper, PDF, SQLite, sync, CLI `urban`
crates/app      UI Dioxus + API JSON /api/v1 + OpenAPI
deploy/         setup.sh, build.sh, deploy.sh, deploy-env.sh, urban.service, urban.nginx, urban.caddy
```

## Prerechizite (Windows)

```powershell
winget install Rustlang.Rustup        # acceptă instalarea Visual Studio Build Tools când întreabă
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --locked     # binarul `dx`
winget install Casey.Just             # `just`, comenzile proiectului (echivalentul scripturilor npm)
```

`rust-toolchain.toml` fixează canalul stable și adaugă automat target-ul wasm.

## Dezvoltare

Comenzile uzuale sunt în `justfile`; `just` fără argumente le listează (`just dev`, `just deploy-server`, …).
Echivalentele directe:

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

## Deploy (box OCI ARM împrumutat, systemd + nginx, fără Docker)

Producția rulează pe un box împrumutat (Oracle Cloud ARM Ampere, Ubuntu 24.04, aliasul ssh `emailbox`), la
`https://horea.hopartean.com` ([ADR-0009](docs/adr/0009-gazduire-box-imprumutat-nginx.md)). Box-ul nu
compilează nimic: build-ul se face pe stație, cu serverul cross-compilat pentru aarch64
([ADR-0010](docs/adr/0010-build-pe-statie-cross-compilare-zig.md)). Porturile 80/443 le ține nginx-ul
proprietarului; noi adăugăm doar un site, iar TLS-ul îl obține certbot.

Unelte pe stație, o singură dată (pe lângă cele din Prerechizite):

```powershell
rustup target add aarch64-unknown-linux-gnu
winget install zig.zig                  # compilator C și linker pentru cross-compilare
cargo install cargo-zigbuild --locked
```

O singură dată, pentru server:

1. DNS, în zona `hopartean.com`: `horea  A  161.153.121.17`.
2. `just setup` (sau `scp -r deploy emailbox:` și `./setup.sh` din `deploy/`): utilizatorul `urban`, unitatea
   systemd și site-ul nginx. Pe un box fără nginx (VPS-ul propriu) instalează Caddy și `urban.caddy`; atunci
   pune domeniul acolo și `sudo systemctl reload caddy`.
3. Când DNS-ul răspunde: `ssh emailbox 'sudo certbot --nginx -d horea.hopartean.com --redirect'`.

Apoi, de pe stație (Git Bash sau PowerShell; ținta implicită e `emailbox`, vezi `DEPLOY_HOST`/`DEPLOY_KEY` în
`deploy/deploy-common.sh`):

```bash
cp .env.example .env.prod      # ajustează dacă e nevoie; .env.prod e ignorat de git
just deploy-env                # .env.prod → /opt/urban/.env
just deploy-server             # build local, artefacte → box, smoke test, instalare atomică, restart, /healthz
```

`just build-cross` face doar build-ul pentru box, în `target/deploy/`; `just build-local` și `just run-local` construiesc și
pornesc pe stație același bundle de release (server.exe + public/), ca să vezi local exact ce rulează în producție. Loguri: `just logs`; accesul prin nginx în
`/var/log/nginx/urban.access.log`. Backup: copia fișierului `/opt/urban/data/urban.db`. Rollback: pe box,
`sudo mv /opt/urban/server.prev /opt/urban/server && sudo systemctl restart urban`.

## API

Read-only, public în v1, documentat OpenAPI: `GET /api/v1/openapi.json`, UI la `/api/docs`.
Planul pentru chei API și cote: [ADR-0004](docs/adr/0004-api-public-openapi-auth-ulterior.md).

## Stare

Funcțional local: scraping, ordine de zi din PDF, SQLite/FTS5, CLI, UI (căutare, ședințe) și API
OpenAPI. Urmează: primul deploy pe VPS cu `deploy/deploy.sh`.
