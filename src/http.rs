//! Transports HTTP : Streamable HTTP (`POST/GET/DELETE /mcp`, session `Mcp-Session-Id` créée à `initialize`,
//! facultative) et ancien transport SSE (`GET /sse` puis `POST /messages/{id}`). Jeton Bearer obligatoire, `Origin`
//! vérifié (DNS rebinding), `MCP-Protocol-Version` vérifié hors `initialize`. `GET /sante` répond `ok` sans jeton.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::{App, VERSIONS};

const SESSIONS_MAX: usize = 256;
const PING: Duration = Duration::from_secs(15);

pub struct Serveur {
    app: Arc<App>,
    jeton: String,
    /// Sessions ouvertes ; l'ancien transport SSE y garde son flux (les réponses partent dessus).
    sessions: Mutex<HashMap<String, Option<mpsc::Sender<String>>>>,
}

type Etat = State<Arc<Serveur>>;

fn texte(statut: StatusCode, corps: &str) -> Response {
    (statut, [(header::CONTENT_TYPE, "text/plain; charset=utf-8")], corps.to_string()).into_response()
}

fn entete<'a>(h: &'a HeaderMap, nom: &str) -> &'a str {
    h.get(nom).and_then(|v| v.to_str().ok()).unwrap_or_default()
}

/// Comparaison en temps constant (la longueur, elle, n'est pas secrète).
fn egaux(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `Origin` absent (client hors navigateur) ou même hôte que `Host`.
/// shortcut: derrière un mandataire qui réécrit `Host`, une page servie sous une autre adresse est refusée ; lire
/// `X-Forwarded-Host` si ce cas apparaît.
pub fn origine_admise(h: &HeaderMap) -> bool {
    let (origine, hote) = (entete(h, header::ORIGIN.as_str()), entete(h, header::HOST.as_str()));
    origine.is_empty() || (!hote.is_empty() && origine.split_once("://").is_some_and(|(_, a)| a.eq_ignore_ascii_case(hote)))
}

/// Refus si la version annoncée n'est pas servie ; en-tête absent : accepté (la spécification suppose 2025-03-26).
fn version_refusee(h: &HeaderMap) -> Option<Response> {
    let v = entete(h, "mcp-protocol-version");
    (!v.is_empty() && !VERSIONS.contains(&v)).then(|| texte(StatusCode::BAD_REQUEST, &format!("Unsupported MCP-Protocol-Version: {v} (supported: {})", VERSIONS.join(", "))))
}

impl Serveur {
    /// Refus si l'origine (403) ou le jeton (401) ne vont pas.
    fn refus(&self, h: &HeaderMap) -> Option<Response> {
        if !origine_admise(h) {
            return Some(texte(StatusCode::FORBIDDEN, "Forbidden: invalid Origin"));
        }
        let donne = entete(h, header::AUTHORIZATION.as_str()).strip_prefix("Bearer ").unwrap_or_default();
        (!egaux(donne.as_bytes(), self.jeton.as_bytes())).then(|| (StatusCode::UNAUTHORIZED, [(header::WWW_AUTHENTICATE, "Bearer")], "Unauthorized").into_response())
    }

    fn nouvelle_session(&self, flux: Option<mpsc::Sender<String>>) -> String {
        let mut octets = [0u8; 16];
        SystemRandom::new().fill(&mut octets).expect("aléa du système");
        let id: String = octets.iter().map(|o| format!("{o:02x}")).collect();
        let mut s = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if s.len() >= SESSIONS_MAX {
            // shortcut: évince une session quelconque (sans flux si possible), suffisant pour quelques clients
            if let Some(k) = s.iter().find(|(_, f)| f.is_none()).or_else(|| s.iter().next()).map(|(k, _)| k.clone()) {
                s.remove(&k);
            }
        }
        s.insert(id.clone(), flux);
        id
    }

    fn session(&self, id: &str) -> Option<Option<mpsc::Sender<String>>> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned()
    }

    fn fermer(&self, id: &str) -> bool {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner()).remove(id).is_some()
    }
}

fn evenement(m: &Value) -> String {
    format!("event: message\ndata: {m}\n\n")
}

fn flux_sse(rx: mpsc::Receiver<String>) -> Response {
    let corps = futures_util::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|m| (Ok::<_, std::convert::Infallible>(m), rx)) });
    ([(header::CONTENT_TYPE, "text/event-stream"), (header::CACHE_CONTROL, "no-cache")], Body::from_stream(corps)).into_response()
}

/// Tâche d'un flux SSE : `notifications/tools/list_changed` à chaque nouvelle liste, `: ping` régulier, fin quand
/// le client ferme ; la session de l'ancien transport disparaît avec son flux.
async fn veiller(s: Arc<Serveur>, tx: mpsc::Sender<String>, session: Option<String>) {
    let mut generation = s.app.generation.subscribe();
    let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + PING, PING);
    loop {
        let message = tokio::select! {
            _ = tx.closed() => break,
            r = generation.changed() => match r {
                Ok(()) => evenement(&json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"})),
                Err(_) => break,
            },
            _ = ping.tick() => ": ping\n\n".to_string(),
        };
        if tx.send(message).await.is_err() {
            break;
        }
    }
    if let Some(id) = session {
        s.fermer(&id);
    }
}

