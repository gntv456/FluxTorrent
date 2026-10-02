# -*- coding: utf-8 -*-
"""Full-site deep audit driver — FluxTorrent generic site builder (ZT81).

Talks to the API on 127.0.0.1:8080 (container). Reads docker/.env for JWT_SECRET
and DB creds. All assertions appended to a rolling report dict; failures never
abort the run unless fatal (DB unreachable).
"""
import json
import os
import subprocess
import sys
import urllib.request
import urllib.error

BASE = os.environ.get("ZT_BASE", "http://127.0.0.1:8080")
OUT = {"env": {}, "cases": []}


def env_file():
    d = {}
    with open(os.path.join(os.path.dirname(__file__), "..", "..", "docker", ".env"), encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if "=" in line and not line.startswith("#"):
                k, v = line.split("=", 1)
                d[k.strip()] = v.strip()
    return d


ENV = env_file()


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True,
    )
    if r.returncode != 0:
        raise RuntimeError("psql failed: " + r.stderr)
    return r.stdout.strip()


def req(method, path, body=None, token=None, headers=None, raw=False):
    url = BASE + path
    data = None
    h = {"Content-Type": "application/json"}
    if headers:
        h.update(headers)
    if token:
        h["Authorization"] = "Bearer " + token
    if body is not None:
        data = json.dumps(body).encode("utf-8")
    r = urllib.request.Request(url, data=data, method=method, headers=h)
    try:
        with urllib.request.urlopen(r, timeout=30) as resp:
            payload = resp.read()
            code = resp.status
    except urllib.error.HTTPError as e:
        payload = e.read()
        code = e.code
    if raw:
        return code, payload
    try:
        j = json.loads(payload.decode("utf-8", "replace"))
    except Exception:
        j = {"_raw": payload[:300].decode("utf-8", "replace")}
    return code, j


def case(name, ok, detail=""):
    OUT["cases"].append({"name": name, "ok": bool(ok), "detail": str(detail)[:500]})
    flag = "PASS" if ok else "FAIL"
    print(f"[{flag}] {name} :: {str(detail)[:220]}")
    return ok


def login(username, password):
    c, j = req("POST", "/api/v1/auth/login", {"username": username, "password": password})
    tok = None
    if isinstance(j, dict) and j.get("code") == 0 and isinstance(j.get("data"), dict):
        tok = j["data"].get("token") or j["data"].get("access_token")
    return c, j, tok
