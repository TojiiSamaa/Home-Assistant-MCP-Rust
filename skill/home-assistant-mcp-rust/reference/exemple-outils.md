<!-- Sortie réelle de `ha-mcp lister` sur un Home Assistant neuf (image home-assistant:stable, onboarding seul, aucune intégration ajoutée). Sur une autre instance la liste diffère : lister_outils. -->

# Outils de Home Assistant MCP Rust

Serveur : Home Assistant 2026.11.0 · 261 outils : 29 génériques, 232 de service (59 domaines).

## Outils génériques

- `etats` — États des entités.
- `services` — Services du serveur (get_services).
- `appeler_service` — Appelle n'importe quel service (call_service), même s'il n'a pas encore son outil domaine__service.
- `template` — Rend un modèle Jinja de Home Assistant (POST /api/template), ex. « {{ states('sun.sun') }} » ou « {{ area_entities('salon') }} ».
- `historique` — Historique d'états (GET /api/history/period) d'une ou plusieurs entités : les `heures` dernières (défaut 24) ou de `debut` à `fin` (ISO 8601).
- `journal` — Journal (GET /api/logbook) : les `heures` dernières (défaut 24) ou de `debut` à `fin`, pour une entité ou toutes.
- `registres` — Un registre : entités, appareils, pièces, étages ou étiquettes (selon ce que le serveur connaît), filtrable par sous-chaîne.
- `recherche_liee` — Ce qui est lié à un élément (search/related) : automatisations, scripts, scènes, appareils, entités, pièces…
- `integrations` — Intégrations : chargées (get_config), entrées de configuration (domaine, titre, état), et domaines qu'on peut ajouter par assistant (flow_handlers).
- `flux_demarrer` — Démarre l'assistant d'ajout d'une intégration ou d'une entrée (helper) : POST /api/config/config_entries/flow {handler}.
- `flux_etape` — Répond à une étape d'un assistant : POST /api/config/config_entries/flow/{flow_id} avec les valeurs du formulaire (data_schema de l'étape), ou {"next_step_id": …} pour un menu.
- `flux_annuler` — Abandonne un assistant en cours (DELETE /api/config/config_entries/flow/{flow_id}).
- `tableaux_lister` — Tableaux de bord (lovelace/dashboards/list).
- `tableau_lire` — Contenu (views, cards) d'un tableau de bord (lovelace/config).
- `tableau_enregistrer` — Remplace le contenu d'un tableau de bord (lovelace/config/save).
- `page_creer` — Crée un tableau de bord (lovelace/dashboards/create) et, si `config` est donnée, enregistre son contenu (lovelace/config/save).
- `page_modifier` — Modifie un tableau de bord repéré par url_path : titre, icône, visibilité (lovelace/dashboards/update) et/ou contenu `config` (lovelace/config/save).
- `page_supprimer` — Supprime un tableau de bord repéré par url_path (lovelace/dashboards/delete).
- `barre_regler` — Règle la barre latérale de l'utilisateur du jeton (frontend/set_user_data, clé sidebar) : `ordre` = panneaux dans l'ordre voulu (ex. ["lovelace", "energy", "salon-tablette"]), `masques` = panneaux cachés.
- `automatisme_lire` — Automatisations, scripts, scènes.
- `automatisme_ecrire` — Crée ou remplace une automatisation, un script ou une scène (POST /api/config/<domaine>/config/<id>) ; le serveur valide puis recharge.
- `automatisme_supprimer` — Supprime une automatisation, un script ou une scène créé par l'interface (DELETE /api/config/<domaine>/config/<id>).
- `presets_lister` — Presets de préconfiguration (embarqués, et ceux du dossier --presets) : id, nom, description, paramètres déclarés avec leurs défauts, pages, thème, barre, cartes requises.
- `preset_appliquer` — Applique un preset en COPIE avec les commandes standard : pages (lovelace/dashboards/*, lovelace/config/save), thème, barre latérale.
- `preset_annuler` — Annule la dernière application de preset : remet les pages, le thème et la barre latérale d'avant.
- `preset_exporter` — Fait un preset de la configuration actuelle (pages stockées, thème et barre de l'utilisateur du jeton).
- `ws` — Envoie N'IMPORTE QUELLE commande WebSocket de Home Assistant (après authentification) et renvoie son résultat : arguments = le message sans id, `type` et ses champs (ex. {"type": "config/entity_registry/list"}).
- `rest` — N'IMPORTE QUELLE requête REST sous /api/ (ex. GET /api/config, GET /api/error_log).
- `lister_outils` — Liste complète, en markdown, des outils de cette instance : génériques et un par service, avec leurs paramètres.

## Outils de service

Arguments = champs du service ; `target` = cible (entity_id, device_id, area_id, floor_id, label_id).

### ai_task (4)

- `ai_task__clear_images` — ai_task.clear_images
  - paramètres : `days` (nombre 1.0…3650.0)
- `ai_task__evaluate` — ai_task.evaluate (renvoie une réponse)
  - paramètres : `task_name` (texte, requis), `entity_id` (texte), `state` (toute valeur), `questions` (toute valeur, requis), `attachments` (liste de objet)
- `ai_task__generate_data` — ai_task.generate_data (renvoie une réponse)
  - paramètres : `task_name` (texte, requis), `instructions` (texte, requis), `entity_id` (texte), `structure` (toute valeur), `attachments` (liste de objet)
- `ai_task__generate_image` — ai_task.generate_image (renvoie une réponse)
  - paramètres : `task_name` (texte, requis), `instructions` (texte, requis), `entity_id` (texte, requis), `attachments` (liste de objet)

### alarm_control_panel (7)

- `alarm_control_panel__alarm_arm_away` — alarm_control_panel.alarm_arm_away
  - paramètres : `code` (texte), `target` (cible)
- `alarm_control_panel__alarm_arm_custom_bypass` — alarm_control_panel.alarm_arm_custom_bypass
  - paramètres : `code` (texte), `target` (cible)
- `alarm_control_panel__alarm_arm_home` — alarm_control_panel.alarm_arm_home
  - paramètres : `code` (texte), `target` (cible)
- `alarm_control_panel__alarm_arm_night` — alarm_control_panel.alarm_arm_night
  - paramètres : `code` (texte), `target` (cible)
- `alarm_control_panel__alarm_arm_vacation` — alarm_control_panel.alarm_arm_vacation
  - paramètres : `code` (texte), `target` (cible)
- `alarm_control_panel__alarm_disarm` — alarm_control_panel.alarm_disarm
  - paramètres : `code` (texte), `target` (cible)
- `alarm_control_panel__alarm_trigger` — alarm_control_panel.alarm_trigger
  - paramètres : `code` (texte), `target` (cible)

### assist_satellite (3)

- `assist_satellite__announce` — assist_satellite.announce
  - paramètres : `message` (texte), `media_id` (objet), `preannounce` (booléen), `preannounce_media_id` (objet), `target` (cible)
- `assist_satellite__ask_question` — assist_satellite.ask_question (renvoie une réponse)
  - paramètres : `entity_id` (texte, requis), `question` (texte), `question_media_id` (objet), `preannounce` (booléen), `preannounce_media_id` (objet), `answers` (liste de objet)
- `assist_satellite__start_conversation` — assist_satellite.start_conversation
  - paramètres : `start_message` (texte), `start_media_id` (objet), `extra_system_prompt` (texte), `preannounce` (booléen), `preannounce_media_id` (objet), `target` (cible)

### automation (5)

- `automation__reload` — automation.reload
- `automation__toggle` — automation.toggle
  - paramètres : `target` (cible)
- `automation__trigger` — automation.trigger
  - paramètres : `skip_condition` (booléen), `target` (cible)
- `automation__turn_off` — automation.turn_off
  - paramètres : `stop_actions` (booléen), `target` (cible)
- `automation__turn_on` — automation.turn_on
  - paramètres : `target` (cible)

### backup (2)

- `backup__create` — backup.create
- `backup__create_automatic` — backup.create_automatic

### button (1)

- `button__press` — button.press
  - paramètres : `target` (cible)

### camera (7)

- `camera__disable_motion_detection` — camera.disable_motion_detection
  - paramètres : `target` (cible)
- `camera__enable_motion_detection` — camera.enable_motion_detection
  - paramètres : `target` (cible)
- `camera__play_stream` — camera.play_stream
  - paramètres : `media_player` (texte, requis), `format` (hls), `target` (cible)
- `camera__record` — camera.record
  - paramètres : `filename` (texte, requis), `duration` (nombre 1.0…3600.0), `lookback` (nombre 0.0…300.0), `target` (cible)
- `camera__snapshot` — camera.snapshot
  - paramètres : `filename` (texte, requis), `target` (cible)
- `camera__turn_off` — camera.turn_off
  - paramètres : `target` (cible)
- `camera__turn_on` — camera.turn_on
  - paramètres : `target` (cible)

### climate (10)

- `climate__set_fan_mode` — climate.set_fan_mode
  - paramètres : `fan_mode` (texte, requis), `target` (cible)
- `climate__set_humidity` — climate.set_humidity
  - paramètres : `humidity` (nombre 30.0…99.0, requis), `target` (cible)
- `climate__set_hvac_mode` — climate.set_hvac_mode
  - paramètres : `hvac_mode` (texte), `target` (cible)
- `climate__set_preset_mode` — climate.set_preset_mode
  - paramètres : `preset_mode` (texte, requis), `target` (cible)
- `climate__set_swing_horizontal_mode` — climate.set_swing_horizontal_mode
  - paramètres : `swing_horizontal_mode` (texte, requis), `target` (cible)
- `climate__set_swing_mode` — climate.set_swing_mode
  - paramètres : `swing_mode` (texte, requis), `target` (cible)
- `climate__set_temperature` — climate.set_temperature
  - paramètres : `temperature` (nombre 0.0…250.0), `target_temp_high` (nombre 0.0…250.0), `target_temp_low` (nombre 0.0…250.0), `hvac_mode` (texte), `target` (cible)
- `climate__toggle` — climate.toggle
  - paramètres : `target` (cible)
- `climate__turn_off` — climate.turn_off
  - paramètres : `target` (cible)
- `climate__turn_on` — climate.turn_on
  - paramètres : `target` (cible)

### cloud (2)

- `cloud__remote_connect` — cloud.remote_connect
- `cloud__remote_disconnect` — cloud.remote_disconnect

### conversation (2)

- `conversation__process` — conversation.process
  - paramètres : `text` (texte, requis), `language` (texte), `agent_id` (texte), `conversation_id` (texte), `return_response` (booléen)
- `conversation__reload` — conversation.reload
  - paramètres : `language` (texte), `agent_id` (texte)

### counter (4)

- `counter__decrement` — counter.decrement
  - paramètres : `target` (cible)
- `counter__increment` — counter.increment
  - paramètres : `target` (cible)
- `counter__reset` — counter.reset
  - paramètres : `target` (cible)
- `counter__set_value` — counter.set_value
  - paramètres : `value` (nombre -9.223372036854776e+18…9.223372036854776e+18, requis), `target` (cible)

### cover (10)

- `cover__close_cover` — cover.close_cover
  - paramètres : `speed` (texte), `target` (cible)
- `cover__close_cover_tilt` — cover.close_cover_tilt
  - paramètres : `target` (cible)
- `cover__open_cover` — cover.open_cover
  - paramètres : `speed` (texte), `target` (cible)
- `cover__open_cover_tilt` — cover.open_cover_tilt
  - paramètres : `target` (cible)
- `cover__set_cover_position` — cover.set_cover_position
  - paramètres : `position` (nombre 0.0…100.0, requis), `speed` (texte), `target` (cible)
- `cover__set_cover_tilt_position` — cover.set_cover_tilt_position
  - paramètres : `tilt_position` (nombre 0.0…100.0, requis), `target` (cible)
- `cover__stop_cover` — cover.stop_cover
  - paramètres : `target` (cible)
- `cover__stop_cover_tilt` — cover.stop_cover_tilt
  - paramètres : `target` (cible)
- `cover__toggle` — cover.toggle
  - paramètres : `target` (cible)
- `cover__toggle_cover_tilt` — cover.toggle_cover_tilt
  - paramètres : `target` (cible)

### date (1)

- `date__set_value` — date.set_value
  - paramètres : `date` (texte, requis), `target` (cible)

### datetime (1)

- `datetime__set_value` — datetime.set_value
  - paramètres : `datetime` (texte, requis), `target` (cible)

### device_tracker (1)

- `device_tracker__see` — device_tracker.see
  - paramètres : `mac` (texte), `dev_id` (texte), `host_name` (texte), `location_name` (texte), `gps` (toute valeur), `gps_accuracy` (nombre ≥ 0.0), `battery` (nombre 0.0…100.0)

### fan (9)

- `fan__decrease_speed` — fan.decrease_speed
  - paramètres : `percentage_step` (nombre 0.0…100.0), `target` (cible)
- `fan__increase_speed` — fan.increase_speed
  - paramètres : `percentage_step` (nombre 0.0…100.0), `target` (cible)
- `fan__oscillate` — fan.oscillate
  - paramètres : `oscillating` (booléen, requis), `target` (cible)
- `fan__set_direction` — fan.set_direction
  - paramètres : `direction` (forward / reverse, requis), `target` (cible)
- `fan__set_percentage` — fan.set_percentage
  - paramètres : `percentage` (nombre 0.0…100.0, requis), `target` (cible)
- `fan__set_preset_mode` — fan.set_preset_mode
  - paramètres : `preset_mode` (texte, requis), `target` (cible)
- `fan__toggle` — fan.toggle
  - paramètres : `target` (cible)
- `fan__turn_off` — fan.turn_off
  - paramètres : `target` (cible)
- `fan__turn_on` — fan.turn_on
  - paramètres : `percentage` (nombre 0.0…100.0), `preset_mode` (texte), `target` (cible)

### ffmpeg (3)

- `ffmpeg__restart` — ffmpeg.restart
  - paramètres : `entity_id` (texte)
- `ffmpeg__start` — ffmpeg.start
  - paramètres : `entity_id` (texte)
- `ffmpeg__stop` — ffmpeg.stop
  - paramètres : `entity_id` (texte)

### file (1)

- `file__read_file` — file.read_file (renvoie une réponse)
  - paramètres : `file_name` (texte), `file_encoding` (JSON / YAML)

### frontend (2)

- `frontend__reload_themes` — frontend.reload_themes
- `frontend__set_theme` — frontend.set_theme
  - paramètres : `name` (texte), `name_dark` (texte)

### homeassistant (13)

- `homeassistant__check_config` — homeassistant.check_config
- `homeassistant__reload_all` — homeassistant.reload_all
- `homeassistant__reload_config_entry` — homeassistant.reload_config_entry
  - paramètres : `entry_id` (texte, requis)
- `homeassistant__reload_core_config` — homeassistant.reload_core_config
- `homeassistant__reload_custom_templates` — homeassistant.reload_custom_templates
- `homeassistant__restart` — homeassistant.restart
- `homeassistant__save_persistent_states` — homeassistant.save_persistent_states
- `homeassistant__set_location` — homeassistant.set_location
  - paramètres : `latitude` (nombre -90.0…90.0, requis), `longitude` (nombre -180.0…180.0, requis), `elevation` (nombre)
- `homeassistant__stop` — homeassistant.stop
- `homeassistant__toggle` — homeassistant.toggle
  - paramètres : `target` (cible)
- `homeassistant__turn_off` — homeassistant.turn_off
  - paramètres : `target` (cible)
- `homeassistant__turn_on` — homeassistant.turn_on
  - paramètres : `target` (cible)
- `homeassistant__update_entity` — homeassistant.update_entity
  - paramètres : `entity_id` (liste de texte, requis)

### humidifier (5)

- `humidifier__set_humidity` — humidifier.set_humidity
  - paramètres : `humidity` (nombre 0.0…100.0, requis), `target` (cible)
- `humidifier__set_mode` — humidifier.set_mode
  - paramètres : `mode` (texte, requis), `target` (cible)
- `humidifier__toggle` — humidifier.toggle
  - paramètres : `target` (cible)
- `humidifier__turn_off` — humidifier.turn_off
  - paramètres : `target` (cible)
- `humidifier__turn_on` — humidifier.turn_on
  - paramètres : `target` (cible)

### image (1)

- `image__snapshot` — image.snapshot
  - paramètres : `filename` (texte, requis), `target` (cible)

### image_processing (1)

- `image_processing__scan` — image_processing.scan
  - paramètres : `target` (cible)

### input_boolean (4)

- `input_boolean__reload` — input_boolean.reload
- `input_boolean__toggle` — input_boolean.toggle
  - paramètres : `target` (cible)
- `input_boolean__turn_off` — input_boolean.turn_off
  - paramètres : `target` (cible)
- `input_boolean__turn_on` — input_boolean.turn_on
  - paramètres : `target` (cible)

### input_button (2)

- `input_button__press` — input_button.press
  - paramètres : `target` (cible)
- `input_button__reload` — input_button.reload

### input_datetime (2)

- `input_datetime__reload` — input_datetime.reload
- `input_datetime__set_datetime` — input_datetime.set_datetime
  - paramètres : `date` (texte), `time` (texte), `datetime` (texte), `timestamp` (nombre 0.0…9.223372036854776e+18), `target` (cible)

### input_number (4)

- `input_number__decrement` — input_number.decrement
  - paramètres : `target` (cible)
- `input_number__increment` — input_number.increment
  - paramètres : `target` (cible)
- `input_number__reload` — input_number.reload
- `input_number__set_value` — input_number.set_value
  - paramètres : `value` (nombre 0.0…9.223372036854776e+18, requis), `target` (cible)

### input_select (7)

- `input_select__reload` — input_select.reload
- `input_select__select_first` — input_select.select_first
  - paramètres : `target` (cible)
- `input_select__select_last` — input_select.select_last
  - paramètres : `target` (cible)
- `input_select__select_next` — input_select.select_next
  - paramètres : `cycle` (booléen), `target` (cible)
- `input_select__select_option` — input_select.select_option
  - paramètres : `option` (texte, requis), `target` (cible)
- `input_select__select_previous` — input_select.select_previous
  - paramètres : `cycle` (booléen), `target` (cible)
- `input_select__set_options` — input_select.set_options
  - paramètres : `options` (liste de texte, requis), `target` (cible)

### input_text (2)

- `input_text__reload` — input_text.reload
- `input_text__set_value` — input_text.set_value
  - paramètres : `value` (texte, requis), `target` (cible)

### lawn_mower (4)

- `lawn_mower__dock` — lawn_mower.dock
  - paramètres : `target` (cible)
- `lawn_mower__pause` — lawn_mower.pause
  - paramètres : `target` (cible)
- `lawn_mower__start_mowing` — lawn_mower.start_mowing
  - paramètres : `target` (cible)
- `lawn_mower__stop` — lawn_mower.stop
  - paramètres : `target` (cible)

### light (3)

- `light__toggle` — light.toggle
  - paramètres : `transition` (nombre 0.0…300.0), `rgb_color` (liste de entier 0…255), `color_temp_kelvin` (nombre 2000…6500), `brightness_pct` (nombre 0.0…100.0), `effect` (texte), `rgbw_color` (toute valeur), `rgbww_color` (toute valeur), `color_name` (homeassistant / aliceblue / antiquewhite / aqua / aquamarine / azure / beige / bisque / blanchedalmond / blue / blueviolet / brown / burlywood / cadetblue / chartreuse / chocolate / coral / cornflowerblue / cornsilk / crimson / cyan / darkblue / darkcyan / darkgoldenrod / darkgray / darkgreen / darkgrey / darkkhaki / darkmagenta / darkolivegreen / darkorange / darkorchid / darkred / darksalmon / darkseagreen / darkslateblue / darkslategray / darkslategrey / darkturquoise / darkviolet / deeppink / deepskyblue / dimgray / dimgrey / dodgerblue / firebrick / floralwhite / forestgreen / fuchsia / gainsboro / ghostwhite / gold / goldenrod / gray / green / greenyellow / grey / honeydew / hotpink / indianred / indigo / ivory / khaki / lavender / lavenderblush / lawngreen / lemonchiffon / lightblue / lightcoral / lightcyan / lightgoldenrodyellow / lightgray / lightgreen / lightgrey / lightpink / lightsalmon / lightseagreen / lightskyblue / lightslategray / lightslategrey / lightsteelblue / lightyellow / lime / limegreen / linen / magenta / maroon / mediumaquamarine / mediumblue / mediumorchid / mediumpurple / mediumseagreen / mediumslateblue / mediumspringgreen / mediumturquoise / mediumvioletred / midnightblue / mintcream / mistyrose / moccasin / navajowhite / navy / navyblue / oldlace / olive / olivedrab / orange / orangered / orchid / palegoldenrod / palegreen / paleturquoise / palevioletred / papayawhip / peachpuff / peru / pink / plum / powderblue / purple / red / rosybrown / royalblue / saddlebrown / salmon / sandybrown / seagreen / seashell / sienna / silver / skyblue / slateblue / slategray / slategrey / snow / springgreen / steelblue / tan / teal / thistle / tomato / turquoise / violet / wheat / white / whitesmoke / yellow / yellowgreen), `hs_color` (toute valeur), `xy_color` (toute valeur), `brightness` (nombre 0.0…255.0), `white` (= true), `profile` (texte), `flash` (long / short), `target` (cible)
- `light__turn_off` — light.turn_off
  - paramètres : `transition` (nombre 0.0…300.0), `flash` (long / short), `target` (cible)
- `light__turn_on` — light.turn_on
  - paramètres : `transition` (nombre 0.0…300.0), `rgb_color` (liste de entier 0…255), `color_temp_kelvin` (nombre 2000…6500), `brightness_pct` (nombre 0.0…100.0), `brightness_step_pct` (nombre -100.0…100.0), `effect` (texte), `rgbw_color` (toute valeur), `rgbww_color` (toute valeur), `color_name` (homeassistant / aliceblue / antiquewhite / aqua / aquamarine / azure / beige / bisque / blanchedalmond / blue / blueviolet / brown / burlywood / cadetblue / chartreuse / chocolate / coral / cornflowerblue / cornsilk / crimson / cyan / darkblue / darkcyan / darkgoldenrod / darkgray / darkgreen / darkgrey / darkkhaki / darkmagenta / darkolivegreen / darkorange / darkorchid / darkred / darksalmon / darkseagreen / darkslateblue / darkslategray / darkslategrey / darkturquoise / darkviolet / deeppink / deepskyblue / dimgray / dimgrey / dodgerblue / firebrick / floralwhite / forestgreen / fuchsia / gainsboro / ghostwhite / gold / goldenrod / gray / green / greenyellow / grey / honeydew / hotpink / indianred / indigo / ivory / khaki / lavender / lavenderblush / lawngreen / lemonchiffon / lightblue / lightcoral / lightcyan / lightgoldenrodyellow / lightgray / lightgreen / lightgrey / lightpink / lightsalmon / lightseagreen / lightskyblue / lightslategray / lightslategrey / lightsteelblue / lightyellow / lime / limegreen / linen / magenta / maroon / mediumaquamarine / mediumblue / mediumorchid / mediumpurple / mediumseagreen / mediumslateblue / mediumspringgreen / mediumturquoise / mediumvioletred / midnightblue / mintcream / mistyrose / moccasin / navajowhite / navy / navyblue / oldlace / olive / olivedrab / orange / orangered / orchid / palegoldenrod / palegreen / paleturquoise / palevioletred / papayawhip / peachpuff / peru / pink / plum / powderblue / purple / red / rosybrown / royalblue / saddlebrown / salmon / sandybrown / seagreen / seashell / sienna / silver / skyblue / slateblue / slategray / slategrey / snow / springgreen / steelblue / tan / teal / thistle / tomato / turquoise / violet / wheat / white / whitesmoke / yellow / yellowgreen), `hs_color` (toute valeur), `xy_color` (toute valeur), `brightness` (nombre 0.0…255.0), `brightness_step` (nombre -225.0…255.0), `white` (= true), `profile` (texte), `flash` (long / short), `target` (cible)

### lock (3)

- `lock__lock` — lock.lock
  - paramètres : `code` (texte), `target` (cible)
- `lock__open` — lock.open
  - paramètres : `code` (texte), `target` (cible)
- `lock__unlock` — lock.unlock
  - paramètres : `code` (texte), `target` (cible)

### logbook (1)

- `logbook__log` — logbook.log
  - paramètres : `name` (texte, requis), `message` (texte, requis), `entity_id` (texte), `domain` (texte)

### logger (2)

- `logger__set_default_level` — logger.set_default_level
  - paramètres : `level` (debug / info / warning / error / fatal / critical)
- `logger__set_level` — logger.set_level

### lovelace (1)

- `lovelace__reload_resources` — lovelace.reload_resources

### media_player (24)

- `media_player__browse_media` — media_player.browse_media (renvoie une réponse)
  - paramètres : `media_content_type` (texte), `media_content_id` (texte), `target` (cible)
- `media_player__clear_playlist` — media_player.clear_playlist
  - paramètres : `target` (cible)
- `media_player__join` — media_player.join
  - paramètres : `group_members` (liste de texte, requis), `target` (cible)
- `media_player__media_next_track` — media_player.media_next_track
  - paramètres : `target` (cible)
- `media_player__media_pause` — media_player.media_pause
  - paramètres : `target` (cible)
- `media_player__media_play` — media_player.media_play
  - paramètres : `target` (cible)
- `media_player__media_play_pause` — media_player.media_play_pause
  - paramètres : `target` (cible)
- `media_player__media_previous_track` — media_player.media_previous_track
  - paramètres : `target` (cible)
- `media_player__media_seek` — media_player.media_seek
  - paramètres : `seek_position` (nombre 0.0…9.223372036854776e+18, requis), `target` (cible)
- `media_player__media_stop` — media_player.media_stop
  - paramètres : `target` (cible)
- `media_player__play_media` — media_player.play_media
  - paramètres : `media` (objet, requis), `enqueue` (play / next / add / replace), `announce` (booléen), `target` (cible)
- `media_player__repeat_set` — media_player.repeat_set
  - paramètres : `repeat` (off / all / one, requis), `target` (cible)
- `media_player__search_media` — media_player.search_media (renvoie une réponse)
  - paramètres : `search_query` (texte, requis), `media_content_type` (texte), `media_content_id` (texte), `media_filter_classes` (liste de texte), `target` (cible)
- `media_player__select_sound_mode` — media_player.select_sound_mode
  - paramètres : `sound_mode` (texte), `target` (cible)
- `media_player__select_source` — media_player.select_source
  - paramètres : `source` (texte, requis), `target` (cible)
- `media_player__shuffle_set` — media_player.shuffle_set
  - paramètres : `shuffle` (booléen, requis), `target` (cible)
- `media_player__toggle` — media_player.toggle
  - paramètres : `target` (cible)
- `media_player__turn_off` — media_player.turn_off
  - paramètres : `target` (cible)
- `media_player__turn_on` — media_player.turn_on
  - paramètres : `target` (cible)
- `media_player__unjoin` — media_player.unjoin
  - paramètres : `target` (cible)
- `media_player__volume_down` — media_player.volume_down
  - paramètres : `target` (cible)
- `media_player__volume_mute` — media_player.volume_mute
  - paramètres : `is_volume_muted` (booléen, requis), `target` (cible)
- `media_player__volume_set` — media_player.volume_set
  - paramètres : `volume_level` (nombre 0.0…1.0, requis), `target` (cible)
- `media_player__volume_up` — media_player.volume_up
  - paramètres : `target` (cible)

### notify (3)

- `notify__notify` — notify.notify — Send a notification with notify : This notification action is provided by a notification integration or notification group. To change where notifications are sent or which options are available, update that integration or group.
  - paramètres : `message` (texte, requis), `title` (texte), `target` (cible), `data` (toute valeur)
- `notify__persistent_notification` — notify.persistent_notification
  - paramètres : `message` (texte, requis), `title` (texte), `data` (toute valeur)
- `notify__send_message` — notify.send_message
  - paramètres : `message` (texte, requis), `title` (texte), `target` (cible)

### number (1)

- `number__set_value` — number.set_value
  - paramètres : `value` (texte, requis), `target` (cible)

### persistent_notification (3)

- `persistent_notification__create` — persistent_notification.create
  - paramètres : `message` (texte, requis), `title` (texte), `notification_id` (texte)
- `persistent_notification__dismiss` — persistent_notification.dismiss
  - paramètres : `notification_id` (texte, requis)
- `persistent_notification__dismiss_all` — persistent_notification.dismiss_all

### person (1)

- `person__reload` — person.reload

### recorder (5)

- `recorder__disable` — recorder.disable
- `recorder__enable` — recorder.enable
- `recorder__get_statistics` — recorder.get_statistics (renvoie une réponse)
  - paramètres : `start_time` (texte, requis), `end_time` (texte), `statistic_ids` (liste de texte, requis), `period` (5minute / hour / day / week / month / year, requis), `types` (liste de change / last_reset / max / mean / min / state / sum, requis), `units` (toute valeur)
- `recorder__purge` — recorder.purge
  - paramètres : `keep_days` (nombre 0.0…365.0), `repack` (booléen), `apply_filter` (booléen)
- `recorder__purge_entities` — recorder.purge_entities
  - paramètres : `entity_id` (liste de texte), `domains` (toute valeur), `entity_globs` (toute valeur), `keep_days` (nombre 0.0…365.0)

### remote (6)

- `remote__delete_command` — remote.delete_command
  - paramètres : `device` (texte), `command` (toute valeur, requis), `target` (cible)
- `remote__learn_command` — remote.learn_command
  - paramètres : `device` (texte), `command` (toute valeur), `command_type` (ir / rf), `alternative` (booléen), `timeout` (nombre 0.0…60.0), `target` (cible)
- `remote__send_command` — remote.send_command
  - paramètres : `device` (texte), `command` (toute valeur, requis), `num_repeats` (nombre 0.0…255.0), `delay_secs` (nombre 0.0…60.0), `hold_secs` (nombre 0.0…60.0), `target` (cible)
- `remote__toggle` — remote.toggle
  - paramètres : `target` (cible)
- `remote__turn_off` — remote.turn_off
  - paramètres : `target` (cible)
- `remote__turn_on` — remote.turn_on
  - paramètres : `activity` (texte), `target` (cible)

### scene (5)

- `scene__apply` — scene.apply
  - paramètres : `entities` (toute valeur, requis), `transition` (nombre 0.0…300.0)
- `scene__create` — scene.create
  - paramètres : `scene_id` (texte, requis), `entities` (toute valeur), `snapshot_entities` (liste de texte)
- `scene__delete` — scene.delete
  - paramètres : `target` (cible)
- `scene__reload` — scene.reload
- `scene__turn_on` — scene.turn_on
  - paramètres : `transition` (nombre 0.0…300.0), `target` (cible)

### schedule (2)

- `schedule__get_schedule` — schedule.get_schedule (renvoie une réponse)
  - paramètres : `target` (cible)
- `schedule__reload` — schedule.reload

### script (4)

- `script__reload` — script.reload
- `script__toggle` — script.toggle
  - paramètres : `target` (cible)
- `script__turn_off` — script.turn_off
  - paramètres : `target` (cible)
- `script__turn_on` — script.turn_on
  - paramètres : `target` (cible)

### select (5)

- `select__select_first` — select.select_first
  - paramètres : `target` (cible)
- `select__select_last` — select.select_last
  - paramètres : `target` (cible)
- `select__select_next` — select.select_next
  - paramètres : `cycle` (booléen), `target` (cible)
- `select__select_option` — select.select_option
  - paramètres : `option` (texte, requis), `target` (cible)
- `select__select_previous` — select.select_previous
  - paramètres : `cycle` (booléen), `target` (cible)

### siren (3)

- `siren__toggle` — siren.toggle
  - paramètres : `target` (cible)
- `siren__turn_off` — siren.turn_off
  - paramètres : `target` (cible)
- `siren__turn_on` — siren.turn_on
  - paramètres : `tone` (texte), `volume_level` (nombre 0.0…1.0), `duration` (texte), `target` (cible)

### switch (3)

- `switch__toggle` — switch.toggle
  - paramètres : `target` (cible)
- `switch__turn_off` — switch.turn_off
  - paramètres : `target` (cible)
- `switch__turn_on` — switch.turn_on
  - paramètres : `target` (cible)

### system_log (2)

- `system_log__clear` — system_log.clear
- `system_log__write` — system_log.write
  - paramètres : `message` (texte, requis), `level` (debug / info / warning / error / critical), `logger` (texte)

### text (1)

- `text__set_value` — text.set_value
  - paramètres : `value` (texte, requis), `target` (cible)

### time (1)

- `time__set_value` — time.set_value
  - paramètres : `time` (texte, requis), `target` (cible)

### timer (6)

- `timer__cancel` — timer.cancel
  - paramètres : `target` (cible)
- `timer__change` — timer.change
  - paramètres : `duration` (texte ou nombre ou objet, requis), `target` (cible)
- `timer__finish` — timer.finish
  - paramètres : `target` (cible)
- `timer__pause` — timer.pause
  - paramètres : `target` (cible)
- `timer__reload` — timer.reload
- `timer__start` — timer.start
  - paramètres : `duration` (texte ou nombre ou objet), `target` (cible)

### tts (3)

- `tts__clear_cache` — tts.clear_cache
- `tts__cloud_say` — tts.cloud_say — Say a TTS message with cloud : Say something using text-to-speech on a media player with cloud.
  - paramètres : `entity_id` (texte, requis), `message` (texte, requis), `cache` (booléen), `language` (texte), `options` (toute valeur)
- `tts__speak` — tts.speak
  - paramètres : `media_player_entity_id` (texte, requis), `message` (texte, requis), `cache` (booléen), `language` (texte), `options` (toute valeur), `target` (cible)

### update (3)

- `update__clear_skipped` — update.clear_skipped
  - paramètres : `target` (cible)
- `update__install` — update.install
  - paramètres : `version` (texte), `backup` (booléen), `target` (cible)
- `update__skip` — update.skip
  - paramètres : `target` (cible)

### vacuum (9)

- `vacuum__clean_area` — vacuum.clean_area
  - paramètres : `cleaning_area_id` (liste de texte, requis), `target` (cible)
- `vacuum__clean_spot` — vacuum.clean_spot
  - paramètres : `target` (cible)
- `vacuum__locate` — vacuum.locate
  - paramètres : `target` (cible)
- `vacuum__pause` — vacuum.pause
  - paramètres : `target` (cible)
- `vacuum__return_to_base` — vacuum.return_to_base
  - paramètres : `target` (cible)
- `vacuum__send_command` — vacuum.send_command
  - paramètres : `command` (texte, requis), `params` (toute valeur), `target` (cible)
- `vacuum__set_fan_speed` — vacuum.set_fan_speed
  - paramètres : `fan_speed` (texte, requis), `target` (cible)
- `vacuum__start` — vacuum.start
  - paramètres : `target` (cible)
- `vacuum__stop` — vacuum.stop
  - paramètres : `target` (cible)

### valve (5)

- `valve__close_valve` — valve.close_valve
  - paramètres : `target` (cible)
- `valve__open_valve` — valve.open_valve
  - paramètres : `target` (cible)
- `valve__set_valve_position` — valve.set_valve_position
  - paramètres : `position` (nombre 0.0…100.0, requis), `target` (cible)
- `valve__stop_valve` — valve.stop_valve
  - paramètres : `target` (cible)
- `valve__toggle` — valve.toggle
  - paramètres : `target` (cible)

### water_heater (5)

- `water_heater__set_away_mode` — water_heater.set_away_mode
  - paramètres : `away_mode` (booléen, requis), `target` (cible)
- `water_heater__set_operation_mode` — water_heater.set_operation_mode
  - paramètres : `operation_mode` (texte, requis), `target` (cible)
- `water_heater__set_temperature` — water_heater.set_temperature
  - paramètres : `temperature` (nombre 0.0…250.0, requis), `operation_mode` (texte), `target` (cible)
- `water_heater__turn_off` — water_heater.turn_off
  - paramètres : `target` (cible)
- `water_heater__turn_on` — water_heater.turn_on
  - paramètres : `target` (cible)

### weather (1)

- `weather__get_forecasts` — weather.get_forecasts (renvoie une réponse)
  - paramètres : `type` (daily / hourly / twice_daily, requis), `target` (cible)

### zone (1)

- `zone__reload` — zone.reload
