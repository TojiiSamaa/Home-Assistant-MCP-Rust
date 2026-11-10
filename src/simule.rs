//! Instance Home Assistant SIMULÉE du mode test (`MCP_TEST_MODE=1` ou `--test`) : REST et commandes WebSocket
//! répondent en mémoire, depuis `test/fixtures.json` (catalogue de services, états, registres, tableaux de bord).
//! Aucun accès réseau ici (preuve A) ; les écritures modifient seulement cette mémoire et sont comptées.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Map, Value, json};

use crate::ha::Rep;
use crate::outils::commande_lecture;

const FIXTURES: &str = include_str!("../test/fixtures.json");

/// Réponse de l'approbateur simulé aux outils qui demandent une validation (réglé au démarrage, jamais par un outil).
/// Défaut : `Deny` (fermé). N'a aucun effet hors mode test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Approbateur {
    Approve,
    #[default]
    Deny,
    Timeout,
    Absent,
}

impl Approbateur {
    pub fn lire(v: &str) -> Result<Approbateur, String> {
        match v.trim().to_ascii_lowercase().as_str() {
            "approve" => Ok(Approbateur::Approve),
            "deny" => Ok(Approbateur::Deny),
            "timeout" => Ok(Approbateur::Timeout),
            "absent" => Ok(Approbateur::Absent),
            autre => Err(format!("MCP_TEST_APPROVER : approve, deny, timeout ou absent, pas « {autre} »")),
        }
    }
}

struct Etat {
    etats: Vec<Value>,
    tableaux: Vec<Value>,
    configs: HashMap<String, Value>,
    donnees: HashMap<String, Value>,
    automatismes: HashMap<String, Value>,
    flux: u64,
}

pub struct Simule {
    fx: Value,
    etat: Mutex<Etat>,
    ecritures: AtomicU64,
}

fn reussite(id: u64, r: Value) -> Value {
    json!({"id": id, "type": "result", "success": true, "result": r})
}

fn echec(id: u64, code: &str, message: &str) -> Value {
    json!({"id": id, "type": "result", "success": false, "error": {"code": code, "message": message}})
}

fn rep(statut: u16, corps: Value) -> Rep {
    Rep { statut, corps }
}

impl Simule {
    pub fn nouveau() -> Simule {
        let fx: Value = serde_json::from_str(FIXTURES).expect("test/fixtures.json invalide");
        let etat = Etat {
            etats: fx["states"].as_array().cloned().unwrap_or_default(),
            tableaux: fx["dashboards"].as_array().cloned().unwrap_or_default(),
            configs: HashMap::new(),
            donnees: HashMap::new(),
            automatismes: fx["automations"].as_object().into_iter().flatten().flat_map(|(d, l)| l.as_object().into_iter().flatten().map(move |(i, c)| (format!("{d}/{i}"), c.clone()))).collect(),
            flux: 0,
        };
        Simule { fx, etat: Mutex::new(etat), ecritures: AtomicU64::new(0) }
    }

    /// Nombre d'écritures simulées exécutées (services, suppressions, remplacements) depuis la création.
    pub fn ecritures(&self) -> u64 {
        self.ecritures.load(Ordering::SeqCst)
    }

    fn ecrit(&self) {
        self.ecritures.fetch_add(1, Ordering::SeqCst);
    }

    // ───────────── REST ─────────────

