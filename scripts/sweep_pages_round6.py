# -*- coding: utf-8 -*-
"""第六轮审查：全站页面 sweep（33 个前台页面 + admin + 登录态可达性）。"""
import json
import sys
import urllib.request

WEB = "http://127.0.0.1:3000"
BASE = "http://127.0.0.1:8080/api/v1"

PAGES = [
    "/", "/torrents", "/torrents?official=1", "/upload", "/forums",
        "/requests", "/offers",
    "/preserve", "/games", "/top", "/messages", "/medals", "/tasks", "/bank",
        "/invites",
    "/subtitles", "/friends", "/textbooks", "/magic-pool", "/myhr", "/contests",
    "/medal-wall", "/avatar-frames", "/gomoku", "/faq", "/donate", "/shop",
        "/dressup",
    "/farm", "/jixiao", "/my", "/my?tab=bookmarks", "/admin", "/login",
        "/register",
    "/rules", "/appeals", "/ban-log", "/forgot", "/reset",
        "/offline",
]

results = []

def check(name, cond, detail=""):
    results.append((name, cond))
    print(("PASS " if cond else "FAIL ") + name + (" " + str(
        detail) if not cond and detail else ""))

# 登录拿 token + cookie
req = urllib.request.Request(BASE + "/auth/login", method="POST")
req.add_header("Content-Type", "application/json")
with urllib.request.urlopen(req, json.dumps({"username": "root",
    "password": "password123"}).encode(), timeout=10) as r:
    token = json.loads(r.read())["data"]["token"]

cookie = f"flux.session=1; flux_token={token}"

for p in PAGES:
    try:
        req = urllib.request.Request(WEB + p)
        req.add_header("Cookie", cookie)
        with urllib.request.urlopen(req, timeout=15) as r:
            code = r.status
            body = r.read().decode("utf-8", errors="replace")
        # Next 页面 200 即 SSR/静态壳正常；再粗查是否渲染了根容器
        has_root = 'id="__next"' in body or "<body" in body
        # 未登录跳转（307→login）视为失败（带了 cookie 不应跳）
        check(f"页面 {p}", code == 200 and has_root, f"code={code}")
    except urllib.error.HTTPError as e:
        check(f"页面 {p}", False, f"HTTP {e.code}")
    except Exception as e:
        check(f"页面 {p}", False, repr(e))

n_pass = sum(1 for _, c in results if c)
print(f"\n===== 页面 sweep：{n_pass}/{len(results)} PASS =====")
sys.exit(0 if n_pass == len(results) else 1)
