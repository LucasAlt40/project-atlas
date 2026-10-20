"""Retrieval quality of Atlas's external memory (ADR 0040). No model, no cost.

A server full of lookalike notes (40 subjects, one right note each, 200 notes about other things
or the same thing), and two kinds of task: worded like the note ("direct") and worded differently
("paraphrase"). For each, it measures what Atlas's query would find with and without the title
check (`precise`). "Foreign" means a note about a different subject than the task.

Run against a THROWAWAY server (it writes 240 pages into workspace `bench`, project `noise`):
  scripts/ai-memory-local.sh --data-dir /tmp/aimem-bench --port 49392 &
  python3 -I docs/architecture/spikes/ai-memory-bench/retrieval_bench.py \
      http://127.0.0.1:49392 /tmp/aimem-bench src-tauri/src/domain/memory.rs [corpus.json]
"""
import json, random, re, sys, urllib.parse, urllib.request

URL, DATA, MEMORY_RS = sys.argv[1], sys.argv[2], sys.argv[3]
if urllib.parse.urlparse(URL).hostname not in ("127.0.0.1", "localhost", "::1"):
    sys.exit("refusing to send the access token anywhere but this machine")
TOKEN = open(f"{DATA}/auth-token").read().strip()
WS, PROJ = "bench", "noise"
random.seed(7)

