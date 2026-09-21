# Harta codului — urban.monitor

Acest fișier spune *unde e ce*. Cerințele sunt în [spec/requirements.md](spec/requirements.md),
designul în [spec/design.md](spec/design.md), iar deciziile, cu motivele lor, în
[adr/](adr/README.md).

```
Cargo.toml                 workspace: shared, core, app
rust-toolchain.toml        stable + target wasm32-unknown-unknown
Dioxus.toml                config `dx`

crates/shared/             urban-shared — compilează nativ și la wasm
  src/model.rs             tipuri de domeniu și de API
  src/text.rs              normalizare, clasificare, date românești, nume de stradă

crates/core/               urban-core — server-only
  src/scrape/              client HTTP politicos, parsere pentru listă, ședință, proiect
  src/pdf.rs               trait PdfText + implementări
  src/agenda.rs            ordine de zi: text → rânduri → potrivire cu cardurile
  src/db/                  migrații, upsert, căutare FTS5, sync_runs
  src/sync.rs              orchestrare incrementală
  src/bin/urban.rs         CLI: sync, search
  tests/fixtures/          HTML și PDF reale, salvate 2026-09-20

crates/app/                urban-app — Dioxus fullstack
  src/main.rs              alege: server (feature `server`) sau hidratare în browser (feature `web`)
  src/server.rs            pornire axum + Dioxus SSR, /healthz, montare API, job de sync
  src/state.rs             starea procesului (config, bază), setată o dată la pornire
  src/query.rs             SearchParams: starea căutării în query string, parsată și din formularul clasic
  src/ui/                  rute și pagini `/`, `/sedinte`, `/sedinte/{id}`, componente
  src/server_fns.rs        funcții #[server] apelate de UI
  src/api/                 /api/v1 (utoipa-axum), OpenAPI la /api/v1/openapi.json, Scalar la /api/docs
  src/auth.rs              Caller (identitate în v1) și RateKey, cheia limitării de rată
  src/jobs.rs              sincronizare periodică, secvențială
  assets/main.css          stilurile, incluse prin asset!()

deploy/                    setup.sh (provizionare), build.sh + cross/ (cross-compilare cu zig), deploy.sh, deploy-env.sh, urban.service, urban.nginx, urban.caddy
.github/workflows/ci.yml   fmt, clippy, test, check wasm și server
```

Regula de dependență: `app → core → shared`. Nimic din `core` nu ajunge în wasm.
