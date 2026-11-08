//! Tests sans Home Assistant : un faux serveur (REST et WebSocket sur un même port) répond comme HA, avec des
//! commandes qu'on déclare inconnues pour vérifier la détection des capacités absentes.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use crate::ha::Ha;
use crate::schema::Filtre;
use crate::{App, Options, VERSIONS, outils};

const JETON: &str = "jeton-de-test";

#[derive(Default)]
struct Faux {
    services: Value,
    inconnues: Vec<&'static str>,
    appels: Vec<Value>,
}

type Partage = Arc<Mutex<Faux>>;

fn services_de_base() -> Value {
    json!({
        "light": {
            "turn_on": {"name": "Allumer", "description": "Allume une lumière.", "target": {"entity": [{"domain": ["light"]}]},
                "fields": {"brightness": {"selector": {"number": {"min": 0, "max": 255}}, "name": "Luminosité"}}},
            "turn_off": {"target": {"entity": [{"domain": ["light"]}]}, "fields": {}},
        },
        "weather": {"get_forecasts": {"fields": {"type": {"required": true, "selector": {"select": {"options": ["daily", "hourly"]}}}}, "target": {}, "response": {"optional": false}}},
    })
}

async fn faux_ha(services: Value, inconnues: Vec<&'static str>) -> (String, Partage) {
    let f: Partage = Arc::new(Mutex::new(Faux { services, inconnues, ..Default::default() }));
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let g = f.clone();
    tokio::spawn(async move {
        while let Ok((s, _)) = l.accept().await {
            let g = g.clone();
            tokio::spawn(async move {
                let _ = connexion(s, g).await;
            });
        }
    });
    (base, f)
}

