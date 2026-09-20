---
status: accepted
date: 2026-09-20
decision-makers: horea
---

# ADR-0001: Rust în loc de Python

## Context și problemă

Proiectul a pornit ca un scraper Python cu FastAPI și SQLite; primele module erau scrise.
Proprietarul a cerut oprirea și reluarea în Rust, cu dorința explicită de a încerca ceva nou
și de a livra serviciul ca un singur binar.

## Factori de decizie

- livrare simplă pe VPS: un binar, fără runtime și fără venv
- dorința de a învăța un stack nou pe un proiect real, dar mic
- posibilitatea de a folosi același limbaj pe server și în browser (vezi ADR-0002)
- costul: timp de compilare, curbă de învățare, toolchain pe Windows

## Opțiuni considerate

1. Continuarea în Python (FastAPI, requests, pypdf, SQLite)
2. Rust

## Decizie

Rust, edition 2024, workspace cu trei crate-uri. Codul Python existent a fost șters.

### Consecințe

- Bune: un binar static, memorie mică, tipare puternică pe parsere; drum deschis către
  full-stack Rust.
- Rele: toolchain de instalat pe Windows (rustup + Visual Studio Build Tools); timpi de
  compilare mai mari; ecosistemul de extragere text din PDF e mai slab decât în Python
  (vezi ADR-0007).

## Argumente pe opțiuni

### Python
- Bun: `pypdf` a extras corect textul cu diacritice din ordinea de zi; dezvoltare rapidă.
- Rău: venv și dependențe pe server; nu răspunde dorinței de a încerca ceva nou.

### Rust
- Bun: cele de la Decizie.
- Rău: cele de la Consecințe.
