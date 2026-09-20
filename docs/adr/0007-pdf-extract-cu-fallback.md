---
status: accepted
date: 2026-09-20
decision-makers: horea
---

# ADR-0007: Extragere text PDF cu `pdf-extract` și fallback `pdftotext`

## Context și problemă

Ordinea de zi e un PDF cu text. Din el luăm beneficiarul, numărul de înregistrare și
marcajul „revenire CTATU”, plus rândurile care nu au card publicat. În Python, `pypdf` a
extras textul corect, cu diacritice. `pdftotext` (poppler) pierde diacriticele pe aceste
PDF-uri. Calitatea crate-ului pur Rust `pdf-extract` pe aceste fișiere nu e încă verificată.

## Factori de decizie

- corectitudinea textului, inclusiv diacritice
- fără dependențe de sistem la runtime, dacă se poate
- izolarea riscului: restul sistemului nu trebuie să depindă de PDF

## Opțiuni considerate

1. `pdf-extract` (pur Rust)
2. `pdftotext` ca proces extern (poppler instalat în imagine)
3. `pdfium-render` (binding la PDFium, bibliotecă nativă)
4. v1 fără agenda din PDF: doar cardurile HTML

## Decizie

Trait `PdfText` în `core`, cu implementarea implicită `pdf-extract` și fallback automat la
`pdftotext` dacă e prezent și dacă rezultatul primei implementări e gol sau suspect.
Datele din PDF sunt îmbogățire: absența lor nu blochează sincronizarea.

Acceptată pe 2026-09-20 după spike-ul pe cele trei PDF-uri din `crates/core/tests/fixtures/`
(`pdf-extract` 0.12.1). Rezultate:

| Fișier | Rânduri așteptate | Rânduri în text | Diacritice | Durată |
|---|---|---|---|---|
| agenda-2026-09-16.pdf | 13 | 13 | corecte | 12 ms |
| agenda-2026-01-14.pdf | 17 | 17 | corecte | 15 ms |
| agenda-2026-05-27.pdf | 10 | 10 | corecte | 10 ms |
| conclusions-2026-09-16-scanned.pdf | 0 (scanat) | 0 caractere, fără eroare | — | 0,5 ms |

Observație care influențează parserul de agendă: textul extras lipește uneori tokenii vecini
(„10739800/24.08.2026Marian Ramona Laura”): numărul de ordine de numărul de înregistrare și
data de beneficiar. Regula de despărțire: numărul de înregistrare are 6 cifre; când sunt mai
multe splituri posibile, se alege cel în care numărul de ordine e precedentul plus unu.

### Consecințe

- Bune: fără dependență nativă în cazul bun; degradare controlată.
- Rele: două implementări de întreținut dacă fallback-ul e necesar.

## Informații suplimentare

Criteriu de acceptare al spike-ului: pentru `agenda-2026-09-16.pdf`, `agenda-2026-01-14.pdf`
și `agenda-2026-05-27.pdf`, parserul de agendă trebuie să găsească toate rândurile (13, 17 și
10) cu numerele de înregistrare corecte și cu adresele lizibile.
