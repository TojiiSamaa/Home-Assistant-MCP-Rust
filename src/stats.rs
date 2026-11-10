//! Statistiques d'usage (outil `ha_stats`, lecture seule) : appels par outil, erreurs, latence p50/p95, dernier appel,
//! appels réels ou en mode test, accès réseau coupés, validations simulées. En mémoire ; export JSON local si
//! `--stats-fichier` (la seule écriture, un fichier local).

use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::{outils, reseau};

/// Latences gardées par outil (les plus récentes).
const FENETRE: usize = 1024;

#[derive(Default)]
struct Outil {
    appels: u64,
    erreurs: u64,
    reels: u64,
    test: u64,
    latences_us: VecDeque<u64>,
    dernier: Option<String>,
}

/// Issue d'une demande de validation simulée (mode test).
#[derive(Clone, Copy)]
pub enum Validation {
    Approuvee,
    Refusee,
    Timeout,
    Indisponible,
}

#[derive(Default)]
pub struct Stats {
    outils: Mutex<BTreeMap<String, Outil>>,
    /// approuvées, refusées, timeouts, indisponibles
    validations: Mutex<[u64; 4]>,
}

/// Rang `p` (0..1) par la méthode du rang le plus proche, en millisecondes.
fn percentile(tries: &[u64], p: f64) -> f64 {
    if tries.is_empty() {
        return 0.0;
    }
    let i = ((p * tries.len() as f64).ceil() as usize).clamp(1, tries.len()) - 1;
    tries[i] as f64 / 1000.0
}

impl Stats {
    pub fn noter(&self, nom: &str, ok: bool, duree: Duration, test: bool) {
        let mut m = self.outils.lock().unwrap_or_else(|e| e.into_inner());
        let o = m.entry(nom.to_string()).or_default();
        o.appels += 1;
        o.erreurs += u64::from(!ok);
        if test {
            o.test += 1;
        } else {
            o.reels += 1;
        }
        if o.latences_us.len() == FENETRE {
            o.latences_us.pop_front();
        }
        o.latences_us.push_back(duree.as_micros() as u64);
        let s = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        o.dernier = Some(outils::iso(s));
    }

    pub fn valider(&self, v: Validation) {
        self.validations.lock().unwrap_or_else(|e| e.into_inner())[v as usize] += 1;
    }

    pub fn json(&self, test: bool) -> Value {
        let m = self.outils.lock().unwrap_or_else(|e| e.into_inner());
        let v = *self.validations.lock().unwrap_or_else(|e| e.into_inner());
        let (mut appels, mut erreurs, mut reels, mut en_test) = (0, 0, 0, 0);
        let par_outil: serde_json::Map<String, Value> = m
            .iter()
            .map(|(nom, o)| {
                let mut l: Vec<u64> = o.latences_us.iter().copied().collect();
                l.sort_unstable();
                (appels, erreurs, reels, en_test) = (appels + o.appels, erreurs + o.erreurs, reels + o.reels, en_test + o.test);
                (nom.clone(), json!({"appels": o.appels, "erreurs": o.erreurs, "reels": o.reels, "test": o.test, "p50_ms": percentile(&l, 0.5), "p95_ms": percentile(&l, 0.95), "dernier_appel": o.dernier}))
            })
            .collect();
        json!({
            "mode": if test { "test" } else { "reel" },
            "total": {"appels": appels, "erreurs": erreurs, "reels": reels, "test": en_test},
            "coupures_reseau": reseau::coupures(),
            "validations_simulees": {"approuvees": v[0], "refusees": v[1], "timeouts": v[2], "indisponibles": v[3]},
            "outils": par_outil,
        })
    }
}
