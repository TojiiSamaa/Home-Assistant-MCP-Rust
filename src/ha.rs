//! Accès à Home Assistant (le vrai, toutes versions récentes, ou maison) : REST sous /api/ et WebSocket
//! /api/websocket, en clair (http/ws) ou chiffré (https/wss : rustls + racines webpki embarquées). Le jeton part
//! seulement dans l'en-tête `Authorization` et le message `auth` : jamais dans une réponse ni un journal.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Map, Value, json};

use crate::reseau::{self, Client, Flux, Message};
use crate::simule::Simule;

pub const DELAI: Duration = Duration::from_secs(30);

pub struct Ha {
    pub base: String,
    jeton: String,
    /// Client réseau réel : `None` en mode test (il n'est jamais construit).
    client: Option<Client>,
    /// Instance Home Assistant simulée : `Some` en mode test seulement.
    pub simule: Option<Arc<Simule>>,
}

/// Réponse REST : statut et corps (JSON lu, sinon le texte brut en `Value::String`).
pub struct Rep {
    pub statut: u16,
    pub corps: Value,
}

impl Rep {
    pub fn ok(&self) -> bool {
        self.statut < 400
    }

    pub fn texte(&self) -> String {
        format!("HTTP {}\n{}", self.statut, texte(&self.corps))
    }

    /// `Ok(texte)` si le statut est bon, sinon `Err(texte)`.
    pub fn issue(self) -> Result<String, String> {
        if self.ok() { Ok(self.texte()) } else { Err(self.texte()) }
    }

    /// Le corps si le statut est bon, sinon `Err(statut + corps)`.
    pub fn valeur(self) -> Result<Value, String> {
        if self.ok() { Ok(self.corps) } else { Err(self.texte()) }
    }
}

/// JSON lisible ; un texte brut est rendu tel quel.
pub fn texte(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        v => serde_json::to_string_pretty(v).unwrap_or_default(),
    }
}

pub fn objet(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// `result` d'un message de réponse WebSocket, ou l'erreur (code + message) en texte.
pub fn resultat(m: &Value) -> Result<Value, String> {
    if m["type"] == "pong" {
        return Ok(json!("pong"));
    }
    if m["success"] == true { Ok(m["result"].clone()) } else { Err(texte(&m["error"])) }
}

/// La commande n'existe pas sur ce serveur (capacité absente de cette version).
pub fn inconnue(m: &Value) -> bool {
    m["success"] == false && m["error"]["code"] == "unknown_command"
}

impl Ha {
    pub fn new(base: &str, jeton: String) -> Result<Ha, String> {
        let base = base.trim().trim_end_matches('/').to_string();
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            return Err(format!("adresse de Home Assistant invalide (http:// ou https:// attendu) : {base}"));
        }
        Ok(Ha { base, jeton, client: Some(reseau::client(DELAI)), simule: None })
    }

    /// Mode test : ni URL ni jeton ne sont lus ni utilisés, le client réseau n'est pas construit, tout répond depuis
    /// l'instance simulée et le coupe-circuit réseau est fermé pour le reste du processus.
    pub fn new_test(base: &str) -> Ha {
        reseau::activer_test();
        Ha { base: base.trim().to_string(), jeton: String::new(), client: None, simule: Some(Arc::new(Simule::nouveau())) }
    }

    pub fn est_test(&self) -> bool {
        self.simule.is_some()
    }

    /// Le client réseau réel existe-t-il ? Jamais en mode test.
    pub fn client_construit(&self) -> bool {
        self.client.is_some()
    }

    /// Une requête REST (`chemin` commence par /api/).
    pub async fn rest(&self, methode: &str, chemin: &str, corps: Option<&Value>) -> Result<Rep, String> {
        if let Some(s) = &self.simule {
            return Ok(s.rest(methode, chemin, corps));
        }
        let client = self.client.as_ref().ok_or("client réseau absent")?;
        let (url, auth, quoi) = (format!("{}{chemin}", self.base), format!("Bearer {}", self.jeton), format!("{methode} {chemin}"));
        let (statut, corps) = reseau::http(client, methode, &url, &auth, corps.map(Value::to_string), &quoi).await?;
        Ok(Rep { statut, corps })
    }

    /// Une commande sur une connexion neuve : le message de réponse (`result` ou `pong`), et les événements de
    /// l'abonnement reçus pendant `attente` (commandes subscribe_*). Fermer la connexion clôt l'abonnement.
    pub async fn ws(&self, commande: Map<String, Value>, attente: Duration) -> Result<(Value, Vec<Value>), String> {
        if let Some(s) = &self.simule {
            return Ok((s.commande(&commande), vec![]));
        }
        let echange = async {
            let mut s = Session::ouvrir(self).await?;
            let r = s.commande(commande).await?;
            let mut evenements = Vec::new();
            if !attente.is_zero() {
                let fin = tokio::time::Instant::now() + attente;
                while let Ok(Ok(m)) = tokio::time::timeout_at(fin, s.suivant()).await {
                    if m["id"] == r["id"] && m["type"] == "event" {
                        evenements.push(m["event"].clone());
                    }
                }
            }
            s.fermer().await;
            Ok((r, evenements))
        };
        tokio::time::timeout(DELAI + attente, echange).await.map_err(|_| format!("délai dépassé ({} s)", (DELAI + attente).as_secs()))?
    }

    /// Plusieurs commandes sur une même connexion, envoyées d'un coup : un message de réponse par commande.
    pub async fn lot(&self, commandes: Vec<Value>) -> Result<Vec<Value>, String> {
        if let Some(s) = &self.simule {
            return Ok(commandes.iter().enumerate().map(|(i, c)| s.commande_numerotee(&objet(c.clone()), i as u64 + 1)).collect());
        }
        let echange = async {
            let mut s = Session::ouvrir(self).await?;
            let mut ids = Vec::new();
            for c in commandes {
                ids.push(s.envoyer(objet(c)).await?);
            }
            let mut reponses = vec![Value::Null; ids.len()];
            let mut restantes = ids.len();
            while restantes > 0 {
                let m = s.suivant().await?;
                if let Some(i) = ids.iter().position(|id| m["id"] == *id).filter(|_| matches!(m["type"].as_str(), Some("result" | "pong"))) {
                    if reponses[i].is_null() {
                        restantes -= 1;
                    }
                    reponses[i] = m;
                }
            }
            s.fermer().await;
            Ok(reponses)
        };
        tokio::time::timeout(DELAI, echange).await.map_err(|_| format!("délai dépassé ({} s)", DELAI.as_secs()))?
    }
}

