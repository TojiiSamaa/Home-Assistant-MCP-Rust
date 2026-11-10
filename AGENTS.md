# AGENTS.md

Serveur MCP Home Assistant en Rust (`ha-mcp`). Pour **utiliser** ses outils depuis un agent (Claude Code, OpenCode, Hermes, Cursor, Codex…), lis la skill :

- [skill/home-assistant-mcp-rust/SKILL.md](skill/home-assistant-mcp-rust/SKILL.md) : les 30 outils fixes (dont `ha_stats`), la règle des outils dynamiques `domaine__service`, ce qui est possible et impossible, les recettes, le mode test (`MCP_TEST_MODE=1`, aucun accès réseau) et les stats.
- Liste réelle de l'instance connectée : appelle l'outil `lister_outils`.

Pour **modifier** le dépôt : `cargo test` (la section « Outils génériques » de la skill est générée : `MAJ_SKILL=1 cargo test skill_a_jour`). Sans cargo, `sh tests/verifier-skill.sh` vérifie que le tableau des outils fixes de la skill correspond à `src/outils.rs`. Preuves du mode test : `sh tests/verifier-reseau.sh` (aucune primitive réseau hors `src/reseau.rs`), `sh tests/b-isole.sh` (docker `--network none`), `sh tests/mutation.sh`. Aucun secret dans le dépôt : jeton Home Assistant par `HA_TOKEN_FILE`, jamais en clair.
