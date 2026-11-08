//! Services de Home Assistant (réponse de `get_services`) → outils MCP : nom d'outil, sélecteur → JSON Schema,
//! champs requis, cible, réponse du service. Rien n'est figé : tout vient de la description renvoyée par le serveur,
//! et un sélecteur inconnu donne un schéma libre (jamais un refus).

use std::collections::HashMap;

use serde_json::{Map, Value, json};

/// Ce qu'il faut pour exécuter l'outil d'un service.
#[derive(Clone, Debug, PartialEq)]
pub struct Cible {
    pub domaine: String,
    pub service: String,
    /// `Some(true)` : le service renvoie toujours une réponse (return_response obligatoire) ; `Some(false)` : réponse
    /// facultative (propriété `return_response`) ; `None` : aucune réponse.
    pub reponse: Option<bool>,
    /// La propriété `target` de l'outil est la cible de l'appel (et non un champ du service nommé « target »).
    pub cible: bool,
}

/// Restrictions de la liste des outils de service.
#[derive(Clone, Debug, Default)]
pub struct Filtre {
    /// Domaines gardés (vide : tous).
    pub domaines: Vec<String>,
    pub sans_domaines: Vec<String>,
    /// Nombre maximal d'outils de service (dans l'ordre des noms).
    pub max: Option<usize>,
}

/// FNV-1a 32 bits : suffixe déterministe des noms raccourcis.
fn fnv(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5, |h, o| (h ^ o as u32).wrapping_mul(0x0100_0193))
}

/// `domaine__service`, conforme à `^[a-zA-Z0-9_-]{1,64}$`. Un nom trop long ou qui a dû être nettoyé garde ses
/// 55 premiers caractères suivis de `_` et de 8 chiffres hexadécimaux tirés de « domaine.service ».
pub fn nom_outil(d: &str, s: &str) -> String {
    let propre = |x: &str| x.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect::<String>();
    let n = format!("{}__{}", propre(d), propre(s));
    if n.len() <= 64 && n == format!("{d}__{s}") {
        return n;
    }
    format!("{}_{:08x}", &n[..n.len().min(55)], fnv(&format!("{d}.{s}")))
}

fn ids() -> Value {
    json!({"anyOf": [{"type": "string"}, {"type": "array", "items": {"type": "string"}}]})
}

/// La cible d'un appel de service.
pub fn schema_cible() -> Value {
    let p: Map<String, Value> = ["entity_id", "device_id", "area_id", "floor_id", "label_id"].iter().map(|k| (k.to_string(), ids())).collect();
    json!({"type": "object", "description": "Cible : entity_id / device_id / area_id / floor_id / label_id (un id ou une liste)", "properties": p, "additionalProperties": false})
}

/// Domaines d'un filtre d'entité : `domain` (texte ou liste) de la config et de chaque `filter`.
fn domaines_filtre(cfg: &Map<String, Value>) -> Option<Vec<String>> {
    let mut filtres: Vec<&Value> = vec![];
    match cfg.get("filter") {
        Some(Value::Array(l)) => filtres.extend(l),
        Some(f @ Value::Object(_)) => filtres.push(f),
        _ => {}
    }
    let mut domaines = vec![];
    let ajouter = |v: &Value, l: &mut Vec<String>| match v {
        Value::String(s) => l.push(s.clone()),
        Value::Array(a) => l.extend(a.iter().filter_map(Value::as_str).map(String::from)),
        _ => {}
    };
    if !filtres.is_empty() {
        // un filtre sans domaine admet tout domaine
        if filtres.iter().any(|f| f.get("domain").is_none()) {
            return None;
        }
        for f in filtres {
            ajouter(&f["domain"], &mut domaines);
        }
    } else if let Some(d) = cfg.get("domain") {
        ajouter(d, &mut domaines);
    }
    let domaines: Vec<String> = domaines.into_iter().filter(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).collect();
    (!domaines.is_empty()).then_some(domaines)
}