SUBJECTS = [
 ("retry limit","MAXTRY","deploy/limits.cfg","retries","config/app.toml"),
 ("cache ttl","CACHE_TTL","deploy/cache.env","ttl","config/cache.toml"),
 ("database pool size","DB_POOL","deploy/db.env","pool_size","config/database.toml"),
 ("request timeout","REQ_TIMEOUT","deploy/net.cfg","timeout","config/http.toml"),
 ("rate limit","RATE_MAX","deploy/gateway.cfg","rate","config/limits.toml"),
 ("log level","LOG_LEVEL","deploy/logging.env","level","config/log.toml"),
 ("batch size","BATCH_N","deploy/jobs.cfg","batch","config/worker.toml"),
 ("session expiry","SESSION_MIN","deploy/auth.env","expiry","config/session.toml"),
 ("upload size limit","MAX_UPLOAD_MB","deploy/web.cfg","upload_limit","config/upload.toml"),
 ("queue concurrency","QUEUE_PAR","deploy/queue.env","concurrency","config/queue.toml"),
 ("cron schedule","CRON_EXPR","deploy/cron.cfg","schedule","config/scheduler.toml"),
 ("feature flag rollout","ROLLOUT_PCT","deploy/flags.env","rollout","config/flags.toml"),
 ("backoff delay","BACKOFF_MS","deploy/retry.cfg","delay","config/backoff.toml"),
 ("page size","PAGE_SZ","deploy/api.cfg","page_size","config/pagination.toml"),
 ("connection idle time","IDLE_SEC","deploy/pool.env","idle","config/conn.toml"),
 ("token lifetime","TOKEN_TTL","deploy/jwt.env","lifetime","config/token.toml"),
 ("max retries for webhooks","HOOK_TRIES","deploy/hooks.cfg","webhook_retries","config/webhooks.toml"),
 ("thread count","THREADS","deploy/server.cfg","threads","config/runtime.toml"),
 ("memory ceiling","MEM_MB","deploy/limits.env","memory","config/resources.toml"),
 ("health check interval","HC_SEC","deploy/health.cfg","interval","config/health.toml"),
 ("lock timeout","LOCK_MS","deploy/locks.env","lock","config/locks.toml"),
 ("polling interval","POLL_SEC","deploy/sync.cfg","poll","config/sync.toml"),
 ("max payload bytes","MAX_BODY","deploy/ingest.env","payload","config/ingest.toml"),
 ("grace period","GRACE_SEC","deploy/shutdown.cfg","grace","config/lifecycle.toml"),
 ("email send limit","MAIL_PER_MIN","deploy/mail.env","mail_rate","config/mail.toml"),
 ("search result cap","SEARCH_TOP","deploy/search.cfg","top","config/search.toml"),
 ("circuit breaker threshold","CB_FAILS","deploy/breaker.env","threshold","config/breaker.toml"),
 ("snapshot interval","SNAP_MIN","deploy/backup.cfg","snapshot","config/backup.toml"),
 ("worker restart limit","RESTARTS","deploy/supervisor.env","restarts","config/supervisor.toml"),
 ("debounce delay","DEBOUNCE_MS","deploy/ui.cfg","debounce","config/ui.toml"),
 ("max open files","NOFILE","deploy/system.env","nofile","config/system.toml"),
 ("retention days","KEEP_DAYS","deploy/retention.cfg","retention","config/retention.toml"),
 ("compression level","ZLEVEL","deploy/storage.env","compression","config/storage.toml"),
 ("warmup duration","WARM_SEC","deploy/boot.cfg","warmup","config/boot.toml"),
 ("queue depth","QDEPTH","deploy/stream.env","depth","config/stream.toml"),
 ("jitter percent","JITTER_PCT","deploy/net.env","jitter","config/network.toml"),
 ("lease duration","LEASE_SEC","deploy/leader.cfg","lease","config/leader.toml"),
 ("cooldown period","COOL_SEC","deploy/alerts.env","cooldown","config/alerts.toml"),
 ("max line length","LINE_MAX","deploy/parser.cfg","line_max","config/parser.toml"),
 ("heap size","HEAP_MB","deploy/jvm.env","heap","config/jvm.toml"),
]
PARA = ["how many attempts the client makes before giving up","how long cached entries stay valid","how many connections to the database we keep open","how long we wait for a response before cutting the call","how many calls per minute a client may make","how verbose the logs are","how many items each job handles at once","how long a login stays valid","the biggest file a user can send","how many jobs run in parallel","when the nightly job fires","what share of users see the new feature","how long we pause between attempts","how many rows an endpoint returns per call","how long an unused connection stays open","how long an access token is good for","how many times a failed callback is resent","how many worker threads the server starts","the most RAM a process may use","how often we ping to see if it is alive","how long we wait to grab a lock","how often we ask for updates","the largest request body we accept","how long a stopping process gets to finish","how many emails go out each minute","the maximum number of hits a search returns","how many failures trip the breaker","how often backups are taken","how many times a crashed process is brought back","how long the UI waits after typing","how many file handles a process may hold","how many days data is kept","how hard files are compressed","how long the service waits before taking traffic","how many messages may wait in line","how much random spread is added to delays","how long the leader holds its lease","how long alerts stay quiet after firing","the longest line the parser reads","how much heap the JVM gets"]
GENERIC = [
 "When you change a value in {f}, restart the service afterwards: it is read once at start.",
 "Do not change the {k} limit in production without telling the on-call; a wrong value took the service down once.",
 "The {n} section of the docs is out of date; check {f} instead of the wiki before you change anything.",
 "A change to {k} needs a deploy, not only a config edit: the service caches {n} until it restarts.",
 "Tests for {n} live in tests/{s}_test.py and need the service running; a limit that is too low makes them flaky.",
 "Changing {n} also changes the dashboard thresholds; the config in {f} is the source of truth, the dashboard follows.",
 "The default for {n} differs between staging and production; compare {f} on both before you change the limit.",
 "Never hard-code {k}: the service reads it from {f} and the deploy script exports it.",
]

def slug(s): return re.sub(r"[^a-z0-9]+", "_", s.lower()).strip("_")

def call(path, obj=None):
    req = urllib.request.Request(URL + path, method="POST" if obj else "GET",
        data=json.dumps(obj).encode() if obj else None,
        headers={"Authorization": "Bearer " + TOKEN, "Content-Type": "application/json"})
    return json.loads(urllib.request.urlopen(req, timeout=10).read() or b"null")