async fn connexion(s: TcpStream, f: Partage) -> Result<(), String> {
    let mut debut = [0u8; 64];
    let n = s.peek(&mut debut).await.map_err(|e| e.to_string())?;
    if !String::from_utf8_lossy(&debut[..n]).contains("/api/websocket") {
        return rest(s).await;
    }
    let mut ws = tokio_tungstenite::accept_async(s).await.map_err(|e| e.to_string())?;
    let envoyer = |v: Value| Message::Text(v.to_string().into());
    ws.send(envoyer(json!({"type": "auth_required", "ha_version": "faux"}))).await.map_err(|e| e.to_string())?;
    while let Some(Ok(m)) = ws.next().await {
        let Message::Text(t) = m else { continue };
        let m: Value = serde_json::from_str(t.as_str()).map_err(|e| e.to_string())?;
        if m["type"] == "auth" {
            let ok = m["access_token"] == JETON;
            ws.send(envoyer(if ok { json!({"type": "auth_ok"}) } else { json!({"type": "auth_invalid", "message": "Invalid access token"}) })).await.map_err(|e| e.to_string())?;
            if !ok {
                return Ok(());
            }
            continue;
        }
        let r = reponse(&f, &m);
        // comme HA : des messages regroupés dans un tableau
        let r = if m["type"] == "get_config" { json!([r]) } else { r };
        ws.send(envoyer(r)).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn reponse(f: &Partage, m: &Value) -> Value {
    let mut f = f.lock().unwrap();
    let t = m["type"].as_str().unwrap_or_default();
    let ok = |r: Value| json!({"id": m["id"], "type": "result", "success": true, "result": r});
    let ko = |code: &str| json!({"id": m["id"], "type": "result", "success": false, "error": {"code": code, "message": code}});
    if f.inconnues.contains(&t) {
        return ko("unknown_command");
    }
    match t {
        "ping" => json!({"id": m["id"], "type": "pong"}),
        "get_config" => ok(json!({"version": "faux", "components": ["light", "weather", "light.demo"]})),
        "get_services" => ok(f.services.clone()),
        "search/related" if m.get("item_type").is_none() => ko("invalid_format"),
        "call_service" => {
            f.appels.push(m.clone());
            ok(json!({"context": {"id": "c"}}))
        }
        _ => ok(Value::Null),
    }
}

/// REST minimal : /api/states et /api/template (rend le modèle tel quel), 404 sinon.
async fn rest(mut s: TcpStream) -> Result<(), String> {
    let mut brut = vec![];
    let mut morceau = [0u8; 4096];
    let fin_entetes = loop {
        let n = s.read(&mut morceau).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(());
        }
        brut.extend_from_slice(&morceau[..n]);
        if let Some(i) = brut.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let entetes = String::from_utf8_lossy(&brut[..fin_entetes]).to_string();
    let longueur: usize = entetes.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
    while brut.len() < fin_entetes + longueur {
        let n = s.read(&mut morceau).await.map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        brut.extend_from_slice(&morceau[..n]);
    }
    let corps: Value = serde_json::from_slice(&brut[fin_entetes..]).unwrap_or(Value::Null);
    let ligne = entetes.lines().next().unwrap_or_default().to_string();
    let autorise = entetes.lines().any(|l| l.eq_ignore_ascii_case(&format!("authorization: Bearer {JETON}")));
    let (statut, rep) = match ligne.split(' ').take(2).collect::<Vec<_>>()[..] {
        _ if !autorise => (401, "401: Unauthorized".to_string()),
        ["GET", "/api/states"] => (200, json!([{"entity_id": "light.salon", "state": "on", "attributes": {"friendly_name": "Salon"}}, {"entity_id": "sun.sun", "state": "above_horizon", "attributes": {}}]).to_string()),
        ["POST", "/api/template"] => (200, corps["template"].as_str().unwrap_or_default().to_string()),
        _ => (404, json!({"message": "Not found"}).to_string()),
    };
    let t = format!("HTTP/1.1 {statut} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{rep}", rep.len());
    s.write_all(t.as_bytes()).await.map_err(|e| e.to_string())
}

fn app(base: &str, opts: Options) -> App {
    App::new(Ha::new(base, JETON.into()).unwrap(), opts)
}

fn noms(app: &App) -> Vec<String> {
    app.etat.read().unwrap().outils.iter().map(|o| o["name"].as_str().unwrap().to_string()).collect()
}

async fn appel(app: &App, nom: &str, args: Value) -> Value {
    app.traiter(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": nom, "arguments": args}})).await.unwrap()["result"].clone()
}

#[tokio::test]
async fn decouverte_et_capacites_absentes() {
    let (base, _) = faux_ha(services_de_base(), vec!["lovelace/dashboards/list", "lovelace/dashboards/create", "config/floor_registry/list", "search/related"]).await;
    let a = app(&base, Options::default());
    let g0 = *a.generation.borrow();
    a.redecouvrir().await.unwrap();
    assert!(*a.generation.borrow() > g0);
    assert_eq!(a.etat.read().unwrap().absentes, BTreeSet::from(["config/floor_registry/list".to_string(), "lovelace/dashboards/create".into(), "lovelace/dashboards/list".into(), "search/related".into()]));
    let n = noms(&a);
    for absent in ["tableaux_lister", "tableau_lire", "page_creer", "preset_exporter", "recherche_liee"] {
        assert!(!n.contains(&absent.to_string()), "{absent} devrait être retiré");
    }
    for present in ["etats", "appeler_service", "barre_regler", "ws", "light__turn_on", "light__turn_off", "weather__get_forecasts"] {
        assert!(n.contains(&present.to_string()), "{present} manque");
    }
    let registres = a.etat.read().unwrap().outils.iter().find(|o| o["name"] == "registres").unwrap()["inputSchema"]["properties"]["type"]["enum"].clone();
    assert_eq!(registres, json!(["entites", "appareils", "pieces", "etiquettes"]));
    // search/related existe ailleurs (invalid_format sans ses champs) : pas pris pour absent
    let (base, _) = faux_ha(services_de_base(), vec![]).await;
    let b = app(&base, Options::default());
    b.redecouvrir().await.unwrap();
    assert!(b.etat.read().unwrap().absentes.is_empty());
    assert!(noms(&b).contains(&"recherche_liee".to_string()));
    // lire sans savoir créer ni enregistrer : seuls page_creer et tableau_enregistrer disparaissent
    let (base, _) = faux_ha(services_de_base(), vec!["lovelace/dashboards/create", "lovelace/config/save"]).await;
    let c = app(&base, Options::default());
    c.redecouvrir().await.unwrap();
    let n = noms(&c);
    assert!(!n.contains(&"page_creer".to_string()) && !n.contains(&"tableau_enregistrer".to_string()));
    assert!(n.contains(&"tableaux_lister".to_string()) && n.contains(&"tableau_lire".to_string()) && n.contains(&"page_modifier".to_string()));
}

#[test]
fn premiere_phrase_sans_couper_ex() {
    use crate::outils::premiere_phrase;
    assert_eq!(premiere_phrase("Rend un modèle, ex. « {{ 1 }} ». Suite."), "Rend un modèle, ex. « {{ 1 }} ».");
    assert_eq!(premiere_phrase("Règle (ex. [a]) la barre. Suite."), "Règle (ex. [a]) la barre.");
    assert_eq!(premiere_phrase("Sans point"), "Sans point");
}

#[tokio::test]
async fn outil_de_service_et_generiques() {
    let (base, f) = faux_ha(services_de_base(), vec![]).await;
    let a = app(&base, Options::default());
    a.redecouvrir().await.unwrap();
    let r = appel(&a, "light__turn_on", json!({"target": {"entity_id": "light.salon"}, "brightness": 10})).await;
    assert_eq!(r["isError"], false, "{r}");
    let r = appel(&a, "weather__get_forecasts", json!({"type": "daily", "target": {"entity_id": "weather.maison"}})).await;
    assert_eq!(r["isError"], false, "{r}");
    let appels = f.lock().unwrap().appels.clone();
    assert_eq!(appels[0]["domain"], "light");
    assert_eq!(appels[0]["service"], "turn_on");
    assert_eq!(appels[0]["target"], json!({"entity_id": "light.salon"}));
    assert_eq!(appels[0]["service_data"], json!({"brightness": 10}));
    assert_eq!(appels[1]["return_response"], true);
    assert_eq!(appels[1]["service_data"], json!({"type": "daily"}));
    // génériques : REST et WebSocket
    let r = appel(&a, "etats", json!({"domaine": "light"})).await;
    let etats: Value = serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(etats["total"], 1);
    assert_eq!(etats["etats"][0]["nom"], "Salon");
    assert_eq!(appel(&a, "template", json!({"template": "{{ 1 + 1 }}"})).await["content"][0]["text"], "{{ 1 + 1 }}");
    let r = appel(&a, "integrations", json!({})).await;
    assert!(r["content"][0]["text"].as_str().unwrap().contains("\"weather\""), "{r}");
    let r = appel(&a, "rest", json!({"path": "/etc/passwd"})).await;
    assert_eq!(r["isError"], true);
    // outil absent de la liste
    let r = a.traiter(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "switch__toggle"}})).await.unwrap();
    assert_eq!(r["error"]["code"], -32602);
    // markdown de l'instance
    let md = outils::markdown(&a);
    assert!(md.contains("Home Assistant faux") && md.contains("### light (2)") && md.contains("`brightness` (nombre 0…255)"), "{md}");
}

