---
name: home-assistant-mcp-rust
description: Piloter une instance Home Assistant (ou maison) par le serveur MCP « Home Assistant MCP Rust » (binaire ha-mcp) : états, services (un outil par service, domaine__service), modèles Jinja, historique, journal, registres, tableaux de bord, barre latérale, intégrations à ajouter, automatisations/scripts/scènes, presets de préconfiguration, et toute commande WebSocket ou route REST. Déclencheurs : « allume/éteins … », « quel est l'état de … », « crée un tableau de bord », « ajoute l'intégration … », « écris une automatisation », « historique de … », « Home Assistant », « HA », outils mcp__ha__* ou noms domaine__service. PAS pour installer ou mettre à jour Home Assistant lui-même.
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
| Ajouter une intégration ou un helper | `integrations` (→ `disponibles`) puis `flux_demarrer` → `flux_etape`… → `create_entry` |
| Automatisations, scripts, scènes | `automatisme_lire`, `automatisme_ecrire`, `automatisme_supprimer` |
| Une configuration toute faite (pages, thème, barre) | `presets_lister`, `preset_appliquer`, `preset_annuler`, `preset_exporter` |
| Tout le reste | `ws` (n'importe quelle commande WebSocket), `rest` (n'importe quelle route sous /api/) |

## Recettes

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

## Pièges

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

Démarre l'assistant d'ajout d'une intégration ou d'une entrée (helper) : POST /api/config/config_entries/flow {handler}. Renvoie flow_id et la première étape (formulaire, menu, ou create_entry si rien n'est demandé).

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

### `lister_outils`

*lecture*

Liste complète, en markdown, des outils de cette instance : génériques et un par service, avec leurs paramètres.

Sans paramètre.

<!-- outils-generiques:fin -->
