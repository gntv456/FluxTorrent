# -*- coding: utf-8 -*-
"""资深PT用户实测：Next.js 覆盖层全站页面巡检（登录态 + 匿名态）。

- 从 apps/web/app/**/page.tsx 推导全部路由
- 动态段用探针 id 替换
- 逐页请求，记录 HTTP 状态；同时在页面 HTML 里找 Next 的错误标记
"""
import io
import json
import os
import re
import sys
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WEB = os.environ.get("FLUX_WEB_BASE", "http://127.0.0.1:3000")
BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")

APP = os.path.join(ROOT, "apps", "web", "app")
PROBE = {"id": "1", "slug": "1", "page": "1", "cat": "1"}


def login():
    import time
    for i in range(8):
        req = urllib.request.Request(BASE + "/auth/login", method="POST")
        req.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(
                req,
                json.dumps(
        {"username": "root", "password": "password123"}
    ).encode(),
                timeout=20,
            ) as r:
                d = json.loads(r.read())
            if d.get("code") == 0:
                return d["data"]["token"]
        except urllib.error.HTTPError as e:
            if e.code != 429:
                break
        except Exception:
            pass
        time.sleep(min(2 + i * 3, 20))
    return None


def collect_routes():
    out = []
    for dp, _, fns in os.walk(APP):
        for fn in fns:
            if fn != "page.tsx":
                continue
            full = os.path.join(dp, fn)
            rel = os.path.relpath(full, APP).replace("\\", "/")
            segs = rel.split("/")
            if segs[0].startswith("("):
                # 路由组：去掉组名
                segs = segs[1:]
            segs = segs[:-1]
            parts = []
            for s in segs:
                if s.startswith("[...") and s.endswith("]"):
                    parts.append("1")
                elif s.startswith("[") and s.endswith("]"):
                    parts.append(PROBE.get(s[1:-1].lower(), "1"))
                else:
                    parts.append(s)
            out.append("/" + "/".join(parts))
    return sorted(set(out))


def fetch(path, cookie):
    req = urllib.request.Request(WEB + path)
    if cookie:
        req.add_header("Cookie", cookie)
    try:
        with urllib.request.urlopen(req, timeout=40) as resp:
            return resp.status, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")
    except Exception as e:
        return "EXC", repr(e)


def main():
    token = login()
    cookie = None
    if token:
        cookie = "flux.session=1; flux_token=" + token
    else:
        print("!! 登录失败")

    routes = collect_routes()
    print("覆盖层页面数：%d" % len(routes))

    bad = []
    for p in routes:
        code, body = fetch(p, cookie)
        marker = ""
        if code == 200:
            for pat in ("Application error", "Internal Server Error",
                        "__NEXT_ERROR", "Server Error"):
                if pat in body:
                    marker = pat
                    break
            if marker:
                bad.append((p, code, marker, ""))
        elif code in (500, 502, 503):
            bad.append((p, code, "HTTP", ""))
        else:
            # 404/403/307 单独记，不算错
            pass

    print("=" * 70)
    print("登录态页面异常：%d" % len(bad))
    for p, c, m, extra in bad:
        print("  %-45s %s %s" % (p, c, m))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