#[tokio::test]
async fn nouvelle_liste_quand_les_services_changent() {
    let (base, f) = faux_ha(services_de_base(), vec![]).await;
    let a = app(&base, Options { filtre: Filtre { sans_domaines: vec!["weather".into()], ..Default::default() }, ..Default::default() });
    a.redecouvrir().await.unwrap();
    let rx = a.generation.subscribe();
    a.redecouvrir().await.unwrap();
    assert!(!rx.has_changed().unwrap(), "même liste : pas de list_changed");
    f.lock().unwrap().services["switch"] = json!({"toggle": {"target": {}, "fields": {}}});
    a.redecouvrir().await.unwrap();
    assert!(rx.has_changed().unwrap());
    let n = noms(&a);
    assert!(n.contains(&"switch__toggle".to_string()) && !n.iter().any(|x| x.starts_with("weather__")));
}

#[tokio::test]
async fn lecture_seule() {
    let (base, f) = faux_ha(services_de_base(), vec![]).await;
    let a = app(&base, Options { lecture_seule: true, ..Default::default() });
    a.redecouvrir().await.unwrap();
    {
        let e = a.etat.read().unwrap();
        assert!(e.cibles.is_empty());
        assert!(e.outils.iter().all(|o| o["annotations"]["readOnlyHint"] == true), "outil d'écriture en lecture seule");
    }
    let r = appel(&a, "ws", json!({"type": "call_service", "domain": "light", "service": "turn_on"})).await;
    assert_eq!(r["isError"], true);
    assert_eq!(appel(&a, "rest", json!({"method": "POST", "path": "/api/services/light/turn_on"})).await["isError"], true);
    assert_eq!(appel(&a, "ws", json!({"type": "config/area_registry/list"})).await["isError"], false);
    assert!(f.lock().unwrap().appels.is_empty());
    for t in ["get_states", "config/entity_registry/list", "config_entries/get", "subscribe_events", "render_template"] {
        assert!(outils::commande_lecture(t), "{t}");
    }
    for t in ["call_service", "config/entity_registry/update", "lovelace/config/save", "frontend/set_user_data", "fire_event", "execute_script"] {
        assert!(!outils::commande_lecture(t), "{t}");
    }
}