    pub fn rest(&self, methode: &str, chemin: &str, corps: Option<&Value>) -> Rep {
        let (route, requete) = chemin.split_once('?').unwrap_or((chemin, ""));
        let parties: Vec<&str> = route.trim_matches('/').split('/').collect();
        let mut e = self.etat.lock().unwrap_or_else(|x| x.into_inner());
        let lecture = methode == "GET" || route == "/api/template";
        if !lecture {
            self.ecrit();
        }
        match (methode, parties.as_slice()) {
            ("GET", ["api", "states"]) => rep(200, json!(e.etats)),
            ("GET", ["api", "states", id]) => match e.etats.iter().find(|s| s["entity_id"] == *id) {
                Some(s) => rep(200, s.clone()),
                None => rep(404, json!({"message": "Entity not found."})),
            },
            ("GET", ["api", "config"]) => rep(200, self.fx["config"].clone()),
            ("POST", ["api", "template"]) => rep(200, Value::String(format!("(simulé) {}", corps.and_then(|c| c["template"].as_str()).unwrap_or_default()))),
            ("GET", ["api", "history", "period", ..]) => {
                let ids = requete.split('&').find_map(|p| p.strip_prefix("filter_entity_id=")).unwrap_or_default();
                let l: Vec<Value> = ids.split(',').filter(|i| !i.is_empty()).map(|i| json!([{"entity_id": i, "state": e.etats.iter().find(|s| s["entity_id"] == i).map(|s| s["state"].clone()).unwrap_or(json!("unknown")), "last_changed": "2026-11-08T08:00:00+00:00"}])).collect();
                rep(200, json!(l))
            }
            ("GET", ["api", "logbook", ..]) => rep(200, json!([{"when": "2026-11-08T08:00:00+00:00", "name": "Porte d'entrée", "state": "fermée", "entity_id": "binary_sensor.porte_entree"}])),
            ("GET", ["api", "error_log"]) => rep(200, Value::String("(journal simulé)".into())),
            ("POST", ["api", "config", "config_entries", "flow"]) => {
                e.flux += 1;
                let h = corps.and_then(|c| c["handler"].as_str()).unwrap_or_default();
                if h == "shopping_list" {
                    rep(200, json!({"type": "create_entry", "flow_id": format!("flux-simule-{}", e.flux), "handler": h, "title": "Liste de courses (simulée)"}))
                } else {
                    rep(200, json!({"type": "form", "flow_id": format!("flux-simule-{}", e.flux), "handler": h, "step_id": "user", "data_schema": []}))
                }
            }
            ("POST", ["api", "config", "config_entries", "flow", id]) => rep(200, json!({"type": "create_entry", "flow_id": id, "title": "Entrée simulée"})),
            ("DELETE", ["api", "config", "config_entries", "flow", _]) => rep(200, json!({"message": "Flow aborted"})),
            ("GET", ["api", "config", "config_entries", "flow_handlers"]) => rep(200, self.fx["flow_handlers"].clone()),
            ("GET", ["api", "config", "config_entries", "entry"]) => rep(200, self.fx["config_entries"].clone()),
            (m, ["api", "config", d, "config", id]) if matches!(*d, "automation" | "script" | "scene") => {
                let cle = format!("{d}/{id}");
                match m {
                    "GET" => match e.automatismes.get(&cle) {
                        Some(c) => rep(200, c.clone()),
                        None => rep(404, json!({"message": "Resource not found"})),
                    },
                    "DELETE" => match e.automatismes.remove(&cle) {
                        Some(_) => rep(200, json!({"result": "ok"})),
                        None => rep(404, json!({"message": "Resource not found"})),
                    },
                    _ => {
                        e.automatismes.insert(cle, corps.cloned().unwrap_or(json!({})));
                        rep(200, json!({"result": "ok"}))
                    }
                }
            }
            // toute autre route : réponse neutre, rien n'est exécuté
            _ => rep(200, json!({"test_mode": true, "simule": format!("{methode} {route}")})),
        }
    }

    // ───────────── WebSocket ─────────────

    pub fn commande(&self, c: &Map<String, Value>) -> Value {
        self.commande_numerotee(c, c.get("id").and_then(Value::as_u64).unwrap_or(1))
    }

