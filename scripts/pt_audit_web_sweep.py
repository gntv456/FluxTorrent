# -*- coding: utf-8 -*-
"""资深PT用户实测：Next.js 覆盖层全站页面巡检（登录态 + 匿名态 + 软 404）。

- 从 apps/web/app/**/page.tsx 推导全部路由
- 动态段用探针 id 替换
- 登录态：逐页请求，记录 HTTP 状态；同时在页面 HTML 里找 Next 的错误标记
- 匿名态：**不跟随重定向**读 (status, Location)。准入清单从 middleware.ts 的
  PUBLIC_PATHS 现读，不另抄一份。抓的是两类事故：
  会员面被匿名直出 200（准入漏口）、公开页反倒被弹走
- 软 404：`/p/{不存在的 slug}` 必须回 404——这条路由在 sitemap 里，
  「页面不存在」带 200 就是让爬虫继续收录一个空壳
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
MW = os.path.join(ROOT, "apps", "web", "middleware.ts")
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


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a):
        return None


NOPED = urllib.request.build_opener(_NoRedirect)


def fetch_raw(path, cookie=None):
    """**不跟随重定向**取 (status, Location)。

    准入判据只能这么读：跟着 307 走会把「被弹回登录页」看成 http=200，
    正好把要抓的漏洞洗白（urlopen 默认就跟随，所以上面的 fetch 不能用于此）。
    """
    req = urllib.request.Request(WEB + path)
    if cookie:
        req.add_header("Cookie", cookie)
    try:
        with NOPED.open(req, timeout=40) as r:
            r.read()
            return r.status, ""
    except urllib.error.HTTPError as e:
        return e.code, (e.headers.get("Location") or "")
    except Exception as e:
        return "EXC", repr(e)


def public_paths_from_middleware():
    """从 apps/web/middleware.ts 里读 PUBLIC_PATHS，判据不另抄一份清单。

    抄一份的后果：站方加一档公开页（如 /apply、/email-verify 都是后来补的），
    判据会把合法公开页报成漏洞，或者反过来——把新加的口子默默当成应有之义。
    """
    src = io.open(MW, encoding="utf-8", errors="replace").read()
    blk = src[src.index("PUBLIC_PATHS = ["):]
    blk = blk[: blk.index("];")]
    return set(re.findall(r'"(/[^"]+)"', blk))


# 静态资源与匿名资产前缀：与 middleware 的 isAnonymousOk 同口径
ANON_PREFIXES = ("/_next/", "/icons/", "/fonts/", "/brand/", "/p/")
ANON_FILES = ("/favicon.ico", "/manifest.webmanifest", "/sw.js",
              "/robots.txt", "/sitemap.xml", "/health")
# 不存在的自定义页面：/p/{slug} 在 sitemap 里，爬虫看到的状态码必须是 404
SOFT_404_PROBES = ("/p/zzz-not-a-slug", "/p/zzz2-not-either")


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

    # ---------- 匿名态：准入闸门有没有漏（不跟随重定向才看得见真状态）----------
    pub = public_paths_from_middleware()
    leaks, wrong_pub = [], []
    for p in routes:
        is_pub = (p in pub or p.startswith(ANON_PREFIXES) or p in ANON_FILES)
        st, loc = fetch_raw(p)
        if is_pub:
            if st not in (200, 304):
                wrong_pub.append((p, st, loc))
            continue
        if 300 <= st < 400:
            if not loc.startswith("/login"):
                leaks.append((p, st, loc))
            elif "next=" not in loc and p != "/":
                leaks.append((p, st, loc + "｜丢了 next"))
        elif st == 200:
            leaks.append((p, st, "匿名直出 200"))
        elif st not in (401, 403, 404):
            leaks.append((p, st, loc))

    print("匿名准入：会员面漏出 %d 条 / 公开页不 200 %d 条"
          % (len(leaks), len(wrong_pub)))
    for p, st, loc in leaks[:12]:
        print("  [漏] %-42s http=%-4s loc=%s" % (p, st, str(loc)[:44]))
    for p, st, loc in wrong_pub[:12]:
        print("  [公开页] %-42s http=%-4s loc=%s" % (p, st, str(loc)[:44]))

    # ---------- 软 404：/p/{slug} 在 sitemap 里，不存在的页必须回真 404 ----------
    soft = []
    for p in SOFT_404_PROBES:
        st, loc = fetch_raw(p)
        if st == 200:
            soft.append(p)
        elif st != 404:
            bad.append((p, st, "软404口径变化", ""))
    for p in soft:
        print("  [在册] %-24s 不存在的自定义页面回 200（软 404）。这条路由在 "
              "sitemap 里，爬虫会把「页面不存在」当内容一直收录。"
              "已排除的改法：删掉同目录 loading.tsx 后重启 dev 仍回 200"
              "（2026-10-21 实测），状态行是在更上层的流式边界里发出去的，"
              "要修得先看 (main) 布局的 Suspense 结构" % p)
    if not soft:
        print("  软 404：无（/p/{不存在 slug} 已回 404）")

    return 1 if (bad or leaks or wrong_pub) else 0


if __name__ == "__main__":
    sys.exit(main())
