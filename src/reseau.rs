//! LE point unique par lequel passe tout accès réseau SORTANT du serveur (REST et WebSocket vers Home Assistant).
//! Aucun autre fichier n'a le droit de nommer `ureq`, `tungstenite`, `TcpStream`, `UdpSocket`, `std::net` ou
//! `Command` : `tests/verifier-reseau.sh` (preuve A) le contrôle, et le test dynamique B le confirme.
//! Coupe-circuit : une fois le mode test activé (`activer_test`, sens unique, jamais désactivable), toute entrée ici
//! est refusée par `TEST_MODE_EGRESS_BLOCKED` et comptée.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use serde_json::Value;
use tokio::net::TcpStream;
pub use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub type Flux = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type Client = ureq::Agent;

static TEST: AtomicBool = AtomicBool::new(false);
static PANIQUE: AtomicBool = AtomicBool::new(false);
static COUPURES: AtomicU64 = AtomicU64::new(0);

/// Active le mode test pour tout le processus. Aucune fonction ne le désactive.
pub fn activer_test() {
    TEST.store(true, Ordering::SeqCst);
}

pub fn est_test() -> bool {
    TEST.load(Ordering::SeqCst)
}

/// Nombre d'accès réseau refusés depuis le démarrage.
pub fn coupures() -> u64 {
    COUPURES.load(Ordering::SeqCst)
}

/// Pour les tests (preuve A) : le transport réel panique dès qu'on l'atteint, quel que soit le mode.
#[doc(hidden)]
pub fn transport_qui_panique() {
    PANIQUE.store(true, Ordering::SeqCst);
}

fn garde(cible: &str) -> Result<(), String> {
    if PANIQUE.load(Ordering::SeqCst) {
        panic!("transport réel atteint ({cible}) alors qu'il ne devait jamais l'être");
    }
    if est_test() {
        COUPURES.fetch_add(1, Ordering::SeqCst);
        return Err(format!("TEST_MODE_EGRESS_BLOCKED : accès réseau refusé en mode test ({cible})"));
    }
    Ok(())
}

/// Le client HTTP réel. Jamais construit en mode test.
pub fn client(delai: Duration) -> Client {
    ureq::AgentBuilder::new().timeout(delai).build()
}

/// Une requête REST : statut et corps (JSON lu, sinon le texte brut en `Value::String`).
pub async fn http(client: &Client, methode: &str, url: &str, auth: &str, corps: Option<String>, quoi: &str) -> Result<(u16, Value), String> {
    garde(quoi)?;
    let (agent, url, auth, m, quoi) = (client.clone(), url.to_string(), auth.to_string(), methode.to_string(), quoi.to_string());
    // ureq est synchrone : hors des fils de l'exécuteur
    tokio::task::spawn_blocking(move || {
        let req = agent.request(&m, &url).set("Authorization", &auth);
        let rep = match corps {
            Some(c) => req.set("Content-Type", "application/json").send_string(&c),
            None => req.call(),
        };
        let rep = match rep {
            Ok(r) | Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(format!("échec de la requête {quoi} : {e}")),
        };
        let statut = rep.status();
        let brut = rep.into_string().map_err(|e| format!("lecture de la réponse {quoi} : {e}"))?;
        Ok((statut, serde_json::from_str(&brut).unwrap_or(Value::String(brut))))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ouvre la connexion WebSocket `url` (ws:// ou wss://).
pub async fn ws_ouvrir(url: &str, delai: Duration) -> Result<Flux, String> {
    garde(url)?;
    let (s, _) = tokio::time::timeout(delai, tokio_tungstenite::connect_async(url))
        .await
        .map_err(|_| "délai de connexion WebSocket dépassé".to_string())?
        .map_err(|e| format!("connexion WebSocket : {e}"))?;
    Ok(s)
}
