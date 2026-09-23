---
status: accepted
date: 2026-09-21
decision-makers: horea
---

# ADR-0011: Alerte pe email la ședințe noi, prin Resend, către o listă fixă; starea în bază

## Context și problemă

Scopul proiectului e ca omul să afle devreme că s-a anunțat o ședință CTATU, ca să apuce să citească
ordinea de zi. Site-ul se sincronizează la fiecare oră (FR-7.1), deci informația există în bază la
scurt timp după publicare, dar nimeni nu e anunțat. Notificările per stradă urmărită rămân în afara
v1 (cerințe, § 5); aici e nevoie de ceva minim: un email la fiecare ședință nou apărută, către o
listă fixă de adrese, deocamdată una singură.

## Factori de decizie

- fără infrastructură nouă: nici server de mail, nici cozi, nici cont în plus
- să nu piardă alerte când trimiterea eșuează și să nu trimită de două ori
- să nu blocheze serverul web și să nu prelungească sensibil sincronizarea
- oprit implicit: fără cheie, nimic nu se schimbă în comportamentul serverului

## Opțiuni considerate

1. Resend, prin API-ul HTTP, cu reqwest (deja dependență)
2. SMTP direct (lettre) către un furnizor
3. Telegram bot
4. Abonare în UI, cu adrese în bază și confirmare pe email

## Decizie

Opțiunea 1. `urban_core::notify` trimite după fiecare sincronizare un singur email cu ședințele care
au data de azi sau din viitor și `meetings.alerted_at IS NULL`, apoi le marchează. Ordinea „trimite,
apoi marchează” face ca un eșec (Resend indisponibil, cheie greșită) să însemne o alertă întârziată
până la sincronizarea următoare, nu pierdută; riscul invers, dublura, apare doar dacă procesul cade
exact între trimitere și marcare. Ședințele din trecut nu se alertează (la prima rulare pe o bază
plină ar fi zgomot). Trimiterea e async, în task-ul de sincronizare, cu timeout de 15 s; serverul
web nu așteaptă după ea.

Configurare prin mediu: `RESEND_API_KEY` (fără ea alertele sunt oprite, cu un rând în jurnal),
`ALERT_FROM` (adresă la un domeniu verificat în Resend, `hopartean.com`), `ALERT_TO` (listă separată
prin virgulă; v1: o adresă), `URBAN_PUBLIC_URL` pentru linkuri. `Reply-To` e primul destinatar, ca
răspunsurile să aibă unde ajunge fără o cutie poștală la expeditor. `urban notify --test` trimite un
email de probă; `urban notify` rulează pasul de alertare manual. Același furnizor ca în oracle.gsl.

### Consecințe

- Bune: puțin cod, o coloană nouă, nicio dependență în plus; alertă în cel mult o oră de la
  publicare; oprit implicit.
- Rele: lista de destinatari e în configurare, nu în produs (fiecare adăugare = `just deploy-env`);
  fără dezabonare; un singur email pentru toate ședințele noi dintr-o rulare; o cheie de API pe box
  (în `/opt/urban/.env`, 0640, doar utilizatorul serviciului).

## Argumente pe opțiuni

### 2. SMTP direct
- Bun: furnizor interschimbabil.
- Rău: încă o dependență (lettre + TLS), credențiale SMTP, livrabilitate de gestionat; nimic în plus
  față de Resend pentru o adresă.

### 3. Telegram
- Bun: instant, fără domeniu.
- Rău: cont de bot, chat id-uri, alt canal decât cel cerut; poate veni ulterior lângă email.

### 4. Abonare în UI
- Bun: e produsul „adevărat”, cu străzi urmărite.
- Rău: formulare, confirmări, dezabonare, date personale; explicit în afara v1.