/// Une connexion WebSocket authentifiée. Les messages regroupés (tableau JSON) sont rendus un par un.
pub struct Session {
    s: Flux,
    file: VecDeque<Value>,
    id: u64,
}

impl Session {
    pub async fn ouvrir(ha: &Ha) -> Result<Session, String> {
        // http → ws, https → wss
        let url = format!("{}/api/websocket", ha.base.replacen("http", "ws", 1));
        let s = reseau::ws_ouvrir(&url, DELAI).await?;
        let mut session = Session { s, file: VecDeque::new(), id: 0 };
        let m = session.suivant().await?;
        if m["type"] != "auth_required" {
            return Err(format!("premier message inattendu : {m}"));
        }
        session.s.send(Message::Text(json!({"type": "auth", "access_token": ha.jeton}).to_string().into())).await.map_err(|e| e.to_string())?;
        let m = session.suivant().await?;
        if m["type"] != "auth_ok" {
            return Err(format!("authentification refusée : {}", m["message"].as_str().unwrap_or("(sans message)")));
        }
        Ok(session)
    }

    pub async fn suivant(&mut self) -> Result<Value, String> {
        loop {
            if let Some(v) = self.file.pop_front() {
                return Ok(v);
            }
            match self.s.next().await {
                Some(Ok(Message::Text(t))) => match serde_json::from_str::<Value>(t.as_str()) {
                    Ok(Value::Array(l)) => self.file.extend(l),
                    Ok(v) => self.file.push_back(v),
                    Err(e) => return Err(format!("message illisible : {e}")),
                },
                Some(Ok(Message::Close(_))) | None => return Err("connexion fermée par le serveur".into()),
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(format!("WebSocket : {e}")),
            }
        }
    }

    pub async fn envoyer(&mut self, mut commande: Map<String, Value>) -> Result<u64, String> {
        self.id += 1;
        commande.insert("id".into(), json!(self.id));
        self.s.send(Message::Text(Value::Object(commande).to_string().into())).await.map_err(|e| e.to_string())?;
        Ok(self.id)
    }

    /// Envoie une commande et rend son message de réponse (les messages d'autres ids sont sautés).
    pub async fn commande(&mut self, commande: Map<String, Value>) -> Result<Value, String> {
        let id = self.envoyer(commande).await?;
        loop {
            let m = self.suivant().await?;
            if m["id"] == id && matches!(m["type"].as_str(), Some("result" | "pong")) {
                return Ok(m);
            }
        }
    }

    pub async fn fermer(mut self) {
        let _ = self.s.close(None).await;
    }
}