fn nombre(cfg: &Map<String, Value>, min: &str, max: &str) -> Value {
    let mut n = json!({"type": "number"});
    for (de, vers) in [(min, "minimum"), (max, "maximum")] {
        if let Some(x) = cfg.get(de).filter(|x| x.is_number()) {
            n[vers] = x.clone();
        }
    }
    n
}

/// Sélecteur HA (`{"genre": {config}}`) → (JSON Schema, précision pour la description).
pub fn schema_selecteur(selecteur: &Value) -> (Value, Option<String>) {
    let Some((genre, cfg)) = selecteur.as_object().and_then(|s| s.iter().next()) else { return (json!({}), None) };
    let vide = Map::new();
    let cfg = cfg.as_object().unwrap_or(&vide);
    let mut note = None;
    let mut multiple = cfg.get("multiple") == Some(&Value::Bool(true));
    let texte = |format: Option<&str>| match format {
        Some(f) => json!({"type": "string", "format": f}),
        None => json!({"type": "string"}),
    };
    let s = match genre.as_str() {
        "boolean" => json!({"type": "boolean"}),
        "number" => {
            let mut precisions = vec![];
            if let Some(p) = cfg.get("step").filter(|p| p.is_number()) {
                precisions.push(format!("pas {p}"));
            }
            if let Some(u) = cfg.get("unit_of_measurement").and_then(Value::as_str) {
                precisions.push(format!("unité {u}"));
            }
            note = (!precisions.is_empty()).then(|| precisions.join(", "));
            nombre(cfg, "min", "max")
        }
        "select" => {
            let valeurs: Vec<Value> = cfg.get("options").and_then(Value::as_array).into_iter().flatten().map(|o| o.get("value").cloned().unwrap_or_else(|| o.clone())).collect();
            if cfg.get("custom_value") == Some(&Value::Bool(true)) || valeurs.is_empty() || !valeurs.iter().all(Value::is_string) {
                if !valeurs.is_empty() {
                    note = Some(format!("valeurs proposées : {}", valeurs.iter().map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string())).collect::<Vec<_>>().join(", ")));
                }
                json!({"type": "string"})
            } else {
                json!({"type": "string", "enum": valeurs})
            }
        }
        "entity" => {
            let mut e = json!({"type": "string"});
            match domaines_filtre(cfg) {
                Some(d) => {
                    note = Some(format!("entity_id ({})", d.join(", ")));
                    e["pattern"] = json!(format!("^({})\\.", d.join("|")));
                }
                None => note = Some("entity_id".into()),
            }
            e
        }
        "device" => {
            note = Some("id d'appareil (registres type appareils)".into());
            json!({"type": "string"})
        }
        "area" | "floor" | "label" => {
            note = Some(format!("id de {}", match genre.as_str() { "area" => "pièce", "floor" => "étage", _ => "étiquette" }));
            json!({"type": "string"})
        }
        "target" => {
            multiple = false;
            schema_cible()
        }
        "object" => match cfg.get("fields").and_then(Value::as_object) {
            Some(champs) => {
                let (p, requis) = proprietes(champs);
                json!({"type": "object", "properties": p, "required": requis, "additionalProperties": true})
            }
            None => {
                note = Some("objet ou liste (JSON)".into());
                json!({})
            }
        },
        "color_rgb" => json!({"type": "array", "items": {"type": "integer", "minimum": 0, "maximum": 255}, "minItems": 3, "maxItems": 3}),
        "color_temp" => {
            let kelvin = cfg.get("unit").and_then(Value::as_str) == Some("kelvin");
            note = Some(if kelvin { "kelvin".into() } else { "mireds".into() });
            if cfg.contains_key("min") || cfg.contains_key("max") { nombre(cfg, "min", "max") } else { nombre(cfg, "min_mireds", "max_mireds") }
        }
        "constant" => json!({"const": cfg.get("value").cloned().unwrap_or(Value::Bool(true))}),
        "duration" => json!({"anyOf": [
            {"type": "string", "description": "HH:MM:SS"},
            {"type": "number", "description": "secondes"},
            {"type": "object", "properties": {"days": {"type": "number"}, "hours": {"type": "number"}, "minutes": {"type": "number"}, "seconds": {"type": "number"}, "milliseconds": {"type": "number"}}}
        ]}),
        "location" => json!({"type": "object", "properties": {"latitude": {"type": "number"}, "longitude": {"type": "number"}, "radius": {"type": "number"}}, "required": ["latitude", "longitude"]}),
        "media" => json!({"type": "object", "properties": {"entity_id": {"type": "string"}, "media_content_id": {"type": "string"}, "media_content_type": {"type": "string"}, "metadata": {"type": "object"}}, "additionalProperties": true}),
        "date" => texte(Some("date")),
        "datetime" => {
            note = Some("AAAA-MM-JJ HH:MM:SS".into());
            json!({"type": "string"})
        }
        "time" => {
            note = Some("HH:MM:SS".into());
            json!({"type": "string"})
        }
        "template" => {
            note = Some("modèle Jinja".into());
            json!({"type": "string"})
        }
        "icon" => {
            note = Some("ex. mdi:lightbulb".into());
            json!({"type": "string"})
        }
        "country" | "language" => {
            let liste = cfg.get(if genre == "country" { "countries" } else { "languages" }).and_then(Value::as_array).filter(|l| !l.is_empty() && l.iter().all(Value::is_string));
            match liste {
                Some(l) => json!({"type": "string", "enum": l}),
                None => json!({"type": "string"}),
            }
        }
        "text" | "theme" | "state" | "attribute" | "conversation_agent" | "addon" | "app" | "backup_location" | "assist_pipeline" | "config_entry" | "qr_code" | "device_class" | "state_class" | "statistic" | "unit_of_measurement" | "file" | "serial_port" => {
            if genre == "text" && cfg.get("multiline") == Some(&Value::Bool(true)) {
                note = Some("texte sur plusieurs lignes".into());
            }
            json!({"type": "string"})
        }
        // action, condition, trigger, choose, numeric_threshold, automation_behavior… et tout genre à venir
        _ => json!({}),
    };
    (if multiple { json!({"type": "array", "items": s}) } else { s }, note)
}

