"""Test réel : ha-mcp contre de vrais conteneurs Home Assistant (et maison), seulement la bibliothèque standard.

    python -I tests/reel/test_reel.py [--image IMAGE_MCP] [--sortie DOSSIER] stable 2025.6 2024.6 maison
    python -I tests/reel/test_reel.py maison-dev=IMAGE    (une autre image : réglages de maison, ou de HA)

Pour chaque cible : conteneur neuf `hamcp-test-<cible>`, création du compte (onboarding, ou connexion si le compte
existe déjà), jeton longue durée créé PAR ha-mcp (outil ws, transport stdio), puis ha-mcp en HTTP
(`hamcp-test-mcp-<cible>`) : tools/list, flux SSE, input_boolean + outil généré input_boolean__toggle, template,
tableau de bord, intégration sans matériel (shopping_list) et list_changed reçu, script créé (service script.<id>
nouveau, list_changed et outil généré), puis lecture seule et `lister`. Étape « sans objet » : capacité absente du
serveur, détectée par ha-mcp (elle ne compte pas comme un échec).
Mots de passe et jetons tirés au hasard, jamais écrits. Conteneurs retirés par leur nom exact à la fin.
"""

import json
import os
import queue
import re
import secrets
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

RESEAU = "hamcp-test-net"
CIBLES = {
    "stable": ("ghcr.io/home-assistant/home-assistant:stable", 8123, 40),
    "2025.6": ("ghcr.io/home-assistant/home-assistant:2025.6", 8123, 40),
    "2024.6": ("ghcr.io/home-assistant/home-assistant:2024.6", 8123, 40),
    "maison": ("ghcr.io/tojiisamaa/home-assistant-rust:latest", 8124, 90),
}
CLIENT = "http://127.0.0.1/"


