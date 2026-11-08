//! `ha-mcp` : Home Assistant MCP Rust. `stdio` (défaut), `http` (Streamable HTTP + ancien SSE) ou `lister`
//! (la liste des outils de l'instance en markdown). Journaux sur la sortie d'erreur seulement.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ha_mcp::ha::Ha;
use ha_mcp::schema::Filtre;
use ha_mcp::simule::Approbateur;
use ha_mcp::{App, Options, erreur_rpc, outils};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

const AIDE: &str = "ha-mcp — Home Assistant MCP Rust

Usage : ha-mcp [stdio|http|lister] [options]
  stdio    serveur MCP sur l'entrée et la sortie standard (défaut)
  http     serveur MCP Streamable HTTP sur /mcp (et ancien SSE sur /sse), jeton Bearer obligatoire
  lister   écrit la liste des outils de cette instance en markdown, puis sort

Options (et variables d'environnement) :
  --url URL                    adresse de Home Assistant (HA_URL, défaut http://127.0.0.1:8123)
  --jeton-fichier FICHIER      fichier du jeton d'accès longue durée (HA_TOKEN_FILE ; ou le jeton dans HA_TOKEN)
  --lecture-seule              aucun outil qui écrit (HA_MCP_LECTURE_SEULE=1)
  --domaines a,b               outils de service de ces domaines seulement (HA_MCP_DOMAINES)
  --sans-domaines a,b          sauf ces domaines (HA_MCP_SANS_DOMAINES)
  --max-services N             au plus N outils de service (HA_MCP_MAX_SERVICES ; 0 = aucun)
  --test                       MODE TEST (MCP_TEST_MODE=1) : instance Home Assistant simulée, aucun réseau, ni URL ni jeton lus
  --test-approbateur R         mode test : approve|deny|timeout|absent pour les outils à validation (MCP_TEST_APPROVER, défaut deny)
  --stats-fichier FICHIER      export JSON local des statistiques, réécrit après chaque appel (MCP_STATS_FILE)
  --presets DOSSIER            presets en plus de ceux embarqués (HA_MCP_PRESETS)
  --ecoute ADRESSE:PORT        http : adresse d'écoute (HA_MCP_ECOUTE, défaut 127.0.0.1:8765)
  --jeton-serveur-fichier F    http : fichier du jeton que les clients présentent (HA_MCP_JETON_FICHIER ; ou HA_MCP_JETON)
  -h, --help                   cette aide
  -V, --version                la version
";

fn env(nom: &str) -> Option<String> {
    std::env::var(nom).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Un secret : la variable `direct`, sinon le contenu du fichier (option, sinon variable `fichier`).
fn secret(direct: &str, fichier: Option<String>, quoi: &str) -> Result<String, String> {
    if let Some(f) = fichier {
        let t = std::fs::read_to_string(&f).map_err(|e| format!("lecture du fichier du {quoi} : {e}"))?;
        return Some(t.trim().to_string()).filter(|t| !t.is_empty()).ok_or(format!("fichier du {quoi} vide"));
    }
    env(direct).ok_or(format!("{quoi} manquant"))
}

fn liste(v: Option<String>) -> Vec<String> {
    v.unwrap_or_default().split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

struct Args {
    mode: String,
    url: String,
    jeton_fichier: Option<String>,
    serveur_fichier: Option<String>,
    ecoute: String,
    opts: Options,
    test: bool,
}

fn lire_args() -> Result<Option<Args>, String> {
    let mut mode = None;
    let mut vals: std::collections::HashMap<String, String> = Default::default();
    let mut lecture_seule = env("HA_MCP_LECTURE_SEULE").is_some_and(|v| !matches!(v.as_str(), "0" | "false" | "non"));
    let mut test = env("MCP_TEST_MODE").is_some_and(|v| !matches!(v.as_str(), "0" | "false" | "non"));
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let (cle, val) = match a.split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k.to_string(), Some(v.to_string())),
            _ => (a.clone(), None),
        };
        match cle.as_str() {
            "-h" | "--help" => {
                print!("{AIDE}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("ha-mcp {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--lecture-seule" => lecture_seule = true,
            "--test" => test = true,
            "stdio" | "http" | "lister" if mode.is_none() => mode = Some(cle),
            "--test-approbateur" | "--stats-fichier" | "--url" | "--jeton-fichier" | "--domaines" | "--sans-domaines" | "--max-services" | "--presets" | "--ecoute" | "--jeton-serveur-fichier" => {
                let v = val.or_else(|| it.next()).ok_or(format!("{cle} attend une valeur"))?;
                vals.insert(cle, v);
            }
            _ => return Err(format!("argument inconnu : {a} (ha-mcp --help)")),
        }
    }
    let mut pris = |cle: &str, var: &str| vals.remove(cle).or_else(|| env(var));
    let max = match pris("--max-services", "HA_MCP_MAX_SERVICES") {
        Some(n) => Some(n.parse::<usize>().map_err(|_| format!("--max-services : nombre attendu, pas « {n} »"))?),
        None => None,
    };
    let filtre = Filtre { domaines: liste(pris("--domaines", "HA_MCP_DOMAINES")), sans_domaines: liste(pris("--sans-domaines", "HA_MCP_SANS_DOMAINES")), max };
    Ok(Some(Args {
        mode: mode.unwrap_or_else(|| "stdio".into()),
        url: pris("--url", "HA_URL").unwrap_or_else(|| "http://127.0.0.1:8123".into()),
        jeton_fichier: pris("--jeton-fichier", "HA_TOKEN_FILE"),
        serveur_fichier: pris("--jeton-serveur-fichier", "HA_MCP_JETON_FICHIER"),
        ecoute: pris("--ecoute", "HA_MCP_ECOUTE").unwrap_or_else(|| "127.0.0.1:8765".into()),
        opts: Options {
            lecture_seule,
            filtre,
            presets: pris("--presets", "HA_MCP_PRESETS").map(Into::into),
            stats_fichier: pris("--stats-fichier", "MCP_STATS_FILE").map(Into::into),
            approbateur: pris("--test-approbateur", "MCP_TEST_APPROVER").map(|v| Approbateur::lire(&v)).transpose()?.unwrap_or_default(),
        },
        test,
    }))
}

#[tokio::main]
async fn main() -> ExitCode {
    match lancer().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ha-mcp : {e}");
            ExitCode::FAILURE
        }
    }
}

async fn lancer() -> Result<(), String> {
    let Some(a) = lire_args()? else { return Ok(()) };
    let jeton_serveur = if a.mode == "http" { Some(secret("HA_MCP_JETON", a.serveur_fichier, "jeton du serveur MCP (HA_MCP_JETON ou HA_MCP_JETON_FICHIER)")?) } else { None };
    // mode test : ni URL ni jeton Home Assistant ne sont lus, le client réseau n'est jamais construit
    let ha = if a.test {
        eprintln!("ha-mcp : MODE TEST : instance simulée, aucun accès réseau, validations : {:?}", a.opts.approbateur);
        Ha::new_test("simule://test")
    } else {
        Ha::new(&a.url, secret("HA_TOKEN", a.jeton_fichier, "jeton de Home Assistant (HA_TOKEN ou HA_TOKEN_FILE)")?)?
    };
    let app = Arc::new(App::new(ha, a.opts));
    let decouverte = tokio::time::timeout(Duration::from_secs(20), app.redecouvrir()).await.unwrap_or_else(|_| Err("délai dépassé (20 s)".into()));
    if a.mode == "lister" {
        decouverte.map_err(|e| format!("découverte impossible sur {} : {e}", a.url))?;
        print!("{}", outils::markdown(&app));
        return Ok(());
    }
    match &decouverte {
        Ok(()) => eprintln!("ha-mcp : {} outils pour {}", app.etat.read().unwrap_or_else(|e| e.into_inner()).outils.len(), a.url),
        // le serveur démarre quand même : la veille réessaie et annonce la liste complète (list_changed)
        Err(e) => eprintln!("ha-mcp : découverte impossible pour l'instant ({e}), nouvel essai en arrière-plan"),
    }
    let veille = app.clone();
    tokio::spawn(async move { veille.veiller().await });
    match jeton_serveur {
        Some(j) => ha_mcp::http::servir(app, &a.ecoute, j).await,
        None => stdio(app).await,
    }
}

async fn ecrire(sortie: &tokio::sync::Mutex<tokio::io::Stdout>, m: &Value) {
    let mut o = sortie.lock().await;
    let _ = o.write_all(format!("{m}\n").as_bytes()).await;
    let _ = o.flush().await;
}

/// Un message JSON par ligne ; les requêtes s'exécutent en parallèle, les réponses partent dans l'ordre où elles
/// sont prêtes. `list_changed` seulement après `notifications/initialized` (spécification).
async fn stdio(app: Arc<App>) -> Result<(), String> {
    let sortie = Arc::new(tokio::sync::Mutex::new(tokio::io::stdout()));
    let initialise = Arc::new(AtomicBool::new(false));
    {
        let (app, sortie, initialise) = (app.clone(), sortie.clone(), initialise.clone());
        let mut generation = app.generation.subscribe();
        tokio::spawn(async move {
            while generation.changed().await.is_ok() {
                if initialise.load(Ordering::Relaxed) {
                    ecrire(&sortie, &json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"})).await;
                }
            }
        });
    }
    let mut entree = tokio::io::BufReader::new(tokio::io::stdin());
    let mut taches = tokio::task::JoinSet::new();
    let mut ligne = Vec::new();
    loop {
        ligne.clear();
        if entree.read_until(b'\n', &mut ligne).await.map_err(|e| format!("entrée standard : {e}"))? == 0 {
            break;
        }
        if ligne.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let Ok(m) = serde_json::from_slice::<Value>(&ligne) else {
            ecrire(&sortie, &erreur_rpc(Value::Null, -32700, "Parse error")).await;
            continue;
        };
        if m["method"] == "notifications/initialized" {
            initialise.store(true, Ordering::Relaxed);
        }
        let (app, sortie) = (app.clone(), sortie.clone());
        taches.spawn(async move {
            if let Some(r) = app.traiter(&m).await {
                ecrire(&sortie, &r).await;
            }
        });
    }
    // fin de l'entrée : les requêtes en cours répondent avant la sortie
    while taches.join_next().await.is_some() {}
    Ok(())
}
