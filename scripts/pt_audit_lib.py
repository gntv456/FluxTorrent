# -*- coding: utf-8 -*-
"""资深 PT 深测公共层：登录/调用/断言输出"""
import urllib.request, urllib.error, json, sys

BASE = "http://127.0.0.1:8080/api/v1"

def call(method, path, body=None, token=None, raw=False):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        r = urllib.request.urlopen(req, data, timeout=30)
        payload = r.read()
        if raw:
            return r.status, payload
        return r.status, json.loads(payload)
    except urllib.error.HTTPError as e:
        payload = e.read()
        if raw:
            return e.code, payload
        try:
            return e.code, json.loads(payload)
        except Exception:
            return e.code, {"raw": payload[:300].decode("utf-8", "replace")}

def login(username="root", pw="password123"):
    s, r = call("POST", "/auth/login", {"username": username, "password": pw})
    if s != 200 or r.get("code") != 0:
        raise SystemExit("login failed for %s: %s %s" % (username, s, r.get("message")))
    return r["data"]["token"]

def ok(name, cond, detail=""):
    mark = "PASS" if cond else "FAIL"
    print("[%s] %s %s" % (mark, name, ("- " + str(detail)[:220]) if (detail and not cond) else ""))
    if not cond:
        FAILURES.append(name)
    return cond

FAILURES = []

def summary():
    print()
    if FAILURES:
        print("== FAILURES (%d): %s" % (len(FAILURES), ", ".join(FAILURES)))
        sys.exit(1)
    print("== ALL PASS")
