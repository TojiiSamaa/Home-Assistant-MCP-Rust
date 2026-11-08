//! Home Assistant MCP Rust (binaire `ha-mcp`) : serveur MCP (Model Context Protocol, JSON-RPC 2.0) pour Home
//! Assistant, toutes versions récentes, et maison. Rien n'est figé : services, registres, tableaux de bord et intégrations sont découverts à l'exécution ;
//! une capacité absente se détecte en ESSAYANT sa commande (réponse `unknown_command`), jamais en comparant des
//! numéros de version. Un outil MCP par service (`domaine__service`), plus des outils génériques (`outils.rs`).
//! Veille : abonnement à service_registered / service_removed / component_loaded (et resynchronisation périodique
//! pour un serveur qui ne les émet pas) → nouvelle liste d'outils et `notifications/tools/list_changed`.

pub mod ha;
pub mod http;
pub mod outils;
pub mod presets;
pub mod schema;

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::Duration;

use serde_json::{Map, Value, json};
use tokio::sync::watch;

use crate::ha::{Ha, Session};
use crate::schema::{Cible, Filtre};

/// Versions du protocole acceptées ; la première est proposée à un client qui en demande une autre.
pub const VERSIONS: [&str; 4] = ["2025-06-18", "2025-11-25", "2025-03-26", "2024-11-05"];
/// Taille au plus du texte rendu par un outil.
pub const TAILLE_MAX: usize = 100_000;
/// Resynchronisation complète même sans événement (un serveur qui n'émet pas service_registered).
pub const RESYNC: Duration = Duration::from_secs(30);
/// Événements qui changent la liste des services.
const EVENEMENTS: [&str; 3] = ["service_registered", "service_removed", "component_loaded"];

pub type Issue = Result<String, String>;

/// Commandes essayées à chaque découverte : `unknown_command` = capacité absente de ce serveur (les outils qui en
/// dépendent sont retirés). Toutes sans effet : `search/related`, `lovelace/dashboards/create` et
/// `lovelace/config/save` sans leurs champs requis répondent `invalid_format` s'ils existent (le schéma est validé
/// avant tout traitement).
pub fn sondes() -> Vec<Value> {
    vec![
        json!({"type": "search/related"}),
        json!({"type": "config/floor_registry/list"}),
        json!({"type": "config/label_registry/list"}),
        json!({"type": "lovelace/dashboards/list"}),
        // un serveur peut lire les tableaux de bord sans savoir en créer ni en enregistrer (maison 0.1.1)
        json!({"type": "lovelace/dashboards/create"}),
        json!({"type": "lovelace/config/save"}),
        json!({"type": "manifest/list", "integrations": ["homeassistant"]}),
        json!({"type": "config_entries/get", "domain": "ha_mcp_sonde"}),
        json!({"type": "frontend/get_user_data", "key": "ha_mcp_sonde"}),
        json!({"type": "frontend/get_themes"}),
    ]
}

#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Retire tous les outils qui écrivent (outils de service compris) et restreint `ws` et `rest` à la lecture.
    pub lecture_seule: bool,
    pub filtre: Filtre,
    /// Dossier de presets en plus de ceux embarqués (lecture, et écriture par preset_exporter).
    pub presets: Option<PathBuf>,
}

#[derive(Default)]
pub struct Etat {
    /// Réponse de `get_config` (version, composants chargés).
    pub config: Value,
    /// Réponse de `get_services`.
    pub services: Value,
    /// Commandes sondées que ce serveur ne connaît pas.
    pub absentes: BTreeSet<String>,
    /// La liste servie par tools/list.
    pub outils: Vec<Value>,
    pub cibles: HashMap<String, Cible>,
}

pub struct App {
    pub ha: Ha,
    pub opts: Options,
    pub etat: RwLock<Etat>,
    /// Incrémenté à chaque changement de la liste des outils (→ notifications/tools/list_changed).
    pub generation: watch::Sender<u64>,
    /// Une application ou annulation de preset à la fois : l'instantané d'annulation doit voir ce qu'il remplace.
    pub verrou_presets: tokio::sync::Mutex<()>,
}