pages, subject_of = {}, {}
for i, (name, key, f, dk, df) in enumerate(SUBJECTS):
    pages[f"gold-{i}"] = (f"{name.capitalize()} lives in {f}",
        f"The {name} is {key} in {f}. The {dk} key in {df} is not read by anything: leave it alone.")
    subject_of[f"gold-{i}"] = i
    for j in range(5):
        o = random.choice(SUBJECTS)
        pages[f"noise-{i}-{j}"] = (f"About {o[0]}", random.choice(GENERIC).format(
            f=o[2], k=o[1], n=o[0], s=slug(o[0])))
        subject_of[f"noise-{i}-{j}"] = SUBJECTS.index(o)
for pid, (title, body) in pages.items():
    call("/admin/write-page", {"workspace": WS, "project": PROJ, "path": f"notes/{pid}.md",
        "body": body, "title": title, "kind": "gotcha", "tier": "semantic", "tags": ["bench"], "pinned": False})
print("pages written:", len(pages))
if len(sys.argv) > 4:  # a 4th argument: also dump the corpus for the native-memory comparison test
    json.dump({"pages": {k: list(v) for k, v in pages.items()}, "subject_of": subject_of,
               "direct": [f"Change the {x[0]} of this service from 3 to 5, and tell me briefly what you changed." for x in SUBJECTS],
               "paraphrase": [f"Change {q} in this service, and tell me briefly what you changed." for q in PARA]},
              open(sys.argv[4], "w"))

block = open(MEMORY_RS).read()
block = block[block.index("const STOP_WORDS"):]; block = block[:block.index("];")]
STOP = set(re.findall(r'"([^"]+)"', block))
def terms_in_order(task, limit=12):
    seen = []
    for w in re.split(r"[^\w]+", task.lower()):
        if len(w) >= 3 and w not in STOP and w not in seen: seen.append(w)
    return seen[:limit]

def search(terms, limit):
    q = urllib.parse.quote(" OR ".join(terms))
    return call(f"/admin/search?q={q}&workspace={WS}&project={PROJ}&limit={limit}")

def stem(word):
    """The same crude stemmer as application/memory/external.rs, so a title meets a task across endings."""
    w = word.lower()
    if len(w) > 4 and w.endswith("ies"):
        w = w[:-3] + "y"
    elif len(w) > 5 and w.endswith("ing"):
        w = w[:-3]
    elif len(w) > 4 and w.endswith("ed"):
        w = w[:-2]
    elif len(w) > 4 and w.endswith("es"):
        w = w[:-2]
    elif len(w) > 3 and w.endswith("s") and not w.endswith("ss"):
        w = w[:-1]
    if len(w) > 3 and w.endswith("e"):
        w = w[:-1]
    return w

def title_ok(title, terms):
    words = set(stem(w) for w in re.split(r"[^\w]+", title.lower()) if w)
    return sum(1 for t in terms if stem(t) in words) >= min(2, len(terms))

def evaluate(label, tasks, precise):
    prompts = notes = gold3 = foreign = 0
    for i, task in enumerate(tasks):
        terms = terms_in_order(task)
        hits = search(terms, 25)
        ids = []
        for h in hits:
            m = re.match(r"^notes/(.+)\.md$", h["path"])
            if not m or (precise and not title_ok(h["title"], terms)): continue
            ids.append(m.group(1))
        ids = ids[:3]
        prompts += bool(ids); notes += len(ids); gold3 += f"gold-{i}" in ids
        foreign += sum(1 for x in ids if subject_of.get(x) != i)
    n = len(tasks)
    print(f"{label:34s} notes in {prompts:2d}/{n} prompts ({notes:3d} notes) | right note in top 3: {gold3:2d}/{n} | other-subject notes: {foreign:2d}")

direct = [f"Change the {s[0]} of this service from 3 to 5, and tell me briefly what you changed." for s in SUBJECTS]
para = [f"Change {p} in this service, and tell me briefly what you changed." for p in PARA]
for precise in (False, True):
    tag = "with title check" if precise else "search only"
    evaluate(f"direct tasks, {tag}", direct, precise)
    evaluate(f"paraphrased tasks, {tag}", para, precise)
