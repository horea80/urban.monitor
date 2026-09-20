# Cerințe — urban.monitor

| | |
|---|---|
| Status | draft, v1 |
| Data | 2026-09-20 |
| Format | user stories + criterii de acceptare în notația EARS („Când X, sistemul trebuie să Y”) |
| Design | [design.md](design.md) · Decizii: [../adr/](../adr/README.md) |

## 1. Scop

Un serviciu web public care urmărește ședințele Comisiei Tehnice de Amenajare a
Teritoriului și Urbanism (CTATU) ale Primăriei Cluj-Napoca, începând cu anul 2026,
și permite căutarea după stradă a inițiativelor de urbanism discutate: planuri
urbanistice zonale (PUZ), planuri urbanistice de detaliu (PUD) și avize de
oportunitate pentru PUZ.

Sursa unică de date: pagina publică
<https://primariaclujnapoca.ro/strategii-urbane/comisia-tehnica-de-amenajare-a-teritoriului-si-urbanism/sedinte-comisie/>
și paginile la care trimite.

## 2. Utilizatori

| Rol | Nevoie |
|---|---|
| Locuitor sau proprietar | „Ce se construiește pe strada mea sau lângă mine?” |
| Arhitect, dezvoltator, jurnalist | „Ce a trecut prin comisie în ultimele luni, pe ce tip de documentație?” |
| Programator | Acces programatic la aceleași date, documentat. |
| Operator (proprietarul serviciului) | Să vadă dacă sincronizarea funcționează și ce s-a stricat. |

## 3. Cerințe funcționale

### FR-1 Colectare

*Ca operator, vreau ca sistemul să colecteze singur ședințele, ca să nu urmăresc manual site-ul primăriei.*

- FR-1.1 Când rulează o sincronizare, sistemul trebuie să citească lista ședințelor și să
  rețină fiecare ședință cu anul ≥ anul de start configurat (implicit 2026), cu: URL, titlu,
  dată, oră, link către PDF-ul ordinii de zi.
- FR-1.2 Când procesează o ședință, sistemul trebuie să rețină fiecare proiect listat pe pagina
  ședinței cu: URL, titlu, adresă, data publicării, categorie (vezi FR-2).
- FR-1.3 Când procesează o ședință, sistemul trebuie să descarce PDF-ul ordinii de zi, să
  extragă textul și să rețină rândurile tabelului (nr. crt., nr. înregistrare, dată, beneficiar,
  descriere, marcaj „revenire CTATU”).
- FR-1.4 Când un rând din ordinea de zi corespunde unui proiect de pe pagină, sistemul trebuie
  să le lege și să completeze proiectul cu beneficiar și nr. înregistrare.
- FR-1.5 Dacă un rând din ordinea de zi nu corespunde niciunui proiect publicat, sistemul
  trebuie să îl păstreze și să îl facă găsibil prin căutare.
- FR-1.6 Când întâlnește un proiect nou, sistemul trebuie să rețină documentele de pe pagina
  proiectului (parte scrisă, parte desenată, altele) ca linkuri.
- FR-1.7 Când există cardul „Concluziile ședinței”, sistemul trebuie să rețină linkul către PDF.
  Extragerea verdictului din PDF-ul scanat NU face parte din v1.
- FR-1.8 Sincronizarea trebuie să fie incrementală: se re-descarcă doar ședințele noi, cele
  mai recente de N zile (implicit 45) și, la cerere explicită, toate.
- FR-1.9 Dacă procesarea unei ședințe eșuează, sistemul trebuie să continue cu celelalte și să
  înregistreze eroarea.
- FR-1.10 Sistemul trebuie să păstreze pe disc conținutul brut descărcat (HTML, PDF), ca să
  poată fi re-parsat fără a mai accesa site-ul sursă.

### FR-2 Clasificare

*Ca utilizator, vreau să filtrez după tipul inițiativei.*

- FR-2.1 Sistemul trebuie să atribuie fiecărui proiect exact una din categoriile:
  `AVIZ_OPORTUNITATE`, `PUZ`, `PUD`, `ALTELE`.
- FR-2.2 Regula: dacă titlul conține „oportunitate” → `AVIZ_OPORTUNITATE`; altfel dacă
  conține PUZ sau P.U.Z → `PUZ`; altfel dacă conține PUD sau P.U.D → `PUD`; altfel `ALTELE`.
  Potrivirea ignoră diacriticele, majusculele și punctele.
- FR-2.3 Cardurile „Concluziile ședinței” și „Anunț privind desfășurarea ședinței” nu sunt
  proiecte și nu primesc categorie.

### FR-3 Căutare

*Ca locuitor, vreau să scriu numele străzii și să văd tot ce a trecut prin comisie acolo.*

- FR-3.1 Când utilizatorul introduce un text, sistemul trebuie să returneze proiectele al căror
  titlu, adresă, stradă, beneficiar sau descriere din ordinea de zi conține fiecare cuvânt
  căutat, potrivit pe început de cuvânt.
- FR-3.2 Căutarea trebuie să ignore diacriticele și majusculele în ambele sensuri:
  „campului” găsește „Câmpului”, „Brâncuși” găsește „Brancusi”.
- FR-3.3 Utilizatorul trebuie să poată filtra după categorie (una sau mai multe) și după an.
- FR-3.4 Rezultatele trebuie sortate după data ședinței, descrescător, apoi după relevanță.
- FR-3.5 Fiecare rezultat trebuie să arate: data ședinței, categoria, titlul cu link către pagina
  proiectului de pe site-ul primăriei, adresa, beneficiarul dacă e cunoscut, documentele.