pub fn erreur_rpc(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

pub fn tronquer(mut s: String) -> String {
    if s.len() > TAILLE_MAX {
        let total = s.len();
        let mut fin = TAILLE_MAX;
        while !s.is_char_boundary(fin) {
            fin -= 1;
        }
        s.truncate(fin);
        s.push_str(&format!("\n[… réponse tronquée : {total} octets au total, {TAILLE_MAX} affichés]"));
    }
    s
}

impl App {
    pub fn new(ha: Ha, opts: Options) -> App {
        let app = App { ha, opts, etat: RwLock::new(Etat::default()), generation: watch::channel(0).0, verrou_presets: Default::default() };
        app.poser(Value::Null, Value::Null, BTreeSet::new());
        app
    }

    /// Remplace l'état ; la génération avance seulement si la liste des outils change.
    fn poser(&self, config: Value, services: Value, absentes: BTreeSet<String>) {
        let mut outils: Vec<Value> = outils::generiques(&absentes, self.opts.lecture_seule).iter().map(outils::Def::json).collect();
        let (de_service, cibles) = schema::outils_services(&services, &self.opts.filtre);
        let cibles = if self.opts.lecture_seule { HashMap::new() } else { cibles };
        if !self.opts.lecture_seule {
            outils.extend(de_service);
        }
        let mut e = self.etat.write().unwrap_or_else(|e| e.into_inner());
        let change = e.outils != outils;
        *e = Etat { config, services, absentes, outils, cibles };
        drop(e);
        if change {
            self.generation.send_modify(|g| *g += 1);
        }
    }

    /// Découverte complète sur une connexion : get_config, get_services et les sondes de capacités.
    pub async fn redecouvrir(&self) -> Result<(), String> {
        let mut commandes = vec![json!({"type": "get_config"}), json!({"type": "get_services"})];
        commandes.extend(sondes());
        let r = self.ha.lot(commandes.clone()).await?;
        let config = ha::resultat(&r[0]).map_err(|e| format!("get_config : {e}"))?;
        let services = ha::resultat(&r[1]).map_err(|e| format!("get_services : {e}"))?;
        let absentes = commandes[2..].iter().zip(&r[2..]).filter(|(_, m)| ha::inconnue(m)).filter_map(|(c, _)| c["type"].as_str().map(String::from)).collect();
        self.poser(config, services, absentes);
        Ok(())
    }

    /// Une commande s'est révélée inconnue en cours d'usage : ses outils disparaissent de la liste.
    fn noter_absente(&self, commande: &str) {
        let (config, services, mut absentes) = {
            let e = self.etat.read().unwrap_or_else(|e| e.into_inner());
            if e.absentes.contains(commande) || !sondes().iter().any(|s| s["type"] == commande) {
                return;
            }
            (e.config.clone(), e.services.clone(), e.absentes.clone())
        };
        absentes.insert(commande.to_string());
        self.poser(config, services, absentes);
    }

    pub fn absente(&self, commande: &str) -> bool {
        self.etat.read().unwrap_or_else(|e| e.into_inner()).absentes.contains(commande)
    }

    /// Le `result` d'une commande WebSocket (connexion neuve), ou l'erreur en texte.
    pub async fn ws_resultat(&self, commande: Value) -> Result<Value, String> {
        let t = commande["type"].as_str().unwrap_or_default().to_string();
        let (m, _) = self.ha.ws(ha::objet(commande), Duration::ZERO).await?;
        if ha::inconnue(&m) {
            self.noter_absente(&t);
            return Err(format!("commande inconnue de ce serveur : {t}"));
        }
        ha::resultat(&m)
    }

    /// Veille sans fin : abonnement aux événements de services, reconnexion avec attente croissante (1 → 30 s).
    pub async fn veiller(&self) {
        let mut pause = 1;
        loop {
            let debut = tokio::time::Instant::now();
            if let Err(e) = self.veille().await {
                eprintln!("ha-mcp : veille interrompue ({e}), reconnexion dans {pause} s");
            }
            if debut.elapsed() > Duration::from_secs(60) {
                pause = 1;
            }
            tokio::time::sleep(Duration::from_secs(pause)).await;
            pause = (pause * 2).min(30);
        }
    }

    async fn veille(&self) -> Result<(), String> {
        let mut s = Session::ouvrir(&self.ha).await?;
        for ev in EVENEMENTS {
            ha::resultat(&s.commande(ha::objet(json!({"type": "subscribe_events", "event_type": ev}))).await?)?;
        }
        // après chaque (re)connexion : ce qui a pu changer pendant la coupure
        self.redecouvrir().await?;
        let mut resync = tokio::time::interval_at(tokio::time::Instant::now() + RESYNC, RESYNC);
        let mut echeance: Option<tokio::time::Instant> = None;
        loop {
            tokio::select! {
                m = s.suivant() => {
                    if m?["type"] == "event" && echeance.is_none() {
                        // une rafale d'événements (une intégration qui se charge) : une seule redécouverte
                        echeance = Some(tokio::time::Instant::now() + Duration::from_secs(1));
                    }
                }
                _ = async { tokio::time::sleep_until(echeance.unwrap_or_else(tokio::time::Instant::now)).await }, if echeance.is_some() => {
                    echeance = None;
                    if let Err(e) = self.redecouvrir().await {
                        eprintln!("ha-mcp : redécouverte impossible : {e}");
                    }
                }
                _ = resync.tick() => {
                    if let Err(e) = self.redecouvrir().await {
                        eprintln!("ha-mcp : resynchronisation impossible : {e}");
                    }
                }
            }
        }
    }

    /// Exécute un outil listé (`None` : outil inconnu ou retiré de la liste).
    pub async fn appeler(&self, nom: &str, a: Map<String, Value>) -> Option<Issue> {
        let (connu, cible) = {
            let e = self.etat.read().unwrap_or_else(|e| e.into_inner());
            (e.outils.iter().any(|o| o["name"] == nom), e.cibles.get(nom).cloned())
        };
        if !connu {
            return None;
        }
        match cible {
            Some(c) => Some(self.ws_resultat(schema::appel(&c, a)).await.map(|r| ha::texte(&r))),
            None => outils::executer(self, nom, a).await,
        }
    }

    /// Un message JSON-RPC → la réponse (`None` : notification ou réponse du client).
    pub async fn traiter(&self, m: &Value) -> Option<Value> {
        let id = m.get("id").cloned();
        // réponse envoyée par le client (result/error sans method) : ce serveur n'émet aucune requête
        if m.get("method").is_none() && (m.get("result").is_some() || m.get("error").is_some()) {
            return None;
        }
        let Some(methode) = m.get("method").and_then(Value::as_str).filter(|_| m["jsonrpc"] == "2.0") else {
            return Some(erreur_rpc(id.unwrap_or(Value::Null), -32600, "requête JSON-RPC 2.0 invalide"));
        };
        // notification (sans id) : jamais de réponse
        let id = id?;
        if !(id.is_string() || id.is_number()) {
            return Some(erreur_rpc(Value::Null, -32600, "id de requête invalide (texte ou nombre attendu, null interdit)"));
        }
        let resultat = match methode {
            "initialize" => {
                let demandee = m["params"]["protocolVersion"].as_str().unwrap_or_default();
                let version = VERSIONS.iter().find(|v| **v == demandee).unwrap_or(&VERSIONS[0]);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": true}},
                    "serverInfo": {"name": env!("CARGO_PKG_NAME"), "title": "Home Assistant MCP Rust", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": "Home Assistant : un outil par service (domaine__service, ex. light__turn_on avec target.entity_id), plus des outils génériques (etats, template, historique, tableaux, intégrations, automatisations, presets, ws et rest pour tout le reste). lister_outils donne la liste complète en markdown.",
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": self.etat.read().unwrap_or_else(|e| e.into_inner()).outils.clone()})),
            "tools/call" => self.appel(&m["params"]).await,
            _ => Err((-32601, format!("méthode inconnue : {methode}"))),
        };
        Some(match resultat {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, msg)) => erreur_rpc(id, code, &msg),
        })
    }

    async fn appel(&self, p: &Value) -> Result<Value, (i64, String)> {
        let nom = p["name"].as_str().ok_or((-32602, "params.name (texte) est requis".to_string()))?;
        let args = match p.get("arguments") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(o)) => o.clone(),
            Some(_) => return Err((-32602, "params.arguments doit être un objet".into())),
        };
        let issue = self.appeler(nom, args).await.ok_or((-32602, format!("outil inconnu : {nom}")))?;
        eprintln!("ha-mcp : outil {nom} → {}", if issue.is_ok() { "ok" } else { "erreur" });
        let (t, erreur) = match issue {
            Ok(t) => (t, false),
            Err(t) => (t, true),
        };
        Ok(json!({"content": [{"type": "text", "text": tronquer(t)}], "isError": erreur}))
    }
}

#[cfg(test)]
mod tests;
