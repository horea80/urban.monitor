---
status: superseded by ADR-0010
date: 2026-09-21
decision-makers: horea
supersedes: ADR-0005
---

# ADR-0008: Deploy fără Docker: binar sub systemd, Caddy, build pe serverul ARM

## Context și problemă

[ADR-0005](0005-deploy-vps-caddy-docker.md) a ales Docker Compose comparând doar cu Fly.io și cu
un PaaS Rust, fără să se uite la cum sunt livrate proiectele vecine ale proprietarului
(oracle.gsl): binar sub systemd, Caddy instalat din apt, scripturi bash peste ssh, configurație
într-un `.env` pe server. Proprietarul nu vrea Docker.

VPS-ul e același box Oracle Cloud ARM Ampere (aarch64, Ubuntu) pe care rulează și celelalte
proiecte. Un binar Rust compilat pe stația Windows x86-64 nu rulează acolo, spre deosebire de
JavaScript, deci trebuie decis și unde se compilează.

## Factori de decizie

- același model operațional ca celelalte proiecte de pe box (systemd, journal, Caddy multi-site)
- fără Docker
- box-ul e aarch64; stația de dezvoltare e Windows x86-64
- deploy repetabil dintr-o comandă, cu verificare la final

## Opțiuni considerate

1. Docker Compose pe VPS (ADR-0005)
2. Binar + systemd + Caddy, cu build pe server
3. Binar + systemd + Caddy, cu cross-compilare locală (`cargo-zigbuild` sau `cross`)
4. Binar construit în GitHub Actions pe un runner ARM și descărcat de scriptul de deploy

## Decizie

Opțiunea 2.

- `deploy/setup.sh`, rulat o dată pe server: unelte de build, rustup cu target wasm, `dx` (aceeași
  versiune ca pe stație, prin `cargo binstall`), Caddy dacă lipsește, utilizatorul de sistem
  `urban`, `/opt/urban/data`, unitatea `urban.service`, fișierul de site `/etc/caddy/sites/urban.caddy`.
  Nu suprascrie Caddyfile-ul principal, doar se asigură că importă `/etc/caddy/sites/*`.
- `deploy/deploy.sh`, de pe stație (Git Bash): sursa pleacă prin `tar | ssh` în
  `~/build/urban.monitor` (fără `target/`, `data/`, `.git`), pe server rulează
  `dx bundle --release --platform web`, apoi `server` și `public/` se instalează atomic în
  `/opt/urban` (`*.new` → mutare), unitatea se repornește și scriptul verifică `/healthz`.
- `deploy/deploy-env.sh`: `.env.prod` (ignorat de git; `.env.example` e șablonul) → `/opt/urban/.env`.
- Unitatea rulează ca `urban`, cu `ProtectSystem=strict`, `ReadWritePaths=/opt/urban/data`,
  `PrivateTmp=true` (SQLite are nevoie de un director temporar), loguri în journal.
- Caddy: TLS, compresie, headere de securitate, `reverse_proxy 127.0.0.1:8081`. Dioxus injectează
  scripturi inline pentru hidratare, deci CSP-ul permite `'unsafe-inline'` la `script-src`;
  restul rămâne strict.

### Consecințe

- Bune: fără Docker; un singur model de operare pentru toate proiectele de pe box; backup =
  copia fișierului SQLite din `/opt/urban/data`; un deploy = o comandă.
- Rele: build-ul pe server durează minute și încarcă CPU-ul și memoria VPS-ului; toolchain-ul
  Rust de pe server trebuie ținut în pas cu `rust-toolchain.toml` și cu versiunea `dx` de pe stație.
- Dacă build-ul pe server devine o problemă, opțiunea 3 sau 4 se poate adopta fără să schimbe
  nimic din instalare și din unitate: doar pasul de build din `deploy.sh`.

## Argumente pe opțiuni

### 3. Cross-compilare locală
- Bun: VPS-ul nu compilează nimic.
- Rău: `dx bundle` nu știe de `zigbuild`; ar fi două comenzi separate (cargo pentru server cu
  linker zig, dx doar pentru client) și un layout de ieșire asamblat manual. `cross` înseamnă Docker.

### 4. GitHub Actions
- Bun: build reproductibil, artefact versionat.
- Rău: depinde de vizibilitatea repo-ului pentru runnere ARM gratuite și adaugă un pas extern
  la un deploy care altfel e local.

### 1. Docker
- Rău: proprietarul nu îl vrea; nu aduce nimic în plus față de systemd pentru un singur binar.