    pub fn commande_numerotee(&self, c: &Map<String, Value>, id: u64) -> Value {
        let t = c.get("type").and_then(Value::as_str).unwrap_or_default();
        let texte = |k: &str| c.get(k).and_then(Value::as_str);
        if !commande_lecture(t) {
            self.ecrit();
        }
        let mut e = self.etat.lock().unwrap_or_else(|x| x.into_inner());
        let ok = |r: Value| reussite(id, r);
        match t {
            "ping" => json!({"id": id, "type": "pong"}),
            "auth/current_user" => ok(json!({"name": "Utilisateur simulé", "is_admin": true})),
            "get_config" => ok(self.fx["config"].clone()),
            "get_services" => ok(self.fx["services"].clone()),
            "get_panels" => ok(self.fx["panels"].clone()),
            "manifest/list" => ok(json!([])),
            "search/related" => match (texte("item_type"), texte("item_id")) {
                (Some(_), Some(_)) => ok(json!({"automation": ["automation.lumiere_coucher"], "entity": ["light.salon"]})),
                _ => echec(id, "invalid_format", "item_type et item_id requis"),
            },
            "config/entity_registry/list" => ok(self.fx["entity_registry"].clone()),
            "config/device_registry/list" => ok(self.fx["device_registry"].clone()),
            "config/area_registry/list" => ok(self.fx["area_registry"].clone()),
            "config/floor_registry/list" => ok(self.fx["floor_registry"].clone()),
            "config/label_registry/list" => ok(self.fx["label_registry"].clone()),
            "config_entries/get" => ok(self.fx["config_entries"].clone()),
            "render_template" => ok(json!({"result": "(simulé)"})),
            "lovelace/dashboards/list" => ok(json!(e.tableaux)),
            "lovelace/dashboards/create" => match texte("url_path") {
                None => echec(id, "invalid_format", "url_path requis"),
                Some(u) => {
                    let mut t = json!({"id": u.replace('-', "_"), "url_path": u, "mode": "storage", "show_in_sidebar": true, "require_admin": false, "title": texte("title").unwrap_or(u)});
                    for k in ["icon", "show_in_sidebar", "require_admin"] {
                        if let Some(v) = c.get(k) {
                            t[k] = v.clone();
                        }
                    }
                    e.tableaux.push(t.clone());
                    ok(t)
                }
            },
            "lovelace/dashboards/update" => match e.tableaux.iter_mut().find(|d| d["id"].as_str() == texte("dashboard_id")) {
                None => echec(id, "not_found", "tableau de bord inconnu"),
                Some(d) => {
                    for k in ["title", "icon", "show_in_sidebar", "require_admin"] {
                        if let Some(v) = c.get(k) {
                            d[k] = v.clone();
                        }
                    }
                    ok(d.clone())
                }
            },
            "lovelace/dashboards/delete" => {
                let avant = e.tableaux.len();
                e.tableaux.retain(|d| d["id"].as_str() != texte("dashboard_id"));
                if e.tableaux.len() < avant { ok(Value::Null) } else { echec(id, "not_found", "tableau de bord inconnu") }
            }
            "lovelace/config" => ok(e.configs.get(texte("url_path").unwrap_or("")).cloned().unwrap_or(json!({"views": [{"title": "Accueil", "cards": []}]}))),
            "lovelace/config/save" => match c.get("config") {
                None => echec(id, "invalid_format", "config requise"),
                Some(cfg) => {
                    e.configs.insert(texte("url_path").unwrap_or("").to_string(), cfg.clone());
                    ok(Value::Null)
                }
            },
            "lovelace/config/delete" => {
                e.configs.remove(texte("url_path").unwrap_or(""));
                ok(Value::Null)
            }
            "frontend/get_user_data" => ok(json!({"value": e.donnees.get(texte("key").unwrap_or("")).cloned().unwrap_or(Value::Null)})),
            "frontend/set_user_data" => {
                e.donnees.insert(texte("key").unwrap_or("").to_string(), c.get("value").cloned().unwrap_or(Value::Null));
                ok(Value::Null)
            }
            "frontend/get_themes" => ok(self.fx["themes"].clone()),
            "call_service" => {
                let (d, s) = (texte("domain").unwrap_or_default(), texte("service").unwrap_or_default());
                let Some(desc) = self.fx["services"][d].get(s) else { return echec(id, "service_not_found", &format!("service inconnu : {d}.{s}")) };
                // l'effet visible : allumer / éteindre des entités
                if matches!(s, "turn_on" | "turn_off") {
                    let cibles = c.get("target").map(|t| &t["entity_id"]).cloned().unwrap_or(Value::Null);
                    let ids: Vec<String> = match cibles {
                        Value::String(x) => vec![x],
                        Value::Array(l) => l.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
                        _ => vec![],
                    };
                    for st in e.etats.iter_mut().filter(|st| st["entity_id"].as_str().is_some_and(|i| ids.iter().any(|x| x == i))) {
                        st["state"] = json!(if s == "turn_on" { "on" } else { "off" });
                    }
                }
                let reponse = if desc.get("response").is_some() { json!({"simule": true}) } else { Value::Null };
                ok(json!({"context": {"id": "SIMULE", "parent_id": null, "user_id": null}, "response": reponse, "simule": format!("{d}.{s} : rien d'exécuté pour de vrai")}))
            }
            t if t.starts_with("subscribe_") => ok(Value::Null),
            // toute autre commande : inconnue de cette instance simulée (comme sur un vrai serveur qui ne l'a pas)
            t => echec(id, "unknown_command", &format!("commande inconnue de l'instance simulée : {t}")),
        }
    }
}