def docker(*args, env=None, check=True, timeout=120):
    r = subprocess.run(
        ["docker", *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        env=env,
        timeout=timeout,
    )
    if check and r.returncode != 0:
        raise RuntimeError(f"docker {args[0]} : {r.stderr.strip()[:500]}")
    return r.stdout.strip()


def http(methode, url, corps=None, entetes=None, forme=False, delai=30):
    donnees = None
    entetes = dict(entetes or {})
    if corps is not None:
        donnees = (
            urllib.parse.urlencode(corps).encode()
            if forme
            else json.dumps(corps).encode()
        )
        entetes.setdefault(
            "Content-Type",
            "application/x-www-form-urlencoded" if forme else "application/json",
        )
    req = urllib.request.Request(url, data=donnees, method=methode, headers=entetes)
    try:
        with urllib.request.urlopen(req, timeout=delai) as r:
            return r.status, r.read().decode(), dict(r.headers)
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode(errors="replace"), dict(e.headers)


def attendre(condition, delai, quoi, pas=1.0):
    fin = time.time() + delai
    while time.time() < fin:
        try:
            r = condition()
            if r:
                return r
        except Exception:
            pass
        time.sleep(pas)
    raise RuntimeError(f"délai dépassé ({delai} s) : {quoi}")


def port_publie(nom, port):
    return int(docker("port", nom, f"{port}/tcp").splitlines()[0].rsplit(":", 1)[1])


def jeton_de_session(base, utilisateur, mot_de_passe):
    """Compte créé par l'onboarding s'il est encore à faire (seulement l'étape « user » : la suivante installerait
    shopping_list d'office), sinon connexion par le flux de connexion standard."""
    statut, corps, _ = http("GET", f"{base}/api/onboarding")
    etapes = json.loads(corps) if statut == 200 else []
    if any(e.get("step") == "user" and not e.get("done") for e in etapes):
        statut, corps, _ = http(
            "POST",
            f"{base}/api/onboarding/users",
            {
                "client_id": CLIENT,
                "name": "Test",
                "username": utilisateur,
                "password": mot_de_passe,
                "language": "en",
            },
        )
        if statut != 200:
            raise RuntimeError(f"onboarding : HTTP {statut} {corps[:200]}")
        code = json.loads(corps)["auth_code"]
    else:
        statut, corps, _ = http(
            "POST",
            f"{base}/auth/login_flow",
            {
                "client_id": CLIENT,
                "handler": ["homeassistant", None],
                "redirect_uri": CLIENT,
            },
        )
        flux = json.loads(corps)["flow_id"]
        statut, corps, _ = http(
            "POST",
            f"{base}/auth/login_flow/{flux}",
            {"client_id": CLIENT, "username": utilisateur, "password": mot_de_passe},
        )
        r = json.loads(corps)
        if r.get("type") != "create_entry":
            raise RuntimeError(f"connexion refusée : {corps[:200]}")
        code = r["result"]
    statut, corps, _ = http(
        "POST",
        f"{base}/auth/token",
        {"grant_type": "authorization_code", "code": code, "client_id": CLIENT},
        forme=True,
    )
    if statut != 200:
        raise RuntimeError(f"/auth/token : HTTP {statut}")
    return json.loads(corps)["access_token"]


class Stdio:
    """ha-mcp en stdio dans un conteneur jetable."""

    def __init__(self, image, nom, url_ha, jeton, *options, env_sup=None):
        env = dict(os.environ, HA_TOKEN=jeton, **(env_sup or {}))
        args = [
            "docker",
            "run",
            "-i",
            "--rm",
            "--name",
            nom,
            "--network",
            RESEAU,
            "-e",
            "HA_TOKEN",
            "-e",
            f"HA_URL={url_ha}",
        ]
        args += [x for k in (env_sup or {}) for x in ("-e", k)]
        self.p = subprocess.Popen(
            [*args, image, *options],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            env=env,
            text=True,
            encoding="utf-8",
        )
        self.q = queue.Queue()
        threading.Thread(target=self._lire, daemon=True).start()
        self.id = 0

    def _lire(self):
        for ligne in self.p.stdout:
            if ligne.strip():
                self.q.put(json.loads(ligne))

    def requete(self, methode, params=None, delai=60):
        self.id += 1
        self.p.stdin.write(
            json.dumps(
                {
                    "jsonrpc": "2.0",
                    "id": self.id,
                    "method": methode,
                    "params": params or {},
                }
            )
            + "\n"
        )
        self.p.stdin.flush()
        fin = time.time() + delai
        while True:
            m = self.q.get(timeout=max(0.1, fin - time.time()))
            if m.get("id") == self.id:
                return m

    def notifier(self, methode):
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": methode}) + "\n")
        self.p.stdin.flush()

    def fermer(self):
        try:
            self.p.stdin.close()
            self.p.wait(timeout=30)
        except Exception:
            self.p.kill()


def initialiser(s):
    r = s.requete(
        "initialize",
        {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "test-reel", "version": "1"},
        },
    )
    s.notifier("notifications/initialized")
    return r


