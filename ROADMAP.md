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
- [x] Seuils, durée minimale, anti-répétition, retour à la normale
- [x] Canaux : ntfy, Telegram, e-mail
- [x] Service systemd utilisateur, consommation mesurée (< 20 Mo de RAM : 12 Mo au plus haut)
- [x] En service sur le VPS (17/11/2026), alertes ntfy vérifiées
- [x] Release 0.1.0 : `.deb`, dépôt APT, release GitHub (Pages en attente : dépôt privé)

## Phase 3 — Tableau de bord
- [x] Historique compact (SQLite), tendance de remplissage du disque
- [x] Interface web légère, authentification (HTTP Basic), modèle nginx
- [ ] Mise en ligne sur vigie.mawena.cloud : DNS, nginx, certbot (root : Charles)
- [ ] Release 0.2.0

## Phase 4 — Plusieurs machines
- [ ] Sondes à distance via l'agent Ricochet (PC sans IP publique)
- [ ] Sondes sauvegardes (Coffre)
