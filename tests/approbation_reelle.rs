//! Une approbation simulée n'autorise JAMAIS rien de réel : hors mode test l'approbateur n'est pas consulté, l'outil
//! prend le chemin réseau normal (ici : une adresse où personne n'écoute → échec de connexion, pas de simulation).

use ha_mcp::ha::Ha;
use ha_mcp::simule::Approbateur;
use ha_mcp::{App, Options, reseau};
use serde_json::json;

#[tokio::test]
async fn approbateur_sans_effet_hors_mode_test() {
    let app = App::new(Ha::new("http://127.0.0.1:9", "jeton".into()).unwrap(), Options { approbateur: Approbateur::Approve, ..Default::default() });
    assert!(!reseau::est_test() && !app.ha.est_test() && app.ha.client_construit());
    let r = app.traiter(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "page_supprimer", "arguments": {"url_path": "carte-test"}}})).await.unwrap();
    let t = r["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(r["result"]["isError"], true);
    assert!(!t.starts_with("[TEST]") && r["result"]["_meta"].is_null(), "réponse simulée hors mode test : {t}");
    assert!(t.contains("connexion") || t.contains("WebSocket"), "attendu un échec de connexion réelle, reçu : {t}");
    let s = app.stats.json(false);
    assert_eq!(s["validations_simulees"]["approuvees"], 0);
    assert_eq!(s["total"]["test"], 0);
}
