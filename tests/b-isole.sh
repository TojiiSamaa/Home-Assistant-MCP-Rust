#!/bin/sh
# Preuve B sous isolement réseau du noyau : compile avec réseau, puis rejoue les tests mode test dans un conteneur
# `--network none` (aucune interface hors loopback). Nécessite docker. Sans docker : `unshare -n -r cargo test --offline`
# (non vérifié ici), sinon dire que seule la sentinelle locale prouve B.
# Usage depuis la racine : sh tests/b-isole.sh
set -eu
racine=$(pwd -W 2>/dev/null || pwd)
img=${RUST_IMG:-rust:1.88}
cache=${CARGO_CACHE:-cargo-cache-ha}
cible=${CARGO_CIBLE:-cargo-target-hamcp}
base="docker run --rm -v $racine:/w -v $cache:/usr/local/cargo/registry -v $cible:/t -e CARGO_TARGET_DIR=/t -w /w"
$base "$img" cargo test --no-run -j2 --locked
echo "--- isolement : docker --network none ---"
$base --network none "$img" sh -c 'ip -o link 2>/dev/null | cut -d: -f2 | tr -d " " | tr "\n" " "; echo; getent hosts example.com >/dev/null 2>&1 && echo "DNS joignable !" && exit 3; cargo test --offline --locked --test mode_test --test coupe_circuit -- --nocapture --test-threads=1'
