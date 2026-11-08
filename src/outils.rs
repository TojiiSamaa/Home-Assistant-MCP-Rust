//! Outils génériques (présents sur tout serveur, sauf capacité absente) et listes en markdown (`lister_outils`,
//! section de la skill vérifiée par un test). Tout passe par les commandes et routes standard de Home Assistant.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::ha::{self, texte};
use crate::{App, Issue, presets};

pub const ATTENTE_MAX_MS: u64 = 20_000;
const LIMITE_ETATS: u64 = 200;
const HEURES: f64 = 24.0;
const DOMAINES_AUTOMATISMES: [&str; 3] = ["automation", "script", "scene"];
/// Registres : nom de l'outil → commande de liste.
const REGISTRES: [(&str, &str); 5] = [
    ("entites", "config/entity_registry/list"),
    ("appareils", "config/device_registry/list"),
    ("pieces", "config/area_registry/list"),
    ("etages", "config/floor_registry/list"),
    ("etiquettes", "config/label_registry/list"),
];
/// Champs d'un tableau de bord transmis tels quels à lovelace/dashboards/*.
const CHAMPS_PAGE: [&str; 4] = ["title", "icon", "show_in_sidebar", "require_admin"];
const LOVELACE: &str = "lovelace/dashboards/list";
const DONNEES: &str = "frontend/get_user_data";

/// Un outil générique.
pub struct Def {
    pub nom: &'static str,
    /// Ne modifie rien (gardé par --lecture-seule).
    pub lecture: bool,
    /// Commande sondée dont l'outil dépend : absent du serveur → outil retiré.
    pub requiert: Option<&'static str>,
    pub description: &'static str,
    pub schema: Value,
}

impl Def {
    pub fn json(&self) -> Value {
        json!({"name": self.nom, "description": self.description, "inputSchema": self.schema, "annotations": {"readOnlyHint": self.lecture}})
    }
}

/// Commande WebSocket permise par --lecture-seule.
pub fn commande_lecture(t: &str) -> bool {
    const EXACTES: [&str; 13] = [
        "ping", "render_template", "search/related", "history/history_during_period", "history/stream", "logbook/get_events", "logbook/event_stream", "lovelace/config", "lovelace/resources", "frontend/get_themes", "frontend/get_user_data", "frontend/get_translations", "auth/current_user",
    ];
    t.starts_with("get_") || t.starts_with("subscribe_") || t.ends_with("/list") || t.ends_with("/get") || t.ends_with("/info") || EXACTES.contains(&t)
}

fn obj(props: Value, requis: &[&str]) -> Value {
    let mut s = json!({"type": "object", "properties": props});
    if !requis.is_empty() {
        s["required"] = json!(requis);
    }
    s
}

fn page(url_requis: bool) -> Value {
    obj(
        json!({
            "url_path": {"type": "string", "description": "adresse du tableau de bord, avec un tiret (ex. salon-tablette)"},
            "title": {"type": "string"},
            "icon": {"type": "string", "description": "ex. mdi:sofa"},
            "show_in_sidebar": {"type": "boolean"},
            "require_admin": {"type": "boolean"},
            "config": {"type": "object", "description": "configuration Lovelace complète ({views: [...]}), enregistrée par lovelace/config/save"},
        }),
        if url_requis { &["url_path", "title"] } else { &[] },
    )
}

