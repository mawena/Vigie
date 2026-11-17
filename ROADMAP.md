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
- [x] Release 0.1.0 : `.deb`, dépôt APT, release GitHub

## Phase 3 — Tableau de bord
- [x] Historique compact (SQLite), tendance de remplissage du disque
- [x] Interface web légère, authentification (HTTP Basic), modèle nginx
- [x] Mise en ligne sur https://vigie.mawena.cloud : DNS, nginx, certbot (fait le 17/11/2026)
- [x] Release 0.2.0

## Phase 4 — Plusieurs machines
- [x] Sondes à distance via le relais Ricochet (PC sans IP publique), release 0.3.0
- [x] Brancher le jeton opérateur sur le relais du VPS (fait le 17/11 : VPS et PC du bureau suivis)
- [x] Sonde générique des sauvegardes (âge du fichier le plus récent)
- [ ] La brancher sur Coffre quand il existera (dossier ou état de ses sauvegardes)

## Plus tard (idées)
- [ ] Garder l'état des alertes au redémarrage (annoncer le retour à la normale d'un problème
      signalé avant un redémarrage)
- [x] Rendre le dépôt public pour servir le site et le dépôt APT (fait le 17/11/2026)
      par GitHub Pages