- FR-3.6 Rezultatele trebuie să includă și rândurile din ordinea de zi fără proiect publicat,
  marcate distinct.

### FR-4 Interfață web

- FR-4.1 Pagina principală `/` conține căutarea; starea căutării (text, filtre) stă în query
  string, astfel încât un link poate fi partajat și redeschis cu aceleași rezultate.
- FR-4.2 Pagina `/sedinte/{id}` arată o ședință cu toate proiectele ei și linkurile către
  ordinea de zi și concluzii.
- FR-4.3 Dacă browserul nu rulează WebAssembly, căutarea trebuie să funcționeze în continuare
  prin trimiterea obișnuită a formularului și randare pe server.
- FR-4.4 Interfața trebuie să fie utilizabilă pe telefon.

### FR-5 API

*Ca programator, vreau să consum datele fără să fac scraping la rândul meu.*

- FR-5.1 Sistemul expune un API JSON read-only sub `/api/v1`: căutare, listă ședințe,
  detaliu ședință, detaliu proiect, status.
- FR-5.2 API-ul este descris printr-un document OpenAPI 3.x generat din cod, servit la
  `/api/v1/openapi.json`, cu interfață de explorare la `/api/docs`.
- FR-5.3 În v1 API-ul este public, fără autentificare. Vezi NFR-5 pentru pregătirea
  autentificării.

### FR-6 Linie de comandă

- FR-6.1 `urban sync [--full]` rulează o sincronizare și raportează contoarele.
- FR-6.2 `urban search <text> [--tip ...] [--an ...]` caută din terminal, cu ieșire text sau JSON.

### FR-7 Operare

- FR-7.1 Serverul rulează sincronizarea automat, la un interval configurabil (implicit 6 ore),
  și nu pornește două sincronizări în paralel.
- FR-7.2 `GET /healthz` răspunde 200 când serverul și baza de date sunt funcționale.
- FR-7.3 `GET /api/v1/status` arată ultima sincronizare, durata, contoarele și eroarea, dacă a fost.
- FR-7.4 Dacă o ședință procesată nu are niciun proiect, sistemul trebuie să înregistreze un
  avertisment (semnal că structura site-ului s-a schimbat).

## 4. Cerințe nefuncționale

- **NFR-1 Politețe față de sursă.** Cel mult o cerere pe secundă către site-ul primăriei,
  User-Agent explicit care identifică serviciul, reîncercare cu backoff exponențial la erori
  tranzitorii, nicio cerere declanșabilă din exterior de către public.
- **NFR-2 Performanță.** Căutarea răspunde sub 100 ms la 10.000 de proiecte. Prima randare a
  paginii vine de pe server, cu rezultatele incluse.
- **NFR-3 Robustețe.** Parserele au teste pe fixture-uri reale salvate în repo. O eroare la
  o ședință nu afectează restul sincronizării.
- **NFR-4 Securitate.** Doar citire pentru public. Limitare de rată per IP. Headere de securitate
  și TLS la proxy. Nu se stochează date personale în plus față de ce publică primăria în
  ordinea de zi oficială.
- **NFR-5 Pregătire pentru autentificare și cote.** Deși v1 e public, codul trebuie să aibă de
  la început: un strat de autentificare montat pe rutele API (identitate în v1), un tip
  `Caller` primit de fiecare handler, schema de securitate `bearerAuth` declarată în OpenAPI
  fără a fi cerută, și limitarea de rată cheiată într-un loc unic, ca să treacă de la IP la
  cheie API fără a atinge handlerele.
- **NFR-6 Operare simplă.** Un singur proces, un singur fișier de bază de date, configurare
  prin variabile de mediu cu valori implicite. Backup = copia fișierului SQLite.
- **NFR-7 Compatibilitate.** Funcționează în browserele din 2017 încoace, pe desktop și mobil,
  inclusiv fără WebAssembly (degradat, dar funcțional).
- **NFR-8 Portabilitate.** Se livrează ca imagine Docker; rulează pe orice VPS Linux.

## 5. În afara scopului v1

Explicit amânate, cu decizii înregistrate în ADR-uri acolo unde e cazul:

- autentificare cu chei API și cote de utilizare (pregătit, neactivat)
- feed RSS/Atom ([ADR-0006](../adr/0006-fara-rss-in-v1.md))
- notificări pe email sau Telegram pentru străzi urmărite
- OCR pe PDF-urile scanate cu concluziile ședinței
- gruparea aparițiilor repetate ale aceluiași proiect în „dosare”
- hartă cu proiectele
- ședințele dinainte de 2026

## 6. Glosar

| Termen | Sens |
|---|---|
| CTATU | Comisia Tehnică de Amenajare a Teritoriului și Urbanism, organ consultativ al primăriei |
| PUZ | Plan Urbanistic Zonal, documentație care modifică reglementările pe o zonă |
| PUD | Plan Urbanistic de Detaliu, documentație pentru o parcelă sau un imobil |
| Aviz de oportunitate | Acordul prealabil pentru inițierea unui PUZ; pe site apare ca „Studiu de oportunitate pentru inițiere PUZ” |
| Revenire CTATU | Proiect rediscutat în comisie după o ședință anterioară |
| Ordine de zi | PDF-ul oficial cu lista lucrărilor unei ședințe |
| Sincronizare (sync) | Procesul de citire a sursei și actualizare a bazei locale |
