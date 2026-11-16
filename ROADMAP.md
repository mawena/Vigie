# Feuille de route — Vigie

## Phase 0 — Cadrage
- [x] Idée et création du projet (16/11/2026)
- [x] Liste des sondes voulues et seuils par défaut (voir docs/cadrage.md)
- [x] Canal d'alerte : ntfy par défaut, Telegram et e-mail en plus
- [x] Lien avec Ricochet : agent séparé
- [x] Vérifier ce qui est lisible sans root (systemd, docker, libvirt, journaux)

## Phase 1 — Fondations
- [x] Projet Cargo, format de configuration TOML
- [x] Sondes système : disque, RAM, charge, températures
- [x] Sondes services : systemd, Docker, libvirt
- [x] Sondes réseau : HTTP(S), certificats TLS, DNS
- [x] Commande `vigie etat`

## Phase 2 — Alertes
- [ ] Seuils, durée minimale, anti-répétition, retour à la normale
- [ ] Canaux : ntfy, Telegram, e-mail
- [ ] Service systemd utilisateur, consommation mesurée (< 20 Mo de RAM)
- [ ] Release 0.1.0 : `.deb`, dépôt APT, release GitHub

## Phase 3 — Tableau de bord
- [ ] Historique compact (SQLite), tendance de remplissage du disque
- [ ] Interface web légère derrière nginx (vigie.mawena.cloud), authentification
- [ ] Release 0.2.0

## Phase 4 — Plusieurs machines
- [ ] Sondes à distance via l'agent Ricochet (PC sans IP publique)
- [ ] Sondes sauvegardes (Coffre)
