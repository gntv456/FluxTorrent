# -*- coding: utf-8 -*-
"""资深 PT 深测公共层：登录/调用/断言输出"""
import os
import urllib.request, urllib.error, json, sys, time

# FLUX_API_BASE：打侧容器（宿主 :8080 常被别的进程占着，直连会拿到
# 别人的响应，判成败全错）。不设则沿用主栈。
BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")

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

def login(username="root", pw="password123", tries=8):
    """带退避的登录。

    dev 实例上 `/auth/login` 会限流：几套闸门连着跑（每套要登好几次）之后
    直接返回无 token。后果比失败本身严重——放在 finally 里的兜底复原也要登录，
    于是兜底一起失败，脏数据留在原地。所以退避收在这一层，各闸门不必各自实现。
    """
    last = None
    for i in range(tries):
        s, r = call("POST", "/auth/login", {"username": username, "password": pw})
        if s == 200 and r.get("code") == 0:
            return r["data"]["token"]
        last = (s, r)
        if s != 429:
            break
        time.sleep(min(2 + i * 3, 20))
    raise SystemExit(
        "login failed for %s: %s %s"
        % (username, last[0], last[1].get("message"))
    )

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
