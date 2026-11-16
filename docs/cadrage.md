# Cadrage de Vigie (phase 0, 17/11/2026)

Choix faits en autonomie à la demande de Charles (« fais tout toi-même ») ; tous se changent
dans la configuration sans toucher au code.

## Sondes et seuils par défaut

| Sonde | Mesure | Attention | Critique |
|---|---|---|---|
| Disque (`/` par défaut) | % utilisé | 85 % | 95 % |
| Mémoire | % utilisé (d'après `MemAvailable`) | 90 % | 97 % |
| Charge | charge sur 5 min par cœur | 1,5 | 3,0 |
| Température | zone thermique la plus chaude | 80 °C | 95 °C |
| systemd | unités en échec, unités voulues inactives | — | toute unité tombée |
| Docker | conteneurs voulus arrêtés, conteneurs `unhealthy` ou qui redémarrent | malade | arrêté |
| libvirt | machines voulues qui ne tournent pas | — | arrêtée |
| HTTP(S) | code de réponse, temps de réponse | lent (> 5 s) | erreur ou code ≥ 400 |
| Certificat TLS | jours avant expiration | 14 j | 5 j (ou invalide) |
| DNS | résolution, adresses attendues | adresse inattendue | ne résout pas |
| Sauvegarde | âge du fichier le plus récent d'un dossier | > max | > 2 × max |

Une sonde qui ne peut pas s'exécuter (socket Docker illisible, commande absente) donne
« Attention » avec le message d'erreur : on préfère savoir que la surveillance est aveugle.

## Alertes
- Une alerte part quand un problème dure **2 tournées** de suite (2 min par défaut), pour ne pas
  réveiller le téléphone pour un raté isolé.
- **Rappel** toutes les 6 h tant que le problème dure ; nouvelle alerte immédiate si ça s'aggrave
  (attention → critique) ; message de **retour à la normale**.
- Les alertes d'une même tournée partent **groupées** en un seul message.

## Canal d'alerte
- **ntfy** par défaut : application Android gratuite, sans compte, notifications instantanées.
  Serveur public `https://ntfy.sh` avec un sujet long et aléatoire (le sujet fait office de
  secret) ; un serveur ntfy auto-hébergé se branche en changeant `serveur` (et `jeton`).
- **Telegram** (bot) et **e-mail** (par `sendmail`) en plus, facultatifs.
- Publication ntfy en JSON (titres accentués sans souci d'en-têtes HTTP).

## Lien avec Ricochet
Agent **séparé**. Ricochet commande les machines (terminal, fichiers) ; Vigie ne fait que lire
et alerter. Les mélanger augmenterait la surface d'attaque de l'agent de surveillance. Pour les
autres machines (phase 4), chaque machine fait tourner Vigie elle-même et alerte directement ;
le relais Ricochet servira plus tard à rapatrier leur état sur le tableau de bord du VPS.

## Ce qui est lisible sans root (vérifié sur le VPS le 17/11/2026)
- `/proc` (mémoire, charge), `statvfs` (disques) : oui.
- `systemctl list-units --state=failed`, `systemctl show` : oui, sans root.
- Docker : oui, Charles est dans le groupe `docker` (socket `/var/run/docker.sock`).
- libvirt : oui, groupe `libvirt` (`virsh -c qemu:///system`).
- Températures : pas de capteur exposé sur le VPS (`/sys/class/hwmon` vide) ; sonde ignorée
  quand il n'y a aucune zone.
- Journaux système : non nécessaires pour la première version.

## Fonctionnement
- Un seul binaire `vigie`, service **systemd utilisateur** (droits de Charles : groupes docker et
  libvirt, aucun root). Synchrone, sans runtime asynchrone : une tournée par minute, les sondes
  réseau en parallèle sur des fils légers.
- Historique dans SQLite (`~/.local/share/vigie/historique.db`), un point toutes les 5 min,
  30 jours gardés.
- Tableau de bord web sur `127.0.0.1:8790`, derrière nginx (vigie.mawena.cloud), mot de passe
  en HTTP Basic (empreinte SHA-256 dans la configuration).
