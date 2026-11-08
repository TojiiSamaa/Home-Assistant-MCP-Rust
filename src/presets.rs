//! Presets de préconfiguration, repris de maison (même format de fichier : pages, thème, barre latérale, cartes
//! requises, paramètres déclarés avec leurs défauts) mais appliqués CÔTÉ CLIENT avec les commandes standard de Home
//! Assistant : lovelace/dashboards/*, lovelace/config/save, frontend/set_user_data, service frontend.set_theme.
//!
//! Appliquer = COPIER : rien ne reste lié au preset. Ce qui est remplacé est lu d'abord puis gardé AVANT toute
//! écriture dans les données de l'utilisateur du jeton (clé `ha_mcp_presets`, les 10 dernières applications) pour
//! `preset_annuler`. Bornes : fichier de 1 Mio au plus, 50 000 valeurs JSON au plus une fois déplié.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::App;

const EMBARQUES: [&str; 3] = [include_str!("../presets/maison-de-base.json"), include_str!("../presets/energie.json"), include_str!("../presets/tableau-minimal.json")];
pub const MORCEAUX: [&str; 3] = ["pages", "theme", "barre"];
const CLE: &str = "ha_mcp_presets";
const ANNULATIONS_MAX: usize = 10;
/// Un fichier du dossier de presets plus gros est refusé (listé avec `erreur`).
const FICHIER_MAX: u64 = 1 << 20;
/// Valeurs JSON au plus produites par le dépliage d'un preset (pages, thème, barre, cartes réunis).
const NOEUDS_MAX: usize = 50_000;
const CHAMPS_PAGE: [&str; 4] = ["title", "icon", "show_in_sidebar", "require_admin"];

pub fn id_valide(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Presets embarqués puis ceux du dossier (id = nom du fichier), avec `source`. Un fichier illisible, trop gros ou
/// qui reprend l'id d'un preset embarqué est listé avec `erreur`.
pub fn tous(dossier: Option<&Path>) -> Vec<Value> {
    let mut l: Vec<Value> = EMBARQUES
        .iter()
        .filter_map(|s| serde_json::from_str::<Value>(s).ok())
        .map(|mut p| {
            p["source"] = json!("embarque");
            p
        })
        .collect();
    let mut fichiers: Vec<PathBuf> = dossier.and_then(|d| std::fs::read_dir(d).ok()).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    fichiers.sort();
    for f in fichiers {
        let id = f.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        let lu = std::fs::metadata(&f)
            .map_err(|e| e.to_string())
            .and_then(|m| if m.len() > FICHIER_MAX { Err(format!("fichier trop gros ({} octets, {FICHIER_MAX} au plus)", m.len())) } else { std::fs::read(&f).map_err(|e| e.to_string()) })
            .and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| format!("JSON illisible : {e}")));
        let erreur = match &lu {
            _ if !id_valide(&id) => Some("nom de fichier refusé (a-z, 0-9, - et _ seulement)".to_string()),
            _ if l.iter().any(|p| p["id"] == id.as_str()) => Some("id déjà pris par un preset embarqué".into()),
            Err(e) => Some(e.clone()),
            Ok(p) if !p.is_object() => Some("le fichier n'est pas un objet JSON".into()),
            Ok(_) => None,
        };
        l.push(match (erreur, lu) {
            (None, Ok(mut p)) => {
                p["id"] = json!(id);
                p["source"] = json!("dossier");
                p
            }
            (e, _) => json!({"id": id, "source": "dossier", "erreur": e}),
        });
    }
    l
}

/// Ce que `presets_lister` montre d'un preset (sans le contenu des pages).
fn resume(p: &Value) -> Value {
    if p.get("erreur").is_some() {
        return p.clone();
    }
    let pages: Vec<Value> = p["pages"].as_array().into_iter().flatten().map(|x| json!({"url_path": x["url_path"], "title": x["title"], "icon": x["icon"]})).collect();
    json!({"id": p["id"], "nom": p["nom"], "description": p["description"], "source": p["source"], "parametres": p["parametres"],
        "pages": pages, "theme": p["theme"], "barre": p["barre"], "cartes": p["cartes"], "morceaux": MORCEAUX})
}

pub fn lister(dossier: Option<&Path>) -> Value {
    Value::Array(tous(dossier).iter().map(resume).collect())
}