/// Les outils génériques de ce serveur : ceux dont la capacité est absente sont retirés, et seuls ceux de lecture
/// restent avec `lecture_seule`.
pub fn generiques(absentes: &BTreeSet<String>, lecture_seule: bool) -> Vec<Def> {
    let types_registres: Vec<&str> = REGISTRES.iter().filter(|(_, c)| !absentes.contains(*c)).map(|(t, _)| *t).collect();
    let domaine = json!({"type": "string", "enum": DOMAINES_AUTOMATISMES});
    let d = |nom, lecture, requiert, description, schema| Def { nom, lecture, requiert, description, schema };
    let l = vec![
        d("etats", true, None, "États des entités. Avec entity_id : l'état complet d'une entité. Sinon une liste abrégée (entity_id, state, nom, unité, last_changed), filtrable par domaine, pièce (id ou nom) et recherche (dans l'entity_id ou le nom).",
          obj(json!({
              "entity_id": {"type": "string"},
              "domaine": {"type": "string", "description": "ex. light"},
              "piece": {"type": "string", "description": "id ou nom de pièce (registres type pieces)"},
              "recherche": {"type": "string", "description": "sous-chaîne, sans casse"},
              "attributs": {"type": "boolean", "description": "états complets avec attributs (défaut : non)"},
              "limite": {"type": "integer", "minimum": 1, "description": "nombre d'états au plus (défaut 200)"},
          }), &[])),
        d("services", true, None, "Services du serveur (get_services). Sans domaine : la liste des services par domaine. Avec domaine : leur description complète (champs, sélecteurs, cible, réponse).",
          obj(json!({"domaine": {"type": "string"}}), &[])),
        d("appeler_service", false, None, "Appelle n'importe quel service (call_service), même s'il n'a pas encore son outil domaine__service.",
          obj(json!({
              "domain": {"type": "string"}, "service": {"type": "string"},
              "data": {"type": "object", "description": "champs du service"},
              "target": {"type": "object", "description": "entity_id / device_id / area_id / floor_id / label_id"},
              "return_response": {"type": "boolean", "description": "obligatoire pour un service qui renvoie une réponse"},
          }), &["domain", "service"])),
        d("template", true, None, "Rend un modèle Jinja de Home Assistant (POST /api/template), ex. « {{ states('sun.sun') }} » ou « {{ area_entities('salon') }} ».",
          obj(json!({"template": {"type": "string"}, "variables": {"type": "object"}}), &["template"])),
        d("historique", true, None, "Historique d'états (GET /api/history/period) d'une ou plusieurs entités : les `heures` dernières (défaut 24) ou de `debut` à `fin` (ISO 8601).",
          obj(json!({
              "entity_ids": {"anyOf": [{"type": "string"}, {"type": "array", "items": {"type": "string"}}]},
              "debut": {"type": "string", "description": "ISO 8601, ex. 2026-01-31T08:00:00+00:00"},
              "fin": {"type": "string", "description": "ISO 8601 (défaut : maintenant)"},
              "heures": {"type": "number", "description": "durée avant fin si debut est absent (défaut 24)"},
              "attributs": {"type": "boolean", "description": "garder les attributs (défaut : non, réponse minimale)"},
          }), &["entity_ids"])),
        d("journal", true, None, "Journal (GET /api/logbook) : les `heures` dernières (défaut 24) ou de `debut` à `fin`, pour une entité ou toutes.",
          obj(json!({"entity_id": {"type": "string"}, "debut": {"type": "string"}, "fin": {"type": "string"}, "heures": {"type": "number"}}), &[])),
        d("registres", true, None, "Un registre : entités, appareils, pièces, étages ou étiquettes (selon ce que le serveur connaît), filtrable par sous-chaîne.",
          obj(json!({"type": {"type": "string", "enum": types_registres}, "filtre": {"type": "string", "description": "sous-chaîne cherchée dans chaque élément, sans casse"}}), &["type"])),
        d("recherche_liee", true, Some("search/related"), "Ce qui est lié à un élément (search/related) : automatisations, scripts, scènes, appareils, entités, pièces…",
          obj(json!({"item_type": {"type": "string", "description": "area, automation, config_entry, device, entity, floor, group, label, scene, script…"}, "item_id": {"type": "string"}}), &["item_type", "item_id"])),
        d("integrations", true, None, "Intégrations : chargées (get_config), entrées de configuration (domaine, titre, état), et domaines qu'on peut ajouter par assistant (flow_handlers). Filtrable par sous-chaîne.",
          obj(json!({"filtre": {"type": "string"}}), &[])),
        d("flux_demarrer", false, None, "Démarre l'assistant d'ajout d'une intégration ou d'une entrée (helper) : POST /api/config/config_entries/flow {handler}. Renvoie flow_id et la première étape (formulaire, menu, ou create_entry si rien n'est demandé).",
          obj(json!({"handler": {"type": "string", "description": "domaine de l'intégration (integrations → disponibles)"}, "show_advanced_options": {"type": "boolean"}}), &["handler"])),
        d("flux_etape", false, None, "Répond à une étape d'un assistant : POST /api/config/config_entries/flow/{flow_id} avec les valeurs du formulaire (data_schema de l'étape), ou {\"next_step_id\": …} pour un menu.",
          obj(json!({"flow_id": {"type": "string"}, "data": {"type": "object"}}), &["flow_id"])),
        d("flux_annuler", false, None, "Abandonne un assistant en cours (DELETE /api/config/config_entries/flow/{flow_id}).",
          obj(json!({"flow_id": {"type": "string"}}), &["flow_id"])),
        d("tableaux_lister", true, Some(LOVELACE), "Tableaux de bord (lovelace/dashboards/list). Le tableau par défaut n'y figure pas : son url_path est absent (null).",
          obj(json!({}), &[])),
        d("tableau_lire", true, Some(LOVELACE), "Contenu (views, cards) d'un tableau de bord (lovelace/config). Sans url_path : le tableau par défaut.",
          obj(json!({"url_path": {"type": "string"}}), &[])),
        d("tableau_enregistrer", false, Some("lovelace/config/save"),"Remplace le contenu d'un tableau de bord (lovelace/config/save). Sans url_path : le tableau par défaut. Lire d'abord avec tableau_lire pour ne rien perdre.",
          obj(json!({"url_path": {"type": "string"}, "config": {"type": "object", "description": "{views: [...]}"}}), &["config"])),
        d("page_creer", false, Some("lovelace/dashboards/create"),"Crée un tableau de bord (lovelace/dashboards/create) et, si `config` est donnée, enregistre son contenu (lovelace/config/save).",
          page(true)),
        d("page_modifier", false, Some(LOVELACE), "Modifie un tableau de bord repéré par url_path : titre, icône, visibilité (lovelace/dashboards/update) et/ou contenu `config` (lovelace/config/save). Sans url_path : contenu du tableau par défaut.",
          page(false)),
        d("page_supprimer", false, Some(LOVELACE), "Supprime un tableau de bord repéré par url_path (lovelace/dashboards/delete).",
          obj(json!({"url_path": {"type": "string"}}), &["url_path"])),
        d("barre_regler", false, Some(DONNEES), "Règle la barre latérale de l'utilisateur du jeton (frontend/set_user_data, clé sidebar) : `ordre` = panneaux dans l'ordre voulu (ex. [\"lovelace\", \"energy\", \"salon-tablette\"]), `masques` = panneaux cachés. Un champ absent garde la valeur actuelle.",
          obj(json!({"ordre": {"type": "array", "items": {"type": "string"}}, "masques": {"type": "array", "items": {"type": "string"}}}), &[])),
        d("automatisme_lire", true, None, "Automatisations, scripts, scènes. Sans id : la liste (entity_id, id, nom, état ; un élément sans id vient du YAML et ne se modifie pas ici). Avec id : sa configuration (GET /api/config/<domaine>/config/<id>).",
          obj(json!({"domaine": domaine, "id": {"type": "string", "description": "id de configuration (automatisation, scène : attribut id ; script : la fin de l'entity_id)"}}), &["domaine"])),
        d("automatisme_ecrire", false, None, "Crée ou remplace une automatisation, un script ou une scène (POST /api/config/<domaine>/config/<id>) ; le serveur valide puis recharge. Nouvel id : n'importe quel texte unique (a-z, 0-9, _).",
          obj(json!({"domaine": domaine, "id": {"type": "string"}, "config": {"type": "object", "description": "ex. automatisation : {alias, triggers, conditions, actions, mode}"}}), &["domaine", "id", "config"])),
        d("automatisme_supprimer", false, None, "Supprime une automatisation, un script ou une scène créé par l'interface (DELETE /api/config/<domaine>/config/<id>).",
          obj(json!({"domaine": domaine, "id": {"type": "string"}}), &["domaine", "id"])),
        d("presets_lister", true, None, "Presets de préconfiguration (embarqués, et ceux du dossier --presets) : id, nom, description, paramètres déclarés avec leurs défauts, pages, thème, barre, cartes requises.",
          obj(json!({}), &[])),
        d("preset_appliquer", false, Some(DONNEES), "Applique un preset en COPIE avec les commandes standard : pages (lovelace/dashboards/*, lovelace/config/save), thème, barre latérale. Ce qui est remplacé est gardé (données de l'utilisateur, clé ha_mcp_presets, 10 dernières) pour preset_annuler. Les cartes requises (HACS) sont signalées, pas installées.",
          obj(json!({
              "preset": {"type": "string", "description": "id du preset (presets_lister)"},
              "parametres": {"type": "object", "description": "valeurs des paramètres déclarés ; un paramètre absent prend sa valeur par défaut"},
              "morceaux": {"type": "array", "items": {"type": "string", "enum": presets::MORCEAUX}, "description": "morceaux à appliquer (défaut : tous)"},
              "pour": {"type": "string", "enum": ["moi", "tous"], "description": "thème : l'utilisateur du jeton (défaut) ou le thème par défaut de tous (frontend.set_theme) ; la barre est toujours celle de l'utilisateur du jeton"},
          }), &["preset"])),
        d("preset_annuler", false, Some(DONNEES), "Annule la dernière application de preset : remet les pages, le thème et la barre latérale d'avant.",
          obj(json!({}), &[])),
        d("preset_exporter", false, Some(LOVELACE), "Fait un preset de la configuration actuelle (pages stockées, thème et barre de l'utilisateur du jeton). Renvoie son JSON ; l'écrit aussi dans le dossier --presets s'il est donné.",
          obj(json!({
              "preset": {"type": "string", "description": "id du nouveau preset : a-z, 0-9, - et _"},
              "nom": {"type": "string"}, "description": {"type": "string"},
              "pages": {"type": "array", "items": {"type": "string"}, "description": "url_path des pages à reprendre (défaut : toutes les pages stockées)"},
              "remplacer": {"type": "boolean", "description": "écraser un fichier de preset du même id"},
          }), &["preset"])),
        d("ws", true, None, "Envoie N'IMPORTE QUELLE commande WebSocket de Home Assistant (après authentification) et renvoie son résultat : arguments = le message sans id, `type` et ses champs (ex. {\"type\": \"config/entity_registry/list\"}). `attente_ms` (max 20000) : collecte des événements d'une commande subscribe_*. En --lecture-seule : commandes de lecture seulement (get_*, subscribe_*, */list, */get, */info…).",
          json!({"type": "object", "properties": {"type": {"type": "string"}, "attente_ms": {"type": "integer", "minimum": 0, "maximum": ATTENTE_MAX_MS}}, "required": ["type"], "additionalProperties": true})),
        d("rest", true, None, "N'IMPORTE QUELLE requête REST sous /api/ (ex. GET /api/config, GET /api/error_log). Réponse : statut + corps. En --lecture-seule : GET seulement.",
          obj(json!({"method": {"type": "string", "description": "GET (défaut), POST, PUT, PATCH, DELETE"}, "path": {"type": "string", "description": "chemin commençant par /api/, requête comprise"}, "body": {"description": "corps JSON"}}), &["path"])),
        d("lister_outils", true, None, "Liste complète, en markdown, des outils de cette instance : génériques et un par service, avec leurs paramètres.",
          obj(json!({}), &[])),
    ];
    l.into_iter().filter(|d| d.requiert.is_none_or(|r| !absentes.contains(r)) && (d.lecture || !lecture_seule)).collect()
}

