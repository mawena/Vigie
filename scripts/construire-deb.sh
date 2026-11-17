#!/usr/bin/env bash
# Construit dist/vigie_<version>_amd64.deb (binaire, service systemd utilisateur), les sommes SHA-256,
# puis passe lintian. Depuis la 0.3.1, la source APT vient du dépôt unique (Depends: mawena-depot).
# Usage : scripts/construire-deb.sh   (les tests se lancent à part : cargo test)
set -euo pipefail

RACINE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$RACINE"
export PATH="$HOME/.cargo/bin:$PATH"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
[ -n "$VERSION" ] || { echo "version introuvable dans Cargo.toml" >&2; exit 1; }
ARCH="$(dpkg --print-architecture)"
MAINTENEUR="mawena <57989130+mawena@users.noreply.github.com>"
DATE="$(date -R)"

nice -n 10 cargo build --release --locked -j 2

CONSTR="$(mktemp -d)"
trap 'rm -rf "$CONSTR"' EXIT
rm -rf dist && mkdir -p dist

P="$CONSTR/vigie"
install -d "$P/DEBIAN" "$P/usr/bin" "$P/usr/lib/systemd/user" "$P/usr/share/doc/vigie" \
  "$P/usr/share/lintian/overrides"
install -m 755 target/release/vigie "$P/usr/bin/"
install -m 644 deploiement/vigie.service "$P/usr/lib/systemd/user/"
install -m 644 exemples/vigie.toml "$P/usr/share/doc/vigie/vigie.toml.exemple"
[ ! -f deploiement/nginx-vigie.conf ] || install -m 644 deploiement/nginx-vigie.conf "$P/usr/share/doc/vigie/"

# Licence et journal Debian
{
  echo "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/"
  echo "Upstream-Name: Vigie"
  echo "Source: https://github.com/mawena/Vigie"
  echo
  echo "Files: *"
  echo "Copyright: 2026 mawena"
  echo "License: MIT"
  sed 's/^$/./; s/^/ /' LICENSE
  echo
  echo "Comment: le binaire contient des bibliothèques Rust (licences MIT, Apache-2.0 ou ISC,"
  echo " voir Cargo.lock) et SQLite (domaine public)."
} > "$P/usr/share/doc/vigie/copyright"
printf 'vigie (%s) stable; urgency=medium\n\n  * Version %s, voir le journal des versions :\n    https://github.com/mawena/Vigie/blob/main/CHANGELOG.md\n\n -- %s  %s\n' \
  "$VERSION" "$VERSION" "$MAINTENEUR" "$DATE" | gzip -9n > "$P/usr/share/doc/vigie/changelog.gz"

# Voulu : rm_conffile de l'ancienne source sans debhelper.
printf 'vigie: %s\n' "maintainer-script-should-not-use-dpkg-maintscript-helper" \
  > "$P/usr/share/lintian/overrides/vigie"

# Passage au dépôt unique (0.3.1) : retrait propre de l'ancienne source, qui était un conffile.
RM_CONFFILE='dpkg-maintscript-helper rm_conffile /etc/apt/sources.list.d/vigie.sources 0.3.1~ -- "$@"'
for s in preinst postrm; do
  printf '#!/bin/sh\nset -e\n%s\n' "$RM_CONFFILE" > "$P/DEBIAN/$s"
  chmod 755 "$P/DEBIAN/$s"
done

# Après installation : rappeler comment démarrer le service (unité utilisateur, sans root).
{ printf '#!/bin/sh\nset -e\n%s\n' "$RM_CONFFILE"; cat <<'EOF'
if [ "$1" = configure ] && [ -z "$2" ]; then
  echo "Vigie installé. Pour chaque utilisateur qui surveille la machine :"
  echo "  vigie config-exemple > ~/.config/vigie/vigie.toml   # puis l'adapter"
  echo "  systemctl --user enable --now vigie"
  echo "  sudo loginctl enable-linger \"\$USER\"                 # tourne sans session ouverte"
fi
exit 0
EOF
} > "$P/DEBIAN/postinst"
chmod 755 "$P/DEBIAN/postinst"

glibc="$(objdump -T target/release/vigie | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -V | tail -1)"
taille="$(du -sk --exclude=DEBIAN "$P" | cut -f1)"
cat > "$P/DEBIAN/control" <<EOF
Package: vigie
Version: $VERSION
Architecture: $ARCH
Maintainer: $MAINTENEUR
Installed-Size: $taille
Depends: libc6 (>= $glibc), libgcc-s1, mawena-depot
Suggests: docker.io, libvirt-clients
Section: admin
Priority: optional
Homepage: https://mawena.github.io/Vigie/
Description: surveillance légère d'un serveur, alertes sur le téléphone
 Vigie surveille un serveur sans root : disques, mémoire, charge,
 températures, unités systemd, conteneurs Docker, machines libvirt, sites
 HTTP(S), certificats TLS, DNS et âge des sauvegardes. Il prévient sur le
 téléphone (ntfy, Telegram ou e-mail) avec durée minimale, rappels et retour
 à la normale, garde un historique court dans SQLite et sert un petit
 tableau de bord web. Un seul binaire, une dizaine de Mo de mémoire.
EOF
(cd "$P" && find . -path ./DEBIAN -prune -o -type f -printf '%P\n' | sort | xargs -r md5sum > DEBIAN/md5sums)

dpkg-deb --root-owner-group -Zxz --build "$P" "dist/vigie_${VERSION}_${ARCH}.deb" >/dev/null
(cd dist && sha256sum ./*.deb | sed 's| \./| |' > SHA256SUMS)
lintian --fail-on error --suppress-tags no-manual-page "dist/vigie_${VERSION}_${ARCH}.deb"
echo "Paquet : dist/vigie_${VERSION}_${ARCH}.deb"
