---
status: accepted
date: 2026-09-21
decision-makers: horea
---

# ADR-0009: Găzduire pe un box împrumutat, sub `urbanism.hopartean.com`, cu nginx și certbot în loc de Caddy

## Context și problemă

[ADR-0008](0008-deploy-fara-docker-systemd-caddy.md) a fixat modelul de deploy (binar sub systemd,
build pe server, reverse proxy cu TLS) presupunând VPS-ul propriu (`millionphones`), unde reverse
proxy-ul e Caddy. Serviciul va rula însă pe box-ul OCI ARM al unui prieten (aliasul ssh `emailbox`,
Ubuntu 24.04 aarch64, 2 OCPU, 11 GB RAM), folosit în 2026 pentru staging-ul serviciului de email din
oracle.gsl și curățat complet în septembrie. Box-ul găzduiește deja un serviciu public al proprietarului,
cu nginx + certbot pe 80/443. Adresa noastră: `urbanism.hopartean.com`, subdomeniu al domeniului lui
(inițial `urbanism.hopartean.com`, redenumit în aceeași zi). Înregistrarea DNS stă în Cloudflare, dar *fără*
proxy (DNS only): cu proxy-ul pornit, nginx ar vedea IP-urile Cloudflare în loc de ale clienților (limitarea
de rată e per IP), certificatul ar trebui emis altfel, iar modul SSL ar trebui să fie Full (strict).

Întrebarea: cum aplicăm ADR-0008 pe un box unde 80/443 sunt deja ocupate de alt proxy?

## Factori de decizie

- nu stricăm serviciul proprietarului: 80/443 rămân la nginx-ul lui, neatins
- schimbări minime față de ADR-0008: aceeași unitate, același deploy, același build pe server
- TLS fără plumbing suplimentar: certbot există pe box, cu plugin nginx și cont Let's Encrypt
- proprietarul acceptă încărcarea box-ului la build (ambele OCPU, câteva minute per deploy)

## Opțiuni considerate

1. Site nginx în `sites-available/`, TLS prin certbot
2. Caddy pe alte porturi (8443) sau înlocuirea nginx-ului cu Caddy
3. Cloudflare Tunnel, fără porturi de intrare
4. Build pe VPS-ul propriu, doar binarul copiat pe box (variantă doar pentru pasul de build)

## Decizie

Opțiunea 1. `deploy/urban.nginx` e echivalentul lui `urban.caddy` (aceleași headere, proxy către
127.0.0.1:8081). `setup.sh` alege proxy-ul după ce găsește pe box (nginx → site nginx; altfel Caddy) și
acceptă `PROXY=` pentru a forța. Certificatul îl obține certbot după ce DNS-ul răspunde, rescriind site-ul
(`listen 443 ssl`, redirect 80 → 443), la fel ca la site-ul proprietarului. `deploy-common.sh` țintește
implicit `emailbox`, cu cheia lui; VPS-ul propriu rămâne accesibil prin `DEPLOY_HOST`/`DEPLOY_KEY`.
Build-ul rămâne pe server (ADR-0008); dacă încarcă prea mult box-ul, opțiunea 4 schimbă doar pasul de build.

CSP-ul are nevoie și de `'unsafe-eval'`, nu doar de `'wasm-unsafe-eval'`: interpretorul web Dioxus își generează
glue-ul JS la rulare cu `eval`; fără el, wasm-ul moare la hidratare și linkurile router-ului schimbă URL-ul fără
să mai randeze nimic (prima versiune a site-ului a avut exact acest bug).

O diferență față de Caddy: nginx completează implicit `X-Forwarded-For` (`$proxy_add_x_forwarded_for`),
iar serverul limitează rata după primul IP din antet. Site-ul îl *suprascrie* cu `$remote_addr`, ca un
client să nu poată falsifica IP-ul; nginx e singurul proxy din față.

### Consecințe

- Bune: nimic instalat pe 80/443; un fișier de site în plus; `deploy.sh` și unitatea neschimbate.
- Rele: două fișiere de site de ținut în pas (nginx și Caddy); proprietarul box-ului are root, acceptat
  fiindcă v1 nu are secrete și datele sunt publice; build-ul ocupă ambele OCPU câteva minute la fiecare deploy.

## Argumente pe opțiuni

### 2. Caddy pe alte porturi sau în locul nginx-ului
- Rău: porturi nestandard sau intervenție în configurația proprietarului, plus schimbări în security list-ul OCI.

### 3. Cloudflare Tunnel
- Bun: fără porturi de intrare, TLS la Cloudflare.
- Rău: domeniul e al proprietarului, nu în Cloudflare-ul nostru; un daemon în plus; nimic în plus față de nginx-ul existent.

### 4. Build pe VPS-ul propriu
- Bun: box-ul împrumutat rămâne minimal (binar, unitate, site); ambele box-uri sunt Ubuntu 24.04 aarch64 cu glibc 2.39, deci binarul e portabil.
- Rău: două gazde în `deploy.sh`; toolchain pe un box unde serviciul nu rulează. Reținută ca variantă.
