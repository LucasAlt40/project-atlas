#!/usr/bin/env python3
"""Demonstração da memória do Atlas num projeto de brinquedo (guia: docs/guia/10-memoria-e-notas.md).

O projeto tem uma isca: `config/app.toml` tem uma chave `retries` que ninguém lê, e o limite de
tentativas de verdade é `MAXTRY` em `deploy/limits.cfg`. Um agente sem memória costuma editar a isca.
Com a nota certa (e outras 14 notas parecidas, para a memória ficar cheia) ele não edita.

  python3 -I scripts/demo-memoria.py project  ~/atlas-demo/billing-client
  python3 -I scripts/demo-memoria.py notes    --workspace "Demo memória"            # só mostra o que faria
  python3 -I scripts/demo-memoria.py notes    --workspace "Demo memória" --write    # grava as notas
  python3 -I scripts/demo-memoria.py clean    --workspace "Demo memória" --write    # remove o que gravou

Feche o Atlas antes de gravar ou remover notas e abra de novo depois: as notas são lidas na partida.
Só o que começa com `memory-demo-` é criado ou removido, e só dentro de `<pasta de dados>/memory/`.
"""

import argparse
import json
import os
import subprocess
import sys
import time

APP_ID = "io.github.lucasalt40.projectatlas"
PREFIX = "memory-demo-"