// ───────────────────────── Aides ─────────────────────────

fn chaine<'a>(a: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn requis<'a>(a: &'a Map<String, Value>, k: &str) -> Result<&'a str, String> {
    chaine(a, k).ok_or(format!("« {k} » (texte) est requis"))
}

fn present<'a>(a: &'a Map<String, Value>, k: &str) -> Option<&'a Value> {
    a.get(k).filter(|v| !v.is_null())
}

/// Segment de chemin sûr (entity_id, flow_id, domaine, id) : pas de `/`, `?`, `#` ni `..`.
fn segment(s: &str, nom: &str) -> Result<String, String> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')) || s.contains("..") {
        return Err(format!("« {nom} » invalide : {s}"));
    }
    Ok(s.to_string())
}

fn maintenant() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Secondes depuis 1970 → ISO 8601 UTC (jours civils d'après H. Hinnant).
pub fn iso(t: i64) -> String {
    let (j, s) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    let z = j + 719_468;
    let ere = z.div_euclid(146_097);
    let jde = z - ere * 146_097;
    let ade = (jde - jde / 1460 + jde / 36_524 - jde / 146_096) / 365;
    let jda = jde - (365 * ade + ade / 4 - ade / 100);
    let mp = (5 * jda + 2) / 153;
    let jour = jda - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let an = ade + ere * 400 + i64::from(mois <= 2);
    format!("{an:04}-{mois:02}-{jour:02}T{:02}:{:02}:{:02}+00:00", s / 3600, s % 3600 / 60, s % 60)
}

/// Horodatage pour une adresse : celui donné (vérifié) ou `defaut` ; `+` encodé.
fn horodatage(v: Option<&str>, defaut: i64, nom: &str) -> Result<String, String> {
    let t = match v {
        Some(t) if t.len() <= 40 && t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '-' | '+')) => t.to_string(),
        Some(t) => return Err(format!("« {nom} » invalide (ISO 8601 attendu) : {t}")),
        None => iso(defaut),
    };
    Ok(t.replace('+', "%2B"))
}

