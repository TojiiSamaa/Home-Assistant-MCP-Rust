#!/bin/sh
# Test de mutation : on ajoute un appel réseau RÉEL hors du point unique, dans le chemin simulé de Ha::rest.
# Attendu : A (statique) ET B (sentinelle) échouent. Le fichier est restauré à la fin quoi qu'il arrive.
# Usage depuis la racine : sh tests/mutation.sh
set -u
racine=$(pwd -W 2>/dev/null || pwd)
f=src/ha.rs
cp "$f" "$f.orig"
trap 'cp "$f.orig" "$f"; rm -f "$f.orig"; echo "--- fichier restauré ---"' EXIT INT TERM
# insère juste après `if let Some(s) = &self.simule {` de rest() (première occurrence)
awk '/if let Some\(s\) = &self.simule \{/ && !d { print; print "            let _ = std::net::TcpStream::connect(self.base.trim_start_matches(\"http://\"));"; d=1; next } { print }' "$f.orig" > "$f"
grep -q 'std::net::TcpStream::connect' "$f" || { echo "mutation non appliquée"; exit 2; }
echo "=== mutation appliquée : connexion TCP réelle ajoutée dans ha.rs ==="
sh tests/verifier-reseau.sh; a=$?
echo "A statique -> code $a (attendu 1)"
img=${RUST_IMG:-rust:1.88}
docker run --rm -v "$racine:/w" -v "${CARGO_CACHE:-cargo-cache-ha}:/usr/local/cargo/registry" -v "${CARGO_CIBLE:-cargo-target-hamcp}:/t" -e CARGO_TARGET_DIR=/t -w /w "$img" cargo test -j2 --test mode_test b_tous 2>&1 | tail -15
b=$?
echo "B dynamique (voir ci-dessus) : succès attendu = échec du test"
[ "$a" -ne 0 ]
