---
status: accepted
date: 2026-09-21
decision-makers: horea
---

# ADR-0012: Conturi fără parolă, un credit = un cuvânt-cheie pe an, planul gratuit cu 2 credite

## Context și problemă

Alertele din FR-8 merg către o listă fixă. Următorul pas e ca oricine să-și poată urmări o stradă, un
cartier sau un beneficiar: primește un email la fiecare proiect nou de pe ordinea de zi CTATU care se
potrivește. Asta cere conturi, o unitate de măsură care să permită mai târziu planuri plătite (și SMS,
mai scump), și limite care să nu transforme un cont gratuit în monitorizare nelimitată. Referința e
oracle.gsl, care are deja autentificare prin link pe email, planuri și un jurnal de credite, ambele prin
Resend și SQLite.

## Factori de decizie

- fără parole și fără furnizor de identitate: adresa de email e contul
- un model de credite care să se extindă la planuri plătite fără rescriere
- o alertă ratată e cel mai rău rezultat pentru un produs de alertare; limita trebuie să stea pe
  ce urmărești, nu pe câte alerte primești
- fără planificatoare; totul event-driven sau leneș la citire, ca restul serverului
- să meargă și fără wasm (NFR-7) și să nu pună secrete în browser

## Opțiuni considerate

1. Un credit = o alertă (email) trimisă; 2 pe an gratuit
2. Un credit = un cuvânt-cheie ținut un an, cu alerte nelimitate; 2 pe an gratuit
3. Fără credite: limită fixă de cuvinte-cheie per plan
4. Parole clasice sau OAuth în loc de link pe email

## Decizie

Opțiunea 2 pentru credite, link pe email pentru autentificare (ca în oracle.gsl), cu următoarele
alegeri, în `urban_core::accounts`, `urban_core::alerts` și `urban_app::account`:

- **Contul e utilizatorul.** Nu există organizații: creditele, planul și ciclul stau pe `users`. Dacă
  vreodată apar echipe, se adaugă o entitate de facturare deasupra, nu se rescrie ce e aici.
- **Creditele sunt două contoare** (`credits_available`, `credits_used`) modificate în tranzacție, cu
  jurnalul `credit_ledger` append-only, `amount >= 0` și `idempotency_key UNIQUE` (`initial-free-grant:{u}`,
  `keyword:{u}:{k}`, `cycle-reset:{u}:{start}`, `keyword-renewal:{u}:{k}:{start}`). Soldul e diferența
  contoarelor, nu suma jurnalului; jurnalul e audit și protecție la dubluri.
- **Un credit = un cuvânt-cheie un an.** Se consumă la adăugare și nu se recuperează la ștergere;
  altfel rotirea săptămânală a cuvintelor ar face din două locuri oricâte. Alertele pentru cuvintele
  ținute sunt nelimitate, deci nu există „credite terminate, alertă ratată”. Refuzul apare doar la
  adăugarea unui cuvânt fără credite, cu data reînnoirii: acesta e momentul natural de upgrade.
- **Ciclul anual se rotește leneș**, la prima citire după `cycle_end_at`: creditele revin la 2, apoi
  cuvintele ținute se reînnoiesc din ele. oracle.gsl promite „yearly refresh” și nu îl face; aici e
  în `ensure_cycle`, cu rânduri idempotente în jurnal și fără planificator.
- **Autentificare prin link pe email**, valabil 15 minute, o singură dată, cel mult un link la două
  minute per adresă; sesiune de 30 de zile într-un cookie `HttpOnly`, `SameSite=Lax`, `Secure` când
  adresa publică e https. În bază stau doar hash-uri SHA-256 ale jetoanelor (oracle.gsl le ține în
  clar; aici nu), iar cele expirate se șterg la fiecare sincronizare. Fără `RESEND_API_KEY`, linkul
  ajunge în jurnal, ceea ce face fluxul testabil local.
- **Acordul** se bifează în formularul de autentificare și se cere doar la crearea contului
  (`consent_at`); un cont nou fără acord e refuzat. Contul se șterge dintr-un click, cu cascadă pe tot
  ce ține de el. Pagina `/confidentialitate` spune ce stocăm și de ce.
- **Potrivirea** folosește aceeași interogare FTS ca site-ul (prefix pe fiecare cuvânt, fără
  diacritice), pe proiectele cu `first_seen_at` de după `alerts_checked_until`; un email per utilizator
  per rulare, grupat pe cuvinte, cu linkuri către ședință și către proiectul primăriei. Dacă emailul
  nu pleacă, fereastra nu avansează și se reîncearcă la sincronizarea următoare.
- **Formulare clasice + redirecturi** pentru toate acțiunile contului (`/cont/login`, `/cont/verifica`,
  `/cont/cuvinte`, `/cont/iesire`, `/cont/sterge`), deci pagina merge și fără wasm; mesajele se întorc
  în query string și pagina le afișează ca text.
- **Validarea cuvintelor**: 3–60 de caractere, cel mult 5 cuvinte, fiecare de cel puțin 3 caractere,
  fără dubluri după normalizare. Minimul contează mai mult decât maximul: un cuvânt de două litere ar
  prinde jumătate din bază.

### Consecințe

- Bune: fără parole, fără date de card, fără infrastructură nouă; un cont gratuit costă zero cât timp
  nu apare nimic pe cuvintele lui; planurile plătite înseamnă doar alte constante și un grant în plus.
- Rele: e-mailurile de autentificare depind de Resend; un utilizator poate ține cel mult două cuvinte
  pe an și nu le poate schimba fără să piardă creditul; nu există administrare în UI (baza se
  inspectează cu CLI-ul sau direct); potrivirea e doar pe proiecte, nu și pe rândurile de agendă
  nepotrivite cu un proiect.

## Argumente pe opțiuni

### 1. Un credit = o alertă
- Bun: măsoară consumul real.
- Rău: două alerte pe an înseamnă că a doua ședință relevantă din martie tace; produsul își pierde
  rostul exact când e nevoie de el. Respinsă după discuție.

### 3. Limită fixă de cuvinte, fără credite
- Bun: și mai simplu.
- Rău: nu se extinde la SMS (care costă per mesaj) și la planuri cu „mai mult din același lucru”;
  creditul e unitatea comună.

### 4. Parole sau OAuth
- Bun: fără dependență de email la fiecare autentificare.
- Rău: parole de gestionat (resetare, scurgeri) sau un furnizor extern; linkul pe email e deja
  drumul dovedit în oracle.gsl și adresa de email e oricum necesară pentru alerte.