fn periode(a: &Map<String, Value>) -> Result<(String, String), String> {
    let heures = a.get("heures").and_then(Value::as_f64).unwrap_or(HEURES).clamp(0.0, 24.0 * 366.0);
    let fin = maintenant();
    Ok((horodatage(chaine(a, "debut"), fin - (heures * 3600.0) as i64, "debut")?, horodatage(chaine(a, "fin"), fin, "fin")?))
}

async fn res(app: &App, commande: Value) -> Issue {
    app.ws_resultat(commande).await.map(|r| texte(&r))
}

fn domaine_automatisme(a: &Map<String, Value>) -> Result<&str, String> {
    let d = requis(a, "domaine")?;
    if DOMAINES_AUTOMATISMES.contains(&d) { Ok(d) } else { Err(format!("« domaine » : automation, script ou scene, pas « {d} »")) }
}

/// L'id du tableau de bord d'adresse `url`.
async fn id_page(app: &App, url: &str) -> Result<String, String> {
    let l = app.ws_resultat(json!({"type": LOVELACE})).await?;
    l.as_array().into_iter().flatten().find(|d| d["url_path"] == url).and_then(|d| d["id"].as_str().map(String::from)).ok_or(format!("aucun tableau de bord à l'adresse « {url} »"))
}

// ───────────────────────── Exécution ─────────────────────────

