---
name: home-assistant-mcp-rust
description: Piloter une instance Home Assistant (ou maison) par le serveur MCP « Home Assistant MCP Rust » (binaire ha-mcp) : états, services (un outil par service, domaine__service), modèles Jinja, historique, journal, registres, tableaux de bord, barre latérale, intégrations à ajouter, automatisations/scripts/scènes, presets de préconfiguration, et toute commande WebSocket ou route REST. Déclencheurs : « allume/éteins … », « quel est l'état de … », « crée un tableau de bord », « ajoute l'intégration … », « écris une automatisation », « historique de … », « Home Assistant », « HA », outils du serveur MCP Home Assistant ou noms domaine__service. PAS pour installer ou mettre à jour Home Assistant lui-même.
---

# Home Assistant MCP Rust

Serveur MCP pour Home Assistant (toutes versions récentes, et maison). Tout est découvert à l'exécution : la liste
des outils dépend de l'instance (ses services, ses capacités), et change quand une intégration se charge ou qu'un
service apparaît (`notifications/tools/list_changed`). **Avant un travail un peu long : `lister_outils`** donne la
liste exacte de l'instance connectée, en markdown (un exemple sur une instance neuve :
[reference/exemple-outils.md](reference/exemple-outils.md)).

## Quel outil pour quel besoin

