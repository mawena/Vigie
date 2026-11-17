# Vigie — consignes pour Claude

Surveillance légère du VPS mawena.cloud et des machines de Charles, alertes sur le téléphone.
Rust. Créé le 16/11/2026. Lire `README.md` et `ROADMAP.md`.

## Où on en est
- Version 0.3.0 (17/11/2026) : phases 0 à 4 faites. En ligne sur https://vigie.mawena.cloud
  (HTTP Basic, utilisateur charles), Ricochet branché (VPS en `toujours_en_ligne`).
- Dépôt GitHub privé : git@github.com:mawena/Vigie.git. Branche gh-pages prête, Pages refusé
  tant que le dépôt est privé.
- En service sur le VPS : `~/.local/bin/vigie`, unité `~/.config/systemd/user/vigie.service`,
  config `~/.config/vigie/vigie.toml` (600), historique `~/.local/share/vigie/`. Pour
  redéployer : `cargo build --release`, copier le binaire, `systemctl --user restart vigie`
  (avec `XDG_RUNTIME_DIR=/run/user/1000`).
- Clé APT : « Vigie APT (mawena) » 3429F5DB…49CB, dans le trousseau GPG du VPS.

## Façon de travailler (en plus de `~/.claude/CLAUDE.md`)
- Le VPS est en production : Vigie ne fait que lire, jamais d'action sur nginx, lnmp, Docker
  ou les bases sans demande de Charles. Pas de root.
- Rester très léger en RAM et CPU (machine partagée, 11 Go).
- Rust dans `~/.cargo/bin` (absent du PATH par défaut). git flow, commits en français au nom de
  Charles, sans `Co-Authored-By`.
