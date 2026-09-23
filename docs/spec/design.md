# Design — urban.monitor

| | |
|---|---|
| Status | draft, v1 |
| Data | 2026-09-20 |
| Șablon | arc42 redus |
| Cerințe | [requirements.md](requirements.md) · Decizii: [../adr/](../adr/README.md) |

## 1. Introducere și obiective

Vezi [requirements.md](requirements.md). Obiectivele de calitate, în ordine:
corectitudinea datelor colectate, politețea față de sursă, simplitatea operării,
pregătirea pentru autentificare fără rescriere.

## 2. Constrângeri

- Rust, un singur binar server, Dioxus 0.7 pentru UI ([ADR-0001](../adr/0001-limbaj-rust.md), [ADR-0002](../adr/0002-dioxus-fullstack.md)).
- SQLite ca singur depozit ([ADR-0003](../adr/0003-sqlite-rusqlite-fts5.md)).
- Sursa nu oferă API sau RSS; singura cale e HTML plus PDF, cu structură WordPress/Avada.
- Găzduire pe VPS propriu, în spatele Caddy ([ADR-0005](../adr/0005-deploy-vps-caddy-docker.md)).
- Dezvoltare pe Windows x86-64, livrare pe Linux aarch64 (Oracle ARM): build-ul de producție se face
  pe server ([ADR-0008](../adr/0008-deploy-fara-docker-systemd-caddy.md)). Fără Docker.

## 3. Context

```
 utilizator ──browser──▶ Caddy (TLS) ──▶ urban-app (axum + Dioxus SSR)
                                              │            │
                                              │            └──▶ SQLite  data/urban.db
                                              │                 data/raw/  snapshot-uri
                                              └──sync, 1 req/s──▶ primariaclujnapoca.ro
                                                                  files.primariaclujnapoca.ro
 programator ──HTTP JSON──▶ /api/v1  (OpenAPI la /api/v1/openapi.json)
 operator ──CLI `urban`──▶ aceeași bibliotecă `urban-core`, aceeași bază
```

## 4. Strategia soluției

- Toată logica de domeniu într-o bibliotecă server-only, `urban-core`, testabilă fără rețea
  pe fixture-uri. UI-ul și API-ul sunt straturi subțiri peste ea.
- Tipurile de domeniu și normalizarea de text într-un crate separat, `urban-shared`, care
  compilează și la wasm, astfel încât browserul și serverul folosesc aceleași definiții.
- Randare pe server cu hidratare; fără wasm, aplicația degradează la formulare clasice.
- Sincronizare incrementală, idempotentă, o tranzacție per ședință.

## 5. Blocuri

```
crates/shared        urban-shared      model.rs   Meeting, Item, Document, AgendaRow, Category, tipuri API
                                       text.rs    normalize(), classify(), parse_ro_date(), street_of()
crates/core          urban-core        scrape/client.rs    reqwest, rate limit, retry, snapshot brut
                                       scrape/listing.rs   lista ședințelor
                                       scrape/meeting.rs   antet + carduri
                                       scrape/project.rs   documente
                                       pdf.rs              trait PdfText; PdfExtract, Pdftotext
                                       agenda.rs           text → rânduri; potrivire rânduri ↔ carduri
                                       db/                 migrații, upsert, căutare FTS5, sync_runs
                                       sync.rs             orchestrare
                                       bin/urban.rs        CLI
crates/app           urban-app         main.rs             server (feature `server`) sau hidratare (feature `web`)
                                       server.rs           axum + Dioxus SSR, /healthz, API montat, job de sync, config din env
                                       state.rs            config + bază, OnceLock setat la pornire
                                       query.rs            SearchParams ↔ query string; formular clasic și linkuri
                                       ui/                 rute `/`, `/sedinte`, `/sedinte/{id}`, componente
                                       server_fns.rs       funcții #[server] apelate de UI
                                       api/                rute /api/v1 (utoipa-axum), extractor Caller, OpenAPI, Scalar
                                       auth.rs             Caller (identitate în v1) și RateKey; loc pentru chei API
                                       jobs.rs             task periodic de sync, strict secvențial
```

Regula de dependență: `app → core → shared`; `shared` nu depinde de nimic din proiect.

