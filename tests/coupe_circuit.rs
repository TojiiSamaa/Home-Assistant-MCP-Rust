//! Coupe-circuit : en mode test, le point réseau unique refuse tout, compte, et l'URL (sentinelle) n'est jamais jointe.

use std::time::Duration;

use ha_mcp::ha::Ha;
use ha_mcp::reseau;

#[tokio::test]
async fn egress_bloque_et_compte() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("127.0.0.1:{}", l.local_addr().unwrap().port());
    let vues = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let v = vues.clone();
    tokio::spawn(async move {
        while let Ok(_c) = l.accept().await {
            v.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    });
    let ha = Ha::new_test(&format!("http://{url}"));
    assert!(reseau::est_test() && !ha.client_construit());
    // un client réel construit à la main ne passe pas non plus le coupe-circuit
    let client = reseau::client(Duration::from_secs(2));
    let e = reseau::http(&client, "GET", &format!("http://{url}/api/"), "Bearer x", None, "GET /api/").await.unwrap_err();
    assert!(e.contains("TEST_MODE_EGRESS_BLOCKED"), "{e}");
    let e = reseau::ws_ouvrir(&format!("ws://{url}/api/websocket"), Duration::from_secs(2)).await.map(|_| ()).unwrap_err();
    assert!(e.contains("TEST_MODE_EGRESS_BLOCKED"), "{e}");
    // une session WebSocket réelle demandée en mode test est coupée aussi
    let e = ha_mcp::ha::Session::ouvrir(&ha).await.map(|_| ()).unwrap_err();
    assert!(e.contains("TEST_MODE_EGRESS_BLOCKED"), "{e}");
    assert_eq!(reseau::coupures(), 3);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(vues.load(std::sync::atomic::Ordering::SeqCst), 0, "la sentinelle a reçu une connexion");
}
