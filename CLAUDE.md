# Vigie — consignes pour Claude

Surveillance légère du VPS mawena.cloud et des machines de Charles, alertes sur le téléphone.
Rust. Créé le 16/11/2026. Lire `README.md` et `ROADMAP.md`.

## Où on en est
- Phase 0 (cadrage). Dépôt GitHub privé : git@github.com:mawena/Vigie.git.

## Façon de travailler (en plus de `~/.claude/CLAUDE.md`)
- Le VPS est en production : Vigie ne fait que lire, jamais d'action sur nginx, lnmp, Docker
  ou les bases sans demande de Charles. Pas de root.
- Rester très léger en RAM et CPU (machine partagée, 11 Go).
- Rust dans `~/.cargo/bin` (absent du PATH par défaut). git flow, commits en français au nom de
  Charles, sans `Co-Authored-By`.