async fn post_mcp(State(s): Etat, h: HeaderMap, corps: String) -> Response {
    if let Some(r) = s.refus(&h) {
        return r;
    }
    let accept = entete(&h, header::ACCEPT.as_str());
    if !accept.is_empty() && !["application/json", "text/event-stream", "*/*"].iter().any(|t| accept.contains(t)) {
        return texte(StatusCode::NOT_ACCEPTABLE, "Client must accept application/json or text/event-stream");
    }
    if !entete(&h, header::CONTENT_TYPE.as_str()).starts_with("application/json") {
        return texte(StatusCode::UNSUPPORTED_MEDIA_TYPE, "Content-Type must be application/json");
    }
    let Ok(m) = serde_json::from_str::<Value>(&corps) else {
        let e = crate::erreur_rpc(Value::Null, -32700, "Parse error");
        return (StatusCode::BAD_REQUEST, [(header::CONTENT_TYPE, "application/json")], e.to_string()).into_response();
    };
    let initialisation = m["method"] == "initialize";
    // la version se négocie dans initialize : l'en-tête ne compte qu'après
    if !initialisation && let Some(r) = version_refusee(&h) {
        return r;
    }
    let sid = entete(&h, "mcp-session-id");
    if !sid.is_empty() && s.session(sid).is_none() {
        return texte(StatusCode::NOT_FOUND, "Session not found");
    }
    let Some(r) = s.app.traiter(&m).await else { return StatusCode::ACCEPTED.into_response() };
    let prefere_flux = match (accept.find("text/event-stream"), accept.find("application/json")) {
        (Some(a), Some(b)) => a < b,
        (Some(_), None) => true,
        _ => false,
    };
    let mut rep = if prefere_flux { ([(header::CONTENT_TYPE, "text/event-stream")], evenement(&r)).into_response() } else { ([(header::CONTENT_TYPE, "application/json")], r.to_string()).into_response() };
    if initialisation && r.get("result").is_some() {
        let id = s.nouvelle_session(None);
        rep.headers_mut().insert("mcp-session-id", id.parse().expect("hexadécimal ASCII"));
    }
    rep
}

/// Flux SSE des notifications (Streamable HTTP).
async fn get_mcp(State(s): Etat, h: HeaderMap) -> Response {
    if let Some(r) = s.refus(&h).or_else(|| version_refusee(&h)) {
        return r;
    }
    if !entete(&h, header::ACCEPT.as_str()).contains("text/event-stream") {
        return texte(StatusCode::NOT_ACCEPTABLE, "Client must accept text/event-stream");
    }
    let sid = entete(&h, "mcp-session-id");
    if !sid.is_empty() && s.session(sid).is_none() {
        return texte(StatusCode::NOT_FOUND, "Session not found");
    }
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(veiller(s, tx, None));
    flux_sse(rx)
}

async fn delete_mcp(State(s): Etat, h: HeaderMap) -> Response {
    if let Some(r) = s.refus(&h).or_else(|| version_refusee(&h)) {
        return r;
    }
    if s.fermer(entete(&h, "mcp-session-id")) { StatusCode::OK.into_response() } else { texte(StatusCode::NOT_FOUND, "Session not found") }
}

/// Ancien transport : `event: endpoint` puis les réponses en `event: message` sur ce flux.
async fn get_sse(State(s): Etat, h: HeaderMap) -> Response {
    if let Some(r) = s.refus(&h) {
        return r;
    }
    let (tx, rx) = mpsc::channel(64);
    let id = s.nouvelle_session(Some(tx.clone()));
    let _ = tx.try_send(format!("event: endpoint\ndata: /messages/{id}\n\n"));
    tokio::spawn(veiller(s, tx, Some(id)));
    flux_sse(rx)
}

async fn post_messages(State(s): Etat, h: HeaderMap, Path(sid): Path<String>, corps: String) -> Response {
    if let Some(r) = s.refus(&h) {
        return r;
    }
    let Some(Some(tx)) = s.session(&sid) else { return texte(StatusCode::NOT_FOUND, "Session not found") };
    let Ok(m) = serde_json::from_str::<Value>(&corps) else { return texte(StatusCode::BAD_REQUEST, "Could not parse message") };
    // la réponse part sur le flux : la requête HTTP est acquittée tout de suite
    tokio::spawn(async move {
        if let Some(r) = s.app.traiter(&m).await {
            let _ = tx.send(evenement(&r)).await;
        }
    });
    StatusCode::ACCEPTED.into_response()
}

pub fn routes(app: Arc<App>, jeton: String) -> Router {
    let s = Arc::new(Serveur { app, jeton, sessions: Mutex::default() });
    Router::new()
        .route("/mcp", post(post_mcp).get(get_mcp).delete(delete_mcp))
        .route("/sse", get(get_sse))
        .route("/messages/{id}", post(post_messages))
        .route("/sante", get(|| async { "ok" }))
        .with_state(s)
}

pub async fn servir(app: Arc<App>, ecoute: &str, jeton: String) -> Result<(), String> {
    let l = tokio::net::TcpListener::bind(ecoute).await.map_err(|e| format!("écoute sur {ecoute} : {e}"))?;
    eprintln!("ha-mcp : MCP sur http://{}/mcp (ancien SSE : /sse)", l.local_addr().map(|a| a.to_string()).unwrap_or_else(|_| ecoute.into()));
    axum::serve(l, routes(app, jeton)).await.map_err(|e| e.to_string())
}