// ───────────────────────── Paramètres ─────────────────────────

/// `*` dans un motif d'entité (« light.* », « sensor.*energy* »).
fn correspond(motif: &str, s: &str) -> bool {
    let morceaux: Vec<&str> = motif.split('*').collect();
    let (premier, dernier) = (morceaux[0], morceaux[morceaux.len() - 1]);
    if morceaux.len() == 1 {
        return motif == s;
    }
    if !s.starts_with(premier) || s.len() < premier.len() + dernier.len() || !s.ends_with(dernier) {
        return false;
    }
    let mut reste = &s[premier.len()..s.len() - dernier.len()];
    for m in &morceaux[1..morceaux.len() - 1] {
        match reste.find(m) {
            Some(i) => reste = &reste[i + m.len()..],
            None => return false,
        }
    }
    true
}

fn textes(v: &Value, nom: &str) -> Result<Vec<String>, String> {
    match v {
        Value::Null => Ok(vec![]),
        Value::String(s) => Ok(vec![s.clone()]),
        Value::Array(l) => l.iter().map(|x| x.as_str().map(String::from).ok_or(format!("« {nom} » : liste de textes attendue"))).collect(),
        _ => Err(format!("« {nom} » : liste de textes attendue")),
    }
}

fn liste_textes(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect()
}

