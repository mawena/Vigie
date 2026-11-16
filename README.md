# Vigie

**Garde un œil sur le VPS (et mes machines) et prévient sur le téléphone avant que ça casse.**
Disque qui se remplit, service tombé, conteneur arrêté, certificat qui expire, sauvegarde ratée,
site qui ne répond plus. Écrit en Rust, léger, sans dépendance lourde (pas de Prometheus/Grafana).

> Statut : phase 0 (cadrage), aucun code encore. Voir [`ROADMAP.md`](ROADMAP.md).

## À quoi ça sert
- **Sondes** : disque, RAM, charge, services systemd, conteneurs Docker, machines libvirt,
  sites HTTP(S), certificats TLS, âge de la dernière sauvegarde.
- **Alertes** sur le téléphone (ntfy, Telegram ou e-mail), avec seuils et anti-répétition.
- **Tableau de bord web** léger (vigie.mawena.cloud), lisible sur téléphone.
- **Historique** court (quelques semaines) pour voir les tendances, par exemple la vitesse
  de remplissage du disque.
- Peut s'appuyer sur l'agent de [Ricochet](https://github.com/mawena/Ricochet) pour les autres machines.

## Installation
Paquet `.deb` et dépôt APT signé dès la première release.

## Utilisation
À venir (`vigie etat`, tableau de bord web).

## Configuration
Un fichier TOML : sondes, seuils, canaux d'alerte. Format à définir en phase 1.

## Structure
```
README.md   ROADMAP.md   CLAUDE.md
docs/       cadrage et choix techniques
```

## Licence
MIT.
