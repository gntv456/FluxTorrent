# -*- coding: utf-8 -*-
"""第七轮：限流有效性复核（throttle 是 INCR 先行，应返回 400 + code=1015）。"""
import json
import time
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080/api/v1"

def call(body):
    req = urllib.request.Request(BASE + "/auth/login", method="POST", data=json.dumps(body).encode())
    req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}

codes = []
for i in range(8):
    st, r = call({"username": "ratelimit_probe_u", "password": "x"})
    codes.append((st, r.get("code")))
    if r.get("code") == 1015:
        print(f"PASS 限流·第{i+1}次触发 code=1015 (HTTP {st})")
        break
else:
    print(f"FAIL 限流未触发: {codes}")