/// Pièces du registre : « * » ou vide = toutes ; sinon par id ou par nom (sans casse).
fn pieces(zones: &[Value], v: &Value, nom: &str) -> Result<Value, String> {
    let voulues = textes(v, nom)?;
    if voulues.is_empty() || voulues.iter().any(|x| x == "*") {
        return Ok(zones.iter().map(|z| json!({"id": z["area_id"], "nom": z["name"]})).collect());
    }
    let mut l = vec![];
    for x in voulues {
        let z = zones.iter().find(|z| z["area_id"] == x.as_str() || z["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(&x))).ok_or(format!("« {nom} » : pièce inconnue « {x} »"))?;
        l.push(json!({"id": z["area_id"], "nom": z["name"]}));
    }
    Ok(Value::Array(l))
}

/// Entités : ids exacts (gardés même s'ils n'existent pas encore) ou motifs « * » sur les états actuels.
fn entites(etats: &[(String, Value)], v: &Value, nom: &str) -> Result<Value, String> {
    let mut l: Vec<Value> = vec![];
    for x in textes(v, nom)? {
        let trouves: Vec<(String, Value)> = if x.contains('*') { etats.iter().filter(|(id, _)| correspond(&x, id)).cloned().collect() } else { vec![(x.clone(), etats.iter().find(|(id, _)| *id == x).map(|(_, n)| n.clone()).unwrap_or(json!(x)))] };
        for (id, n) in trouves {
            if !l.iter().any(|e| e["id"] == id.as_str()) {
                l.push(json!({"id": id, "nom": n}));
            }
        }
    }
    Ok(Value::Array(l))
}

/// Valeurs des paramètres déclarés : saisie, sinon défaut. Un paramètre non déclaré est refusé. Pièces et états
/// sont lus sur le serveur seulement si un paramètre en a besoin.
async fn valeurs(app: &App, p: &Value, saisie: &Map<String, Value>) -> Result<Map<String, Value>, String> {
    let decl = p["parametres"].as_object().cloned().unwrap_or_default();
    if let Some(k) = saisie.keys().find(|k| !decl.contains_key(*k)) {
        return Err(format!("paramètre inconnu : {k} (déclarés : {})", decl.keys().cloned().collect::<Vec<_>>().join(", ")));
    }
    let besoin = |t: &str| decl.values().any(|d| d["type"] == t);
    let zones: Vec<Value> = if besoin("pieces") { app.ws_resultat(json!({"type": "config/area_registry/list"})).await?.as_array().cloned().unwrap_or_default() } else { vec![] };
    let mut etats: Vec<(String, Value)> = vec![];
    if besoin("entites") {
        let l = app.ha.rest("GET", "/api/states", None).await?.valeur()?;
        etats = l.as_array().into_iter().flatten().filter_map(|e| Some((e["entity_id"].as_str()?.to_string(), e["attributes"].get("friendly_name").cloned().unwrap_or(e["entity_id"].clone())))).collect();
        etats.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let mut sortie = Map::new();
    for (nom, d) in &decl {
        let v = saisie.get(nom).filter(|v| !v.is_null()).cloned().unwrap_or_else(|| d["defaut"].clone());
        let v = match d["type"].as_str().unwrap_or("texte") {
            "texte" | "icone" | "theme" => match v {
                Value::String(_) => v,
                Value::Number(n) => json!(n.to_string()),
                _ => return Err(format!("« {nom} » : texte attendu")),
            },
            "nombre" => match v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())) {
                Some(n) if n.fract() == 0.0 && n.abs() < 1e15 => json!(n as i64),
                Some(n) => json!(n),
                None => return Err(format!("« {nom} » : nombre attendu")),
            },
            "booleen" => json!(v.as_bool().ok_or(format!("« {nom} » : vrai ou faux attendu"))?),
            "liste" => json!(textes(&v, nom)?),
            "pieces" => pieces(&zones, &v, nom)?,
            "entites" => entites(&etats, &v, nom)?,
            t => return Err(format!("« {nom} » : type de paramètre inconnu « {t} »")),
        };
        sortie.insert(nom.clone(), v);
    }
    Ok(sortie)
}

/// Valeur d'un paramètre utilisée telle quelle : une liste de pièces ou d'entités devient la liste de leurs ids.
fn brute(v: &Value) -> Value {
    match v.as_array() {
        Some(l) if l.iter().all(|x| x.get("id").is_some()) => Value::Array(l.iter().map(|x| x["id"].clone()).collect()),
        _ => v.clone(),
    }
}

fn en_texte(v: &Value) -> String {
    match brute(v) {
        Value::String(s) => s,
        Value::Array(l) => l.iter().map(en_texte).collect::<Vec<_>>().join(", "),
        Value::Null => String::new(),
        autre => autre.to_string(),
    }
}

/// `[[param]]` seul dans une chaîne : la valeur (type gardé) ; dans un texte : interpolée.
/// shortcut: un texte de page qui contient « [[ » est lu comme une référence ; à échapper si un preset en a besoin.
fn chaine(s: &str, vals: &Map<String, Value>) -> Result<Value, String> {
    let inconnu = |n: &str| format!("paramètre inconnu : [[{n}]]");
    if let Some(n) = s.strip_prefix("[[").and_then(|r| r.strip_suffix("]]")).filter(|n| !n.contains("[[") && !n.contains("]]")) {
        return vals.get(n).map(brute).ok_or_else(|| inconnu(n));
    }
    let (mut sortie, mut reste) = (String::new(), s);
    while let Some(i) = reste.find("[[") {
        sortie.push_str(&reste[..i]);
        let apres = &reste[i + 2..];
        let j = apres.find("]]").ok_or(format!("« [[ » sans « ]] » dans « {s} »"))?;
        sortie.push_str(&en_texte(vals.get(&apres[..j]).ok_or_else(|| inconnu(&apres[..j]))?));
        reste = &apres[j + 2..];
    }
    sortie.push_str(reste);
    Ok(json!(sortie))
}

/// Remplace les `[[param]]` et déplie les `{"$pour_chaque": "param", "$modele": {…}}` des listes (dans le modèle :
/// `[[element]]` = id, `[[element_nom]]` = nom). `budget` : valeurs JSON encore permises (un preset hostile qui
/// imbrique des `$pour_chaque` multiplie la sortie).
fn substituer(v: &Value, vals: &Map<String, Value>, budget: &mut usize) -> Result<Value, String> {
    *budget = budget.checked_sub(1).ok_or(format!("preset trop gros une fois déplié (plus de {NOEUDS_MAX} valeurs)"))?;
    Ok(match v {
        Value::String(s) => chaine(s, vals)?,
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| substituer(x, vals, budget).map(|x| (k.clone(), x))).collect::<Result<_, _>>()?),
        Value::Array(l) => {
            let mut sortie = vec![];
            for x in l {
                let Some(nom) = x.get("$pour_chaque") else {
                    sortie.push(substituer(x, vals, budget)?);
                    continue;
                };
                let nom = nom.as_str().unwrap_or_default();
                let liste = vals.get(nom).and_then(Value::as_array).ok_or(format!("« $pour_chaque » : « {nom} » n'est pas un paramètre liste"))?;
                let modele = x.get("$modele").ok_or("« $pour_chaque » sans « $modele »")?;
                for e in liste {
                    let mut local = vals.clone();
                    local.insert("element".into(), e.get("id").cloned().unwrap_or_else(|| e.clone()));
                    local.insert("element_nom".into(), e.get("nom").cloned().unwrap_or_else(|| e.clone()));
                    sortie.push(substituer(modele, &local, budget)?);
                }
            }
            Value::Array(sortie)
        }
        autre => autre.clone(),
    })
}

