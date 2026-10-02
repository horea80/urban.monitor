---
status: accepted
date: 2026-10-02
decision-makers: horea
---

# ADR-0013: Certificatele de urbanism, a doua sursă, citite din lista HTML, fără pagini de detaliu

> Completat în aceeași zi de [ADR-0014](0014-pagina-certificatului-doar-pentru-puz-si-pud.md): pagina
> certificatului se descarcă totuși, dar doar pentru PUZ și PUD și doar pentru câteva câmpuri.

## Context și problemă

Ordinea de zi CTATU anunță ce se discută în comisie, dar certificatul de urbanism vine și mai devreme:
orice construire, PUZ sau PUD începe cu un certificat, iar primăria publică fiecare certificat emis, cu
scopul și adresa lucrării, în ziua emiterii. Vrem ca cineva care urmărește o stradă să afle și de
certificatele emise acolo, nu doar de proiectele ajunse în comisie, și vrem istoricul din 2024 încoace.

Ce am măsurat pe 2026-10-02:

- lista HTML are 24 de certificate pe pagină, paginată prin `?sf_paged=N`; prima pagină răspunde repede,
  paginile adânci în 10–17 s;
- API-ul REST WordPress (`/wp-json/wp/v2/certificate-de-urbanism`) numără 58 071 certificate din 2013
  (2 802 în 2024, 2 791 în 2025, 1 733 în 2026 până la 1 octombrie), 100 pe pagină, cu filtre pe dată, dar
  nu expune scopul și adresa, doar titlul, data și linkul;
- pagina de detaliu a unui certificat (~280 KB) adaugă CF, nr. cadastral, cererea, regimul juridic,
  economic și tehnic (UTR, suprafață); scopul și adresa sunt aceleași ca în listă;
- scopul e text liber cu variante și greșeli („INFORMARE”, „informare”, „INFOMARE”, descrieri de 400 de
  caractere); pe un eșantion de 144, o treime sunt informări, o treime autorizări de lucrări, 12% PUZ;
- cam o treime din certificate nu au stradă în adresă, doar județul și municipiul.

## Factori de decizie

- cele patru lucruri cerute: numărul, tipul, strada cu numărul, data; toate sunt în listă
- o cerere pe secundă către site-ul primăriei (NFR-1) și o singură sincronizare la un moment dat
- istoricul din 2024 să se descarce o singură dată, iar sincronizarea curentă să coste câteva cereri
- alertele pe cuvinte-cheie (FR-9) să prindă certificatele fără să inunde utilizatorii la descărcarea istoricului
- fără pagini de detaliu, care ar însemna 2 800 de cereri și 800 MB de snapshot-uri pe an

## Opțiuni considerate

1. Lista HTML, pagină cu pagină, doar câmpurile din listă
2. API-ul REST pentru enumerare, plus pagina de detaliu a fiecărui certificat pentru scop și adresă
3. Lista HTML plus pagina de detaliu, pentru CF, cadastru, regimuri

## Decizie

Opțiunea 1, în `urban_core::scrape::certificates`, `sync::sync_certificates` și tabelul `certificates`:

- **Sursa e lista HTML**, parcursă de la prima pagină spre trecut; fiecare pagină se scrie într-o tranzacție,
  idempotent pe URL. Pagina de detaliu nu se descarcă; dacă vor fi vreodată necesare CF sau regimul tehnic,
  se adaugă ca pas separat, nu se schimbă acest pas.
- **Istoricul începe din 2024** (`URBAN_CERT_START_YEAR`). Prima sincronizare parcurge toate paginile până
  la primul certificat emis înainte de anul de start (~310 pagini, aproximativ o oră) și notează anul în
  `meta.certificates_backfill_year`. Apoi fiecare sincronizare citește paginile de la început până la prima
  fără certificate noi, de obicei a doua. O parcurgere întreruptă nu lasă marcajul, deci se reia de la capăt;
  `full` o reface oricând.
- **Tipul se derivă din scop** prin reguli pe textul normalizat, în ordinea PUZ/PUD cerute explicit, lucrări
  de autorizat, operațiuni notariale sau cadastrale, informare; „conform PUZ aprobat” nu face dintr-o
  informare un PUZ. Scopul brut se păstrează și se indexează, ca orice cuvânt din el să fie căutabil.
- **Adresa** pierde prefixul cu județul și municipiul; strada iese prin aceeași funcție ca la proiecte, iar
  numărul stradal se ia după markerul „nr”; „FN”, „f.nr.”, „FM” înseamnă fără număr.
- **Alertele** folosesc aceeași interogare FTS și aceeași fereastră `alerts_checked_until` ca proiectele, în
  același email, grupate separat, cu o condiție în plus: certificatul să fi fost emis cu cel mult 14 zile
  înaintea ultimei verificări. Fără ea, descărcarea istoricului ar fi alertat 7 000 de certificate „noi”.
- **Snapshot-urile** rămân ca la restul (FR-1.10): fiecare pagină de listă se scrie pe disc, ~277 KB, cam
  85 MB o singură dată pentru 2024–2026; prima pagină se suprascrie la fiecare sincronizare.

### Consecințe

- Bune: ~120 de cereri pe an de istoric și două pe sincronizare; 1,5 MB pe an în bază; alertele prind
  certificatele din ziua publicării; parserul are un fixture real și reguli de clasificare testate pe
  scopurile observate.
- Rele: fără CF și regim tehnic; o treime din certificate nu au stradă, deci nu pot fi prinse de un
  cuvânt-cheie cu numele străzii; clasificarea e euristică și va avea excepții, de aceea scopul brut rămâne
  vizibil și căutabil; dacă primăria schimbă structura paginii, sincronizarea certificatelor raportează
  eroare fără să afecteze ședințele.