/// Exécute un outil générique (`None` : nom inconnu).
pub async fn executer(app: &App, nom: &str, a: Map<String, Value>) -> Option<Issue> {
    Some(match nom {
        "etats" => etats(app, &a).await,
        "services" => services(app, &a).await,
        "appeler_service" => appeler_service(app, &a).await,
        "template" => template(app, &a).await,
        "historique" => historique(app, &a).await,
        "journal" => journal(app, &a).await,
        "registres" => registres(app, &a).await,
        "recherche_liee" => match (requis(&a, "item_type"), requis(&a, "item_id")) {
            (Ok(t), Ok(i)) => res(app, json!({"type": "search/related", "item_type": t, "item_id": i})).await,
            (Err(e), _) | (_, Err(e)) => Err(e),
        },
        "integrations" => integrations(app, &a).await,
        "flux_demarrer" => match requis(&a, "handler").and_then(|h| segment(h, "handler")) {
            Ok(h) => {
                let avance = a.get("show_advanced_options") == Some(&json!(true));
                corps(app.ha.rest("POST", "/api/config/config_entries/flow", Some(&json!({"handler": h, "show_advanced_options": avance}))).await)
            }
            Err(e) => Err(e),
        },
        "flux_etape" => match requis(&a, "flow_id").and_then(|f| segment(f, "flow_id")) {
            Ok(f) => corps(app.ha.rest("POST", &format!("/api/config/config_entries/flow/{f}"), Some(present(&a, "data").unwrap_or(&json!({})))).await),
            Err(e) => Err(e),
        },
        "flux_annuler" => match requis(&a, "flow_id").and_then(|f| segment(f, "flow_id")) {
            Ok(f) => app.ha.rest("DELETE", &format!("/api/config/config_entries/flow/{f}"), None).await.and_then(|r| r.issue()),
            Err(e) => Err(e),
        },
        "tableaux_lister" => res(app, json!({"type": LOVELACE})).await,
        "tableau_lire" => res(app, json!({"type": "lovelace/config", "url_path": present(&a, "url_path")})).await,
        "tableau_enregistrer" => match present(&a, "config").filter(|c| c.is_object()) {
            Some(c) => res(app, json!({"type": "lovelace/config/save", "url_path": present(&a, "url_path"), "config": c})).await.map(|_| "enregistré".into()),
            None => Err("« config » (objet) est requis".into()),
        },
        "page_creer" => page_creer(app, &a).await,
        "page_modifier" => page_modifier(app, &a).await,
        "page_supprimer" => match requis(&a, "url_path") {
            Ok(url) => match id_page(app, url).await {
                Ok(id) => res(app, json!({"type": "lovelace/dashboards/delete", "dashboard_id": id})).await.map(|_| format!("tableau de bord « {url} » supprimé")),
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        },
        "barre_regler" => barre_regler(app, &a).await,
        "automatisme_lire" => automatisme_lire(app, &a).await,
        "automatisme_ecrire" => match (domaine_automatisme(&a), requis(&a, "id").and_then(|i| segment(i, "id")), present(&a, "config").filter(|c| c.is_object())) {
            (Ok(d), Ok(id), Some(c)) => corps(app.ha.rest("POST", &format!("/api/config/{d}/config/{id}"), Some(c)).await),
            (Err(e), _, _) | (_, Err(e), _) => Err(e),
            (_, _, None) => Err("« config » (objet) est requis".into()),
        },
        "automatisme_supprimer" => match (domaine_automatisme(&a), requis(&a, "id").and_then(|i| segment(i, "id"))) {
            (Ok(d), Ok(id)) => corps(app.ha.rest("DELETE", &format!("/api/config/{d}/config/{id}"), None).await),
            (Err(e), _) | (_, Err(e)) => Err(e),
        },
        "presets_lister" => Ok(texte(&presets::lister(app.opts.presets.as_deref()))),
        "preset_appliquer" => presets::appliquer(app, &a).await.map(|r| texte(&r)),
        "preset_annuler" => presets::annuler(app).await.map(|r| texte(&r)),
        "preset_exporter" => presets::exporter(app, &a).await.map(|r| texte(&r)),
        "ws" => ws(app, a).await,
        "rest" => rest(app, &a).await,
        "lister_outils" => Ok(markdown(app)),
        _ => return None,
    })
}

/// Corps d'une réponse REST (sans la ligne de statut), ou l'erreur avec son statut.
fn corps(r: Result<ha::Rep, String>) -> Issue {
    r?.valeur().map(|v| texte(&v))
}

async fn etats(app: &App, a: &Map<String, Value>) -> Issue {
    if let Some(e) = chaine(a, "entity_id") {
        return corps(app.ha.rest("GET", &format!("/api/states/{}", segment(e, "entity_id")?), None).await);
    }
    let mut l: Vec<Value> = app.ha.rest("GET", "/api/states", None).await?.valeur()?.as_array().cloned().unwrap_or_default();
    if let Some(d) = chaine(a, "domaine") {
        let p = format!("{d}.");
        l.retain(|e| e["entity_id"].as_str().is_some_and(|x| x.starts_with(&p)));
    }
    if let Some(r) = chaine(a, "recherche").map(str::to_lowercase) {
        l.retain(|e| [&e["entity_id"], &e["attributes"]["friendly_name"]].iter().any(|v| v.as_str().is_some_and(|x| x.to_lowercase().contains(&r))));
    }
    if let Some(p) = chaine(a, "piece") {
        let ids = entites_de_piece(app, p).await?;
        l.retain(|e| e["entity_id"].as_str().is_some_and(|x| ids.contains(x)));
    }
    l.sort_by(|x, y| x["entity_id"].as_str().cmp(&y["entity_id"].as_str()));
    let total = l.len();
    let limite = a.get("limite").and_then(Value::as_u64).unwrap_or(LIMITE_ETATS).max(1) as usize;
    let complets = a.get("attributs") == Some(&json!(true));
    let etats: Vec<Value> = l
        .into_iter()
        .take(limite)
        .map(|e| {
            if complets {
                return e;
            }
            let mut r = json!({"entity_id": e["entity_id"], "state": e["state"]});
            for (k, n) in [("friendly_name", "nom"), ("unit_of_measurement", "unite")] {
                if let Some(v) = e["attributes"].get(k) {
                    r[n] = v.clone();
                }
            }
            r["last_changed"] = e["last_changed"].clone();
            r
        })
        .collect();
    Ok(texte(&json!({"total": total, "affiches": etats.len(), "etats": etats})))
}

/// Entités d'une pièce (id ou nom) : la sienne, ou celle de leur appareil.
async fn entites_de_piece(app: &App, p: &str) -> Result<HashSet<String>, String> {
    let r = app.ha.lot(REGISTRES[..3].iter().map(|(_, c)| json!({"type": c})).collect()).await?;
    let (entites, appareils, pieces) = (ha::resultat(&r[0])?, ha::resultat(&r[1])?, ha::resultat(&r[2])?);
    let pl = p.to_lowercase();
    let zone = pieces
        .as_array()
        .into_iter()
        .flatten()
        .find(|z| z["area_id"] == p || z["name"].as_str().is_some_and(|n| n.to_lowercase() == pl))
        .and_then(|z| z["area_id"].as_str().map(String::from))
        .ok_or(format!("pièce inconnue : {p}"))?;
    let dans: HashSet<&str> = appareils.as_array().into_iter().flatten().filter(|d| d["area_id"] == zone.as_str()).filter_map(|d| d["id"].as_str()).collect();
    Ok(entites
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["area_id"] == zone.as_str() || (e["area_id"].is_null() && e["device_id"].as_str().is_some_and(|d| dans.contains(d))))
        .filter_map(|e| e["entity_id"].as_str().map(String::from))
        .collect())
}

