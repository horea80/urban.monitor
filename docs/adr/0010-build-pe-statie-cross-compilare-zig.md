---
status: accepted
date: 2026-09-21
decision-makers: horea
supersedes: ADR-0008
---

# ADR-0010: Build pe stație, serverul cross-compilat pentru aarch64 cu zig; box-ul primește doar binarul

## Context și problemă

[ADR-0008](0008-deploy-fara-docker-systemd-caddy.md) a pus build-ul de producție pe server, fiindcă box-ul
e aarch64 și stația e Windows x86-64, iar cross-compilarea părea complicată (`dx bundle` nu știe de un
linker străin). Primul deploy pe box-ul împrumutat ([ADR-0009](0009-gazduire-box-imprumutat-nginx.md),
2 OCPU) a durat ~8 minute, din care ultimele ~2 doar link-ul cu LTO al binarului, în timp ce stația are
32 de nuclee. Fiecare deploy ar fi încărcat box-ul prietenului și ar fi cerut un toolchain Rust ținut la
zi acolo, plus sursa proiectului.

## Factori de decizie

- durata unui deploy și încărcarea box-ului împrumutat
- box-ul cât mai gol: fără toolchain, fără sursă
- același artefact ca până acum (`server` + `public/`), aceeași unitate și aceeași instalare
- unelte puține și stabile pe stație

## Opțiuni considerate

1. Build pe box (ADR-0008)
2. Build pe VPS-ul propriu (4 OCPU, aceeași distribuție), binarul copiat pe box
3. Cross-compilare pe stație: `dx` pentru client, `cargo zigbuild` pentru server
4. Cross-compilare pe stație, totul prin `dx bundle`, cu serverul țintit la `aarch64-unknown-linux-gnu`
   și zig drept linker prin variabile de mediu

## Decizie

Opțiunea 4, în `deploy/build.sh`. O singură comandă,
`dx bundle --release -p urban-app --platform web @server --platform server --features server --target aarch64-unknown-linux-gnu`,
produce clientul wasm, `public/` și serverul ELF aarch64. Toolchain-ul C vine din zig, prin shim-ul
`cargo-zigbuild zig cc`, care adaptează argumentele rustc pentru zig (scoate, de exemplu,
`-Wl,--fix-cortex-a53-843419`, pe care zig nu îl acceptă): `build.sh` exportă
`CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER`, `CC_*`, `CXX_*`, `AR_*`, `RANLIB_*` către wrapper-ele din
`deploy/cross/`, ținute în repo (nu depindem de fișierele `.bat` pe care cargo-zigbuild le generează
într-un director de cache cu nume hash-uit). Singurele dependențe cu cod C sunt SQLite (bundled) și
`ring`; TLS e rustls.

De ce nu opțiunea 3, deși `cargo zigbuild` singur produce un binar care pornește: `asset!()` (manganis)
lasă în binar un placeholder pe care **dx îl înlocuiește după link** cu calea hash-uită a asset-ului. Un
server construit cu cargo direct răspunde la `/healthz`, dar servește
`<link href="This should be replaced by dx …">` și pagina rămâne fără stiluri. S-a întâmplat la primul
deploy cross-compilat; serverul trebuie construit de dx.

Detaliile de care depinde soluția, toate verificate pe dx 0.7.10:

- `@server` primește explicit `--platform server --features server`, nu doar `--target`: doar cu
  `--target`, dx construiește pentru aarch64 un binar cu feature-urile clientului, care moare la pornire
  în js-sys („cannot call wasm-bindgen imported functions on non-wasm targets”).
- dx se pune pe sine drept linker și cheamă linkerul „adevărat” de două ori: o dată cu argumente
  explicite, o dată printr-un fișier de răspuns numit `link_args.json`, care nu e JSON, ci un argument pe
  linie, cu escapare GNU: exact formatul fișierelor de răspuns ale rustc. Shim-ul cargo-zigbuild
  expandează doar fișiere numite `*linker-arguments` (convenția rustc), altfel îl dă neatins lui zig, care
  se împiedică de argumentele nefiltrate. `deploy/cross/ziglink.ps1` copiază fișierul lui dx sub un nume
  cu sufixul `linker-arguments` și îi dă shim-ului `@fișier`. Nu îl expandează în argumente explicite: la
  LTO thin linia de link are mii de fișiere obiect, mult peste limita liniei de comandă din Windows.
