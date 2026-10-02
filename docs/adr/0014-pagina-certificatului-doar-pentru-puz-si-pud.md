---
status: accepted
date: 2026-10-02
decision-makers: horea
---

# ADR-0014: Pagina certificatului se descarcă doar pentru PUZ și PUD, și se păstrează doar prefixele specifice

## Context și problemă

ADR-0013 a decis ca certificatele de urbanism să vină doar din listă, fără pagini de detaliu. Problema
semnalată imediat: la un PUZ lista dă doar strada, de multe ori fără număr, deci nu se vede unde e zona.
Am verificat 10 pagini de certificat, 6 PUZ și 4 PUD sau autorizări:

- numărul cadastral și cartea funciară lipsesc la toate cele 6 PUZ-uri („identificat prin plan de
  încadrare în zonă și plan de situație”) și sunt prezente la toate PUD-urile și autorizările;
- suprafața apare la 3 din 6 PUZ-uri și la toate celelalte, ca prefix al „regimului tehnic”:
  „S = 5393 mp EM”, „S= 23 434 mp TF”, urmat de 8–30 mii de caractere copiate din regulamentul PUG;
- codurile UTR apar în același prefix și în „regimul economic” („Destinația: UTR=ULC, …”), iar
  folosința actuală e prima propoziție din „regimul economic”.

Deci pagina nu localizează exact un PUZ, dar îl îngustează: suprafață, ce e acum pe teren, încadrarea
în PUG. Pentru PUD dă parcela exactă.

## Factori de decizie

- cererile către primărie: toate certificatele ar însemna ~2 800 de pagini pe an și 800 MB de snapshot-uri
- câmpurile utile sunt prefixe scurte; textul de regulament nu are valoare și ar umfla baza și căutarea
- descărcarea istoricului listei rulează deja pe server; pasul nou trebuie să fie reluabil și plafonat

## Decizie

- **Doar PUZ și PUD** primesc pagina de detaliu, ~15% din certificate, în jur de 400 pe an. Tipurile sunt
  o constantă în `sync` (`DETAIL_KINDS`), nu configurare.
- **Pas separat**, după lista de certificate: cele fără `detail_fetched_at`, cele mai recente întâi, cel
  mult 300 pe rulare (~10 minute la o cerere pe secundă). Istoricul de ~1 100 de pagini se termină în câteva
  rulări. O pagină care nu are câmpurile așteptate se marchează totuși descărcată, ca să nu se reîncerce la
  nesfârșit; o descărcare eșuată rămâne nedescărcată și se reia.
- **Doar prefixele**: suprafața ca întreg în mp, codurile UTR fără dubluri, folosința actuală până la
  „Destinația”, CF și cadastralul după etichetă, `None` când sunt „-”. Textul de regulament nu se stochează.
  Câmpurile intră în indexul de căutare al certificatului și pe card.
- **Re-citirea listei nu atinge** aceste câmpuri: upsert-ul listei nu le include.

### Consecințe

- Bune: PUD-urile au parcela exactă; PUZ-urile au suprafața, folosința și UTR-ul, care împreună cu strada
  localizează destul de bine; cost mic și plafonat.
- Rele: la jumătate din PUZ-uri nu există suprafață nici pe pagină; extragerea UTR-urilor e euristică pe
  text liber și poate rata coduri rare; dacă primăria schimbă formatul prefixului, câmpurile rămân goale,
  dar certificatul rămâne în bază cu datele din listă.