/// Les champs, sections dépliées (une section a ses propres `fields`).
pub fn champs_a_plat(champs: &Map<String, Value>, sortie: &mut Vec<(String, Value)>) {
    for (k, v) in champs {
        match v.get("fields").and_then(Value::as_object).filter(|_| v.get("selector").is_none()) {
            Some(sous) => champs_a_plat(sous, sortie),
            None => sortie.push((k.clone(), v.clone())),
        }
    }
}

fn phrase(nom: Option<&str>, description: Option<&str>) -> Option<String> {
    match (nom.filter(|x| !x.trim().is_empty()), description.filter(|x| !x.trim().is_empty())) {
        (Some(n), Some(d)) if n != d => Some(format!("{n} : {d}")),
        (a, b) => a.or(b).map(String::from),
    }
}

fn exemple(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) if s.is_empty() => None,
        Value::String(s) => Some(s.clone()),
        v => Some(v.to_string()),
    }
}

/// Propriétés et champs requis d'une liste de champs (`fields` d'un service ou d'un sélecteur object).
fn proprietes(champs: &Map<String, Value>) -> (Map<String, Value>, Vec<String>) {
    let mut a_plat = Vec::new();
    champs_a_plat(champs, &mut a_plat);
    let mut p = Map::new();
    let mut requis = Vec::new();
    for (k, c) in a_plat {
        let (mut s, note) = c.get("selector").map(schema_selecteur).unwrap_or((json!({}), None));
        let morceaux: Vec<String> = [phrase(c["name"].as_str(), c["description"].as_str()), note, exemple(&c["example"]).map(|e| format!("ex. : {e}"))].into_iter().flatten().collect();
        if let Some(o) = s.as_object_mut() {
            if !morceaux.is_empty() {
                o.insert("description".into(), json!(morceaux.join(" — ")));
            }
            if let Some(d) = c.get("default").filter(|d| !d.is_null()) {
                o.insert("default".into(), d.clone());
            }
        }
        if c["required"] == true {
            requis.push(k.clone());
        }
        p.insert(k, s);
    }
    (p, requis)
}