- dx nu raportează un link eșuat: la primul link cu LTO thin, wrapper-ul (atunci încă expandând
  argumentele) a depășit limita, link-ul a picat, iar dx a scris „Server build completed successfully”
  și a livrat binarul *anterior*, cu mtime proaspăt: același hash, textele vechi, smoke test trecut.
  De aceea `ziglink.ps1` scrie codul de ieșire al fiecărui link în `URBAN_LINK_MARKER`, iar `build.sh`
  oprește build-ul dacă vreun link n-a ieșit cu 0.
- profilurile de release sunt relaxate la build: dx impune `lto = true` prin `--config` pentru client și
  server (bate manifestul), iar cu fat LTO și o unitate de codegen fiecare crate final se compilează pe
  un singur fir. `build.sh` pune, prin `--cargo-args` (un `--config` mai târziu pe linie câștigă),
  `lto = false` și `codegen-units = 16` pe ambele. Măsurat la o schimbare în `crates/app`, 8 nuclee:
  wasm 22 s → 4 s, server 80 s → 11 s, build incremental cap-coadă 90 s → 19 s. Costul: wasm
  1,02 → 1,19 MB (servit comprimat), server 12,7 → 15,3 MB. LTO thin și 16 unități sub fat LTO au fost
  măsurate și respinse (56–118 s). Dacă dimensiunea wasm devine o problemă, override-ul de la `@client`
  se scoate și clientul revine la fat LTO + `opt-level = "z"`.
- dx construiește cele două jumătăți secvențial, nu în paralel, deci timpii se adună; și nu curăță
  `public/assets` de bundle-urile vechi, așa că `build.sh` șterge `public/` înainte de build.
- `CARGO_BUILD_JOBS` (implicit 8, `BUILD_JOBS=32 just build-cross`) limitează nucleele folosite, inclusiv
  de codegen-ul LLVM, prin jobserver; stația are 32 și trebuie să rămână utilizabilă în timpul build-ului.
- wrapper-ul pentru compilatorul C (`zigcc.bat`) rămâne un `.bat` simplu, ca să nu pornim PowerShell
  pentru fiecare fișier C din ring; `build.sh` afișează un rând la 15 s cât dx tace pe ultimul crate.

Pe box, `deploy.sh` face un smoke test înainte de instalare: binarul pornit pe un port temporar, cu
sincronizarea oprită, trebuie să răspundă la `/healthz`, iar pagina `/` nu trebuie să conțină textul
placeholder; apoi instalarea atomică de până acum, cu binarul vechi păstrat ca `server.prev`.

Măsurat pe stație (8 nuclee): build complet ~1,5 min, deploy incremental cap-coadă ~45 s, din care
build-ul ~19 s; un deploy fără schimbări de cod ~25 s. Pe box, ~8 min pentru un build complet. Binarul
e ELF aarch64 legat dinamic de glibc (zig țintește glibc 2.31 implicit, box-ul are 2.39).

Ce rămâne din ADR-0008: fără Docker, binar sub systemd cu întărire, `.env` pe server, reverse proxy cu
TLS (nginx pe box-ul împrumutat, ADR-0009; Caddy pe VPS-ul propriu), instalare atomică și `/healthz` la
final. Ce se schimbă: locul build-ului, iar `setup.sh` nu mai instalează rustup și `dx`.

### Consecințe

- Bune: deploy în secunde; box-ul nu compilează și nu ține sursa; artefact identic cu cel de până acum;
  smoke test-ul prinde și binarele fără asset-uri rezolvate, și binarele care nu pornesc.
- Rele: trei unelte în plus pe stație (target rustup `aarch64-unknown-linux-gnu`, zig, cargo-zigbuild);
  wrapper-ele din `deploy/cross/` sunt Windows-only (`.bat`/`.ps1`), deci `build.sh` rulează doar pe
  stație; dx construiește și un server Windows inutil (rapid, incremental); `ziglink.ps1` depinde de două
  detalii interne (numele fișierului de răspuns al dx, convenția shim-ului): dacă se schimbă, se schimbă
  acolo.

## Argumente pe opțiuni

### 2. Build pe VPS-ul propriu
- Bun: fără unelte noi; cam de două ori mai rapid decât box-ul; binar portabil (aceeași Ubuntu 24.04 aarch64).
- Rău: tot minute bune; toolchain de întreținut pe un box unde serviciul nu rulează.

### 3. `dx` pentru client + `cargo zigbuild` pentru server
- Bun: fără wrapper-e; cargo-zigbuild face totul singur; build-ul serverului 3m36s.
- Rău: asset-urile rămân nerezolvate (vezi Decizie). Respinsă după ce a ajuns o dată în producție fără stiluri.
