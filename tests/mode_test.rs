//! Mode test : preuve B (dynamique) — tous les outils de `tools/list` appelés en mode test avec une URL Home Assistant
//! qui pointe vers un écouteur SENTINELLE local : 0 connexion reçue, 0 accès coupé, toutes les réponses marquées.
//! Le transport réel est de plus remplacé par un transport qui panique (preuve A dynamique). Lancé aussi sous
//! isolement réseau OS par `tests/b-isole.sh` (`docker run --network none`).
//! Contient aussi les 4 scénarios de validation simulée pour chaque outil à validation.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ha_mcp::ha::Ha;
use ha_mcp::simule::Approbateur;
use ha_mcp::{App, Options, outils, reseau};
use serde_json::{Value, json};

static CONNEXIONS: AtomicU64 = AtomicU64::new(0);

/// Écouteur sentinelle : compte toute connexion entrante. Rend l'adresse http://127.0.0.1:port.
async fn sentinelle() -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok(_c) = l.accept().await {
            CONNEXIONS.fetch_add(1, Ordering::SeqCst);
        }
    });
    format!("http://127.0.0.1:{port}")
}

async fn nouvelle(approbateur: Approbateur) -> Arc<App> {
    reseau::transport_qui_panique();
    let base = sentinelle().await;
    let ha = Ha::new_test(&base);
    assert!(ha.est_test() && !ha.client_construit(), "le vrai client ne doit jamais être construit en mode test");
    let app = Arc::new(App::new(ha, Options { approbateur, ..Default::default() }));
    app.redecouvrir().await.expect("découverte simulée");
    app
}

