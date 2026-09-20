---
status: accepted
date: 2026-09-20
decision-makers: horea
---

# ADR-0003: SQLite prin rusqlite, căutare FTS5 pe text normalizat

## Context și problemă

Volumul de date e mic: aproximativ 20 de ședințe și 250 de proiecte pe an. Avem un
singur proces care scrie (sincronizarea) și mulți care citesc (căutări). Căutarea trebuie
să ignore diacriticele și să potrivească pe început de cuvânt.

## Factori de decizie

- zero administrare pe VPS
- backup trivial
- căutare cu prefix și ranking
- acces sigur din cod async (axum) fără complicații

## Opțiuni considerate

1. SQLite prin `rusqlite` (sincron, bundled) + pool `r2d2`
2. SQLite prin `sqlx` (async)
3. PostgreSQL

Pentru căutare:

1. `LIKE` pe o coloană normalizată
2. FTS5 cu tokenizer `unicode61 remove_diacritics`
3. FTS5 pe text normalizat de noi în Rust

## Decizie

`rusqlite` cu feature `bundled`, pool `r2d2_sqlite`, WAL activat, migrații cu
`rusqlite_migration`, apeluri din `tokio::task::spawn_blocking`. Căutare cu FTS5 peste
text normalizat în Rust (fără diacritice, litere mici), interogare `token*` legată cu AND.

### Consecințe

- Bune: un fișier, backup prin copiere, FTS5 dă prefix și ranking bm25 gratis;
  normalizarea într-un singur loc, aceeași la indexare și la căutare.
- Rele: `bundled` compilează SQLite din C, deci cere un compilator C (Build Tools pe Windows);
  apelurile sincrone trebuie ținute în `spawn_blocking`.

## Argumente pe opțiuni

### rusqlite
- Bun: API simplu, matur, SQLite compilat cu FTS5; nu cere `DATABASE_URL` la compilare.
- Rău: sincron.

### sqlx
- Bun: async nativ, verificare a interogărilor la compilare.
- Rău: compilare mai grea, mod offline de întreținut; avantajul async e irelevant la acest volum.

### PostgreSQL
- Rău: un serviciu în plus de operat pentru câteva mii de rânduri.

### FTS5 cu remove_diacritics din SQLite
- Rău: acoperirea pentru ș și ț cu virgulă depinde de versiunea SQLite; preferăm control
  explicit în Rust.