| Besoin | Outil |
|---|---|
| Agir sur un appareil (allumer, régler, ouvrir…) | l'outil du service, `domaine__service` (ex. `light__turn_on` avec `target: {entity_id: "light.salon"}`) ; à défaut `appeler_service` |
| Un service sans outil (filtré, apparu à l'instant, nom inconnu) | `appeler_service {domain, service, data, target}` |
| Savoir ce qui existe | `etats` (filtres domaine, pièce, recherche), `registres`, `services`, `integrations` |
| Une valeur calculée, une liste d'entités d'une pièce, une condition | `template` (Jinja de HA : `states()`, `area_entities()`, `is_state()`…) |
| Ce qui s'est passé | `historique` (valeurs), `journal` (événements lisibles) |
| Ce qui dépend d'un élément | `recherche_liee` |
| Tableaux de bord | `tableaux_lister`, `tableau_lire`, `tableau_enregistrer`, `page_creer`, `page_modifier`, `page_supprimer` |
| Barre latérale | `barre_regler` |
| Ajouter une intégration ou un helper à assistant (template, group…) | `integrations` (→ `disponibles`) puis `flux_demarrer` → `flux_etape`… → `create_entry` |
| Helpers input_*, timer, counter, schedule | `helper` (lister, creer, modifier, supprimer) |
| Automatisations, scripts, scènes | `automatisme_lire`, `automatisme_ecrire`, `automatisme_supprimer` |
| Une configuration toute faite (pages, thème, barre) | `presets_lister`, `preset_appliquer`, `preset_annuler`, `preset_exporter` |
| Tout le reste | `ws` (n'importe quelle commande WebSocket), `rest` (n'importe quelle route sous /api/) |

## Outils fixes

<!-- outils-fixes:debut -->

30 outils, toujours dans `tools/list` sauf si l'instance n'a pas la capacité (commande inconnue) ou si `--lecture-seule` retire l'outil. Noms nus, tels que le serveur les expose ; ton client peut les préfixer (`mcp__ha__etats`, `ha_etats`…). Détail complet de chaque paramètre : section « Outils génériques » plus bas.

| Outil | Rôle | Paramètres (* = requis) | Exemple | Validation admin |
|---|---|---|---|---|
| `etats` | Lire les états des entités | `entity_id`, `domaine`, `piece`, `recherche`, `attributs`, `limite` | `{piece: "salon", domaine: "light"}` | non |
| `services` | Décrire les services (champs, cible) | `domaine` | `{domaine: "light"}` | non |
| `appeler_service` | Appeler n'importe quel service | `domain`*, `service`*, `data`, `target`, `return_response` | `{domain: "light", service: "turn_off", target: {area_id: "salon"}}` | selon le service (redémarrer/arrêter HA, hôte, purge) |
| `template` | Rendre un modèle Jinja de HA | `template`*, `variables` | `{template: "{{ states('sun.sun') }}"}` | non |
| `historique` | Historique d'états | `entity_ids`*, `debut`, `fin`, `heures`, `attributs` | `{entity_ids: "sensor.temp_salon", heures: 6}` | non |
| `journal` | Journal d'événements lisible | `entity_id`, `debut`, `fin`, `heures` | `{entity_id: "binary_sensor.porte_entree"}` | non |
| `registres` | Entités, appareils, pièces, étages, étiquettes | `type`*, `filtre` | `{type: "pieces"}` | non |
| `recherche_liee` | Ce qui dépend d'un élément | `item_type`*, `item_id`* | `{item_type: "device", item_id: "abc123"}` | non |
| `integrations` | Intégrations chargées, entrées, disponibles | `filtre` | `{filtre: "shopping"}` | non |
| `flux_demarrer` | Ouvrir l'assistant d'une intégration | `handler`*, `show_advanced_options` | `{handler: "shopping_list"}` | non |
| `flux_etape` | Répondre à une étape d'assistant | `flow_id`*, `data` | `{flow_id: "…", data: {}}` | non |
| `flux_annuler` | Abandonner un assistant | `flow_id`* | `{flow_id: "…"}` | non |
| `tableaux_lister` | Lister les tableaux de bord | aucun | `{}` | non |
| `tableau_lire` | Lire le contenu d'un tableau | `url_path` | `{url_path: "salon-tablette"}` | non |
| `tableau_enregistrer` | Remplacer le contenu d'un tableau | `url_path`, `config`* | `{url_path: "salon-tablette", config: {views: []}}` | oui |
| `page_creer` | Créer un tableau (et son contenu) | `url_path`*, `title`*, `icon`, `show_in_sidebar`, `require_admin`, `config` | `{url_path: "cuisine-tablette", title: "Cuisine"}` | non |
| `page_modifier` | Changer titre, icône, visibilité, contenu | `url_path`, `title`, `icon`, `show_in_sidebar`, `require_admin`, `config` | `{url_path: "cuisine-tablette", icon: "mdi:stove"}` | non |
| `page_supprimer` | Supprimer un tableau | `url_path`* | `{url_path: "cuisine-tablette"}` | oui |
| `barre_regler` | Ordre et masquage de la barre latérale | `ordre`, `masques` | `{masques: ["energy"]}` | non |
| `automatisme_lire` | Lister / lire automatisations, scripts, scènes | `domaine`*, `id` | `{domaine: "automation"}` | non |
| `automatisme_ecrire` | Créer ou remplacer l'un des trois | `domaine`*, `id`*, `config`* | `{domaine: "script", id: "bonne_nuit", config: {sequence: []}}` | non |
| `automatisme_supprimer` | Supprimer l'un des trois | `domaine`*, `id`* | `{domaine: "scene", id: "film"}` | oui |
| `presets_lister` | Lister les presets | aucun | `{}` | non |
| `preset_appliquer` | Appliquer un preset (pages, thème, barre) | `preset`*, `parametres`, `morceaux`, `pour` | `{preset: "tableau-minimal", morceaux: ["pages"]}` | oui |
| `preset_annuler` | Annuler le dernier preset appliqué | aucun | `{}` | oui |
| `preset_exporter` | Faire un preset de la configuration actuelle | `preset`*, `nom`, `description`, `pages`, `remplacer` | `{preset: "ma-maison"}` | non |
| `ws` | Toute commande WebSocket de HA | `type`*, `attente_ms`, champs de la commande | `{type: "config/area_registry/list"}` | selon (toute commande qui écrit) |
| `rest` | Toute requête REST sous `/api/` | `path`*, `method`, `body` | `{path: "/api/config"}` | selon (toute méthode autre que GET) |
| `ha_stats` | Compteurs d'usage du serveur (lecture seule) | aucun | `{}` | non |
| `lister_outils` | Liste markdown des outils de l'instance | aucun | `{}` | non |

**Validation admin** : `oui` = écriture destructrice ou irréversible, à ne faire qu'après accord explicite de l'humain ; `selon` = dépend de l'appel (redémarrer ou arrêter HA, `hassio.host_reboot`/`host_shutdown`, `recorder.purge`, tout `DELETE`, toute commande `ws` qui écrit) ; `non` = lecture ou écriture bénigne. En mode réel le serveur ne bloque rien lui-même : l'accord est celui de ton client ou de l'humain. En mode test, c'est l'approbateur simulé (section « Mode test ») qui joue ce rôle.

<!-- outils-fixes:fin -->

## Outils dynamiques

Un outil par service de l'instance (`get_services`), reconstruit à chaque changement.
- **Nom** : `domaine__service` (deux soulignés), ex. `light__turn_on`, `shopping_list__add_item`. Contrainte MCP `^[a-zA-Z0-9_-]{1,64}$` : un nom trop long ou nettoyé devient ses 55 premiers caractères + `_` + 8 hex déterministes (rare ; `lister_outils` donne le vrai nom).
- **Schéma** : un paramètre par champ du service (sélecteurs HA convertis en JSON Schema, champs requis marqués) ; `target` (`entity_id`, `device_id`, `area_id`, `floor_id`, `label_id`, texte ou liste) si le service accepte une cible ; `return_response` (booléen) si la réponse est facultative, imposée d'office si elle est obligatoire.
- **Lister** : `tools/list` (le client le fait), ou `lister_outils` (markdown, génériques + services avec paramètres), ou `services {domaine}` pour la description brute d'un domaine.
- **Évolution** : `notifications/tools/list_changed` quand un service apparaît ou disparaît (ajout d'intégration, script créé) ; nouvelle liste en quelques secondes, relue aussi toutes les 30 s.
- **Limites** : `--domaines`, `--sans-domaines`, `--max-services N` réduisent la liste (`appeler_service` reste) ; `--lecture-seule` retire TOUS les outils de service. Ces outils n'ont pas de `readOnlyHint` : les croire potentiellement actifs.
- **Exemples** : `light__turn_on {target: {entity_id: "light.salon"}, brightness_pct: 50}` ; `climate__set_temperature {target: {entity_id: "climate.salon"}, temperature: 20}` ; `weather__get_forecasts {target: {entity_id: "weather.maison"}, type: "daily"}` (réponse renvoyée d'office).

## Mode test

Pour valider un client, un agent ou une chaîne **sans instance Home Assistant réelle** et sans rien risquer.
- **Activation, au démarrage seulement** : `MCP_TEST_MODE=1` ou `--test`. Aucun outil ne peut l'activer ni le couper ; pas d'URL ni de jeton lus ni requis.
- **Simulé** : tous les outils, fixes et dynamiques (catalogue de services de `test/fixtures.json`), répondent depuis une instance en mémoire qui garde son état (allumer une lumière la change). Les appels destructifs (redémarrer, arrêter, purge, hôte) sont simulés.
- **Marqueurs** : chaque réponse commence par `[TEST] ` et porte `_meta.test_mode: true` ; `ha_stats` annonce `mode: "test"`.
- **Garanties** : le client réseau réel n'est jamais construit ; un seul fichier (`src/reseau.rs`) fait tout accès réseau ; en mode test il est fermé et toute tentative est coupée avec `TEST_MODE_EGRESS_BLOCKED` et comptée (`coupures_reseau`). Preuves rejouables : `sh tests/verifier-reseau.sh` (A, statique), `cargo test --test mode_test --test coupe_circuit` (A dynamique + B), `sh tests/b-isole.sh` (B sous `docker run --network none`), `sh tests/mutation.sh`.
- **Approbateur simulé** : `MCP_TEST_APPROVER=approve|deny|timeout|absent` (ou `--test-approbateur`), défaut `deny`. Il ne joue que sur les outils de la colonne « Validation admin » (oui, ou selon quand l'appel est destructif) : `approve` laisse le simulateur exécuter ; `deny` répond `VALIDATION_REFUSEE`, `timeout` `VALIDATION_TIMEOUT`, `absent` (client sans capacité de confirmation) `VALIDATION_INDISPONIBLE` ; dans ces trois cas rien n'est exécuté. Hors mode test il est ignoré : une approbation simulée n'autorise jamais rien de réel.
- **Recette « valider toutes les commandes »** : lancer `MCP_TEST_MODE=1 MCP_TEST_APPROVER=approve ha-mcp` (ou `ha-mcp --test`), `tools/list`, appeler chaque outil (les outils dynamiques acceptent `{}` ou les champs de leur schéma), vérifier `[TEST]` partout, puis `ha_stats` : `total.reels` = 0, `coupures_reseau` = 0.
- **Recette « tester la validation »** : pour chaque outil `oui`/`selon`, relancer avec `approve`, `deny`, `timeout`, `absent` et vérifier : exécuté dans le simulateur une seule fois, puis trois refus nets sans effet ; `ha_stats.validations_simulees` compte `approuvees`, `refusees`, `timeouts`, `indisponibles`.
- **Avec un agent de surveillance** (ex. Hermes, de Toji) : lancer le serveur en mode test et lui faire rejouer les deux recettes. DÉDUIT : son format de configuration MCP n'a pas été vérifié ici ; déclarer le serveur comme n'importe quel serveur MCP stdio (commande `ha-mcp --test`).

## Stats

`ha_stats` (lecture seule, sans paramètre) renvoie des compteurs en mémoire depuis le démarrage : `mode` (`test` ou `reel`), `total` (`appels`, `erreurs`, `reels`, `test`), `coupures_reseau`, `validations_simulees` (`approuvees`, `refusees`, `timeouts`, `indisponibles`) et, par outil, `appels`, `erreurs`, `reels`, `test`, `p50_ms`, `p95_ms` (rang le plus proche, 1024 dernières durées), `dernier_appel`. Export local : `--stats-fichier chemin.json` ou `MCP_STATS_FILE`, réécrit après chaque appel. Aucun envoi réseau, aucun contenu d'appel n'est stocké.

## Possible

- Lire : états, services, registres, historique, journal, modèles Jinja, intégrations, tableaux, automatisations/scripts/scènes, liens entre éléments.
- Agir : tout service HA (outil dédié ou `appeler_service`), ajouter une intégration par assistant, créer/modifier/supprimer tableaux de bord, automatisations, scripts, scènes, régler la barre latérale, appliquer/annuler/exporter des presets.
- Le reste via `ws` (toute commande WebSocket) et `rest` (toute route `/api/`).
- Le serveur agit avec les droits du jeton longue durée de son utilisateur HA, pas plus : certains outils demandent un utilisateur **administrateur** (registres, tableaux, assistants d'intégration, automatisations).
- Mode `--lecture-seule` : seuls restent les outils de lecture ; `ws` limité aux commandes de lecture (`get_*`, `subscribe_*`, `*/list`, `*/get`, `*/info`), `rest` au GET.

## Impossible

- Installer, mettre à jour ou redémarrer Home Assistant lui-même, gérer les add-ons ou le Supervisor : aucun outil dédié (DÉDUIT : `ws`/`rest` ne couvrent que le WebSocket et `/api/` de Core).
- Dépasser les droits du jeton : utilisateur non administrateur = erreurs sur registres, tableaux, assistants, automatisations. Le serveur ne contourne rien.
- Installer des cartes HACS : un preset les signale, ne les installe pas.
- Modifier un tableau de bord en mode YAML, ou une automatisation venue du YAML (sans `id`) : refusé par Home Assistant.
- Lire une réponse de plus de 100 000 octets : elle est coupée ; filtrer.
- Sous `--lecture-seule` : aucune écriture, aucun outil de service, même via `ws`/`rest`.
- Un outil absent de la liste n'est pas forcément une panne : capacité absente de l'instance, ou filtre (`--domaines`, `--max-services`) ; voir `lister_outils`.
- Piège de sécurité : `ws` et `rest` sont annotés lecture seule (`readOnlyHint`) mais ÉCRIVENT hors `--lecture-seule`. Ne les croire inoffensifs que dans ce mode.

## Comment faire

Ordre général : `lister_outils` (ou `etats`, `registres`) pour savoir ce qui existe, lire avant d'écrire, écrire, relire pour vérifier.

### Recettes

**Allumer une lumière à 50 %** : `light__turn_on {target: {entity_id: "light.salon"}, brightness_pct: 50}`. La cible
accepte aussi `area_id`, `device_id`, `floor_id`, `label_id` (texte ou liste).

**Un service qui renvoie une réponse** (ex. `weather__get_forecasts`) : la réponse est demandée d'office quand le
service l'exige ; quand elle est facultative, passer `return_response: true`.

**Trouver les entités d'une pièce** : `etats {piece: "salon"}` (id ou nom de pièce), ou
`template {template: "{{ area_entities('salon') }}"}`.

**Ajouter une intégration sans matériel** (ex. liste de courses) :
1. `flux_demarrer {handler: "shopping_list"}` → `flow_id`, étape `user` avec son `data_schema` ;
2. `flux_etape {flow_id, data: {}}` (les champs du `data_schema`) → `type: "create_entry"` ;
3. une étape `menu` se répond par `flux_etape {flow_id, data: {next_step_id: "…"}}` ; `abort` = refus (déjà configuré,
   matériel absent…). Abandonner : `flux_annuler`.
Les nouveaux services arrivent seuls dans la liste des outils (list_changed) quelques secondes après.

**Créer un tableau de bord** : `page_creer {url_path: "salon-tablette", title: "Salon", icon: "mdi:sofa", config:
{views: [{title: "Salon", cards: [{type: "entities", entities: ["light.salon"]}]}]}}`. L'adresse doit contenir un
tiret. Modifier son contenu : `tableau_lire` puis `tableau_enregistrer` (remplace tout : partir de ce qui a été lu).

**Écrire une automatisation** : `automatisme_ecrire {domaine: "automation", id: "lumiere_coucher", config: {alias:
"Lumière au coucher", triggers: [{trigger: "sun", event: "sunset"}], actions: [{action: "light.turn_on", target:
{entity_id: "light.salon"}}], mode: "single"}}`. Sur une version ancienne de HA, les clés sont `trigger`/`action`
(singulier) et `platform` au lieu de `trigger` dans un déclencheur : en cas d'erreur de validation, lire une
automatisation existante (`automatisme_lire`) pour voir la forme attendue.

**Appliquer un preset** : `presets_lister` → `preset_appliquer {preset: "tableau-minimal", parametres: {…}}` ;
`morceaux: ["pages"]` pour n'appliquer qu'une partie ; `pour: "tous"` change le thème par défaut de tous les
utilisateurs. Revenir en arrière : `preset_annuler`.

**Une commande sans outil dédié** : `ws {type: "config/entity_registry/update", entity_id: "light.salon", name:
"Plafonnier"}` ; `ws {type: "subscribe_trigger", trigger: {…}, attente_ms: 5000}` collecte des événements 5 s.

### Pièges

- Un outil absent de la liste n'est pas une erreur : la capacité manque sur ce serveur (commande inconnue, détectée
  en l'essayant) ou le mode `--lecture-seule` l'a retiré. `lister_outils` dit lesquelles manquent.
- Les outils de service peuvent être limités (`--domaines`, `--sans-domaines`, `--max-services`) : `appeler_service`
  reste toujours là.
- `tableau_enregistrer` et `page_modifier {config}` remplacent tout le contenu du tableau : lire d'abord.
- Un tableau en mode YAML ne s'enregistre pas par l'API (erreur du serveur) : c'est voulu par Home Assistant.
- `automatisme_lire` sans id liste aussi les éléments venus du YAML : sans `id`, ils ne se modifient pas ici.
- Les réponses longues sont coupées à 100 000 octets : filtrer (`etats {domaine}`, `registres {filtre}`,
  `historique` sur moins d'heures).
- `historique` et `journal` prennent des dates ISO 8601 ; sans date, les 24 dernières heures.
- Le thème et la barre latérale réglés par `barre_regler` et les presets sont ceux de l'utilisateur du jeton.

### Erreurs et remèdes

- 401/403, « unauthorized » : jeton invalide, expiré ou utilisateur non administrateur. En créer un (profil HA, Sécurité, jetons longue durée) ; ne pas réessayer en boucle.
- `unknown_command` ou route 404 : l'instance n'a pas la capacité ; passer par `ws`/`rest` ou renoncer.
- Erreur de validation sur `automatisme_ecrire` : copier la forme d'une automatisation existante (`automatisme_lire`) ; clés `trigger`/`action` sur HA ancien, `triggers`/`actions` sur récent.
- `abort` dans un assistant : déjà configuré ou matériel absent ; lire `reason`, ne pas insister.
- Entité introuvable : `etats {recherche}` ou `registres {type: "entites", filtre}` avant de deviner un `entity_id`.
- Outil de service absent juste après un ajout d'intégration : attendre quelques secondes (`list_changed`), sinon `appeler_service`.

## Outils génériques

Liste générée depuis le code (un test vérifie qu'elle est à jour). Les outils de service, eux, dépendent de
l'instance : `lister_outils`.

<!-- outils-generiques:debut -->

### `etats`

*lecture*

États des entités. Avec entity_id : l'état complet d'une entité. Sinon une liste abrégée (entity_id, state, nom, unité, last_changed), filtrable par domaine, pièce (id ou nom) et recherche (dans l'entity_id ou le nom).

| paramètre | type | requis | description |
|---|---|---|---|
| `entity_id` | texte |  |  |
| `domaine` | texte |  | ex. light |
| `piece` | texte |  | id ou nom de pièce (registres type pieces) |
| `recherche` | texte |  | sous-chaîne, sans casse |
| `attributs` | booléen |  | états complets avec attributs (défaut : non) |
| `limite` | entier ≥ 1 |  | nombre d'états au plus (défaut 200) |

### `services`

*lecture*

Services du serveur (get_services). Sans domaine : la liste des services par domaine. Avec domaine : leur description complète (champs, sélecteurs, cible, réponse).

| paramètre | type | requis | description |
|---|---|---|---|
| `domaine` | texte |  |  |

### `appeler_service`

*écriture (retiré par --lecture-seule)*

Appelle n'importe quel service (call_service), même s'il n'a pas encore son outil domaine__service.

| paramètre | type | requis | description |
|---|---|---|---|
| `domain` | texte | oui |  |
| `service` | texte | oui |  |
| `data` | objet |  | champs du service |
| `target` | objet |  | entity_id / device_id / area_id / floor_id / label_id |
| `return_response` | booléen |  | obligatoire pour un service qui renvoie une réponse |

### `template`

*lecture*

Rend un modèle Jinja de Home Assistant (POST /api/template), ex. « {{ states('sun.sun') }} » ou « {{ area_entities('salon') }} ».

| paramètre | type | requis | description |
|---|---|---|---|
| `template` | texte | oui |  |
| `variables` | objet |  |  |

### `historique`

*lecture*

Historique d'états (GET /api/history/period) d'une ou plusieurs entités : les `heures` dernières (défaut 24) ou de `debut` à `fin` (ISO 8601).

| paramètre | type | requis | description |
|---|---|---|---|
| `entity_ids` | texte ou liste de texte | oui |  |
| `debut` | texte |  | ISO 8601, ex. 2026-01-31T08:00:00+00:00 |
| `fin` | texte |  | ISO 8601 (défaut : maintenant) |
| `heures` | nombre |  | durée avant fin si debut est absent (défaut 24) |
| `attributs` | booléen |  | garder les attributs (défaut : non, réponse minimale) |

### `journal`

*lecture*

Journal (GET /api/logbook) : les `heures` dernières (défaut 24) ou de `debut` à `fin`, pour une entité ou toutes.

| paramètre | type | requis | description |
|---|---|---|---|
| `entity_id` | texte |  |  |
| `debut` | texte |  |  |
| `fin` | texte |  |  |
| `heures` | nombre |  |  |

### `registres`

*lecture*

Un registre : entités, appareils, pièces, étages ou étiquettes (selon ce que le serveur connaît), filtrable par sous-chaîne.

| paramètre | type | requis | description |
|---|---|---|---|
| `type` | entites / appareils / pieces / etages / etiquettes | oui |  |
| `filtre` | texte |  | sous-chaîne cherchée dans chaque élément, sans casse |

### `recherche_liee`

*lecture · absent si le serveur ne connaît pas `search/related`*

Ce qui est lié à un élément (search/related) : automatisations, scripts, scènes, appareils, entités, pièces…

| paramètre | type | requis | description |
|---|---|---|---|
| `item_type` | texte | oui | area, automation, config_entry, device, entity, floor, group, label, scene, script… |
| `item_id` | texte | oui |  |

### `integrations`

*lecture*

Intégrations : chargées (get_config), entrées de configuration (domaine, titre, état), et domaines qu'on peut ajouter par assistant (flow_handlers). Filtrable par sous-chaîne.

| paramètre | type | requis | description |
|---|---|---|---|
| `filtre` | texte |  |  |

### `flux_demarrer`

*écriture (retiré par --lecture-seule)*

Démarre l'assistant d'ajout d'une intégration, ou d'un helper qui a un assistant (template, group, utility_meter…) : POST /api/config/config_entries/flow {handler}. Renvoie flow_id et la première étape (formulaire, menu, ou create_entry si rien n'est demandé). Les helpers input_*, timer, counter et schedule n'ont pas d'assistant : outil helper.

| paramètre | type | requis | description |
|---|---|---|---|
| `handler` | texte | oui | domaine de l'intégration (integrations → disponibles) |
| `show_advanced_options` | booléen |  |  |

### `flux_etape`

*écriture (retiré par --lecture-seule)*

Répond à une étape d'un assistant : POST /api/config/config_entries/flow/{flow_id} avec les valeurs du formulaire (data_schema de l'étape), ou {"next_step_id": …} pour un menu.

| paramètre | type | requis | description |
|---|---|---|---|
| `flow_id` | texte | oui |  |
| `data` | objet |  |  |

### `flux_annuler`

*écriture (retiré par --lecture-seule)*

Abandonne un assistant en cours (DELETE /api/config/config_entries/flow/{flow_id}).

| paramètre | type | requis | description |
|---|---|---|---|
| `flow_id` | texte | oui |  |

### `helper`

*écriture (retiré par --lecture-seule)*

Helpers sans assistant (input_boolean, input_number, input_text, input_select, input_datetime, input_button, timer, counter, schedule) : lister, creer, modifier, supprimer (commandes <domaine>/list, create, update, delete). L'entité (ex. input_boolean.<id>) apparaît aussitôt.

| paramètre | type | requis | description |
|---|---|---|---|
| `action` | lister / creer / modifier / supprimer | oui |  |
| `domaine` | input_boolean / input_number / input_text / input_select / input_datetime / input_button / timer / counter / schedule | oui |  |
| `id` | texte |  | modifier, supprimer : id du helper (champ id de lister, pas l'entity_id) |
| `champs` | objet |  | creer, modifier : ex. {name, icon} ; input_number {min, max, step, mode} ; input_select {options} ; input_text {min, max, pattern, mode} ; input_datetime {has_date, has_time} ; timer {duration: "00:05:00"} ; counter {initial, step, minimum, maximum} ; schedule {monday: [{from: "07:00:00", to: "09:00:00"}], …} |

### `tableaux_lister`

*lecture · absent si le serveur ne connaît pas `lovelace/dashboards/list`*

Tableaux de bord (lovelace/dashboards/list). Le tableau par défaut n'y figure pas : son url_path est absent (null).

Sans paramètre.

### `tableau_lire`

*lecture · absent si le serveur ne connaît pas `lovelace/dashboards/list`*

Contenu (views, cards) d'un tableau de bord (lovelace/config). Sans url_path : le tableau par défaut.

| paramètre | type | requis | description |
|---|---|---|---|
| `url_path` | texte |  |  |

### `tableau_enregistrer`

*écriture (retiré par --lecture-seule) · absent si le serveur ne connaît pas `lovelace/config/save`*

Remplace le contenu d'un tableau de bord (lovelace/config/save). Sans url_path : le tableau par défaut. Lire d'abord avec tableau_lire pour ne rien perdre.

| paramètre | type | requis | description |
|---|---|---|---|
| `url_path` | texte |  |  |
| `config` | objet | oui | {views: [...]} |

### `page_creer`

*écriture (retiré par --lecture-seule) · absent si le serveur ne connaît pas `lovelace/dashboards/create`*

Crée un tableau de bord (lovelace/dashboards/create) et, si `config` est donnée, enregistre son contenu (lovelace/config/save).

| paramètre | type | requis | description |
|---|---|---|---|
| `url_path` | texte | oui | adresse du tableau de bord, avec un tiret (ex. salon-tablette) |
| `title` | texte | oui |  |
| `icon` | texte |  | ex. mdi:sofa |
| `show_in_sidebar` | booléen |  |  |
| `require_admin` | booléen |  |  |
| `config` | objet |  | configuration Lovelace complète ({views: [...]}), enregistrée par lovelace/config/save |

### `page_modifier`

*écriture (retiré par --lecture-seule) · absent si le serveur ne connaît pas `lovelace/dashboards/list`*

Modifie un tableau de bord repéré par url_path : titre, icône, visibilité (lovelace/dashboards/update) et/ou contenu `config` (lovelace/config/save). Sans url_path : contenu du tableau par défaut.

| paramètre | type | requis | description |
|---|---|---|---|
| `url_path` | texte |  | adresse du tableau de bord, avec un tiret (ex. salon-tablette) |
| `title` | texte |  |  |
| `icon` | texte |  | ex. mdi:sofa |
| `show_in_sidebar` | booléen |  |  |
| `require_admin` | booléen |  |  |
| `config` | objet |  | configuration Lovelace complète ({views: [...]}), enregistrée par lovelace/config/save |

### `page_supprimer`

*écriture (retiré par --lecture-seule) · absent si le serveur ne connaît pas `lovelace/dashboards/list`*

Supprime un tableau de bord repéré par url_path (lovelace/dashboards/delete).

| paramètre | type | requis | description |
|---|---|---|---|
| `url_path` | texte | oui |  |

### `barre_regler`

*écriture (retiré par --lecture-seule) · absent si le serveur ne connaît pas `frontend/get_user_data`*

Règle la barre latérale de l'utilisateur du jeton (frontend/set_user_data, clé sidebar) : `ordre` = panneaux dans l'ordre voulu (ex. ["lovelace", "energy", "salon-tablette"]), `masques` = panneaux cachés. Un champ absent garde la valeur actuelle.

| paramètre | type | requis | description |
|---|---|---|---|
| `ordre` | liste de texte |  |  |
| `masques` | liste de texte |  |  |

### `automatisme_lire`

*lecture*

Automatisations, scripts, scènes. Sans id : la liste (entity_id, id, nom, état ; un élément sans id vient du YAML et ne se modifie pas ici). Avec id : sa configuration (GET /api/config/<domaine>/config/<id>).

| paramètre | type | requis | description |
|---|---|---|---|
| `domaine` | automation / script / scene | oui |  |
| `id` | texte |  | id de configuration (automatisation, scène : attribut id ; script : la fin de l'entity_id) |

### `automatisme_ecrire`

*écriture (retiré par --lecture-seule)*

Crée ou remplace une automatisation, un script ou une scène (POST /api/config/<domaine>/config/<id>) ; le serveur valide puis recharge. Nouvel id : n'importe quel texte unique (a-z, 0-9, _).

| paramètre | type | requis | description |
|---|---|---|---|
| `domaine` | automation / script / scene | oui |  |
| `id` | texte | oui |  |
| `config` | objet | oui | ex. automatisation : {alias, triggers, conditions, actions, mode} |

### `automatisme_supprimer`

*écriture (retiré par --lecture-seule)*

Supprime une automatisation, un script ou une scène créé par l'interface (DELETE /api/config/<domaine>/config/<id>).

| paramètre | type | requis | description |
|---|---|---|---|
| `domaine` | automation / script / scene | oui |  |
| `id` | texte | oui |  |

### `presets_lister`

*lecture*

Presets de préconfiguration (embarqués, et ceux du dossier --presets) : id, nom, description, paramètres déclarés avec leurs défauts, pages, thème, barre, cartes requises.

Sans paramètre.

### `preset_appliquer`

*écriture (retiré par --lecture-seule) · absent si le serveur ne connaît pas `frontend/get_user_data`*

Applique un preset en COPIE avec les commandes standard : pages (lovelace/dashboards/*, lovelace/config/save), thème, barre latérale. Ce qui est remplacé est gardé (données de l'utilisateur, clé ha_mcp_presets, 10 dernières) pour preset_annuler. Les cartes requises (HACS) sont signalées, pas installées.

| paramètre | type | requis | description |
|---|---|---|---|
| `preset` | texte | oui | id du preset (presets_lister) |
| `parametres` | objet |  | valeurs des paramètres déclarés ; un paramètre absent prend sa valeur par défaut |
| `morceaux` | liste de pages / theme / barre |  | morceaux à appliquer (défaut : tous) |
| `pour` | moi / tous |  | thème : l'utilisateur du jeton (défaut) ou le thème par défaut de tous (frontend.set_theme) ; la barre est toujours celle de l'utilisateur du jeton |

### `preset_annuler`

*écriture (retiré par --lecture-seule) · absent si le serveur ne connaît pas `frontend/get_user_data`*

Annule la dernière application de preset : remet les pages, le thème et la barre latérale d'avant.

Sans paramètre.

### `preset_exporter`

*écriture (retiré par --lecture-seule) · absent si le serveur ne connaît pas `lovelace/dashboards/list`*

Fait un preset de la configuration actuelle (pages stockées, thème et barre de l'utilisateur du jeton). Renvoie son JSON ; l'écrit aussi dans le dossier --presets s'il est donné.

| paramètre | type | requis | description |
|---|---|---|---|
| `preset` | texte | oui | id du nouveau preset : a-z, 0-9, - et _ |
| `nom` | texte |  |  |
| `description` | texte |  |  |
| `pages` | liste de texte |  | url_path des pages à reprendre (défaut : toutes les pages stockées) |
| `remplacer` | booléen |  | écraser un fichier de preset du même id |

### `ws`

*lecture*

Envoie N'IMPORTE QUELLE commande WebSocket de Home Assistant (après authentification) et renvoie son résultat : arguments = le message sans id, `type` et ses champs (ex. {"type": "config/entity_registry/list"}). `attente_ms` (max 20000) : collecte des événements d'une commande subscribe_*. En --lecture-seule : commandes de lecture seulement (get_*, subscribe_*, */list, */get, */info…).

| paramètre | type | requis | description |
|---|---|---|---|
| `type` | texte | oui |  |
| `attente_ms` | entier 0…20000 |  |  |
| *(autres)* | toute valeur |  | les champs de la commande |

### `rest`

*lecture*

N'IMPORTE QUELLE requête REST sous /api/ (ex. GET /api/config, GET /api/error_log). Réponse : statut + corps. En --lecture-seule : GET seulement.

| paramètre | type | requis | description |
|---|---|---|---|
| `method` | texte |  | GET (défaut), POST, PUT, PATCH, DELETE |
| `path` | texte | oui | chemin commençant par /api/, requête comprise |
| `body` | toute valeur |  | corps JSON |

### `ha_stats`

*lecture*

Statistiques d'usage de ce serveur : appels par outil, erreurs, latence p50/p95, dernier appel, appels réels ou en mode test, accès réseau coupés, validations simulées (JSON).

Sans paramètre.

### `lister_outils`

*lecture*

Liste complète, en markdown, des outils de cette instance : génériques et un par service, avec leurs paramètres.

Sans paramètre.

<!-- outils-generiques:fin -->
