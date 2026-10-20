"""An OpenAI-compatible chat endpoint backed by the `claude -p` CLI (experiment tool, not shipped).

Lets `ai-memory` (AI_MEMORY_LLM_PROVIDER=openai-compat) use the person's logged-in Claude without ever
holding a credential: the token stays inside the Claude CLI. Every call is a plain text completion
(`--tools ""`, no session, no slash commands, no MCP, and none of the person's own settings or
CLAUDE.md), so the model can read nothing and run nothing.

Safety rails: loopback only; a random bearer key; a ceiling on calls and on dollars; each call is logged
(tokens and cost) so an experiment can account for what the LLM step costs.

  python3 -I claude_llm_shim.py --port 49396 --key-file KEY --log calls.jsonl [--model haiku]
"""

import argparse
import hmac
import json
import os
import subprocess
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ARGS = None
LOCK = threading.Lock()
# One completion at a time: the cost of a call is only known when it ends, so with several at once the
# ceiling could be passed by as many calls as were in flight. ai-memory asks one at a time anyway.
RUN = threading.Lock()
STATE = {"calls": 0, "usd": 0.0}


def text_of(content):
    if isinstance(content, str):
        return content
    return "".join(part.get("text", "") for part in content or [] if isinstance(part, dict))


def run_claude(system, user, model):
    cmd = [
        "claude", "-p", "--model", model, "--tools", "", "--no-session-persistence",
        "--output-format", "json", "--disable-slash-commands", "--strict-mcp-config",
        # only the (empty) project: the person's own CLAUDE.md and settings must not shape a completion
        "--setting-sources", "project",
    ]
    if system:
        cmd += ["--system-prompt", system]
    with tempfile.TemporaryDirectory() as empty:
        done = subprocess.run(cmd, input=user, capture_output=True, text=True, timeout=300, cwd=empty)
    if done.returncode != 0:
        raise RuntimeError(done.stderr.strip()[:300] or f"claude exited {done.returncode}")
    return json.loads(done.stdout)


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):  # quiet
        pass

    def send(self, status, obj):
        body = json.dumps(obj).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def authorized(self):
        got = self.headers.get("Authorization")
        ok = hmac.compare_digest((got or "").encode(), f"Bearer {ARGS.key}".encode())
        if not ok:  # what arrived, never the key itself
            with open(ARGS.log, "a") as log:
                log.write(json.dumps({"refused": self.path, "auth_header": got is not None,
                                      "scheme": (got or "").split(" ")[0], "length": len(got or "")}) + "\n")
        return ok

    def do_GET(self):
        if not self.authorized():
            return self.send(401, {"error": "unauthorized"})
        if self.path.rstrip("/").endswith("/models"):
            return self.send(200, {"object": "list", "data": [{"id": ARGS.model, "object": "model"}]})
        self.send(404, {"error": "not found"})

    def do_POST(self):
        if not self.authorized():
            return self.send(401, {"error": "unauthorized"})
        if not self.path.rstrip("/").endswith("/chat/completions"):
            return self.send(404, {"error": "not found"})
        try:
            request = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        except (ValueError, TypeError):
            return self.send(400, {"error": "bad request"})
        RUN.acquire()
        try:
            self.complete(request)
        finally:
            RUN.release()

    def complete(self, request):
        with LOCK:
            if STATE["calls"] >= ARGS.max_calls or STATE["usd"] >= ARGS.max_usd:
                return self.send(429, {"error": {"message": "budget of the experiment reached"}})
            STATE["calls"] += 1
        system, parts = [], []
        for message in request.get("messages", []):
            body = text_of(message.get("content"))
            (system if message.get("role") == "system" else parts).append(body)
        user = "\n\n".join(parts)
        schema = (request.get("response_format") or {}).get("json_schema", {}).get("schema")
        if schema:
            system.append("Reply with ONLY a JSON object that matches this JSON Schema, no prose, no code fence:\n"
                          + json.dumps(schema))
        started = time.time()
        try:
            result = run_claude("\n\n".join(system), user, ARGS.model)
        except Exception as error:  # the caller falls back or reports; never crash the server
            return self.send(502, {"error": {"message": str(error)}})
        usage = result.get("usage") or {}
        cost = float(result.get("total_cost_usd") or 0)
        with LOCK:
            STATE["usd"] += cost
        entry = {
            "at": started, "seconds": round(time.time() - started, 2), "usd": cost,
            "input": usage.get("input_tokens", 0) + usage.get("cache_read_input_tokens", 0)
            + usage.get("cache_creation_input_tokens", 0),
            "output": usage.get("output_tokens", 0), "prompt_chars": len(user), "schema": bool(schema),
        }
        with open(ARGS.log, "a") as log:
            log.write(json.dumps(entry) + "\n")
        self.send(200, {
            "id": "chatcmpl-claude-shim", "object": "chat.completion", "created": int(started),
            "model": ARGS.model,
            "choices": [{"index": 0, "finish_reason": "stop",
                         "message": {"role": "assistant", "content": result.get("result", "")}}],
            "usage": {"prompt_tokens": entry["input"], "completion_tokens": entry["output"],
                      "total_tokens": entry["input"] + entry["output"]},
        })


def main():
    global ARGS
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--key-file", required=True, help="file holding the bearer key the caller must send")
    parser.add_argument("--log", required=True)
    parser.add_argument("--model", default="haiku")
    parser.add_argument("--max-calls", type=int, default=80)
    parser.add_argument("--max-usd", type=float, default=1.5)
    ARGS = parser.parse_args()
    ARGS.key = open(ARGS.key_file).read().strip()
    if len(ARGS.key) < 16:
        parser.error("the key file must hold a key of at least 16 characters")
    ThreadingHTTPServer(("127.0.0.1", ARGS.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