#[tokio::test]
async fn mauvais_jeton() {
    let (base, _) = faux_ha(services_de_base(), vec![]).await;
    let a = App::new(Ha::new(&base, "faux-jeton".into()).unwrap(), Options::default());
    let e = a.redecouvrir().await.unwrap_err();
    assert!(e.contains("authentification refusée") && !e.contains("faux-jeton"), "{e}");
    assert!(Ha::new("ftp://x", String::new()).is_err());
}

#[tokio::test]
async fn protocole() {
    // aucune connexion : le serveur ne joint jamais cette adresse dans ce test
    let a = app("http://127.0.0.1:9", Options::default());
    let r = |m: Value| {
        let a = &a;
        async move { a.traiter(&m).await }
    };
    let init = |v: &str| json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": v, "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}});
    let i = r(init("2024-11-05")).await.unwrap();
    assert_eq!(i["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(i["result"]["capabilities"]["tools"]["listChanged"], true);
    assert_eq!(i["result"]["serverInfo"]["name"], "home-assistant-mcp-rust");
    assert_eq!(r(init("1999-01-01")).await.unwrap()["result"]["protocolVersion"], VERSIONS[0]);
    assert_eq!(r(json!({"jsonrpc": "2.0", "id": "a", "method": "ping"})).await.unwrap(), json!({"jsonrpc": "2.0", "id": "a", "result": {}}));
    assert_eq!(r(json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"})).await.unwrap()["error"]["code"], -32601);
    assert_eq!(r(json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {}})).await.unwrap()["error"]["code"], -32602);
    assert_eq!(r(json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "etats", "arguments": 3}})).await.unwrap()["error"]["code"], -32602);
    assert_eq!(r(json!({"id": 6, "method": "ping"})).await.unwrap()["error"]["code"], -32600);
    assert_eq!(r(json!({"jsonrpc": "2.0", "id": null, "method": "ping"})).await.unwrap()["error"]["code"], -32600);
    assert_eq!(r(json!([{"jsonrpc": "2.0", "id": 7, "method": "ping"}])).await.unwrap()["error"]["code"], -32600);
    assert!(r(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).await.is_none());
    assert!(r(json!({"jsonrpc": "2.0", "id": 8, "result": {}})).await.is_none());
    let l = r(json!({"jsonrpc": "2.0", "id": 9, "method": "tools/list"})).await.unwrap();
    let noms: Vec<&str> = l["result"]["tools"].as_array().unwrap().iter().map(|o| o["name"].as_str().unwrap()).collect();
    assert!(noms.contains(&"etats") && noms.contains(&"lister_outils"));
    // un outil en erreur : isError, pas une erreur JSON-RPC ; le jeton n'apparaît pas
    let e = appel(&a, "etats", json!({})).await;
    assert_eq!(e["isError"], true);
    assert!(!e.to_string().contains(JETON));
    assert!(crate::tronquer("é".repeat(60_000)).contains("tronquée"));
}