def default_data_dir():
    if sys.platform == "darwin":
        return os.path.expanduser(f"~/Library/Application Support/{APP_ID}")
    if sys.platform.startswith("win"):
        return os.path.join(os.environ.get("APPDATA", ""), APP_ID)
    return os.path.join(os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share")), APP_ID)


PROJECT_FILES = {
    "README.md": "# Billing client\n\nA small client for the billing API.\n",
    "config/app.toml": '# Application settings\nname = "billing"\nretries = 3\ntimeout_seconds = 30\n',
    "deploy/limits.cfg": "# Limits applied when the service starts\nMAXTRY=3\nTIMEOUT=30\n",
    "src/client.py": (
        "import os\n\nMAX_TRY = int(os.environ.get(\"MAXTRY\", \"3\"))\n\n\n"
        "def call(api):\n    for _ in range(MAX_TRY):\n        result = api()\n"
        "        if result is not None:\n            return result\n"
        "    raise RuntimeError(\"billing API unavailable\")\n"
    ),
    "src/config_loader.py": (
        "import tomllib\n\n\ndef load(path=\"config/app.toml\"):\n"
        "    with open(path, \"rb\") as handle:\n        data = tomllib.load(handle)\n"
        "    return {\"name\": data[\"name\"], \"timeout\": data[\"timeout_seconds\"]}\n"
    ),
    "docs/operations.md": "# Operations\n\nThe service is restarted by the deploy script, which exports the limits file.\n",
}

# (kind, title, content). The first is the note that matters; the rest are lessons of the same
# kind that share words like "change", "service" and "limit" with the task, so the memory is a crowd.
NOTES = [
    ("pitfall", "Retry limit lives in deploy/limits.cfg",
     "The retry limit is MAXTRY in deploy/limits.cfg. The retries key in config/app.toml is not read by anything: leave it alone."),
    ("pitfall", "Cache TTL lives in deploy/cache.env",
     "The cache TTL is CACHE_TTL in deploy/cache.env; the ttl key in config/cache.toml is not read. Restart the service after you change it."),
    ("pitfall", "Request timeout lives in deploy/net.cfg",
     "The request timeout is REQ_TIMEOUT in deploy/net.cfg; the timeout key in config/http.toml is not read. Restart the service after you change it."),
    ("pitfall", "Log level is set in deploy/logging.env",
     "The log level is LOG_LEVEL in deploy/logging.env. When you change it, restart the service: it is read once at start."),
    ("pitfall", "Database pool size lives in deploy/db.env",
     "The pool size is DB_POOL in deploy/db.env, not in config/database.toml. Do not raise the limit past what the database allows."),
    ("pitfall", "Rate limit lives in deploy/gateway.cfg",
     "The rate limit is RATE_MAX in deploy/gateway.cfg. A wrong value took the service down once; tell the on-call when you change it."),
    ("pitfall", "Batch size lives in deploy/jobs.cfg",
     "The batch size is BATCH_N in deploy/jobs.cfg. Changing it also changes the memory the service uses."),
    ("pitfall", "Session expiry lives in deploy/auth.env",
     "The session expiry is SESSION_MIN in deploy/auth.env. When you change it, restart the service."),
    ("pitfall", "Upload size limit lives in deploy/web.cfg",
     "The upload size limit is MAX_UPLOAD_MB in deploy/web.cfg; the proxy has its own limit that must change too."),
    ("pitfall", "Queue concurrency lives in deploy/queue.env",
     "The concurrency is QUEUE_PAR in deploy/queue.env. The service caches it until it restarts."),
    ("pitfall", "Feature flag rollout lives in deploy/flags.env",
     "The rollout percentage is ROLLOUT_PCT in deploy/flags.env. Change it in steps, not all at once."),
    ("how_to", "Restart the service after a deploy change",
     "Values in the deploy folder are read once at start: when you change one, restart the service afterwards."),
    ("how_to", "The tests need the service running",
     "Tests in tests/ need the service running; a limit that is too low makes them flaky."),
    ("decision", "Never hard-code a limit",
     "Never hard-code a limit in the code; the service reads it from the deploy folder and the deploy script exports it."),
    ("decision", "Dashboard thresholds follow the deploy files",
     "Changing a limit also changes the dashboard thresholds; the deploy file is the source of truth, the dashboard follows."),
]


def run_git(root, *args):
    subprocess.run(
        ["git", "-c", "user.email=demo@example.invalid", "-c", "user.name=demo", *args],
        cwd=root, check=True, capture_output=True,
    )


def make_project(target):
    target = os.path.abspath(os.path.expanduser(target))
    if os.path.exists(target) and os.listdir(target):
        sys.exit(f"{target} já existe e não está vazia; escolha outra pasta")
    for name, content in PROJECT_FILES.items():
        path = os.path.join(target, name)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w") as handle:
            handle.write(content)
    run_git(target, "init", "-q", "-b", "main")
    run_git(target, "add", ".")
    run_git(target, "commit", "-q", "-m", "init")
    print(f"Projeto criado em {target}")
    print("Próximo passo: no Atlas, crie um workspace com essa pasta (por exemplo 'Demo memória').")


def find_workspace(data_dir, name):
    config = os.path.join(data_dir, "config.json")
    try:
        workspaces = json.load(open(config)).get("workspaces", [])
    except (OSError, ValueError):
        sys.exit(f"não consegui ler {config}; use --data-dir se o Atlas guarda os dados em outro lugar")
    found = [w for w in workspaces if w.get("name", "").lower() == name.lower()]
    if len(found) != 1:
        names = ", ".join(repr(w.get("name")) for w in workspaces) or "nenhum"
        sys.exit(f"workspace {name!r} não encontrado (ou repetido). Workspaces do Atlas: {names}")
    return found[0]["id"]


def note_files(workspace_id):
    now = int(time.time() * 1000)
    out = []
    for index, (kind, title, content) in enumerate(NOTES, start=1):
        assert len(title) <= 80 and len(content) <= 480
        note_id = f"{PREFIX}{index:02d}"
        out.append((note_id, {
            "id": note_id, "scope": "workspace", "owner": workspace_id, "kind": kind,
            "title": title, "content": content, "topic": None,
            "provenance": {"origin": "user", "automatic": False},
            "status": "active", "validity": {}, "uses": 0,
            "createdAt": now + index, "updatedAt": now + index,
        }))
    return out


def notes(args):
    data_dir = os.path.abspath(os.path.expanduser(args.data_dir))
    workspace_id = find_workspace(data_dir, args.workspace)
    folder = os.path.join(data_dir, "memory")
    files = note_files(workspace_id)
    for note_id, note in files:
        print(f"  {note_id}  [{note['kind']}] {note['title']}")
    if not args.write:
        print(f"\n{len(files)} notas para o workspace {args.workspace!r} em {folder}")
        print("Nada foi gravado. Feche o Atlas e repita com --write.")
        return
    os.makedirs(folder, exist_ok=True)
    for note_id, note in files:
        with open(os.path.join(folder, f"{note_id}.json"), "w") as handle:
            json.dump(note, handle, indent=2, ensure_ascii=False)
    print(f"\n{len(files)} notas gravadas em {folder}. Abra o Atlas de novo.")


def clean(args):
    data_dir = os.path.abspath(os.path.expanduser(args.data_dir))
    find_workspace(data_dir, args.workspace)  # confirms this is an Atlas folder, nothing else
    folder = os.path.join(data_dir, "memory")
    mine = sorted(f for f in (os.listdir(folder) if os.path.isdir(folder) else []) if f.startswith(PREFIX) and f.endswith(".json"))
    for name in mine:
        print(f"  {'removendo' if args.write else 'removeria'} {name}")
        if args.write:
            os.remove(os.path.join(folder, name))
    if not args.write:
        print("Nada foi removido. Feche o Atlas e repita com --write.")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("project", help="cria o projeto de demonstração")
    p.add_argument("folder")
    for name in ("notes", "clean"):
        p = sub.add_parser(name)
        p.add_argument("--workspace", required=True, help="nome do workspace do Atlas")
        p.add_argument("--data-dir", default=default_data_dir())
        p.add_argument("--write", action="store_true", help="grava de verdade (sem isto, só mostra)")
    args = parser.parse_args()
    {"project": lambda a: make_project(a.folder), "notes": notes, "clean": clean}[args.command](args)


if __name__ == "__main__":
    main()
