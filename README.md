# Vigie

Site et installation : <https://mawena.github.io/Vigie/>

**Garde un œil sur le VPS (et mes machines) et prévient sur le téléphone avant que ça casse.**
Disque qui se remplit, service tombé, conteneur arrêté, certificat qui expire, sauvegarde ratée,
site qui ne répond plus. Écrit en Rust, léger (une dizaine de Mo de mémoire), sans root, sans
dépendance lourde (pas de Prometheus/Grafana). Voir la [feuille de route](ROADMAP.md).

## À quoi ça sert
- **Sondes** : disques, mémoire, charge, températures, unités systemd, conteneurs Docker,
  machines libvirt, sites HTTP(S), certificats TLS, DNS, âge de la dernière sauvegarde.
- **Alertes** sur le téléphone (ntfy, Telegram ou e-mail) : durée minimale avant d'alerter,
  alertes d'une même tournée groupées, aggravation signalée tout de suite, rappel espacé tant que
  ça dure, message de retour à la normale.
- **Lecture seule** : Vigie regarde, il ne redémarre ni ne modifie rien.

Les choix de départ (seuils, canaux, lien avec Ricochet) sont expliqués dans
[`docs/cadrage.md`](docs/cadrage.md).

## Installation

Ubuntu 24.04 (amd64), par le dépôt APT signé, en une ligne :

```sh
curl -fsSL https://mawena.github.io/Vigie/installer.sh | sudo sh
```

Pas à pas (ce que fait le script) :

```sh
sudo curl -fsSLo /usr/share/keyrings/vigie.gpg https://mawena.github.io/Vigie/apt/vigie.gpg
sudo curl -fsSLo /etc/apt/sources.list.d/vigie.sources https://mawena.github.io/Vigie/apt/vigie.sources
sudo apt update && sudo apt install vigie
```

Mises à jour : `sudo apt update && sudo apt upgrade`. En dernier recours, le `.deb` est joint à
chaque [release GitHub](https://github.com/mawena/Vigie/releases).

Depuis les sources : `cargo build --release`, binaire dans `target/release/vigie`.

## Utilisation

En tant qu'utilisateur qui surveille la machine (pas root ; il doit être dans les groupes
`docker` et `libvirt` pour ces sondes) :

```sh
mkdir -p ~/.config/vigie && vigie config-exemple > ~/.config/vigie/vigie.toml   # puis l'adapter
vigie etat                     # lance les sondes une fois (code 0 OK, 1 attention, 2 critique)
vigie etat --json              # la même chose en JSON
vigie test-alerte              # message d'essai sur tous les canaux
systemctl --user enable --now vigie      # surveillance continue (vigie surveiller)
sudo loginctl enable-linger "$USER"      # pour tourner sans session ouverte
journalctl --user -u vigie -f            # journal du service
```

Pour recevoir les alertes avec ntfy : installer l'application ntfy sur le téléphone et s'abonner
au sujet choisi dans `[ntfy]` (un nom long et aléatoire, il fait office de secret).

## Configuration

Un fichier TOML, `~/.config/vigie/vigie.toml` (ou `--config`, ou `$VIGIE_CONFIG`). Toutes les
sections sont facultatives : sans fichier, Vigie surveille le disque `/`, la mémoire, la charge,
les unités systemd en échec et les conteneurs Docker malades. Modèle commenté :
[`exemples/vigie.toml`](exemples/vigie.toml) (`vigie config-exemple`).

| Section | Contenu |
|---|---|
| `nom`, `intervalle` | nom de la machine dans les alertes, secondes entre deux tournées (60) |
| `[disque]` `[memoire]` `[charge]` `[temperature]` | seuils `attention` et `critique` |
| `[systemd]` | `echecs`, `ignorer`, `unites`, `unites_utilisateur` |
| `[docker]` | `conteneurs` qui doivent tourner, `signaler_malades` |
| `[libvirt]` | `uri`, `domaines` qui doivent tourner |
| `[[http]]` `[[tls]]` `[[dns]]` `[[sauvegarde]]` | un bloc par site, certificat, nom ou dossier |
| `[alertes]` | `duree_min` (tournées), `rappel` (minutes), `retour`, `attention` |
| `[ntfy]` `[telegram]` `[courriel]` | canaux d'alerte |

Le fichier contient des secrets (sujet ntfy, jeton Telegram) : droits `600`.

## Structure
```
src/main.rs           commandes (etat, surveiller, test-alerte…)
src/config.rs         configuration TOML et valeurs par défaut
src/modele.rs         niveaux et mesures
src/sondes/           systeme (disque, mémoire, charge, température), services (systemd,
                      Docker, libvirt), reseau (HTTP, TLS, DNS), fichiers (sauvegardes)
src/alertes.rs        décision d'alerter, messages groupés, ntfy / Telegram / e-mail
exemples/vigie.toml   configuration commentée
deploiement/          service systemd utilisateur
paquets/apt/          clé et source du dépôt APT
scripts/              construire-deb.sh, publier-apt.sh
site/                 page d'accueil du dépôt APT (GitHub Pages)
docs/cadrage.md       choix de la phase 0
```

## Développement
```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo test
scripts/construire-deb.sh            # dist/vigie_<version>_amd64.deb + SHA256SUMS
scripts/publier-apt.sh dist/*.deb    # dépôt APT signé sur la branche gh-pages
```

## Licence
MIT.