async fn appel(app: &App, nom: &str, args: Value) -> Value {
    let r = app.traiter(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": nom, "arguments": args}})).await.expect("réponse");
    assert!(r.get("error").is_none(), "{nom} : erreur JSON-RPC {r}");
    r["result"].clone()
}

fn texte(r: &Value) -> &str {
    r["content"][0]["text"].as_str().unwrap_or_default()
}

fn marque(nom: &str, r: &Value) {
    assert_eq!(r["_meta"]["test_mode"], true, "{nom} : _meta.test_mode absent");
    assert!(texte(r).starts_with("[TEST] "), "{nom} : préfixe [TEST] absent : {}", texte(r));
}

/// Arguments valides de chaque outil fixe (un outil fixe sans entrée ici fait échouer le test).
fn args_fixes() -> Vec<(&'static str, Value)> {
    vec![
        ("etats", json!({"piece": "salon"})),
        ("services", json!({"domaine": "light"})),
        ("appeler_service", json!({"domain": "light", "service": "turn_on", "target": {"entity_id": "light.salon"}})),
        ("template", json!({"template": "{{ 1 }}"})),
        ("historique", json!({"entity_ids": "sensor.temp_salon"})),
        ("journal", json!({})),
        ("registres", json!({"type": "pieces"})),
        ("recherche_liee", json!({"item_type": "entity", "item_id": "light.salon"})),
        ("integrations", json!({})),
        ("flux_demarrer", json!({"handler": "met"})),
        ("flux_etape", json!({"flow_id": "flux-simule-1", "data": {}})),
        ("flux_annuler", json!({"flow_id": "flux-simule-1"})),
        ("tableaux_lister", json!({})),
        ("tableau_lire", json!({"url_path": "carte-test"})),
        ("tableau_enregistrer", json!({"url_path": "carte-test", "config": {"views": []}})),
        ("page_creer", json!({"url_path": "cuisine-tablette", "title": "Cuisine"})),
        ("page_modifier", json!({"url_path": "cuisine-tablette", "icon": "mdi:stove"})),
        ("page_supprimer", json!({"url_path": "cuisine-tablette"})),
        ("barre_regler", json!({"masques": ["energy"]})),
        ("automatisme_lire", json!({"domaine": "automation"})),
        ("automatisme_ecrire", json!({"domaine": "script", "id": "essai", "config": {"sequence": []}})),
        ("automatisme_supprimer", json!({"domaine": "script", "id": "essai"})),
        ("presets_lister", json!({})),
        ("preset_appliquer", json!({"preset": "tableau-minimal"})),
        ("preset_annuler", json!({})),
        ("preset_exporter", json!({"preset": "ma-maison"})),
        ("ws", json!({"type": "config/area_registry/list"})),
        ("rest", json!({"path": "/api/config"})),
        ("ha_stats", json!({})),
        ("lister_outils", json!({})),
    ]
}

#[tokio::test]
async fn b_tous_les_outils_en_mode_test_zero_connexion() {
    let app = nouvelle(Approbateur::Approve).await;
    let l = app.traiter(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).await.unwrap();
    let noms: Vec<String> = l["result"]["tools"].as_array().unwrap().iter().map(|o| o["name"].as_str().unwrap().to_string()).collect();
    let fixes: BTreeSet<&str> = noms.iter().map(String::as_str).filter(|n| !n.contains("__")).collect();
    let attendus: Vec<(&str, Value)> = args_fixes();
    assert_eq!(fixes, attendus.iter().map(|(n, _)| *n).collect::<BTreeSet<_>>(), "outils fixes de tools/list ≠ table d'arguments du test");
    assert_eq!(fixes.len(), outils::generiques(&BTreeSet::new(), false).len());
    let dynamiques: Vec<&String> = noms.iter().filter(|n| n.contains("__")).collect();
    assert!(dynamiques.len() >= 20, "catalogue simulé : {} outils dynamiques", dynamiques.len());

    let mut erreurs = vec![];
    for (nom, args) in attendus.iter().cloned().chain(dynamiques.iter().map(|n| (n.as_str(), json!({})))) {
        let r = appel(&app, nom, args).await;
        marque(nom, &r);
        if r["isError"] == true {
            erreurs.push(format!("{nom} : {}", texte(&r)));
        }
    }
    println!("outils appelés : {} fixes + {} dynamiques", fixes.len(), dynamiques.len());
    assert!(erreurs.is_empty(), "outils en erreur : {erreurs:#?}");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    // preuve B : rien n'a atteint l'écouteur sentinelle, rien n'a été coupé non plus (personne n'a même essayé)
    assert_eq!(CONNEXIONS.load(Ordering::SeqCst), 0, "la sentinelle a reçu une connexion");
    assert_eq!(reseau::coupures(), 0, "un accès réseau a été tenté (et coupé)");
    let s = app.stats.json(true);
    assert_eq!(s["mode"], "test");
    assert_eq!(s["total"]["reels"], 0);
    assert_eq!(s["total"]["test"], (fixes.len() + dynamiques.len()) as u64);
    assert_eq!(s["coupures_reseau"], 0);
}

/// Un appel qui exige une validation : outil, arguments, et s'il écrit vraiment dans l'instance simulée une fois approuvé.
fn cas_valides() -> Vec<(&'static str, Value, bool)> {
    vec![
        ("tableau_enregistrer", json!({"url_path": "carte-test", "config": {"views": []}}), true),
        ("page_supprimer", json!({"url_path": "carte-test"}), true),
        ("automatisme_supprimer", json!({"domaine": "automation", "id": "lumiere_coucher"}), true),
        ("preset_appliquer", json!({"preset": "tableau-minimal"}), true),
        ("preset_annuler", json!({}), false),
        ("appeler_service", json!({"domain": "homeassistant", "service": "restart"}), true),
        ("rest", json!({"method": "DELETE", "path": "/api/config/script/config/bonne_nuit"}), true),
        ("ws", json!({"type": "config/entity_registry/remove", "entity_id": "light.salon"}), true),
        ("homeassistant__restart", json!({}), true),
        ("homeassistant__stop", json!({}), true),
        ("hassio__host_reboot", json!({}), true),
        ("recorder__purge", json!({"keep_days": 0}), true),
    ]
}

#[tokio::test]
async fn validation_quatre_scenarios_par_outil() {
    assert_eq!(Options::default().approbateur, Approbateur::Deny, "le défaut doit être fermé");
    let cas = cas_valides();
    for fixe in outils::VALIDATION_FIXE {
        assert!(cas.iter().any(|(n, _, _)| *n == fixe), "outil à validation sans test : {fixe}");
    }
    for (nom, args, ecrit) in &cas {
        for (approb, marqueur) in [(Approbateur::Approve, ""), (Approbateur::Deny, "VALIDATION_REFUSEE"), (Approbateur::Timeout, "VALIDATION_TIMEOUT"), (Approbateur::Absent, "VALIDATION_INDISPONIBLE")] {
            let app = nouvelle(approb).await;
            let avant = app.ha.simule.as_ref().unwrap().ecritures();
            let r = appel(&app, nom, args.clone()).await;
            marque(nom, &r);
            let delta = app.ha.simule.as_ref().unwrap().ecritures() - avant;
            let v = app.stats.json(true)["validations_simulees"].clone();
            if approb == Approbateur::Approve {
                // approuvé : l'outil s'exécute contre l'instance SIMULÉE (rien de réel)
                assert!(!texte(&r).contains("VALIDATION_"), "{nom} approuvé mais bloqué : {}", texte(&r));
                assert_eq!(v["approuvees"], 1, "{nom}");
                if *ecrit {
                    assert!(delta >= 1, "{nom} approuvé mais rien exécuté dans le mock : {}", texte(&r));
                }
            } else {
                assert_eq!(r["isError"], true, "{nom} {approb:?}");
                assert!(texte(&r).contains(marqueur), "{nom} {approb:?} : {}", texte(&r));
                assert_eq!(delta, 0, "{nom} {approb:?} : le mock a exécuté malgré le refus");
                let cle = match approb {
                    Approbateur::Deny => "refusees",
                    Approbateur::Timeout => "timeouts",
                    _ => "indisponibles",
                };
                assert_eq!(v[cle], 1, "{nom} {approb:?}");
            }
        }
    }
    println!("validation : {} outils/appels × 4 scénarios", cas.len());
    // un outil sans validation n'est jamais arrêté, même quand l'approbateur refuse tout
    let app = nouvelle(Approbateur::Deny).await;
    let r = appel(&app, "etats", json!({})).await;
    assert_eq!(r["isError"], false);
    assert_eq!(app.stats.json(true)["validations_simulees"]["refusees"], 0);
    assert_eq!(CONNEXIONS.load(Ordering::SeqCst), 0, "la sentinelle a reçu une connexion");
    assert_eq!(reseau::coupures(), 0);
}

#[test]
fn approbateur_valeurs() {
    assert_eq!(Approbateur::lire("approve"), Ok(Approbateur::Approve));
    assert_eq!(Approbateur::lire("TIMEOUT"), Ok(Approbateur::Timeout));
    assert!(Approbateur::lire("oui").is_err());
}

/// La colonne « Validation admin » de la skill dit la même chose que le code.
#[test]
fn skill_colonne_validation() {
    let skill = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skill/home-assistant-mcp-rust/SKILL.md")).unwrap().replace("\r\n", "\n");
    let tableau = skill.split("<!-- outils-fixes:debut -->").nth(1).unwrap().split("<!-- outils-fixes:fin -->").next().unwrap();
    let mut vus = 0;
    for l in tableau.lines().filter(|l| l.starts_with("| `")) {
        let c: Vec<&str> = l.split('|').map(str::trim).collect();
        let nom = c[1].trim_matches('`');
        let attendu = if outils::VALIDATION_FIXE.contains(&nom) {
            "oui"
        } else if ["appeler_service", "rest", "ws"].contains(&nom) {
            "selon"
        } else {
            "non"
        };
        assert!(c[5].starts_with(attendu), "{nom} : colonne validation « {} », attendu « {attendu} »", c[5]);
        vus += 1;
    }
    assert_eq!(vus, outils::generiques(&BTreeSet::new(), false).len());
}
