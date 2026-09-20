# Registrul deciziilor de arhitectură (ADR)

Format: [MADR](https://adr.github.io/madr/), un fișier per decizie, numerotat crescător.
O decizie acceptată nu se rescrie; dacă se schimbă, se adaugă un ADR nou care o înlocuiește
și cel vechi primește status `superseded by ADR-XXXX`.

Statusuri: `proposed` · `accepted` · `rejected` · `deprecated` · `superseded`.

| Nr. | Titlu | Status | Data |
|---|---|---|---|
| [0001](0001-limbaj-rust.md) | Rust în loc de Python | accepted | 2026-09-20 |
| [0002](0002-dioxus-fullstack.md) | Dioxus fullstack pentru UI, nu Leptos și nu htmx | accepted | 2026-09-20 |
| [0003](0003-sqlite-rusqlite-fts5.md) | SQLite prin rusqlite, căutare FTS5 pe text normalizat | accepted | 2026-09-20 |
| [0004](0004-api-public-openapi-auth-ulterior.md) | API public read-only cu OpenAPI; autentificare pregătită, neactivată | accepted | 2026-09-20 |
| [0005](0005-deploy-vps-caddy-docker.md) | Deploy pe VPS propriu, Docker, Caddy | superseded by 0008 | 2026-09-20 |
| [0006](0006-fara-rss-in-v1.md) | Fără RSS în v1 | accepted | 2026-09-20 |
| [0007](0007-pdf-extract-cu-fallback.md) | Extragere text PDF cu `pdf-extract` și fallback `pdftotext` | accepted | 2026-09-20 |
| [0008](0008-deploy-fara-docker-systemd-caddy.md) | Deploy fără Docker: binar sub systemd, Caddy, build pe serverul ARM | accepted | 2026-09-21 |

## Cum adaugi o decizie

1. Copiază `template.md` în `NNNN-titlu-scurt.md`.
2. Completează contextul, opțiunile considerate și consecințele. Scrie de ce, nu doar ce.
3. Adaugă rândul în tabelul de mai sus.