## 6. Runtime

### 6.1 Sincronizare

1. GET lista → filtrare an ≥ `URBAN_START_YEAR`.
2. Pentru fiecare ședință selectată (nouă, sau ≤ `URBAN_REFRESH_DAYS`, sau `--full`):
   1. GET pagina ședinței → antet, link ordine de zi, carduri.
   2. GET PDF ordine de zi → `PdfText` → `agenda::parse` → rânduri.
   3. `agenda::match_rows` leagă rândurile de carduri după tokeni comuni; tokenii din adresă
      cântăresc mai mult; potrivire unu-la-unu, prag minim.
   4. Pentru cardurile noi: GET pagina proiectului → documente.
   5. Pentru cardul „Concluziile ședinței”: GET pagina → link PDF.
   6. O tranzacție: upsert ședință, proiecte, documente, rânduri; reindexare FTS.
3. Scriere `sync_runs`. Erorile per ședință se loghează și nu opresc bucla.

Ordinea cererilor e serializată printr-un singur client cu pauză minimă între cereri.

### 6.2 Căutare

- Browser cu wasm: componenta apelează funcția `#[server] search(...)`, care rulează
  interogarea FTS5 în `spawn_blocking` și întoarce `Vec<ItemView>`.
- Fără wasm: formularul face `GET /?q=...&tip=...&an=...`; serverul randează pagina completă
  cu aceleași rezultate.
- API: `GET /api/v1/search` folosește aceeași funcție din `core`.

Interogarea: textul e normalizat (fără diacritice, litere mici, doar `[a-z0-9 ]`), fiecare
token devine `token*`, tokenii sunt legați cu AND; filtrele pe categorie și an sunt clauze SQL.

## 7. Deploy

Fără Docker ([ADR-0008](../adr/0008-deploy-fara-docker-systemd-caddy.md)). `deploy/deploy.sh` trimite
sursa pe VPS prin `tar | ssh`, rulează acolo `dx bundle --release --platform web` (box-ul e aarch64) și
instalează atomic `server` + `public/` în `/opt/urban`, sub unitatea systemd `urban` (utilizator
dedicat, `ProtectSystem=strict`, `ReadWritePaths=/opt/urban/data`, `PrivateTmp`). Configurația vine din
`/opt/urban/.env`. Caddy, instalat din apt, face TLS, compresie și headerele de securitate, cu un fișier
de site per proiect în `/etc/caddy/sites/`. Provizionarea inițială: `deploy/setup.sh`.

## 8. Concepte transversale

- **Normalizare text**: `unicode-normalization` NFKD + eliminarea semnelor combinate, plus
  tabel explicit pentru ș/ț cu virgulă și cu sedilă. Aceeași funcție indexează și caută.
- **Erori**: `thiserror` în `core` pentru tipuri, `anyhow` în binare. Erorile de parsare
  poartă URL-ul sursă.
- **Logging**: `tracing`, filtru prin `RUST_LOG`.
- **Configurare**: variabile de mediu cu valori implicite, citite o singură dată la pornire.
- **Autentificare (pregătită)**: `auth::layer()` pe `/api/v1`, extractor `Caller`
  (`Anonymous` în v1), `bearerAuth` declarat în OpenAPI, limitare de rată cheiată într-un
  singur loc. Tabelul `api_keys` rezervat ca migrație viitoare. Vezi [ADR-0004](../adr/0004-api-public-openapi-auth-ulterior.md).
- **Idempotență**: chei unice pe URL-uri; re-rularea unui sync nu duplică nimic.

## 9. Decizii

Registrul de decizii este [docs/adr/](../adr/README.md), în format MADR.

## 10. Riscuri și datorie tehnică

| Risc | Impact | Măsură |
|---|---|---|
| `pdf-extract` extrage prost textul | fără beneficiar și nr. înregistrare | spike pe fixture-uri; fallback `pdftotext`; v1 fără beneficiar în ultimă instanță ([ADR-0007](../adr/0007-pdf-extract-cu-fallback.md)) |
| Structura site-ului se schimbă | sync fără carduri | teste pe fixture-uri; avertisment la 0 carduri; snapshot brut pentru re-parsare |
| Dioxus 0.7 → 0.8 incompatibil | efort la upgrade | pin pe 0.7.x; UI subțire, logica în `core` |
| Concluziile sunt scanate | nu știm verdictul | v2: OCR |
| Potrivirea rânduri ↔ carduri greșește | beneficiar atribuit greșit | prag conservator; rândurile nepotrivite rămân vizibile separat |

