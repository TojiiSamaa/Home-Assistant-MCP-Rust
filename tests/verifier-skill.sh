#!/bin/sh
# Échoue si un outil fixe de src/outils.rs manque dans le tableau « Outils fixes » de SKILL.md, ou s'il y est fantôme.
# Sans cargo : grep seulement. Lancer depuis la racine du dépôt : sh tests/verifier-skill.sh
set -eu
skill=skill/home-assistant-mcp-rust/SKILL.md
src=$(grep -o '^ *d("[a-z_]*"' src/outils.rs | sed 's/.*d("//; s/"//' | sort -u)
doc=$(sed -n '/outils-fixes:debut/,/outils-fixes:fin/p' "$skill" | grep -o '^| `[a-z_]*` |' | sed 's/^| `//; s/` |//' | sort -u)
manquants=$(printf '%s\n' "$src" | grep -vxF "$doc" || true)
fantomes=$(printf '%s\n' "$doc" | grep -vxF "$src" || true)
echo "source : $(printf '%s\n' "$src" | wc -l) outils fixes ; skill : $(printf '%s\n' "$doc" | wc -l)"
[ -z "$manquants" ] || echo "MANQUANTS dans SKILL.md : $manquants"
[ -z "$fantomes" ] || echo "FANTOMES dans SKILL.md : $fantomes"
[ -z "$manquants$fantomes" ] && echo OK
[ -z "$manquants$fantomes" ]
