#!/bin/sh
# Preuve A (statique) : aucune primitive réseau hors du point unique src/reseau.rs (src/tests.rs = #[cfg(test)], absent du binaire livré).
# Sans cargo : grep seulement. Lancer depuis la racine : sh tests/verifier-reseau.sh
set -eu
interdit='ureq|tungstenite|TcpStream|UdpSocket|std::net|reqwest|hyper|connect_async|Command::new|curl|wget|tokio::net::'
hits=$(grep -rnE "$interdit" src --include='*.rs' | grep -v '^src/reseau.rs' | grep -v '^src/tests.rs' | grep -vE 'tokio::net::TcpListener::bind' || true)
n=$(ls src/*.rs | grep -vc -e 'reseau.rs' -e 'tests.rs')
if [ -n "$hits" ]; then
  echo "ECHEC A : primitive réseau hors src/reseau.rs :"
  echo "$hits"
  exit 1
fi
echo "OK A : $n fichiers source scannés, 0 primitive réseau hors src/reseau.rs (écoute entrante de http.rs seule tolérée)"
