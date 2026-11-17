# Vigie — consignes pour Claude

Surveillance légère du VPS mawena.cloud et des machines de Charles, alertes sur le téléphone.
Rust. Créé le 16/11/2026. Lire `README.md` et `ROADMAP.md`.

## Où on en est
- Version 0.3.1 (17/11/2026, passage au dépôt APT unique https://apt.mawena.cloud :
  `Depends: mawena-depot`, `rm_conffile` de l'ancienne source) : phases 0 à 4 faites. En ligne sur https://vigie.mawena.cloud
  (HTTP Basic, utilisateur charles), Ricochet branché (VPS en `toujours_en_ligne`).
- Dépôt GitHub public : git@github.com:mawena/Vigie.git. Site et dépôt APT sur GitHub Pages
  (https://mawena.github.io/Vigie/, branche gh-pages).
- En service sur le VPS : `~/.local/bin/vigie`, unité `~/.config/systemd/user/vigie.service`,
  config `~/.config/vigie/vigie.toml` (600), historique `~/.local/share/vigie/`. Pour
  redéployer : `cargo build --release`, copier le binaire, `systemctl --user restart vigie`
  (avec `XDG_RUNTIME_DIR=/run/user/1000`).
- Clé APT de l'ancien dépôt : « Vigie APT (mawena) » 3429F5DB…49CB, dans le trousseau GPG du VPS
  (ancien dépôt gardé jusqu'à mai 2027 ; les nouvelles releases passent par le dépôt unique).

## Façon de travailler (en plus de `~/.claude/CLAUDE.md`)
- Le VPS est en production : Vigie ne fait que lire, jamais d'action sur nginx, lnmp, Docker
  ou les bases sans demande de Charles. Pas de root.
- Rester très léger en RAM et CPU (machine partagée, 11 Go).
- Rust dans `~/.cargo/bin` (absent du PATH par défaut). git flow, commits en français au nom de
  Charles, sans `Co-Authored-By`.
