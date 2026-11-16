#!/bin/sh
# Installe Vigie depuis son dépôt APT signé : https://mawena.github.io/Vigie/
#   curl -fsSL https://mawena.github.io/Vigie/installer.sh | sudo sh
# Il fait exactement les trois étapes de la page : clé du dépôt, dépôt, installation. Ensuite,
# `sudo apt upgrade` apporte les nouvelles versions.
set -eu

DEPOT=https://mawena.github.io/Vigie/apt
CLE=/usr/share/keyrings/vigie.gpg
SOURCE=/etc/apt/sources.list.d/vigie.sources

if [ "$(id -u)" -ne 0 ]; then
  echo "Vigie : à lancer en administrateur : curl -fsSL https://mawena.github.io/Vigie/installer.sh | sudo sh" >&2
  exit 1
fi
if ! command -v apt-get >/dev/null 2>&1; then
  echo "Vigie : apt-get introuvable ; Vigie s'installe sur Ubuntu ou une distribution Debian." >&2
  exit 1
fi
if [ "$(dpkg --print-architecture)" != amd64 ]; then
  echo "Vigie : seuls les processeurs amd64 (x86-64) ont un paquet pour l'instant." >&2
  exit 1
fi

telecharger() {   # adresse fichier
  if command -v curl >/dev/null 2>&1; then curl -fsSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then wget -qO "$2" "$1"
  else echo "Vigie : il faut curl ou wget." >&2; exit 1
  fi
}

echo "1/3 Clé du dépôt -> $CLE"
telecharger "$DEPOT/vigie.gpg" "$CLE.tmp" && chmod 644 "$CLE.tmp" && mv "$CLE.tmp" "$CLE"
echo "2/3 Dépôt -> $SOURCE"
telecharger "$DEPOT/vigie.sources" "$SOURCE.tmp" && chmod 644 "$SOURCE.tmp" && mv "$SOURCE.tmp" "$SOURCE"
echo "3/3 Installation de vigie"
apt-get update
apt-get install -y vigie
echo
echo "Vigie installé. En tant qu'utilisateur qui surveille la machine (pas root) :"
echo "  mkdir -p ~/.config/vigie && vigie config-exemple > ~/.config/vigie/vigie.toml"
echo "  vigie etat                              # vérifier les sondes"
echo "  systemctl --user enable --now vigie"
