---
status: accepted
date: 2026-09-20
decision-makers: horea
---

# ADR-0005: Deploy pe VPS propriu, Docker, Caddy

## Context și problemă

Serviciul trebuie expus public pe HTTPS, cu stare persistentă (fișier SQLite) și un task
periodic de sincronizare în același proces.

## Opțiuni considerate

1. VPS propriu, Docker Compose, Caddy ca reverse proxy cu TLS automat
2. Fly.io cu volum persistent
3. Shuttle sau alt PaaS Rust

## Decizie

VPS propriu. Imagine Docker în două etape (builder cu `dx bundle`, runtime `debian-slim`),
volum pentru `/data`, Caddy pentru TLS, compresie și headere de securitate, inclusiv CSP cu
`'wasm-unsafe-eval'`.

### Consecințe

- Bune: control total, cost fix mic, un singur `docker compose up -d`; Caddy elimină
  administrarea certificatelor.
- Rele: administrarea serverului (actualizări, backup al volumului) cade pe proprietar.

## Argumente pe opțiuni

### Fly.io
- Bun: fără administrare de server, volume persistente.
- Rău: cost variabil, model de oprire a mașinilor inactive incompatibil cu un timer intern
  fără configurare suplimentară.

### PaaS Rust
- Rău: constrângeri pe stocarea locală și pe procesele de fundal.
