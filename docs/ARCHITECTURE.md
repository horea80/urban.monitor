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
  src/ui/                  pagini `/` și `/sedinte/{id}`
  src/server_fns.rs        funcții #[server]
  src/api/                 /api/v1, extractor Caller, OpenAPI (utoipa)
  src/auth.rs              strat de autentificare (identitate în v1)
  src/jobs.rs              sync periodic
  src/main.rs              pornire axum + Dioxus

deploy/                    Dockerfile, Caddyfile, docker-compose.yml
.github/workflows/ci.yml   fmt, clippy, test, check wasm și server
```

Regula de dependență: `app → core → shared`. Nimic din `core` nu ajunge în wasm.
