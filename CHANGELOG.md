# Journal des versions

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), versions
[SemVer](https://semver.org/lang/fr/).

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
