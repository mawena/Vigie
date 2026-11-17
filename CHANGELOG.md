# Journal des versions

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), versions
[SemVer](https://semver.org/lang/fr/).

## [0.4.0] — 2026-11-17

Mode Portier.

### Ajouté
- Section `[portier]` (`actif`, `utilisateurs`, `url`, `deconnexion`), désactivée par défaut :
  derrière nginx et Portier, une requête venue de 127.0.0.1/::1 avec un seul en-tête
  `X-Portier-Utilisateur` dont la valeur est dans `utilisateurs` passe sans HTTP Basic. Sinon,
  HTTP Basic comme avant.
- Page 401 avec un bouton « Se connecter via Portier »
  (`<url>/connexion?retour=<adresse demandée>`) quand `url` est renseignée.
- Lien « Se déconnecter » dans le pied de page en mode Portier (`deconnexion`, sinon `<url>/`).

## [0.3.1] — 2026-11-17

Passage au dépôt APT unique.

### Modifié
- Le paquet dépend de `mawena-depot` (dépôt unique <https://apt.mawena.cloud>) et ne fournit
  plus ni la source `/etc/apt/sources.list.d/vigie.sources` (retirée proprement par
  `rm_conffile`) ni la clé `/usr/share/keyrings/vigie.gpg`. Un `sudo apt upgrade` suffit pour
  passer au nouveau dépôt.
- Installation : README et site passent par
  `curl -fsSL https://apt.mawena.cloud/installer.sh | sudo sh -s -- vigie` ; l'ancien
  `installer.sh` du site lance désormais celui du dépôt unique.

## [0.3.0] — 2026-11-17

Plusieurs machines.

### Ajouté
- Section `[ricochet]` : état des machines vues par le relais de Ricochet (API opérateur,
  jeton) — en ligne ou non (critique seulement pour `toujours_en_ligne`), disques et mémoire
  distants avec les mêmes seuils, historique et courbes. Marche pour un PC sans IP publique.
- Machine attendue mais absente du relais signalée.

## [0.2.0] — 2026-11-17

Historique et tableau de bord.

### Ajouté
- Historique SQLite (`[historique]`) : valeurs numériques des mesures toutes les 5 min, alertes,
  purge après 30 jours.
- Tendance de remplissage des disques sur 7 jours (« plein dans environ N jours »).
- Tableau de bord web (`[web]`, `127.0.0.1:8790`) : état global, problèmes en cours, courbes
  24 h / 7 j / 30 j avec infobulle, toutes les mesures, dernières alertes ; clair et sombre,
  lisible sur téléphone. API JSON `/api/etat` et `/api/serie`.
- Mot de passe HTTP Basic (empreinte SHA-256, `vigie mot-de-passe`) ; refus d'écouter hors de
  127.0.0.1 sans mot de passe ; en-têtes de sécurité (CSP stricte).
- Modèle nginx (`deploiement/nginx-vigie.conf`).

## [0.1.0] — 2026-11-17

Première version : sondes et alertes.

### Ajouté
- Sondes système : disques, mémoire, charge par cœur, températures.
- Sondes services : unités systemd en échec ou arrêtées (système et utilisateur), conteneurs
  Docker (socket local, lecture seule), machines libvirt.
- Sondes réseau : sites HTTP(S) (code, temps de réponse, texte attendu), certificats TLS (jours
  restants, validité), DNS (adresses attendues) ; en parallèle.
- Sonde des sauvegardes : âge du fichier le plus récent d'un dossier.
- `vigie etat` (texte ou `--json`, code de sortie selon la gravité).
- `vigie surveiller` : durée minimale avant d'alerter, alertes groupées, aggravation, rappels,
  retour à la normale, messages réessayés si aucun canal ne répond.
- Canaux ntfy, Telegram, e-mail (sendmail) ; `vigie test-alerte`.
- Service systemd utilisateur ; paquet `.deb` et dépôt APT signé.