## 11. Anexă A — structura sursei (observată 2026-09-20)

- **Lista**: o pagină, fără paginare; linkuri „Ședința din 16 septembrie 2026” →
  `/urbanism/sedinte-comisie/sedinta-din-16-septembrie-2026/`. 20 ședințe în 2026 până în septembrie.
- **Ședința**: `h1.entry-title` (data), `h3` („Orele: 10.00”), `a.sedinta-urbanism-content`
  (PDF ordine de zi), `article.proiect_urbanism` per proiect cu `h2.entry-title a` (titlu,
  link), `div.fusion-post-content-container p` (adresa), `span.updated` (data publicării).
  Carduri speciale: „Concluziile ședinței”, „ANUNȚ PRIVIND DESFĂȘURAREA ȘEDINȚEI”.
- **Proiectul**: `article .post-content a[href]` către `files.primariaclujnapoca.ro` (parte
  scrisă, parte desenată, uneori adresa).
- **Ordinea de zi**: PDF cu text în 17 din 20 de ședințe din 2026; 3 sunt scanate (19 ian., 15 apr.,
  24 iun.) și rămân fără rânduri, doar cu cardurile (fără OCR în v1, decizie 2026-09-21). PDF-ul apare
  cu 3–8 zile înaintea ședinței; cardurile încep să apară în aceeași zi și se completează până în ziua
  ședinței, deci ordinea de zi e semnalul cel mai timpuriu și complet. Rând = număr de ordine, `NNNNNN/ZZ.LL.AAAA`, beneficiar,
  descriere cu amplasament; uneori „revenire CTATU”. Titluri inconsistente: P.U.Z / PUZ,
  P.U.D / PUD; „Studiu de oportunitate pentru inițiere PUZ” = aviz de oportunitate.
  `pdf-extract` lipește uneori tokenii vecini („10739800/24.08.2026Marian Ramona”): numărul
  de înregistrare are 6 cifre, iar numărul de ordine se validează ca precedentul plus unu.
- **Concluziile**: PDF scanat, fără text.

Fixture-uri: `crates/core/tests/fixtures/`.

## 12. Anexă B — model de date

```sql
meetings(id, url UNIQUE, title, date, time, agenda_url, agenda_text,
         conclusions_pdf_url, announcement_url, first_seen_at, last_seen_at, alerted_at)  -- FR-8
items(id, meeting_id → meetings, url UNIQUE, title, address, street, category,
      published_at, beneficiary, reg_number, reg_date, revenire, first_seen_at, last_seen_at)
documents(id, item_id → items, label, url, UNIQUE(item_id, url))
agenda_rows(id, meeting_id → meetings, nr, reg_number, reg_date, beneficiary,
            description, revenire, item_id → items NULL)
items_fts  -- FTS5, content=items: title, address, street, beneficiary, agenda_description (normalizate)
sync_runs(id, started_at, finished_at, ok, meetings_seen, items_new, error)
-- conturi (FR-9, ADR-0012); jetoanele doar ca hash SHA-256
users(id, email UNIQUE, plan, credits_available, credits_used, cycle_start_at, cycle_end_at,
      alerts_checked_until, consent_at, created_at, last_login_at)
sessions(token_hash PK, user_id → users CASCADE, expires_at, created_at)
magic_links(id, email, token_hash UNIQUE, consent, expires_at, used_at, created_at)
keywords(id, user_id → users CASCADE, text, normalized, created_at, UNIQUE(user_id, normalized))
credit_ledger(id, user_id → users CASCADE, direction, amount >= 0, reason, idempotency_key UNIQUE,
              balance_after, created_at)
alert_log(id, user_id → users CASCADE, kind, items, window_until, sent_at)
-- rezervat, nu în v1:
-- api_keys(id, key_hash, label, plan, created_at, revoked_at)
```
