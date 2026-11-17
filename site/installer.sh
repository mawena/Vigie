#!/bin/sh
# Installe Vigie depuis le dépôt APT unique de mawena : https://apt.mawena.cloud/
#   curl -fsSL https://mawena.github.io/Vigie/installer.sh | sudo sh
# Il lance l'installateur du dépôt unique avec le paquet vigie ; c'est l'équivalent de
#   curl -fsSL https://apt.mawena.cloud/installer.sh | sudo sh -s -- vigie
# Ensuite, `sudo apt upgrade` apporte les nouvelles versions.
set -eu

INSTALLATEUR=https://apt.mawena.cloud/installer.sh

if [ "$(id -u)" -ne 0 ]; then
  echo "Vigie : à lancer en administrateur : curl -fsSL $INSTALLATEUR | sudo sh -s -- vigie" >&2
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

SCRIPT="$(mktemp)"
trap 'rm -f "$SCRIPT"' EXIT
if command -v curl >/dev/null 2>&1; then curl -fsSL "$INSTALLATEUR" -o "$SCRIPT"
elif command -v wget >/dev/null 2>&1; then wget -qO "$SCRIPT" "$INSTALLATEUR"
else echo "Vigie : il faut curl ou wget." >&2; exit 1
fi
sh "$SCRIPT" vigie
echo
echo "Vigie installé. En tant qu'utilisateur qui surveille la machine (pas root) :"
echo "  mkdir -p ~/.config/vigie && vigie config-exemple > ~/.config/vigie/vigie.toml"
echo "  vigie etat                              # vérifier les sondes"
echo "  systemctl --user enable --now vigie"