async fn services(app: &App, a: &Map<String, Value>) -> Issue {
    let r = app.ws_resultat(json!({"type": "get_services"})).await?;
    match chaine(a, "domaine") {
        Some(d) => r.get(d).map(|s| texte(&json!({d: s}))).ok_or(format!("aucun service dans le domaine « {d} »")),
        None => {
            let resume: Map<String, Value> = r.as_object().into_iter().flatten().map(|(d, s)| (d.clone(), json!(s.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()))).collect();
            Ok(texte(&Value::Object(resume)))
        }
    }
}

async fn appeler_service(app: &App, a: &Map<String, Value>) -> Issue {
    let (d, s) = (segment(requis(a, "domain")?, "domain")?, segment(requis(a, "service")?, "service")?);
    let mut m = json!({"type": "call_service", "domain": d, "service": s});
    for (de, vers) in [("data", "service_data"), ("target", "target"), ("return_response", "return_response")] {
        if let Some(v) = present(a, de) {
            m[vers] = v.clone();
        }
    }
    res(app, m).await
}

async fn template(app: &App, a: &Map<String, Value>) -> Issue {
    let mut c = json!({"template": requis(a, "template")?});
    if let Some(v) = present(a, "variables") {
        c["variables"] = v.clone();
    }
    corps(app.ha.rest("POST", "/api/template", Some(&c)).await)
}

async fn historique(app: &App, a: &Map<String, Value>) -> Issue {
    let ids: Vec<String> = match present(a, "entity_ids") {
        Some(Value::String(s)) => s.split(',').map(|x| x.trim().to_string()).collect(),
        Some(Value::Array(l)) => l.iter().filter_map(Value::as_str).map(String::from).collect(),
        _ => vec![],
    };
    if ids.is_empty() {
        return Err("« entity_ids » (texte ou liste) est requis".into());
    }
    let ids = ids.iter().map(|i| segment(i, "entity_ids")).collect::<Result<Vec<_>, _>>()?;
    let (debut, fin) = periode(a)?;
    let mut chemin = format!("/api/history/period/{debut}?filter_entity_id={}&end_time={fin}", ids.join(","));
    if a.get("attributs") != Some(&json!(true)) {
        chemin.push_str("&minimal_response&no_attributes");
    }
    corps(app.ha.rest("GET", &chemin, None).await)
}

async fn journal(app: &App, a: &Map<String, Value>) -> Issue {
    let (debut, fin) = periode(a)?;
    let mut chemin = format!("/api/logbook/{debut}?end_time={fin}");
    if let Some(e) = chaine(a, "entity_id") {
        chemin.push_str(&format!("&entity={}", segment(e, "entity_id")?));
    }
    corps(app.ha.rest("GET", &chemin, None).await)
}

async fn registres(app: &App, a: &Map<String, Value>) -> Issue {
    let t = requis(a, "type")?;
    let (_, c) = REGISTRES.iter().find(|(n, _)| *n == t).ok_or(format!("« type » inconnu : {t} ({})", REGISTRES.map(|r| r.0).join(", ")))?;
    let r = app.ws_resultat(json!({"type": c})).await?;
    let Some(f) = chaine(a, "filtre").map(str::to_lowercase) else { return Ok(texte(&r)) };
    let l: Vec<&Value> = r.as_array().into_iter().flatten().filter(|x| x.to_string().to_lowercase().contains(&f)).collect();
    Ok(texte(&json!(l)))
}

async fn integrations(app: &App, a: &Map<String, Value>) -> Issue {
    let f = chaine(a, "filtre").unwrap_or_default().to_lowercase();
    let garde = |s: &str| s.to_lowercase().contains(&f);
    let config = app.ws_resultat(json!({"type": "get_config"})).await?;
    let mut chargees: Vec<&str> = config["components"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|c| !c.contains('.') && garde(c)).collect();
    chargees.sort_unstable();
    chargees.dedup();
    // config_entries/get (WebSocket) si le serveur l'a, sinon la route REST historique
    let brutes = match app.absente("config_entries/get") {
        false => app.ws_resultat(json!({"type": "config_entries/get"})).await,
        true => Err(String::new()),
    };
    let brutes = match brutes {
        Ok(v) => v,
        Err(_) => app.ha.rest("GET", "/api/config/config_entries/entry", None).await?.valeur()?,
    };
    let entrees: Vec<Value> = brutes
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| garde(e["domain"].as_str().unwrap_or_default()) || garde(e["title"].as_str().unwrap_or_default()))
        .map(|e| json!({"entry_id": e["entry_id"], "domain": e["domain"], "title": e["title"], "state": e["state"], "source": e["source"], "disabled_by": e["disabled_by"]}))
        .collect();
    let disponibles = match app.ha.rest("GET", "/api/config/config_entries/flow_handlers", None).await.and_then(ha::Rep::valeur) {
        Ok(v) => {
            let mut l: Vec<&str> = v.as_array().into_iter().flatten().filter_map(Value::as_str).filter(|h| garde(h)).collect();
            l.sort_unstable();
            json!(l)
        }
        Err(e) => json!(format!("indisponible : {e}")),
    };
    Ok(texte(&json!({"chargees": chargees, "entrees": entrees, "disponibles": disponibles, "ajouter": "flux_demarrer {handler} puis flux_etape jusqu'à create_entry"})))
}

