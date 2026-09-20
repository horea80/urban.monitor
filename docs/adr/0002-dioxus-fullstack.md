---
status: accepted
date: 2026-09-20
decision-makers: horea
---

# ADR-0002: Dioxus fullstack pentru UI, nu Leptos și nu htmx

## Context și problemă

Serviciul trebuie expus pe web cu o interfață de căutare. Proprietarul a cerut o
experiență „ca TypeScript, același limbaj pe client și pe server”. În Rust asta înseamnă
un framework care compilează la WebAssembly pentru browser și rulează nativ pe server.

## Factori de decizie

- cod și tipuri partajate între server și browser
- sănătatea pe termen lung a proiectului ales (întreținere, echipă, ritm de release)
- SSR, ca pagina să fie utilizabilă înainte de încărcarea wasm-ului și fără wasm
- integrare cu axum, pe care îl folosim pentru API
- costul de compilare și de învățare

## Opțiuni considerate

1. Leptos + axum
2. Dioxus fullstack
3. axum + askama + htmx (fără wasm)

## Decizie

Dioxus 0.7 fullstack, SSR + hidratare, pe axum 0.8. Formularul de căutare funcționează și
fără wasm prin GET clasic randat pe server.

### Consecințe

- Bune: același model de date în browser și pe server; server functions în loc de API
  intern; ecosistem cu echipă finanțată și release-uri regulate.
- Rele: bundle wasm de câteva sute de KB, relevant pe mobil, compensat de SSR;
  toolchain suplimentar (`dx`, target wasm); schimbări incompatibile posibile la 0.8.

## Argumente pe opțiuni

### Leptos
- Bun: cel mai apropiat de modelul „TS pe ambele părți”; integrare nativă cu axum;
  activitate pe 90 de zile comparabilă cu Dioxus (44 vs 35 PR-uri merge-uite).
- Rău: în mai 2026 creatorul a anunțat că proiectul va fi „întreținut lejer”; 55% din
  commit-urile recente sunt ale lui; risc de bus factor 1.

### Dioxus
- Bun: companie finanțată (Y Combinator), echipă de cel puțin trei oameni de bază, 311 PR-uri
  merge-uite în 12 luni, 0.7 stabil cu patch-uri lunare; SSR, server functions, `dx serve`
  cu hot reload; rulează pe axum.
- Rău: backlog mare (650 issue-uri deschise); breaking changes între versiuni minore.

### htmx
- Bun: cel mai rapid drum la rezultat; fără wasm; fără dependență de un framework de frontend.
- Rău: nu e full-stack Rust; interacțiuni bogate viitoare (hartă, filtrare instant) ar cere
  JavaScript separat.

## Informații suplimentare

- Status update Leptos, mai 2026: https://github.com/leptos-rs/leptos/issues/4707
- Măsurători GitHub API din 2026-09-20 (fereastră 90 zile / 12 luni): Leptos 44 / 234
  PR-uri merge-uite, 15 autori, 74 issue-uri deschise; Dioxus 35 / 311 PR-uri, 17 autori,
  650 issue-uri deschise.
- Suport WebAssembly: toate browserele majore din 2017, ~96% din trafic (caniuse).