/// L'outil MCP d'un service (`desc` : entrée de `get_services`) et ce qu'il faut pour l'exécuter.
pub fn outil_service(d: &str, s: &str, desc: &Value) -> (Value, Cible) {
    let vide = Map::new();
    let (mut p, requis) = proprietes(desc.get("fields").and_then(Value::as_object).unwrap_or(&vide));
    let cible = desc.get("target").is_some_and(|t| !t.is_null()) && !p.contains_key("target");
    if cible {
        p.insert("target".into(), schema_cible());
    }
    let reponse = desc.get("response").filter(|r| r.is_object()).map(|r| r["optional"] != true);
    if reponse == Some(false) && !p.contains_key("return_response") {
        p.insert("return_response".into(), json!({"type": "boolean", "default": false, "description": "renvoyer la réponse du service"}));
    }
    let mut description = format!("{d}.{s}");
    if let Some(t) = phrase(desc["name"].as_str(), desc["description"].as_str()) {
        description.push_str(" — ");
        description.push_str(&t);
    }
    if reponse == Some(true) {
        description.push_str(" (renvoie une réponse)");
    }
    let mut schema = json!({"type": "object", "properties": p});
    if !requis.is_empty() {
        schema["required"] = json!(requis);
    }
    (json!({"name": nom_outil(d, s), "description": description, "inputSchema": schema}), Cible { domaine: d.into(), service: s.into(), reponse, cible })
}

/// Un outil par service de `get_services`, triés par nom, filtrés ; les noms en double (cas extrême du nettoyage)
/// sont écartés.
pub fn outils_services(services: &Value, f: &Filtre) -> (Vec<Value>, HashMap<String, Cible>) {
    let mut l: Vec<(Value, Cible)> = vec![];
    for (d, ss) in services.as_object().into_iter().flatten() {
        if (!f.domaines.is_empty() && !f.domaines.contains(d)) || f.sans_domaines.contains(d) {
            continue;
        }
        for (s, desc) in ss.as_object().into_iter().flatten() {
            l.push(outil_service(d, s, desc));
        }
    }
    l.sort_by(|a, b| a.0["name"].as_str().cmp(&b.0["name"].as_str()));
    let mut cibles = HashMap::new();
    let mut outils = vec![];
    for (o, c) in l {
        if f.max.is_some_and(|m| outils.len() >= m) {
            break;
        }
        let nom = o["name"].as_str().unwrap_or_default().to_string();
        if cibles.contains_key(&nom) {
            continue;
        }
        cibles.insert(nom, c);
        outils.push(o);
    }
    (outils, cibles)
}