// ───────────────────────── Données de l'utilisateur ─────────────────────────

async fn donnee(app: &App, cle: &str) -> Result<Value, String> {
    Ok(app.ws_resultat(json!({"type": "frontend/get_user_data", "key": cle})).await?["value"].clone())
}

async fn poser(app: &App, cle: &str, valeur: Value) -> Result<(), String> {
    app.ws_resultat(json!({"type": "frontend/set_user_data", "key": cle, "value": valeur})).await.map(|_| ())
}

async fn annulations(app: &App) -> Result<Vec<Value>, String> {
    Ok(donnee(app, CLE).await?["annulations"].as_array().cloned().unwrap_or_default())
}

/// Thème par défaut de tous (`mode` dark : thème sombre ; « none » le retire).
async fn theme_de_tous(app: &App, nom: &str, mode: Option<&str>) -> Result<(), String> {
    let mut data = json!({"name": nom});
    if let Some(m) = mode {
        data["mode"] = json!(m);
    }
    app.ws_resultat(json!({"type": "call_service", "domain": "frontend", "service": "set_theme", "service_data": data})).await.map(|_| ())
}

async fn tableaux(app: &App) -> Result<Vec<Value>, String> {
    Ok(app.ws_resultat(json!({"type": "lovelace/dashboards/list"})).await?.as_array().cloned().unwrap_or_default())
}

// ───────────────────────── Appliquer ─────────────────────────

