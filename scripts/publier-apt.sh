#!/usr/bin/env bash
# Publie un ou plusieurs .deb dans le dépôt APT signé de Vigie (branche gh-pages,
# https://mawena.github.io/Vigie/apt), avec la page d'accueil du site (dossier site/ du dépôt, icône).
# Idempotent ; les anciennes versions restent dans pool/.
# Usage : scripts/publier-apt.sh dist/*.deb...   (sans paquet : met seulement le site à jour)
# Variables :
#   VIGIE_APT_CLE     empreinte de la clé de signature (défaut ci-dessous)
#   VIGIE_APT_PUSH=0  ne pas pousser gh-pages
#   VIGIE_APT_SORTIE  essai à blanc : écrit le dépôt (apt/, site) dans ce dossier, sans toucher à la branche gh-pages
set -euo pipefail

CLE="${VIGIE_APT_CLE:-3429F5DB3EDE539A8DBE1E1A4DA892AE987049CB}"
RACINE="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
for f in "$@"; do [ -f "$f" ] || { echo "introuvable : $f" >&2; exit 2; }; done
DEBS=()
for f in "$@"; do DEBS+=("$(readlink -f "$f")"); done

cd "$RACINE"
TMP="$(mktemp -d)"
SORTIE="${VIGIE_APT_SORTIE:-}"
if [ -n "$SORTIE" ]; then
  # Essai à blanc : un simple dossier, pas de git
  mkdir -p "$SORTIE"; WT="$(readlink -f "$SORTIE")"
  trap 'rm -rf "$TMP"' EXIT
