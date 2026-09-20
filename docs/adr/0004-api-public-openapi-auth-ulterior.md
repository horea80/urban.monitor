---
status: accepted
date: 2026-09-20
decision-makers: horea
---

# ADR-0004: API public read-only cu OpenAPI; autentificare și cote pregătite, neactivate

## Context și problemă

Proprietarul vrea un API JSON conform OpenAPI pe lângă UI. La început totul e public, dar
a cerut explicit ca autentificarea și limitele de utilizare să fie posibile mai târziu fără
rescriere.

## Factori de decizie

- utilitate pentru terți fără scraping suplimentar al site-ului primăriei
- spec generat din cod, nu întreținut manual
- costul adăugării ulterioare de chei API și cote să fie local, nu în fiecare handler

## Opțiuni considerate

1. Doar server functions Dioxus, fără API public
2. API public read-only, OpenAPI generat cu `utoipa`, fără autentificare, dar cu punctele de
   extensie montate de la început
3. Autentificare de la început

## Decizie

Opțiunea 2. Rute sub `/api/v1`, spec la `/api/v1/openapi.json`, explorare la `/api/docs`
(Scalar). Puncte de extensie obligatorii din v1:

- strat `auth::layer()` montat pe `/api/v1`, identitate în v1, produce `Caller::Anonymous`
- fiecare handler primește extractorul `Caller`
- limitare de rată cu `tower-governor`, cheiată într-un singur loc (IP acum, `Caller` apoi)
- schema de securitate `bearerAuth` declarată în OpenAPI, cerută pe nicio rută
- tabelul `api_keys` rezervat ca migrația următoare, necreat în v1
- nicio rută publică de declanșare a sincronizării

### Consecințe

- Bune: activarea autentificării înseamnă o implementare a stratului, o migrație și
  adnotarea rutelor în spec; handlerele nu se ating.
- Rele: puțin cod „gol” în v1 (`Caller` mereu anonim).

## Argumente pe opțiuni

### Fără API public
- Rău: contrazice scopul de „serviciu”; terții ar face scraping la rândul lor.

### Autentificare de la început
- Rău: frecare pentru utilizatori pe date oricum publice; efort înainte de a avea utilizatori.