async fn page_creer(app: &App, a: &Map<String, Value>) -> Issue {
    let url = requis(a, "url_path")?;
    let mut m = json!({"type": "lovelace/dashboards/create", "url_path": url});
    for k in CHAMPS_PAGE {
        if let Some(v) = present(a, k) {
            m[k] = v.clone();
        }
    }
    let cree = app.ws_resultat(m).await?;
    if let Some(c) = present(a, "config") {
        app.ws_resultat(json!({"type": "lovelace/config/save", "url_path": url, "config": c})).await?;
    }
    Ok(texte(&cree))
}

async fn page_modifier(app: &App, a: &Map<String, Value>) -> Issue {
    let url = chaine(a, "url_path");
    let mut faits = vec![];
    let mut m = json!({"type": "lovelace/dashboards/update"});
    for k in CHAMPS_PAGE {
        if let Some(v) = present(a, k) {
            m[k] = v.clone();
        }
    }
    if m.as_object().is_some_and(|o| o.len() > 1) {
        let url = url.ok_or("« url_path » est requis pour changer titre, icône ou visibilité")?;
        m["dashboard_id"] = json!(id_page(app, url).await?);
        faits.push(json!({"dashboards/update": app.ws_resultat(m).await?}));
    }
    if let Some(c) = present(a, "config") {
        app.ws_resultat(json!({"type": "lovelace/config/save", "url_path": url, "config": c})).await?;
        faits.push(json!({"config/save": "ok"}));
    }
    if faits.is_empty() {
        return Err("rien à modifier : donner title, icon, show_in_sidebar, require_admin ou config".into());
    }
    Ok(texte(&Value::Array(faits)))
}

async fn barre_regler(app: &App, a: &Map<String, Value>) -> Issue {
    let liste = |k: &str| match present(a, k) {
        None => Ok(None),
        Some(Value::Array(l)) if l.iter().all(Value::is_string) => Ok(Some(Value::Array(l.clone()))),
        Some(_) => Err(format!("« {k} » doit être une liste de textes")),
    };
    let (ordre, masques) = (liste("ordre")?, liste("masques")?);
    let actuel = app.ws_resultat(json!({"type": DONNEES, "key": "sidebar"})).await?;
    let actuel = &actuel["value"];
    let valeur = json!({
        "panelOrder": ordre.unwrap_or_else(|| actuel.get("panelOrder").cloned().unwrap_or(json!([]))),
        "hiddenPanels": masques.unwrap_or_else(|| actuel.get("hiddenPanels").cloned().unwrap_or(json!([]))),
    });
    app.ws_resultat(json!({"type": "frontend/set_user_data", "key": "sidebar", "value": valeur})).await?;
    Ok(texte(&valeur))
}

async fn automatisme_lire(app: &App, a: &Map<String, Value>) -> Issue {
    let d = domaine_automatisme(a)?;
    if let Some(id) = chaine(a, "id") {
        return corps(app.ha.rest("GET", &format!("/api/config/{d}/config/{}", segment(id, "id")?), None).await);
    }
    let p = format!("{d}.");
    let l: Vec<Value> = app
        .ha
        .rest("GET", "/api/states", None)
        .await?
        .valeur()?
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let eid = e["entity_id"].as_str()?.strip_prefix(&p)?;
            let id = if d == "script" { json!(eid) } else { e["attributes"]["id"].clone() };
            Some(json!({"entity_id": e["entity_id"], "id": id, "nom": e["attributes"]["friendly_name"], "etat": e["state"]}))
        })
        .collect();
    Ok(texte(&json!(l)))
}

async fn ws(app: &App, mut a: Map<String, Value>) -> Issue {
    let attente = a.remove("attente_ms").and_then(|v| v.as_u64()).unwrap_or(0).min(ATTENTE_MAX_MS);
    let t = requis(&a, "type")?.to_string();
    if app.opts.lecture_seule && !commande_lecture(&t) {
        return Err(format!("lecture seule : la commande « {t} » n'est pas permise"));
    }
    a.remove("id");
    let (r, evenements) = app.ha.ws(a, Duration::from_millis(attente)).await?;
    let resultat = ha::resultat(&r)?;
    Ok(if evenements.is_empty() { texte(&resultat) } else { texte(&json!({"resultat": resultat, "evenements": evenements})) })
}

async fn rest(app: &App, a: &Map<String, Value>) -> Issue {
    let methode = chaine(a, "method").unwrap_or("GET").to_ascii_uppercase();
    let chemin = requis(a, "path")?;
    if !chemin.starts_with("/api/") || chemin.contains("..") || chemin.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("« path » doit commencer par /api/ (sans espace ni ..)".into());
    }
    if !matches!(methode.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE") {
        return Err(format!("méthode HTTP refusée : {methode}"));
    }
    if app.opts.lecture_seule && methode != "GET" {
        return Err(format!("lecture seule : la méthode {methode} n'est pas permise"));
    }
    app.ha.rest(&methode, chemin, present(a, "body")).await?.issue()
}

// ───────────────────────── Markdown ─────────────────────────

