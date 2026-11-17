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
- **Autres machines**, même sans IP publique (PC derrière une box), par le relais de
  [Ricochet](https://github.com/mawena/Ricochet) : machine en ligne ou non, disques, mémoire.
- **Historique** court dans SQLite (un point toutes les 5 min, 30 jours) et **tendance** de
  remplissage des disques (« plein dans environ N jours »).
- **Tableau de bord web** léger, lisible sur téléphone : problèmes en cours, courbes du disque,
  de la mémoire et de la charge (24 h, 7 ou 30 jours), toutes les mesures, dernières alertes.
- **Lecture seule** : Vigie regarde, il ne redémarre ni ne modifie rien.

Les choix de départ (seuils, canaux, lien avec Ricochet) sont expliqués dans
[`docs/cadrage.md`](docs/cadrage.md).

## Installation

Ubuntu 24.04 (amd64), par le dépôt APT signé unique de mawena (<https://apt.mawena.cloud/>),
en une ligne :

```sh
curl -fsSL https://apt.mawena.cloud/installer.sh | sudo sh -s -- vigie
```

Pas à pas (ce que fait le script) :

```sh
sudo curl -fsSLo /usr/share/keyrings/mawena-depot.gpg https://apt.mawena.cloud/mawena-depot.gpg
sudo curl -fsSLo /etc/apt/sources.list.d/mawena-depot.sources https://apt.mawena.cloud/mawena-depot.sources
sudo apt update && sudo apt install mawena-depot vigie
```

Mises à jour : `sudo apt update && sudo apt upgrade`. Depuis la 0.3.1, une machine qui suivait
l'ancien dépôt du projet (`https://mawena.github.io/Vigie/apt`) passe d'elle-même au dépôt
unique au premier `sudo apt upgrade` : `vigie` dépend de `mawena-depot` et retire l'ancienne
source `/etc/apt/sources.list.d/vigie.sources`. En dernier recours, le `.deb` est joint à
chaque [release GitHub](https://github.com/mawena/Vigie/releases) (avec `mawena-depot`, pris
sur <https://apt.mawena.cloud/>).

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

### Tableau de bord

`vigie surveiller` sert le tableau de bord sur `127.0.0.1:8790` (section `[web]`). Pour y
accéder de l'extérieur, mettre nginx devant en HTTPS (modèle :
[`deploiement/nginx-vigie.conf`](deploiement/nginx-vigie.conf), installé dans
`/usr/share/doc/vigie/`) et choisir un mot de passe :

```sh
vigie mot-de-passe        # tape le mot de passe, affiche son empreinte SHA-256
```

puis recopier l'empreinte dans `mot_de_passe` de `[web]` et redémarrer
(`systemctl --user restart vigie`). Sans mot de passe, Vigie refuse d'écouter ailleurs que sur
127.0.0.1. API : `GET /api/etat` (dernière tournée), `GET /api/serie?cle=disque:/&jours=7`.

### Autres machines (Ricochet)

Si les machines font tourner l'agent de Ricochet, Vigie lit leur état sur le relais avec un
jeton opérateur, en lecture seule :

```sh
ricochet-serveur jeton    # affiche un jeton et son empreinte
# empreinte -> jeton_operateur_sha256 dans /etc/ricochet/serveur.toml, puis
sudo systemctl restart ricochet-serveur
```

puis, dans `vigie.toml` : section `[ricochet]` avec `relais`, `jeton`, `machines` (vide :
toutes) et `toujours_en_ligne` (machines dont la déconnexion est critique ; un PC éteint la
nuit n'en fait pas partie). Les disques et la mémoire distants suivent les seuils de
`[disque]` et `[memoire]` et ont leurs courbes sur le tableau de bord.

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
| `[ricochet]` | `relais`, `jeton`, `machines`, `toujours_en_ligne` |
| `[alertes]` | `duree_min` (tournées), `rappel` (minutes), `retour`, `attention` |
| `[ntfy]` `[telegram]` `[courriel]` | canaux d'alerte |
| `[historique]` | `actif`, `pas` (secondes), `garder_jours`, `fichier` |
| `[web]` | `actif`, `adresse`, `utilisateur`, `mot_de_passe` (empreinte) |

Le fichier contient des secrets (sujet ntfy, jeton Telegram) : droits `600`.

## Structure
```
src/main.rs           commandes (etat, surveiller, test-alerte…)
src/config.rs         configuration TOML et valeurs par défaut
src/modele.rs         niveaux et mesures
src/sondes/           systeme (disque, mémoire, charge, température), services (systemd,
                      Docker, libvirt), reseau (HTTP, TLS, DNS), fichiers (sauvegardes),
                      distant (machines vues par le relais de Ricochet)
src/alertes.rs        décision d'alerter, messages groupés, ntfy / Telegram / e-mail
src/historique.rs     historique SQLite, tendance de remplissage
src/web.rs, src/web/  tableau de bord (HTML, CSS, JS) et API JSON
exemples/vigie.toml   configuration commentée
deploiement/          service systemd utilisateur, modèle nginx
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