/// Arguments d'un outil de service → message `call_service`.
pub fn appel(c: &Cible, mut a: Map<String, Value>) -> Value {
    let mut m = json!({"type": "call_service", "domain": c.domaine, "service": c.service});
    if c.cible
        && let Some(t) = a.remove("target").filter(|t| !t.is_null())
    {
        m["target"] = t;
    }
    let demandee = c.reponse == Some(false) && a.remove("return_response") == Some(Value::Bool(true));
    if c.reponse == Some(true) || demandee {
        m["return_response"] = json!(true);
    }
    m["service_data"] = Value::Object(a);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nom_valide(n: &str) -> bool {
        (1..=64).contains(&n.len()) && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    }

    #[test]
    fn noms_d_outils() {
        assert_eq!(nom_outil("light", "turn_on"), "light__turn_on");
        let long = nom_outil(&"d".repeat(40), &"s".repeat(40));
        assert_eq!(long.len(), 64);
        assert!(nom_valide(&long));
        assert_eq!(long, nom_outil(&"d".repeat(40), &"s".repeat(40)), "déterministe");
        assert_ne!(long, nom_outil(&"d".repeat(40), &format!("{}t", "s".repeat(39))), "deux services, deux noms");
        let sale = nom_outil("dom.aine", "sérvice");
        assert!(nom_valide(&sale), "{sale}");
        assert_ne!(sale, nom_outil("dom_aine", "s_rvice"), "un nom nettoyé ne prend pas celui d'un service propre");
    }

    #[test]
    fn selecteurs() {
        let s = |v: Value| schema_selecteur(&v).0;
        assert_eq!(s(json!({"boolean": {}})), json!({"type": "boolean"}));
        assert_eq!(s(json!({"number": {"min": 0, "max": 255, "step": 1}})), json!({"type": "number", "minimum": 0, "maximum": 255}));
        assert_eq!(schema_selecteur(&json!({"number": {"min": 0, "max": 100, "step": 0.5, "unit_of_measurement": "%"}})).1.as_deref(), Some("pas 0.5, unité %"));
        assert_eq!(s(json!({"select": {"options": ["a", "b"]}})), json!({"type": "string", "enum": ["a", "b"]}));
        assert_eq!(s(json!({"select": {"options": [{"value": "x", "label": "X"}], "multiple": true}})), json!({"type": "array", "items": {"type": "string", "enum": ["x"]}}));
        assert_eq!(s(json!({"select": {"options": ["a"], "custom_value": true}})), json!({"type": "string"}));
        assert_eq!(s(json!({"text": null})), json!({"type": "string"}));
        assert_eq!(s(json!({"entity": {"domain": "light"}}))["pattern"], "^(light)\\.");
        assert_eq!(s(json!({"entity": {"filter": [{"domain": ["light", "switch"]}]}}))["pattern"], "^(light|switch)\\.");
        assert!(s(json!({"entity": {"filter": [{"domain": "light"}, {"integration": "hue"}]}})).get("pattern").is_none(), "un filtre sans domaine admet tout");
        assert_eq!(s(json!({"entity": {"multiple": true}})), json!({"type": "array", "items": {"type": "string"}}));
        let t = s(json!({"target": {"entity": {"domain": "light"}}}));
        assert_eq!(t["type"], "object");
        assert!(["entity_id", "device_id", "area_id", "floor_id", "label_id"].iter().all(|k| t["properties"].get(*k).is_some()));
        let o = s(json!({"object": {"fields": {"nom": {"selector": {"text": {}}, "required": true}, "age": {"selector": {"number": {}}}}}}));
        assert_eq!(o["properties"]["nom"]["type"], "string");
        assert_eq!(o["required"], json!(["nom"]));
        assert_eq!(s(json!({"object": {}})), json!({}));
        assert_eq!(s(json!({"color_rgb": {}}))["maxItems"], 3);
        assert_eq!(s(json!({"color_temp": {"unit": "kelvin", "min": 2000, "max": 6500}})), json!({"type": "number", "minimum": 2000, "maximum": 6500}));
        assert_eq!(s(json!({"color_temp": {"min_mireds": 153, "max_mireds": 500}})), json!({"type": "number", "minimum": 153, "maximum": 500}));
        assert_eq!(s(json!({"constant": {"value": "on"}})), json!({"const": "on"}));
        assert!(s(json!({"duration": {}}))["anyOf"].is_array());
        assert_eq!(s(json!({"location": {}}))["required"], json!(["latitude", "longitude"]));
        assert_eq!(s(json!({"country": {"countries": ["FR", "BE"]}}))["enum"], json!(["FR", "BE"]));
        // genre inconnu ou à venir : schéma libre, jamais un refus
        assert_eq!(s(json!({"genre_de_2030": {"x": 1}})), json!({}));
        assert_eq!(s(json!({"action": {}})), json!({}));
        assert_eq!(s(json!(null)), json!({}));
    }

    #[test]
    fn outil_d_un_service() {
        let desc = json!({
            "name": "Turn on", "description": "Turns on one or more lights.",
            "fields": {
                "transition": {"selector": {"number": {"min": 0, "max": 300}}, "name": "Transition", "description": "Duration", "example": 60},
                "advanced_fields": {"collapsed": true, "fields": {"brightness": {"selector": {"number": {"min": 0, "max": 255}}, "required": true, "default": 128}}}
            },
            "target": {"entity": [{"domain": ["light"]}]}
        });
        let (o, c) = outil_service("light", "turn_on", &desc);
        assert_eq!(o["name"], "light__turn_on");
        assert_eq!(o["description"], "light.turn_on — Turn on : Turns on one or more lights.");
        let p = &o["inputSchema"]["properties"];
        assert_eq!(p["transition"]["description"], "Transition : Duration — ex. : 60");
        assert_eq!(p["brightness"]["default"], 128, "section dépliée, défaut gardé");
        assert_eq!(o["inputSchema"]["required"], json!(["brightness"]));
        assert_eq!(p["target"]["type"], "object");
        assert_eq!(c, Cible { domaine: "light".into(), service: "turn_on".into(), reponse: None, cible: true });

        let m = appel(&c, serde_json::from_value(json!({"target": {"entity_id": "light.salon"}, "brightness": 10})).unwrap());
        assert_eq!(m, json!({"type": "call_service", "domain": "light", "service": "turn_on", "target": {"entity_id": "light.salon"}, "service_data": {"brightness": 10}}));

        let (o, c) = outil_service("weather", "get_forecasts", &json!({"fields": {"type": {"required": true, "selector": {"select": {"options": ["daily", "hourly"]}}}}, "target": {}, "response": {"optional": false}}));
        assert_eq!(c.reponse, Some(true));
        assert!(o["inputSchema"]["properties"].get("return_response").is_none());
        assert_eq!(appel(&c, Map::new())["return_response"], true);

        let (o, c) = outil_service("script", "x", &json!({"fields": {}, "response": {"optional": true}}));
        assert_eq!(o["inputSchema"]["properties"]["return_response"]["type"], "boolean");
        assert!(o["inputSchema"].get("required").is_none());
        assert_eq!(appel(&c, serde_json::from_value(json!({"return_response": true})).unwrap())["return_response"], true);
        assert!(appel(&c, Map::new()).get("return_response").is_none());

        // un champ du service nommé « target » reste un champ de service_data
        let (o, c) = outil_service("notify", "send", &json!({"fields": {"target": {"selector": {"text": {}}}}, "target": {}}));
        assert_eq!(o["inputSchema"]["properties"]["target"]["type"], "string");
        assert!(!c.cible);
        assert_eq!(appel(&c, serde_json::from_value(json!({"target": "x"})).unwrap())["service_data"]["target"], "x");
    }

    #[test]
    fn filtre_des_services() {
        let services = json!({"light": {"turn_on": {}, "turn_off": {}}, "switch": {"toggle": {}}, "zone": {"reload": {}}});
        let n = |f: Filtre| outils_services(&services, &f).0.iter().map(|o| o["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(n(Filtre::default()), ["light__turn_off", "light__turn_on", "switch__toggle", "zone__reload"]);
        assert_eq!(n(Filtre { domaines: vec!["light".into()], ..Default::default() }), ["light__turn_off", "light__turn_on"]);
        assert_eq!(n(Filtre { sans_domaines: vec!["light".into()], ..Default::default() }), ["switch__toggle", "zone__reload"]);
        assert_eq!(n(Filtre { max: Some(1), ..Default::default() }), ["light__turn_off"]);
    }
}
