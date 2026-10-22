#!/usr/bin/env bash
# Régénère le corpus PDF à partir des sources HTML de sources/.
# À lancer depuis WSL ou Git Bash sous Windows : utilise Microsoft Edge en
# mode headless pour imprimer en PDF, puis l'outil encrypt/ (lopdf) pour le
# PDF chiffré.
set -euo pipefail
cd "$(dirname "$0")"

edge="${EDGE:-/mnt/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe}"
sources="$(cd sources && pwd)"
to_windows() { if command -v wslpath >/dev/null; then wslpath -w "$1"; else cygpath -w "$1"; fi; }
profile="$(to_windows "$(mktemp -d)")"

print() { # print <source.html> <cible.pdf>
  "$edge" --headless --disable-gpu --no-pdf-header-footer --user-data-dir="$profile" \
    --print-to-pdf="$(to_windows "$PWD")\\$2" "file:///$(to_windows "$sources/$1" | tr '\\' '/')"
}

print facture.html facture.pdf
print police.html police.pdf
print vide.html vide.pdf
# Le « scan » : une capture d'écran de la facture, imprimée comme image seule.
"$edge" --headless --disable-gpu --hide-scrollbars --window-size=1240,1754 \
  --user-data-dir="$profile" --screenshot="$(to_windows "$sources")\\facture.png" \
  "file:///$(to_windows "$sources/facture.html" | tr '\\' '/')"
print scan.html scan.pdf
rm -f sources/facture.png

(cd encrypt && cargo run --quiet --release)
echo "Corpus régénéré."