class Http:
    def __init__(self, base, jeton):
        self.base, self.jeton, self.sid, self.id = base, jeton, None, 0
        self.evenements = []

    def entetes(self):
        e = {
            "Authorization": f"Bearer {self.jeton}",
            "Accept": "application/json, text/event-stream",
            "MCP-Protocol-Version": "2025-06-18",
        }
        if self.sid:
            e["Mcp-Session-Id"] = self.sid
        return e

    def requete(self, methode, params=None, delai=60):
        self.id += 1
        statut, corps, entetes = http(
            "POST",
            f"{self.base}/mcp",
            {
                "jsonrpc": "2.0",
                "id": self.id,
                "method": methode,
                "params": params or {},
            },
            self.entetes(),
            delai=delai,
        )
        if statut != 200:
            raise RuntimeError(f"{methode} : HTTP {statut} {corps[:200]}")
        self.sid = self.sid or {k.lower(): v for k, v in entetes.items()}.get(
            "mcp-session-id"
        )
        return json.loads(corps)

    def notifier(self, methode):
        return http(
            "POST",
            f"{self.base}/mcp",
            {"jsonrpc": "2.0", "method": methode},
            self.entetes(),
        )[0]

    def ecouter(self):
        """Flux SSE (GET /mcp) lu dans un fil : (heure, message) de chaque `data:`."""
        import http.client

        hote = urllib.parse.urlparse(self.base)
        c = http.client.HTTPConnection(hote.hostname, hote.port, timeout=600)
        e = self.entetes()
        e["Accept"] = "text/event-stream"
        c.request("GET", "/mcp", headers=e)
        r = c.getresponse()
        if r.status != 200:
            raise RuntimeError(f"GET /mcp : HTTP {r.status}")

        def lire():
            try:
                for ligne in r:
                    ligne = ligne.decode().strip()
                    if ligne.startswith("data:"):
                        self.evenements.append((time.time(), json.loads(ligne[5:])))
            except Exception:
                pass

        threading.Thread(target=lire, daemon=True).start()

    def outil(self, nom, args=None, delai=60):
        r = self.requete("tools/call", {"name": nom, "arguments": args or {}}, delai)
        if "error" in r:
            raise RuntimeError(f"{nom} : {r['error']}")
        return r["result"]["isError"], r["result"]["content"][0]["text"]

    def outils(self):
        return [o["name"] for o in self.requete("tools/list")["result"]["tools"]]


def ok(res, etape, condition, detail=""):
    res["etapes"][etape] = "ok" if condition else f"ÉCHEC {detail}"[:300]
    print(f"  [{res['cible']}] {etape} : {res['etapes'][etape]}", flush=True)
    return condition


