---
status: accepted
date: 2026-09-20
decision-makers: horea
---

# ADR-0006: Fără RSS în v1

## Context și problemă

Un feed RSS/Atom per interogare (`/feed.xml?q=strada`) ar fi o formă ieftină de notificare.
A fost propus pentru v1.

## Decizie

Nu în v1, la cererea explicită a proprietarului. Interogările de căutare rămân în `core`,
astfel încât un feed se poate adăuga ulterior ca o rută de randare peste aceleași rezultate.

### Consecințe

- Bune: scop mai mic pentru v1.
- Rele: „monitorizarea” în v1 înseamnă vizită manuală sau consum de API; notificările rămân
  pentru v2, alături de email sau Telegram.