pub async fn appliquer(app: &App, a: &Map<String, Value>) -> Result<Value, String> {
    let id = a.get("preset").and_then(Value::as_str).ok_or("« preset » (texte) est requis")?;
    let brut = tous(app.opts.presets.as_deref()).into_iter().find(|p| p["id"] == id).ok_or(format!("preset inconnu : {id}"))?;
    if let Some(e) = brut["erreur"].as_str() {
        return Err(format!("preset {id} illisible : {e}"));
    }
    let morceaux: Vec<String> = match a.get("morceaux").filter(|v| !v.is_null()) {
        None => MORCEAUX.map(String::from).to_vec(),
        Some(v) => {
            let l = liste_textes(v);
            if let Some(m) = l.iter().find(|m| !MORCEAUX.contains(&m.as_str())) {
                return Err(format!("morceau inconnu : {m} (pages, theme ou barre)"));
            }
            l
        }
    };
    if morceaux.is_empty() {
        return Err("aucun morceau choisi (pages, theme, barre)".into());
    }
    let avec = |m: &str| morceaux.iter().any(|x| x == m);
    let pour_tous = match a.get("pour").and_then(Value::as_str) {
        None | Some("moi") => false,
        Some("tous") => true,
        Some(p) => return Err(format!("« pour » : moi ou tous, pas « {p} »")),
    };
    let saisie = a.get("parametres").and_then(Value::as_object).cloned().unwrap_or_default();
    let vals = valeurs(app, &brut, &saisie).await?;
    let mut p = Map::new();
    let mut budget = NOEUDS_MAX;
    for k in ["pages", "theme", "barre", "cartes"] {
        p.insert(k.into(), substituer(&brut[k], &vals, &mut budget)?);
    }

    let _g = app.verrou_presets.lock().await;
    let existants = if avec("pages") { tableaux(app).await? } else { vec![] };
    let mut pages: Vec<Value> = vec![];
    let mut cartes = vec![];
    if avec("pages") {
        let panneaux = app.ws_resultat(json!({"type": "get_panels"})).await?;
        for x in p["pages"].as_array().into_iter().flatten() {
            let url = x["url_path"].as_str().unwrap_or_default();
            if !url.contains('-') || url.contains(['/', ' ']) {
                return Err(format!("page « {url} » : l'adresse doit contenir un tiret (-), sans espace ni /"));
            }
            let titre = x["title"].as_str().map(str::trim).filter(|t| !t.is_empty()).ok_or(format!("page « {url} » : titre vide"))?;
            if pages.iter().any(|q| q["url_path"] == url) {
                return Err(format!("page « {url} » en double"));
            }
            if panneaux.get(url).is_some() && !existants.iter().any(|t| t["url_path"] == url) {
                return Err(format!("l'adresse « {url} » est prise par un panneau intégré"));
            }
            let mut page = json!({"url_path": url, "title": titre, "show_in_sidebar": x["show_in_sidebar"] != false, "require_admin": x["require_admin"] == true, "config": x.get("config").cloned().unwrap_or(json!({"views": []}))});
            if let Some(i) = x["icon"].as_str().filter(|i| !i.is_empty()) {
                page["icon"] = json!(i);
            }
            pages.push(page);
        }
        cartes = liste_textes(&p["cartes"]);
    }

    let mut theme = None;
    if let Some(nom) = avec("theme").then(|| p["theme"]["nom"].as_str()).flatten().filter(|n| !n.is_empty()) {
        let t = app.ws_resultat(json!({"type": "frontend/get_themes"})).await?;
        let installes: Vec<String> = t["themes"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        let trouve = if nom == "default" || installes.iter().any(|i| i == nom) { Some(nom.to_string()) } else { installes.iter().find(|k| k.eq_ignore_ascii_case(nom)).cloned() };
        let Some(nom) = trouve else {
            return Err(format!("thème introuvable : « {nom} » (installés : {})", installes.join(", ")));
        };
        theme = Some((nom, p["theme"]["sombre"].as_bool(), t));
    }
    let barre = (avec("barre") && p["barre"].is_object()).then(|| {
        let b = &p["barre"];
        (liste_textes(&b["ordre"]), liste_textes(&b["masques"]), b["position"].as_u64().unwrap_or(0) as usize)
    });

    // 1. ce qui sera remplacé
    let mut elements = vec![];
    for page in &pages {
        let url = page["url_path"].as_str().unwrap_or_default();
        let avant = match existants.iter().find(|t| t["url_path"] == url) {
            Some(item) => json!({"item": item, "config": app.ws_resultat(json!({"type": "lovelace/config", "url_path": url})).await.ok()}),
            None => Value::Null,
        };
        elements.push(json!({"page": url, "avant": avant}));
    }
    if let Some((_, _, t)) = &theme {
        elements.push(if pour_tous {
            json!({"theme": "tous", "avant": {"default_theme": t["default_theme"], "default_dark_theme": t["default_dark_theme"]}})
        } else {
            json!({"theme": "moi", "avant": donnee(app, "theme").await?})
        });
    }
    let barre_avant = match barre {
        Some(_) => Some(donnee(app, "sidebar").await?),
        None => None,
    };
    if let Some(v) = &barre_avant {
        elements.push(json!({"barre": true, "avant": v}));
    }

    // 2. l'instantané d'annulation, AVANT toute écriture
    let mut l = annulations(app).await?;
    l.push(json!({"preset": id, "elements": elements}));
    let trop = l.len().saturating_sub(ANNULATIONS_MAX);
    l.drain(..trop);
    poser(app, CLE, json!({"annulations": l})).await?;

    // 3. les écritures
    for page in &pages {
        let url = page["url_path"].as_str().unwrap_or_default();
        let mut m = json!({});
        for k in CHAMPS_PAGE {
            if let Some(v) = page.get(k) {
                m[k] = v.clone();
            }
        }
        match existants.iter().find(|t| t["url_path"] == url) {
            Some(item) => {
                m["type"] = json!("lovelace/dashboards/update");
                m["dashboard_id"] = item["id"].clone();
            }
            None => {
                m["type"] = json!("lovelace/dashboards/create");
                m["url_path"] = json!(url);
            }
        }
        app.ws_resultat(m).await?;
        app.ws_resultat(json!({"type": "lovelace/config/save", "url_path": url, "config": page["config"]})).await?;
    }
    if let Some((nom, sombre, _)) = &theme {
        if pour_tous {
            theme_de_tous(app, nom, None).await?;
            if *sombre == Some(true) {
                theme_de_tous(app, nom, Some("dark")).await?;
            }
        } else {
            let mut v = json!({"theme": nom});
            if let Some(s) = sombre {
                v["dark"] = json!(s);
            }
            poser(app, "theme", v).await?;
        }
    }
    if let (Some((ordre, masques, position)), Some(actuel)) = (&barre, &barre_avant) {
        let mut o: Vec<String> = liste_textes(&actuel["panelOrder"]).into_iter().filter(|x| !ordre.contains(x)).collect();
        let i = (*position).min(o.len());
        o.splice(i..i, ordre.iter().cloned());
        let mut m: Vec<String> = liste_textes(&actuel["hiddenPanels"]);
        m.extend(masques.iter().filter(|x| !m.contains(x)).cloned().collect::<Vec<_>>());
        m.retain(|x| !ordre.contains(x));
        poser(app, "sidebar", json!({"panelOrder": o, "hiddenPanels": m})).await?;
    }
    let mut r = json!({
        "preset": id,
        "pages": pages.iter().map(|p| p["url_path"].clone()).collect::<Vec<_>>(),
        "theme": theme.as_ref().map(|t| t.0.clone()),
        "barre": barre.is_some(),
        "pour": if pour_tous { "tous" } else { "moi" },
        "cartes_requises": cartes,
    });
    if !cartes.is_empty() {
        r["note"] = json!("cartes à installer (HACS) si elles ne le sont pas : sans elles, les cartes qui les utilisent s'affichent en erreur");
    }
    Ok(r)
}

/// Annule la dernière application : restaure, dans l'ordre inverse, ce qu'elle avait remplacé.
pub async fn annuler(app: &App) -> Result<Value, String> {
    let _g = app.verrou_presets.lock().await;
    let mut l = annulations(app).await?;
    let derniere = l.pop().ok_or("aucune application de preset à annuler")?;
    let elements: Vec<Value> = derniere["elements"].as_array().cloned().unwrap_or_default();
    let existants = if elements.iter().any(|e| e.get("page").is_some()) { tableaux(app).await? } else { vec![] };
    let mut urls = vec![];
    for e in elements.iter().rev() {
        if let Some(url) = e["page"].as_str() {
            let id = existants.iter().find(|t| t["url_path"] == url).map(|t| t["id"].clone());
            match (&e["avant"], id) {
                (Value::Null, Some(id)) => {
                    app.ws_resultat(json!({"type": "lovelace/dashboards/delete", "dashboard_id": id})).await?;
                }
                (Value::Null, None) => {}
                (avant, id) => {
                    let mut m = json!({});
                    for k in CHAMPS_PAGE {
                        if let Some(v) = avant["item"].get(k).filter(|v| !v.is_null()) {
                            m[k] = v.clone();
                        }
                    }
                    match id {
                        Some(id) => {
                            m["type"] = json!("lovelace/dashboards/update");
                            m["dashboard_id"] = id;
                        }
                        None => {
                            m["type"] = json!("lovelace/dashboards/create");
                            m["url_path"] = json!(url);
                        }
                    }
                    app.ws_resultat(m).await?;
                    match &avant["config"] {
                        // tableau sans contenu enregistré avant : on retire celui du preset (absent : sans gravité)
                        Value::Null => {
                            let _ = app.ws_resultat(json!({"type": "lovelace/config/delete", "url_path": url})).await;
                        }
                        c => {
                            app.ws_resultat(json!({"type": "lovelace/config/save", "url_path": url, "config": c})).await?;
                        }
                    }
                }
            }
            urls.push(url.to_string());
        } else if e["theme"] == "moi" {
            poser(app, "theme", e["avant"].clone()).await?;
        } else if e["theme"] == "tous" {
            theme_de_tous(app, e["avant"]["default_theme"].as_str().unwrap_or("default"), None).await?;
            theme_de_tous(app, e["avant"]["default_dark_theme"].as_str().unwrap_or("none"), Some("dark")).await?;
        } else if e["barre"] == true {
            poser(app, "sidebar", e["avant"].clone()).await?;
        }
    }
    let restantes = l.len();
    poser(app, CLE, json!({"annulations": l})).await?;
    Ok(json!({"preset": derniere["preset"], "pages": urls, "restantes": restantes}))
}

// ───────────────────────── Exporter ─────────────────────────

/// Fait un preset de la configuration actuelle : pages stockées, thème et barre de l'utilisateur du jeton. Les
/// titres de page et le thème deviennent des paramètres. Écrit `<dossier>/<id>.json` si un dossier est donné.
pub async fn exporter(app: &App, a: &Map<String, Value>) -> Result<Value, String> {
    let id = a.get("preset").and_then(Value::as_str).ok_or("« preset » (texte) est requis")?;
    if !id_valide(id) {
        return Err("id refusé : a-z, 0-9, - et _ seulement (64 caractères au plus)".into());
    }
    if EMBARQUES.iter().filter_map(|s| serde_json::from_str::<Value>(s).ok()).any(|p| p["id"] == id) {
        return Err(format!("« {id} » est l'id d'un preset embarqué : choisir un autre id"));
    }
    let fichier = app.opts.presets.as_ref().map(|d| d.join(format!("{id}.json")));
    if fichier.as_ref().is_some_and(|f| f.exists()) && a.get("remplacer") != Some(&json!(true)) {
        return Err(format!("le preset « {id} » existe déjà (remplacer: true pour l'écraser)"));
    }
    let stockes: Vec<Value> = tableaux(app).await?.into_iter().filter(|t| t["mode"] != "yaml").collect();
    let choisis: Vec<&Value> = match a.get("pages").filter(|v| !v.is_null()) {
        None => stockes.iter().collect(),
        Some(v) => liste_textes(v).iter().map(|u| stockes.iter().find(|t| t["url_path"] == u.as_str()).ok_or(format!("aucune page stockée à l'adresse « {u} »"))).collect::<Result<_, _>>()?,
    };
    let mut parametres = Map::new();
    let mut pages = vec![];
    for (n, t) in choisis.iter().enumerate() {
        let p = format!("titre_{}", n + 1);
        parametres.insert(p.clone(), json!({"type": "texte", "defaut": t["title"], "description": format!("Titre de la page {}", t["url_path"].as_str().unwrap_or_default())}));
        let config = app.ws_resultat(json!({"type": "lovelace/config", "url_path": t["url_path"]})).await.ok().filter(|c| c.is_object()).unwrap_or(json!({"views": []}));
        let mut page = json!({"url_path": t["url_path"], "title": format!("[[{p}]]"), "show_in_sidebar": t["show_in_sidebar"] != false, "require_admin": t["require_admin"] == true, "config": config});
        if let Some(i) = t.get("icon").filter(|v| v.is_string()) {
            page["icon"] = i.clone();
        }
        pages.push(page);
    }
    let theme_utilisateur = donnee(app, "theme").await?;
    let theme = match theme_utilisateur["theme"].as_str() {
        Some(t) => Some(t.to_string()),
        None => app.ws_resultat(json!({"type": "frontend/get_themes"})).await.ok().and_then(|t| t["default_theme"].as_str().map(String::from)),
    }
    .filter(|t| t != "default");
    let mut preset = json!({
        "id": id,
        "nom": a.get("nom").and_then(Value::as_str).unwrap_or(id),
        "description": a.get("description").and_then(Value::as_str).unwrap_or("Exporté depuis la configuration actuelle."),
        "parametres": {},
        "cartes": [],
        "pages": pages,
    });
    if let Some(t) = theme {
        parametres.insert("theme".into(), json!({"type": "theme", "defaut": t, "description": "Thème à activer"}));
        preset["theme"] = json!({"nom": "[[theme]]"});
        if let Some(s) = theme_utilisateur["dark"].as_bool() {
            preset["theme"]["sombre"] = json!(s);
        }
    }
    let barre = donnee(app, "sidebar").await?;
    if barre.is_object() {
        preset["barre"] = json!({"ordre": barre["panelOrder"].as_array().cloned().unwrap_or_default(), "masques": barre["hiddenPanels"].as_array().cloned().unwrap_or_default(), "position": 0});
    }
    preset["parametres"] = Value::Object(parametres);
    if let Some(f) = &fichier {
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("dossier de presets : {e}"))?;
        }
        let texte = serde_json::to_string_pretty(&preset).map_err(|e| e.to_string())?;
        std::fs::write(f, texte + "\n").map_err(|e| format!("écriture de {id}.json : {e}"))?;
    }
    Ok(json!({"fichier": fichier.map(|_| format!("{id}.json")), "preset": preset}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embarques_valides() {
        let l = tous(None);
        assert_eq!(l.len(), 3);
        assert!(l.iter().all(|p| p.get("erreur").is_none() && id_valide(p["id"].as_str().unwrap())));
    }

    #[test]
    fn motifs() {
        assert!(correspond("light.*", "light.salon"));
        assert!(correspond("sensor.*energy*", "sensor.cuisine_energy_total"));
        assert!(!correspond("sensor.*energy*", "sensor.cuisine_power"));
        assert!(correspond("sun.sun", "sun.sun") && !correspond("sun.sun", "sun.lune"));
    }

    #[test]
    fn substitution_et_budget() {
        let vals: Map<String, Value> = serde_json::from_value(json!({"titre": "Accueil", "pieces": [{"id": "salon", "nom": "Salon"}, {"id": "cuisine", "nom": "Cuisine"}], "n": 3})).unwrap();
        let mut budget = NOEUDS_MAX;
        let v = substituer(&json!({"t": "[[titre]] !", "n": "[[n]]", "l": [{"$pour_chaque": "pieces", "$modele": {"area": "[[element]]", "nom": "[[element_nom]]"}}], "ids": "[[pieces]]"}), &vals, &mut budget).unwrap();
        assert_eq!(v, json!({"t": "Accueil !", "n": 3, "l": [{"area": "salon", "nom": "Salon"}, {"area": "cuisine", "nom": "Cuisine"}], "ids": ["salon", "cuisine"]}));
        assert!(substituer(&json!("[[absent]]"), &vals, &mut NOEUDS_MAX.clone()).is_err());
        // preset hostile : des $pour_chaque imbriqués dépassent le budget au lieu d'exploser en mémoire
        let grand: Vec<Value> = (0..400).map(|i| json!(i.to_string())).collect();
        let vals: Map<String, Value> = serde_json::from_value(json!({"l": grand})).unwrap();
        let imbrique = json!([{"$pour_chaque": "l", "$modele": [{"$pour_chaque": "l", "$modele": "[[element]]"}]}]);
        let mut budget = NOEUDS_MAX;
        assert!(substituer(&imbrique, &vals, &mut budget).unwrap_err().contains("trop gros"));
    }

    #[test]
    fn pieces_et_entites() {
        let zones = vec![json!({"area_id": "salon", "name": "Salon"}), json!({"area_id": "cuisine", "name": "Cuisine"})];
        assert_eq!(pieces(&zones, &json!("*"), "p").unwrap().as_array().unwrap().len(), 2);
        assert_eq!(pieces(&zones, &json!(["salon"]), "p").unwrap(), json!([{"id": "salon", "nom": "Salon"}]));
        assert_eq!(pieces(&zones, &json!(["CUISINE"]), "p").unwrap()[0]["id"], "cuisine");
        assert!(pieces(&zones, &json!(["grenier"]), "p").is_err());
        let etats = vec![("light.a".to_string(), json!("A")), ("light.b".to_string(), json!("B")), ("sun.sun".to_string(), json!("Soleil"))];
        assert_eq!(entites(&etats, &json!(["light.*", "sun.sun", "switch.futur"]), "e").unwrap(), json!([{"id": "light.a", "nom": "A"}, {"id": "light.b", "nom": "B"}, {"id": "sun.sun", "nom": "Soleil"}, {"id": "switch.futur", "nom": "switch.futur"}]));
    }

    #[test]
    fn dossier_de_presets() {
        let d = std::env::temp_dir().join(format!("ha-mcp-test-presets-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("perso.json"), r#"{"nom": "Perso", "pages": []}"#).unwrap();
        std::fs::write(d.join("maison-de-base.json"), "{}").unwrap();
        std::fs::write(d.join("Mauvais nom.json"), "{}").unwrap();
        std::fs::write(d.join("gros.json"), vec![b' '; (FICHIER_MAX + 1) as usize]).unwrap();
        let l = tous(Some(&d));
        let _ = std::fs::remove_dir_all(&d);
        let par_id = |id: &str| l.iter().find(|p| p["id"] == id).cloned().unwrap();
        assert_eq!(par_id("perso")["source"], "dossier");
        assert!(par_id("perso").get("erreur").is_none());
        assert!(l.iter().filter(|p| p["id"] == "maison-de-base").any(|p| p["erreur"].as_str().is_some_and(|e| e.contains("déjà pris"))));
        assert!(par_id("Mauvais nom")["erreur"].is_string());
        assert!(par_id("gros")["erreur"].as_str().unwrap().contains("trop gros"));
    }
}