/// Requête HTTP brute sur le transport (ureq est synchrone : hors de l'exécuteur).
async fn http(methode: &'static str, url: String, entetes: Vec<(&'static str, String)>, corps: Option<Value>) -> (u16, String, Option<String>) {
    tokio::task::spawn_blocking(move || {
        let mut r = ureq::request(methode, &url);
        for (k, v) in &entetes {
            r = r.set(k, v);
        }
        let rep = match corps {
            Some(c) => r.send_string(&c.to_string()),
            None => r.call(),
        };
        let rep = match rep {
            Ok(r) | Err(ureq::Error::Status(_, r)) => r,
            Err(e) => panic!("{e}"),
        };
        let sid = rep.header("mcp-session-id").map(String::from);
        (rep.status(), rep.into_string().unwrap_or_default(), sid)
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn transport_http() {
    let a = Arc::new(app("http://127.0.0.1:9", Options::default()));
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, crate::http::routes(a, "secret-serveur".into())).await });
    let url = format!("{base}/mcp");
    let auth = ("Authorization", "Bearer secret-serveur".to_string());
    let json_ct = ("Content-Type", "application/json".to_string());
    let accept = ("Accept", "application/json, text/event-stream".to_string());
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}});
    assert_eq!(http("GET", format!("{base}/sante"), vec![], None).await.1, "ok");
    assert_eq!(http("POST", url.clone(), vec![json_ct.clone()], Some(init.clone())).await.0, 401);
    assert_eq!(http("POST", url.clone(), vec![("Authorization", "Bearer autre".into()), json_ct.clone()], Some(init.clone())).await.0, 401);
    assert_eq!(http("POST", url.clone(), vec![auth.clone(), json_ct.clone(), ("Origin", "http://ailleurs.example".into())], Some(init.clone())).await.0, 403);
    let (statut, corps, sid) = http("POST", url.clone(), vec![auth.clone(), json_ct.clone(), accept.clone()], Some(init.clone())).await;
    assert_eq!(statut, 200, "{corps}");
    assert_eq!(serde_json::from_str::<Value>(&corps).unwrap()["result"]["protocolVersion"], "2025-06-18");
    let sid = sid.expect("Mcp-Session-Id");
    let avec_session = |mut v: Vec<(&'static str, String)>| {
        v.push(("Mcp-Session-Id", sid.clone()));
        v
    };
    let ping = json!({"jsonrpc": "2.0", "id": 2, "method": "ping"});
    assert_eq!(http("POST", url.clone(), avec_session(vec![auth.clone(), json_ct.clone(), ("MCP-Protocol-Version", "2025-06-18".into())]), Some(ping.clone())).await.0, 200);
    assert_eq!(http("POST", url.clone(), avec_session(vec![auth.clone(), json_ct.clone(), ("MCP-Protocol-Version", "1999-01-01".into())]), Some(ping.clone())).await.0, 400);
    assert_eq!(http("POST", url.clone(), vec![auth.clone(), json_ct.clone(), ("Mcp-Session-Id", "inconnue".into())], Some(ping.clone())).await.0, 404);
    assert_eq!(http("POST", url.clone(), vec![auth.clone(), ("Content-Type", "text/plain".into())], Some(ping.clone())).await.0, 415);
    assert_eq!(http("POST", url.clone(), avec_session(vec![auth.clone(), json_ct.clone()]), Some(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))).await.0, 202);
    let (statut, corps, _) = http("POST", url.clone(), vec![auth.clone(), ("Content-Type", "application/json".into()), ("Accept", "text/event-stream".into())], Some(ping.clone())).await;
    assert!(statut == 200 && corps.starts_with("event: message\ndata: "), "{corps}");
    assert_eq!(http("DELETE", url.clone(), avec_session(vec![auth.clone()]), None).await.0, 200);
    assert_eq!(http("DELETE", url.clone(), avec_session(vec![auth.clone()]), None).await.0, 404);
}

#[test]
fn dates_iso() {
    assert_eq!(outils::iso(0), "1970-01-01T00:00:00+00:00");
    assert_eq!(outils::iso(1_700_000_000), "2023-11-14T22:13:20+00:00");
    assert_eq!(outils::iso(951_782_400), "2000-02-29T00:00:00+00:00");
}

#[test]
fn noms_et_schemas_des_generiques() {
    let tous = outils::generiques(&BTreeSet::new(), false);
    let mut vus = BTreeSet::new();
    for d in &tous {
        assert!(d.nom.len() <= 64 && d.nom.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && vus.insert(d.nom), "{}", d.nom);
        assert_eq!(d.schema["type"], "object", "{}", d.nom);
    }
    assert!(outils::generiques(&BTreeSet::new(), true).iter().all(|d| d.lecture));
}

/// La section « outils génériques » de la skill est générée du code (`MAJ_SKILL=1 cargo test` la réécrit).
#[test]
fn skill_a_jour() {
    const DEBUT: &str = "<!-- outils-generiques:debut -->\n";
    const FIN: &str = "<!-- outils-generiques:fin -->";
    let chemin = concat!(env!("CARGO_MANIFEST_DIR"), "/skill/home-assistant-mcp-rust/SKILL.md");
    let texte = std::fs::read_to_string(chemin).unwrap().replace("\r\n", "\n");
    let (i, j) = (texte.find(DEBUT).expect("marqueur de début") + DEBUT.len(), texte.find(FIN).expect("marqueur de fin"));
    let attendu = format!("\n{}", outils::doc_generiques());
    if std::env::var("MAJ_SKILL").is_ok() {
        std::fs::write(chemin, format!("{}{attendu}{}", &texte[..i], &texte[j..])).unwrap();
        return;
    }
    assert!(texte[i..j] == attendu, "section des outils génériques de SKILL.md périmée : MAJ_SKILL=1 cargo test skill_a_jour");
}
