# Home Assistant MCP Rust

[English](README.md)

Un serveur [MCP](https://modelcontextprotocol.io) pour Home Assistant, en Rust. C'est un seul binaire statique,
`ha-mcp` (environ 5 Mo, sans runtime), et l'image Docker est construite `FROM scratch`. Il marche avec toute version
récente de Home Assistant et avec maison (réécriture de Home Assistant en Rust, image
`ghcr.io/tojiisamaa/home-assistant-rust`), parce qu'il découvre tout à l'exécution :
- **Un outil par service**, nommé `domaine__service` (par exemple `light__turn_on` ou `shopping_list__add_item`).
  Chacun est construit à partir de la description du service : champs, sélecteurs convertis en JSON Schema, champs
  requis, `target` et `return_response`.
- **Les capacités sont détectées en les essayant**, jamais en comparant des versions. Si une commande répond
  `unknown_command` (ou une route 404), les outils qui en dépendent sont retirés, et `lister_outils` dit lesquelles
  manquent.
- **La liste des outils se met à jour en direct.** Le serveur écoute `service_registered`, `service_removed` et
  `component_loaded`, relit la liste des services toutes les 30 s par sécurité, et se reconnecte quand le lien tombe.
  Chaque changement est annoncé aux clients par `notifications/tools/list_changed`. Ajoute une intégration : ses
  services arrivent comme outils quelques secondes après.

## Outils

Les outils génériques sont toujours là, sauf si le serveur n'a pas la capacité dont ils dépendent ou si
`--lecture-seule` retire l'outil.

| Besoin | Outils |
|---|---|
| Entités et services | `etats`, `services`, `appeler_service` (n'importe quel service, toujours présent), `template` |
| Historique | `historique`, `journal` |
| Registres et liens | `registres` (entités, appareils, pièces, étages, étiquettes), `recherche_liee` |
| Intégrations (assistants) | `integrations`, `flux_demarrer`, `flux_etape`, `flux_annuler` |
| Tableaux de bord et barre latérale | `tableaux_lister`, `tableau_lire`, `tableau_enregistrer`, `page_creer`, `page_modifier`, `page_supprimer`, `barre_regler` |
| Automatisations, scripts, scènes | `automatisme_lire`, `automatisme_ecrire`, `automatisme_supprimer` |
| Presets (pages, thème, barre) | `presets_lister`, `preset_appliquer`, `preset_annuler`, `preset_exporter` |
| Tout le reste | `ws` (n'importe quelle commande WebSocket), `rest` (n'importe quelle route sous `/api/`) |
| Auto-description | `lister_outils` (tous les outils de l'instance, en markdown) |

La [skill](skill/home-assistant-mcp-rust/SKILL.md) documente les paramètres de chaque outil générique, dans une
section générée depuis le code et vérifiée par un test.
[skill/home-assistant-mcp-rust/reference/exemple-outils.md](skill/home-assistant-mcp-rust/reference/exemple-outils.md)
est la vraie sortie de `ha-mcp lister` sur un Home Assistant neuf.

**Les presets** s'appliquent côté client avec les commandes standard : ils créent des pages de tableau de bord,
règlent le thème et la barre latérale. Ce qu'ils remplacent est d'abord sauvegardé (données frontend de l'utilisateur
du jeton, les 10 dernières), donc `preset_annuler` le remet. Trois presets sont embarqués ; `--presets DOSSIER` ajoute
tes fichiers JSON, et `preset_exporter` fait un preset de la configuration actuelle.

## Installation

**Binaire** : télécharge `ha-mcp-linux-amd64`, `ha-mcp-linux-arm64` ou `ha-mcp-windows-amd64.exe` depuis les
[publications](https://github.com/TojiiSamaa/Home-Assistant-MCP-Rust/releases) (sommes de contrôle dans
`SHA256SUMS`), renomme-le `ha-mcp` et mets-le dans ton `PATH`. Les versions Linux sont statiques (musl) et tournent
sur toute distribution.

**Docker** : `ghcr.io/tojiisamaa/home-assistant-mcp-rust` (linux/amd64 et linux/arm64). L'image tourne sous un
utilisateur sans droits et écoute sur `0.0.0.0:8765` en mode `http`.

**Depuis les sources** : `cargo build --release` (Rust 1.88 ou plus), puis `target/release/ha-mcp`.

Il faut un **jeton d'accès longue durée** de Home Assistant : dans Home Assistant, va dans ton profil, puis Sécurité,
puis « Jetons d'accès longue durée ». Certains outils demandent un utilisateur administrateur (registres, tableaux de
bord, assistants d'intégration, automatisations).

## Configuration

| Option | Variable | Défaut | Rôle |
|---|---|---|---|
| `--url URL` | `HA_URL` | `http://127.0.0.1:8123` | adresse de Home Assistant (`http` ou `https`) |
| `--jeton-fichier FICHIER` | `HA_TOKEN_FILE` |  | fichier du jeton ; ou le jeton lui-même dans `HA_TOKEN` |
| `--lecture-seule` | `HA_MCP_LECTURE_SEULE=1` | non | lecture seule : aucun outil qui écrit, `ws` limité aux commandes de lecture, `rest` au GET |
| `--domaines a,b` | `HA_MCP_DOMAINES` | tous | outils de service de ces domaines seulement |
| `--sans-domaines a,b` | `HA_MCP_SANS_DOMAINES` |  | pas d'outils de service pour ces domaines |
| `--max-services N` | `HA_MCP_MAX_SERVICES` | illimité | au plus N outils de service (`0` = aucun ; `appeler_service` reste) |
| `--presets DOSSIER` | `HA_MCP_PRESETS` |  | presets en plus (`*.json`), et là où `preset_exporter` écrit |
| `--ecoute ADRESSE:PORT` | `HA_MCP_ECOUTE` | `127.0.0.1:8765` | mode `http` : adresse d'écoute |
| `--jeton-serveur-fichier FICHIER` | `HA_MCP_JETON_FICHIER` |  | mode `http` : fichier du jeton Bearer que les clients envoient ; ou `HA_MCP_JETON` |

Modes : `ha-mcp` ou `ha-mcp stdio` (défaut), `ha-mcp http`, `ha-mcp lister` (écrit la liste des outils en markdown
puis sort). `ha-mcp --help` liste tout.

Les jetons ne sont jamais journalisés ni renvoyés. Préfère les formes `*_FILE` à un jeton sur la ligne de commande.

## Avec Claude Code

stdio (Claude Code lance le serveur) :

```sh
claude mcp add ha -e HA_URL=http://homeassistant.local:8123 -e HA_TOKEN_FILE=$HOME/.config/ha-mcp/token -- ha-mcp
```

HTTP (un serveur partagé par plusieurs clients, ici avec Docker) :

```sh
docker run -d --name ha-mcp -p 127.0.0.1:8765:8765 \
  -e HA_URL=http://homeassistant.local:8123 -e HA_TOKEN -e HA_MCP_JETON \
  ghcr.io/tojiisamaa/home-assistant-mcp-rust http
claude mcp add --transport http ha http://127.0.0.1:8765/mcp --header "Authorization: Bearer $HA_MCP_JETON"
```

## Avec Claude Desktop

Dans `claude_desktop_config.json` :

```json
{
  "mcpServers": {
    "ha": {
      "command": "ha-mcp",
      "env": { "HA_URL": "http://homeassistant.local:8123", "HA_TOKEN_FILE": "/chemin/du/jeton" }
    }
  }
}
```

Avec Docker plutôt que le binaire : `"command": "docker"` et `"args": ["run", "-i", "--rm", "-e", "HA_URL", "-e",
"HA_TOKEN", "ghcr.io/tojiisamaa/home-assistant-mcp-rust"]`, avec `HA_URL` et `HA_TOKEN` dans `env`.

## Installer la skill

La skill apprend au modèle quel outil prendre pour quel besoin, avec des recettes et des pièges. Copie le dossier dans
tes skills :

```sh
cp -r skill/home-assistant-mcp-rust ~/.claude/skills/
```

Sous Windows : `Copy-Item -Recurse skill\home-assistant-mcp-rust $HOME\.claude\skills\`.

## Exemples

Ce que tu peux demander une fois le serveur branché :
- « Allume la lumière du salon à 50 % » → `light__turn_on {target: {entity_id: "light.salon"}, brightness_pct: 50}`
- « Ajoute une liste de courses » → `flux_demarrer {handler: "shopping_list"}`, puis `flux_etape`, puis le nouvel
  outil `shopping_list__add_item` apparaît.
- « Fais un tableau de bord tablette pour la cuisine » → `page_creer {url_path: "cuisine-tablette", title: "Cuisine", config: {views: [...]}}`
- « Qu'est-ce qui s'est passé avec la porte d'entrée aujourd'hui ? » → `journal {entity_id: "binary_sensor.porte_entree"}`
- « Qu'est-ce qui utilise cet appareil ? » → `recherche_liee {item_type: "device", item_id: "..."}`

## Transports et sécurité

- **stdio** : un message JSON par ligne ; les requêtes s'exécutent en parallèle.
- **Streamable HTTP** sur `/mcp` (POST, GET pour le flux de notifications, DELETE pour fermer une session). La
  session (`Mcp-Session-Id`) est créée à `initialize` et reste facultative. L'ancien transport SSE est sur `/sse` et
  `/messages/{id}`. `/sante` répond `ok` sans jeton, pour les sondes.
- **Contrôles** : un jeton Bearer est obligatoire (comparé en temps constant). Un `Origin` étranger reçoit 403
  (protection contre le DNS rebinding). Après `initialize`, `MCP-Protocol-Version` doit être 2025-11-25, 2025-06-18,
  2025-03-26 ou 2024-11-05.
- **Réponses** : coupées à 100 000 octets. Chaque outil porte `readOnlyHint`.
- **Exposition** : garde le serveur HTTP sur localhost ou derrière un mandataire TLS ; il détient un jeton
  administrateur de ta maison.

## Tests

- `cargo test` lance les tests unitaires et les tests du protocole contre un faux Home Assistant (REST et WebSocket
  sur un seul port). Un test vérifie que la section générée de la skill correspond au code ;
  `MAJ_SKILL=1 cargo test skill_a_jour` la réécrit.
- `python3 -I tests/reel/test_reel.py stable 2025.6 2024.6 maison` lance de vrais conteneurs (`hamcp-test-*`,
  retirés à la fin) après `docker build -t hamcp-test-mcp:local .`. Pour chaque cible : onboarding, jeton longue durée
  créé par `ws`, liste des outils, `input_boolean__toggle`, un modèle, une page de tableau de bord, une intégration
  ajoutée sans matériel (liste de courses) et le `list_changed` qui suit, un script créé (un service `script.<id>`
  nouveau, son `list_changed` et son outil généré), puis le mode lecture seule. `nom=IMAGE` ajoute une cible tirée
  d'une autre image.

## Licence

[Apache-2.0](LICENSE). Voir [NOTICE](NOTICE).