def tester(cible, image_mcp, sortie):
    image, port, delai_liste = CIBLES[cible]
    nom = "hamcp-test-" + cible.replace(".", "-")
    nom_mcp = "hamcp-test-mcp-" + cible.replace(".", "-")
    res = {"cible": cible, "image": image, "etapes": {}}
    utilisateur, mot_de_passe = "hamcp", secrets.token_urlsafe(18)
    debut = time.time()
    try:
        docker("network", "create", RESEAU, check=False)
        for n in (nom, nom_mcp):
            docker("rm", "-f", n, check=False)
        env = dict(
            os.environ, MAISON_ADMIN=utilisateur, MAISON_ADMIN_MOT_DE_PASSE=mot_de_passe
        )
        docker(
            "run",
            "-d",
            "--name",
            nom,
            "--network",
            RESEAU,
            "-p",
            f"127.0.0.1::{port}",
            "-e",
            "TZ=UTC",
            "-e",
            "MAISON_ADMIN",
            "-e",
            "MAISON_ADMIN_MOT_DE_PASSE",
            image,
            env=env,
        )
        base = f"http://127.0.0.1:{port_publie(nom, port)}"
        url_ha = f"http://{nom}:{port}"
        attendre(
            lambda: http("GET", f"{base}/api/", delai=5)[0] in (200, 401),
            240,
            "démarrage du serveur",
        )
        res["demarrage_s"] = round(time.time() - debut, 1)
        jeton = attendre(
            lambda: jeton_de_session(base, utilisateur, mot_de_passe),
            60,
            "compte et jeton",
            pas=3,
        )
        ok(res, "compte", True)
        # serveur complètement démarré (HA : state RUNNING ; un serveur sans ce champ passe)
        attendre(
            lambda: (
                json.loads(
                    http(
                        "GET",
                        f"{base}/api/config",
                        entetes={"Authorization": f"Bearer {jeton}"},
                    )[1]
                ).get("state", "RUNNING")
                == "RUNNING"
            ),
            180,
            "état RUNNING",
            pas=2,
        )
        res["version"] = json.loads(
            http(
                "GET",
                f"{base}/api/config",
                entetes={"Authorization": f"Bearer {jeton}"},
            )[1]
        ).get("version")

        # jeton longue durée créé par ha-mcp lui-même (stdio, outil ws)
        s = Stdio(
            image_mcp, f"hamcp-test-stdio-{cible.replace('.', '-')}", url_ha, jeton
        )
        init = initialiser(s)
        r = s.requete(
            "tools/call",
            {
                "name": "ws",
                "arguments": {
                    "type": "auth/long_lived_access_token",
                    "client_name": "hamcp-test",
                    "lifespan": 1,
                },
            },
        )
        s.fermer()
        llat = (
            None
            if r["result"]["isError"]
            else r["result"]["content"][0]["text"].strip().strip('"')
        )
        ok(
            res,
            "stdio_et_jeton_longue_duree",
            init.get("result", {}).get("serverInfo", {}).get("name")
            == "home-assistant-mcp-rust"
            and bool(llat),
            r["result"]["content"][0]["text"][:200],
        )
        jeton_mcp = llat or jeton

        # ha-mcp lister sur l'instance vierge
        md = docker(
            "run",
            "--rm",
            "--network",
            RESEAU,
            "-e",
            "HA_TOKEN",
            "-e",
            f"HA_URL={url_ha}",
            image_mcp,
            "lister",
            env=dict(os.environ, HA_TOKEN=jeton_mcp),
            timeout=120,
        )
        with open(
            os.path.join(sortie, f"outils-{cible}.md"), "w", encoding="utf-8"
        ) as f:
            f.write(md + "\n")
        m = re.search(
            r"Commandes absentes de ce serveur \(outils qui en dépendent retirés\) : (.*)\.",
            md,
        )
        res["absentes"] = re.findall(r"`([^`]+)`", m.group(1)) if m else []
        ok(res, "lister", md.startswith("# Outils de Home Assistant MCP Rust"))

        # serveur HTTP
        secret = secrets.token_urlsafe(24)
        docker(
            "run",
            "-d",
            "--name",
            nom_mcp,
            "--network",
            RESEAU,
            "-p",
            "127.0.0.1::8765",
            "-e",
            "HA_TOKEN",
            "-e",
            f"HA_URL={url_ha}",
            "-e",
            "HA_MCP_JETON",
            image_mcp,
            "http",
            env=dict(os.environ, HA_TOKEN=jeton_mcp, HA_MCP_JETON=secret),
        )
        mcp = Http(f"http://127.0.0.1:{port_publie(nom_mcp, 8765)}", secret)
        attendre(
            lambda: http("GET", f"{mcp.base}/sante", delai=5)[1] == "ok",
            60,
            "ha-mcp http",
        )
        ok(
            res,
            "http_initialize",
            mcp.requete(
                "initialize",
                {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "test-reel", "version": "1"},
                },
            )["result"]["protocolVersion"]
            == "2025-06-18"
            and mcp.sid,
        )
        mcp.notifier("notifications/initialized")
        mcp.ecouter()
        noms = mcp.outils()
        de_service = [n for n in noms if "__" in n]
        res["outils"] = {
            "total": len(noms),
            "generiques": len(noms) - len(de_service),
            "service": len(de_service),
        }
        ok(
            res,
            "tools_list",
            len(de_service) > 20 and "appeler_service" in noms,
            str(res["outils"]),
        )
        ok(
            res,
            "noms_valides",
            all(re.fullmatch(r"[a-zA-Z0-9_-]{1,64}", n) for n in noms),
        )

        # input_boolean : création (ws), outil généré, état
        err, t = mcp.outil("ws", {"type": "input_boolean/create", "name": "Test MCP"})
        entite = "input_boolean." + json.loads(t)["id"] if not err else None
        if ok(res, "input_boolean_create", not err, t) and ok(
            res, "outil_input_boolean__toggle_present", "input_boolean__toggle" in noms
        ):
            err, t = mcp.outil(
                "input_boolean__toggle", {"target": {"entity_id": entite}}
            )
            etat = (
                attendre(
                    lambda: (
                        json.loads(mcp.outil("etats", {"entity_id": entite})[1])[
                            "state"
                        ]
                        == "on"
                    ),
                    10,
                    "état on",
                    pas=0.5,
                )
                if not err
                else False
            )
            ok(res, "input_boolean__toggle", not err and etat, t)

        err, t = mcp.outil("template", {"template": "{{ 6 * 7 }}"})
        ok(res, "template", not err and t.strip() == "42", t)

        contenu = {
            "views": [
                {
                    "title": "Test",
                    "cards": [{"type": "markdown", "content": "ha-mcp ok"}],
                }
            ]
        }
        if "page_creer" in noms:
            err, t = mcp.outil(
                "page_creer",
                {
                    "url_path": "hamcp-test",
                    "title": "Test MCP",
                    "icon": "mdi:test-tube",
                    "config": contenu,
                },
            )
            err2, t2 = (
                mcp.outil("tableau_lire", {"url_path": "hamcp-test"})
                if not err
                else (True, "")
            )
            ok(
                res,
                "tableau_de_bord",
                not err and not err2 and "ha-mcp ok" in t2,
                t + t2,
            )
        elif "tableau_enregistrer" in noms:
            # serveur qui ne sait pas créer de tableau (sonde lovelace/dashboards/create) : le tableau par défaut
            res["etapes"]["page_creer"] = (
                "sans objet : lovelace/dashboards/create absent du serveur"
            )
            err, t = mcp.outil("tableau_enregistrer", {"config": contenu})
            err2, t2 = mcp.outil("tableau_lire", {}) if not err else (True, "")
            ok(
                res,
                "tableau_de_bord",
                not err and not err2 and "ha-mcp ok" in t2,
                t + t2,
            )
        else:
            # écriture des tableaux absente du serveur (sondes) : la lecture doit marcher, les outils d'écriture absents
            err, t = mcp.outil("tableaux_lister", {})
            ok(res, "tableaux_lister", not err, t)
            res["etapes"]["tableau_de_bord"] = (
                "sans objet : lovelace/dashboards/create et lovelace/config/save absents du serveur"
            )

        def list_changed(etape, avant):
            try:
                recu = attendre(
                    lambda: next(
                        (
                            h
                            for h, m in mcp.evenements
                            if h >= avant
                            and m.get("method") == "notifications/tools/list_changed"
                        ),
                        None,
                    ),
                    delai_liste + 5,
                    "list_changed",
                    pas=0.5,
                )
                res[etape + "_s"] = round(recu - avant, 1)
                ok(
                    res,
                    etape,
                    res[etape + "_s"] <= delai_liste,
                    f"{res[etape + '_s']} s",
                )
            except RuntimeError as e:
                ok(res, etape, False, str(e))

        # intégration sans matériel, puis list_changed et le nouvel outil (maison : chargée d'office, sans assistant)
        if "shopping_list__add_item" in noms:
            for etape in (
                "integration_shopping_list",
                "list_changed",
                "nouvel_outil_shopping_list__add_item",
            ):
                res["etapes"][etape] = (
                    "sans objet : shopping_list déjà chargée au démarrage"
                )
        else:
            avant = time.time()
            err, t = mcp.outil("flux_demarrer", {"handler": "shopping_list"})
            flux = json.loads(t) if not err else {}
            if flux.get("type") == "form":
                err, t = mcp.outil(
                    "flux_etape", {"flow_id": flux["flow_id"], "data": {}}
                )
                flux = json.loads(t) if not err else {}
            ok(res, "integration_shopping_list", flux.get("type") == "create_entry", t)
            list_changed("list_changed", avant)
            ok(
                res,
                "nouvel_outil_shopping_list__add_item",
                "shopping_list__add_item" in mcp.outils(),
            )
        if "shopping_list__add_item" in mcp.outils():
            err, t = mcp.outil("shopping_list__add_item", {"name": "lait"})
            ok(res, "appel_shopping_list__add_item", not err, t)

        # un script créé = un service de plus (script.hamcp_test) : list_changed et l'outil script__hamcp_test
        avant = time.time()
        err, t = mcp.outil(
            "automatisme_ecrire",
            {
                "domaine": "script",
                "id": "hamcp_test",
                "config": {"alias": "Test MCP", "sequence": [{"event": "hamcp_test"}]},
            },
        )
        if ok(res, "script_cree", not err, t):
            list_changed("list_changed_script", avant)
            noms2 = mcp.outils()
            if "script__hamcp_test" not in noms2 and "script__reload" in noms2:
                # serveur qui ne recharge pas après l'enregistrement : la veille doit voir le service après script.reload
                res["notes"] = (
                    "script absent après l'enregistrement, script.reload appelé à la main"
                )
                avant = time.time()
                mcp.outil("script__reload", {})
                list_changed("list_changed_apres_script_reload", avant)
                noms2 = mcp.outils()
            if ok(
                res,
                "nouvel_outil_script__hamcp_test",
                "script__hamcp_test" in noms2,
                f"{len(noms2)} outils",
            ):
                err, t = mcp.outil("script__hamcp_test", {})
                ok(res, "appel_script__hamcp_test", not err, t)
        res["outils_apres"] = len(mcp.outils())

        # lecture seule
        s = Stdio(
            image_mcp,
            f"hamcp-test-stdio-{cible.replace('.', '-')}",
            url_ha,
            jeton_mcp,
            env_sup={"HA_MCP_LECTURE_SEULE": "1"},
        )
        initialiser(s)
        l = s.requete("tools/list")["result"]["tools"]
        r = s.requete(
            "tools/call",
            {
                "name": "ws",
                "arguments": {
                    "type": "call_service",
                    "domain": "input_boolean",
                    "service": "toggle",
                    "target": {"entity_id": entite},
                },
            },
        )
        s.fermer()
        ok(
            res,
            "lecture_seule",
            all(o["annotations"]["readOnlyHint"] for o in l)
            and not any("__" in o["name"] for o in l)
            and r["result"]["isError"],
            f"{len(l)} outils",
        )
    except Exception as e:
        res["erreur"] = f"{type(e).__name__}: {e}"[:500]
        print(f"  [{cible}] ERREUR : {res['erreur']}", flush=True)
    finally:
        for n in (nom_mcp, nom):
            docker("rm", "-f", n, check=False)
        docker("network", "rm", RESEAU, check=False)
        res["duree_s"] = round(time.time() - debut, 1)
    res["reussi"] = "erreur" not in res and all(
        v == "ok" or v.startswith("sans objet") for v in res["etapes"].values()
    )
    return res


def main():
    args = sys.argv[1:]
    image_mcp, sortie = "hamcp-test-mcp:local", "."
    while args and args[0].startswith("--"):
        cle, val, args = args[0], args[1], args[2:]
        if cle == "--image":
            image_mcp = val
        elif cle == "--sortie":
            sortie = val
    os.makedirs(sortie, exist_ok=True)
    cibles = []
    for a in args or ["stable", "maison"]:
        # « nom=image » : une cible de plus (réglages de maison si le nom commence par maison, sinon ceux de HA)
        if "=" in a:
            a, image = a.split("=", 1)
            CIBLES[a] = (
                image,
                *CIBLES["maison" if a.startswith("maison") else "stable"][1:],
            )
        cibles.append(a)
    resultats = [tester(c, image_mcp, sortie) for c in cibles]
    with open(
        os.path.join(
            sortie, "resultats-" + "-".join(r["cible"] for r in resultats) + ".json"
        ),
        "w",
        encoding="utf-8",
    ) as f:
        json.dump(resultats, f, ensure_ascii=False, indent=2)
    print(json.dumps(resultats, ensure_ascii=False, indent=2))
    sys.exit(0 if all(r["reussi"] for r in resultats) else 1)


if __name__ == "__main__":
    main()