/// Type d'une propriété en mots (pour les listes en markdown).
pub fn type_de(s: &Value) -> String {
    if let Some(e) = s.get("enum").and_then(Value::as_array) {
        return e.iter().map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string())).collect::<Vec<_>>().join(" / ");
    }
    if let Some(c) = s.get("const") {
        return format!("= {c}");
    }
    if let Some(l) = s.get("anyOf").and_then(Value::as_array) {
        return l.iter().map(type_de).collect::<Vec<_>>().join(" ou ");
    }
    let base = match s["type"].as_str() {
        Some("string") => "texte".to_string(),
        Some("boolean") => "booléen".into(),
        Some("integer") => "entier".into(),
        Some("number") => "nombre".into(),
        Some("object") => "objet".into(),
        Some("array") => match s.get("items") {
            Some(i) if i.as_object().is_some_and(|o| !o.is_empty()) => format!("liste de {}", type_de(i)),
            _ => "liste".into(),
        },
        _ => "toute valeur".into(),
    };
    match (s.get("minimum"), s.get("maximum")) {
        (Some(a), Some(b)) => format!("{base} {a}…{b}"),
        (Some(a), None) => format!("{base} ≥ {a}"),
        (None, Some(b)) => format!("{base} ≤ {b}"),
        _ => base,
    }
}

fn cellule(t: &str) -> String {
    t.replace('|', "\\|").replace('\n', " ")
}

fn requis_de(schema: &Value) -> Vec<&str> {
    schema["required"].as_array().into_iter().flatten().filter_map(Value::as_str).collect()
}

/// Section « outils génériques » de la skill : générée d'ici, comparée au fichier par un test.
pub fn doc_generiques() -> String {
    let mut s = String::new();
    for d in generiques(&BTreeSet::new(), false) {
        let mut tags = vec![if d.lecture { "lecture".to_string() } else { "écriture (retiré par --lecture-seule)".to_string() }];
        if let Some(r) = d.requiert {
            tags.push(format!("absent si le serveur ne connaît pas `{r}`"));
        }
        s.push_str(&format!("### `{}`\n\n*{}*\n\n{}\n\n", d.nom, tags.join(" · "), d.description));
        let props = d.schema["properties"].as_object().filter(|p| !p.is_empty());
        let Some(props) = props else {
            s.push_str("Sans paramètre.\n\n");
            continue;
        };
        let requis = requis_de(&d.schema);
        s.push_str("| paramètre | type | requis | description |\n|---|---|---|---|\n");
        for (k, p) in props {
            s.push_str(&format!("| `{k}` | {} | {} | {} |\n", cellule(&type_de(p)), if requis.contains(&k.as_str()) { "oui" } else { "" }, cellule(p["description"].as_str().unwrap_or_default())));
        }
        if d.schema["additionalProperties"] == true {
            s.push_str("| *(autres)* | toute valeur |  | les champs de la commande |\n");
        }
        s.push('\n');
    }
    s
}

/// Jusqu'au premier « . » suivi d'une espace qui n'est pas celui de « ex. ».
pub(crate) fn premiere_phrase(t: &str) -> &str {
    t.match_indices(". ").map(|(i, _)| i).find(|&i| !t[..i].ends_with(" ex") && !t[..i].ends_with("(ex")).map_or(t, |i| &t[..=i])
}

/// Tous les outils de l'instance connectée, en markdown (`lister_outils`, `ha-mcp lister`).
pub fn markdown(app: &App) -> String {
    let e = app.etat.read().unwrap_or_else(|e| e.into_inner());
    let (generiques, de_service): (Vec<&Value>, Vec<&Value>) = e.outils.iter().partition(|o| !e.cibles.contains_key(o["name"].as_str().unwrap_or_default()));
    let mut par_domaine: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for o in &de_service {
        par_domaine.entry(e.cibles[o["name"].as_str().unwrap_or_default()].domaine.as_str()).or_default().push(o);
    }
    let mut s = format!(
        "# Outils de Home Assistant MCP Rust\n\nServeur : Home Assistant {} · {} outils : {} génériques, {} de service ({} domaines).\n",
        e.config["version"].as_str().unwrap_or("(version inconnue)"),
        e.outils.len(),
        generiques.len(),
        de_service.len(),
        par_domaine.len()
    );
    if !e.absentes.is_empty() {
        s.push_str(&format!("\nCommandes absentes de ce serveur (outils qui en dépendent retirés) : {}.\n", e.absentes.iter().map(|a| format!("`{a}`")).collect::<Vec<_>>().join(", ")));
    }
    if app.opts.lecture_seule {
        s.push_str("\nMode lecture seule : outils d'écriture et outils de service retirés.\n");
    }
    s.push_str("\n## Outils génériques\n\n");
    for o in &generiques {
        s.push_str(&format!("- `{}` — {}\n", o["name"].as_str().unwrap_or_default(), premiere_phrase(o["description"].as_str().unwrap_or_default())));
    }
    if !par_domaine.is_empty() {
        s.push_str("\n## Outils de service\n\nArguments = champs du service ; `target` = cible (entity_id, device_id, area_id, floor_id, label_id).\n");
    }
    for (d, l) in &par_domaine {
        s.push_str(&format!("\n### {d} ({})\n\n", l.len()));
        for o in l {
            s.push_str(&format!("- `{}` — {}\n", o["name"].as_str().unwrap_or_default(), o["description"].as_str().unwrap_or_default().replace('\n', " ")));
            let schema = &o["inputSchema"];
            let requis = requis_de(schema);
            let params: Vec<String> = schema["properties"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(k, p)| format!("`{k}` ({}{})", if k == "target" { "cible".into() } else { type_de(p) }, if requis.contains(&k.as_str()) { ", requis" } else { "" }))
                .collect();
            if !params.is_empty() {
                s.push_str(&format!("  - paramètres : {}\n", params.join(", ")));
            }
        }
    }
    s
}