else
  if [ ${#DEBS[@]} -eq 0 ] && ! git rev-parse -q --verify refs/remotes/origin/gh-pages >/dev/null; then
    git fetch -q origin gh-pages 2>/dev/null || true
    git rev-parse -q --verify refs/remotes/origin/gh-pages >/dev/null \
      || { echo "usage : $0 fichier.deb...   (la première publication demande au moins un paquet)" >&2; exit 2; }
  fi
  git fetch -q origin gh-pages 2>/dev/null || true
  WT="$TMP/pages"
  trap 'git -C "$RACINE" worktree remove --force "$WT" 2>/dev/null || true; rm -rf "$TMP"' EXIT
  if git rev-parse -q --verify refs/remotes/origin/gh-pages >/dev/null; then
    git worktree add -q -B gh-pages "$WT" origin/gh-pages
  else
    git worktree add -q --detach "$WT" HEAD
    git -C "$WT" checkout -q --orphan gh-pages
    git -C "$WT" rm -rq --cached . 2>/dev/null || true
    find "$WT" -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +
  fi
fi

APT="$WT/apt"
mkdir -p "$APT/pool/main"
touch "$WT/.nojekyll"
[ ${#DEBS[@]} -eq 0 ] || cp "${DEBS[@]}" "$APT/pool/main/"
cp "$RACINE/paquets/apt/vigie.gpg" "$APT/vigie.gpg"
cp "$RACINE/paquets/apt/vigie.sources" "$APT/vigie.sources"

# Site : page d'accueil, feuille de style, installateur
for f in "$RACINE"/site/*; do cp "$f" "$WT/"; done

# Idempotence (mode git) : les index ne sont régénérés et signés que si les paquets ou la clé ont changé (Date change à
# chaque fois) ; un changement du site seul est publié sans toucher aux index.
REGENERER=1
if [ -z "$SORTIE" ]; then
  git -C "$WT" add -A
  if git -C "$WT" diff --cached --quiet && [ -f "$APT/dists/stable/InRelease" ]; then
    echo "Dépôt APT et site déjà à jour."
    exit 0
  fi
  if git -C "$WT" diff --cached --quiet -- apt/pool apt/vigie.gpg && [ -f "$APT/dists/stable/InRelease" ]; then REGENERER=0; fi
fi
if [ "$REGENERER" = 1 ]; then
(
  cd "$APT"
  D=dists/stable/main
  mkdir -p "$D/binary-all" "$D/binary-amd64"
  # binary-amd64 : paquets amd64 et « all » ; binary-all : paquets « all » seuls.
  dpkg-scanpackages --multiversion --arch all pool/main /dev/null > "$D/binary-all/Packages" 2>/dev/null
  dpkg-scanpackages --multiversion --arch amd64 pool/main /dev/null > "$D/binary-amd64/Packages" 2>/dev/null
  for a in all amd64; do gzip -9nc "$D/binary-$a/Packages" > "$D/binary-$a/Packages.gz"; done
  rm -f dists/stable/Release dists/stable/InRelease dists/stable/Release.gpg
  apt-ftparchive \
    -o APT::FTPArchive::Release::Origin=Vigie \
    -o APT::FTPArchive::Release::Label=Vigie \
    -o APT::FTPArchive::Release::Suite=stable \
    -o APT::FTPArchive::Release::Codename=stable \
    -o APT::FTPArchive::Release::Components=main \
    -o APT::FTPArchive::Release::Architectures="all amd64" \
    -o APT::FTPArchive::Release::Description="Vigie, surveillance légère et alertes" \
    release dists/stable > "$TMP/Release"
  mv "$TMP/Release" dists/stable/Release
  gpg --batch --yes --local-user "$CLE" --clearsign -o dists/stable/InRelease dists/stable/Release
  gpg --batch --yes --local-user "$CLE" --armor --detach-sign -o dists/stable/Release.gpg dists/stable/Release
)
fi

# Vérification : apt-get update sans root, dans un dossier temporaire, doit lire le dépôt et voir les paquets publiés
verifier() {  # adresse du dépôt
  local V="$TMP/verif"
  rm -rf "$V"; mkdir -p "$V/state/lists/partial" "$V/cache/archives/partial" "$V/etc/preferences.d" "$V/etc/apt.conf.d"
  : > "$V/sources.list"; : > "$V/status"
  cat > "$V/vigie.sources" <<EOF
Types: deb
URIs: $1
Suites: stable
Components: main
Signed-By: $RACINE/paquets/apt/vigie.gpg
EOF
  mkdir -p "$V/sources.list.d"; mv "$V/vigie.sources" "$V/sources.list.d/"
  local OPT=(-o Dir::State="$V/state" -o Dir::State::status="$V/status" -o Dir::Cache="$V/cache"
    -o Dir::Etc::sourcelist="$V/sources.list" -o Dir::Etc::sourceparts="$V/sources.list.d"
    -o Dir::Etc::preferences="$V/etc/preferences" -o Dir::Etc::preferencesparts="$V/etc/preferences.d"
    -o Dir::Etc::parts="$V/etc/apt.conf.d" -o Debug::NoLocking=1 -o APT::Architecture=amd64)
  LC_ALL=C apt-get "${OPT[@]}" update 2>&1 | sed 's/^/  apt-get update : /'
  local VUES
  VUES="$(LC_ALL=C apt-cache "${OPT[@]}" policy vigie | sed 's/^/  /')"
  echo "$VUES"
  local DERNIERE
  DERNIERE="$(dpkg-scanpackages --multiversion "$APT/pool/main" /dev/null 2>/dev/null | awk '/^Version:/ {print $2}' | sort -V | tail -1)"
  if [ -n "$DERNIERE" ] && ! grep -q "Candidate: *$DERNIERE" <<<"$VUES"; then
    echo "Vérification : la version $DERNIERE n'est pas vue par apt." >&2; return 1
  fi
  echo "Vérification : apt voit bien vigie ${DERNIERE:-(aucun paquet)}."
}

if [ -n "$SORTIE" ]; then
  verifier "file://$APT"
  echo "Essai à blanc : dépôt écrit dans $WT"
  exit 0
fi

cd "$WT"
git add -A
if git diff --cached --quiet; then
  echo "Dépôt APT déjà à jour."
else
  VERS="$(for d in "${DEBS[@]}"; do basename "$d"; done | tr '\n' ' ')"
  if [ -n "${VERS% }" ]; then MESSAGE="feat(apt): publication de ${VERS% }"; else MESSAGE="docs(site): page d'accueil mise à jour"; fi
  git -c user.name="$(git -C "$RACINE" config user.name)" -c user.email="$(git -C "$RACINE" config user.email)" \
    commit -q -m "$MESSAGE"
fi
if [ "${VIGIE_APT_PUSH:-1}" = 1 ]; then
  git push -q origin gh-pages; echo "gh-pages poussée."
  # Pages met quelques minutes à servir la nouvelle version : on réessaie.
  ok=0
  for essai in 1 2 3 4 5 6 7 8 9 10; do
    if verifier "https://mawena.github.io/Vigie/apt"; then ok=1; break; fi
    echo "  GitHub Pages n'est pas encore à jour (essai $essai/10), nouvelle tentative dans 30 s…" >&2
    sleep 30
  done
  [ "$ok" = 1 ] || { echo "Vérification en ligne échouée (Pages activé ? voir paquets/apt/LISEZMOI)." >&2; exit 1; }
else
  verifier "file://$APT"
fi
