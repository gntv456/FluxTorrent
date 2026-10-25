# -*- coding: utf-8 -*-
"""整站页面广度体检（会员身份，只读）：找 5xx、掉登录、错误屏、模块未开放。

路由清单从 apps/web/app/**/page.{tsx,ts} 现算（动态段用样例值填），
带真实会话 cookie 打现网 web。

为什么手工拼 Cookie 头：python 的 CookieJar 会把没有 `Domain` 的
`flux_token` 绑成 `localhost.local`，跨端口（api :8080 登录 → web :3000 取页）
根本不发出去——上一版因此把每一页都读成登录页，还打印了"全部干净"。
为什么收尾要断基数：遍历类脚本最怕"0 条目标也报通过"（同一天的 `find`
版就是 stdout 空 → 0 路由 → rc=0）。

两类判读分开，别混成一条"红/绿"：
  * 硬故障：5xx、掉登录、Next 错误屏文案（「出错了 / 服务异常 /
    Something went wrong」）、模块未开放（4101 的界面口径）；
  * 覆盖天花板：SSR 可见正文短≠坏——本仓大量屏是客户端渲染，
    骨架 + 页头就能只有两三百字，所以只列计数，不判红。

用法：python scripts/page_breadth_probe.py [--json 路径]
      FLUX_WEB / FLUX_API / FLUX_AUDIT_USER / FLUX_AUDIT_PW 可覆盖。
"""
import html
import json
import os
import re
import sys
import urllib.error
import urllib.request as up

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
WEB = os.environ.get("FLUX_WEB", "http://localhost:3000")
API = os.environ.get("FLUX_API", "http://localhost:8080/api/v1")
USER = os.environ.get("FLUX_AUDIT_USER", "root")
PW = os.environ.get("FLUX_AUDIT_PW", "password123")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DYN = {"[id]": "1", "[slug]": "rules", "[uid]": "1", "[tab]": "torrents",
       "[code]": "NOPE123", "[kind]": "uploads", "[board]": "1"}
ERR_MARK = [u"出错了", u"服务异常", u"服务器错误", u"Something went wrong",
            u"Application error", u"页面不存在"]
GATE_MARK = [u"未开放", u"模块已关闭", u"功能未启用"]
LOGIN_MARK = u"用户登录"


def routes():
    base = os.path.join(ROOT, "apps", "web", "app")
    out = []
    for dirpath, _, fns in os.walk(base):
        for fn in fns:
            if fn not in ("page.tsx", "page.ts"):
                continue
            rel = os.path.join(dirpath, fn).split("app" + os.sep, 1)[1]
            rel = rel.rsplit(os.sep + "page.", 1)[0]
            segs = [s for s in rel.split(os.sep) if not s.startswith("(")]
            p = "/" + "/".join(segs)
            for k, v in DYN.items():
                p = p.replace(k, v)
            p = re.sub(r"\[\[?\.[.A-Za-z0-9]*\]?\]", "rules", p)
            p = re.sub(r"\[[A-Za-z0-9_]+\]", "1", p)
            out.append(p.replace("//", "/") or "/")
    out = sorted(set(out))
    assert len(out) > 100, "只认出 %d 条路由，覆盖不足以下结论" % len(out)
    return out


def login():
    req = up.Request(API + "/auth/login",
                     data=json.dumps({"username": USER, "password": PW})
                     .encode(),
                     headers={"content-type": "application/json"})
    with up.urlopen(req, timeout=30) as r:
        sc = [v for k, v in r.getheaders() if k.lower() == "set-cookie"]
        code = json.loads(r.read().decode("utf-8", "replace")).get("code")
    for line in sc:
        if line.startswith("flux_token="):
            return code, line.split(";", 1)[0].split("=", 1)[1]
    return code, ""


def visible(raw):
    s = re.sub(r"<script[^>]*>.*?</script>", " ", raw, flags=re.S | re.I)
    s = re.sub(r"<style[^>]*>.*?</style>", " ", s, flags=re.S | re.I)
    s = re.sub(r"<[^>]+>", " ", s)
    return re.sub(r"\s+", " ", html.unescape(s)).strip()


def main():
    as_json = None
    if "--json" in sys.argv:
        as_json = sys.argv[sys.argv.index("--json") + 1]
    code, tok = login()
    print("登录 code=%s token=%s…" % (code, tok[:12]))
    if not tok:
        raise SystemExit("拿不到会话：每页都会读成登录页，判据会假绿")
    paths = routes()
    rows = []
    for p in paths:
        req = up.Request(WEB + p, headers={"user-agent": "page-breadth",
                                          "cookie": "flux_token=%s; "
                                          "flux.session=1" % tok})
        try:
            with up.urlopen(req, timeout=45) as r:
                st, raw, final = r.status, r.read().decode(
                    "utf-8", "replace"), str(r.geturl())
        except urllib.error.HTTPError as e:
            st, raw = e.code, (e.read().decode("utf-8", "replace")
                               if e.fp else "")
            final = str(e.url)
        except Exception as e:
            rows.append({"path": p, "http": 0, "why": "抓取异常 %s"
                         % str(e)[:60]})
            continue
        txt = visible(raw)
        final_path = final.replace(WEB, "") or "/"
        marks = [m for m in ERR_MARK if m in txt]
        gates = [m for m in GATE_MARK if m in txt]
        why = []
        if st >= 500:
            why.append("5xx")
        if final_path.startswith("/login"):
            why.append("掉登录")
        if marks:
            why.append("错误屏:" + ",".join(marks))
        if gates:
            why.append("模块门:" + ",".join(gates))
        rows.append({"path": p, "http": st, "len": len(txt),
                     "final": final_path, "why": "; ".join(why)})
    hard = [r for r in rows if r.get("why")]
    print("路由 %d 条；硬故障 %d 条" % (len(rows), len(hard)))
    for r in hard:
        print("   %-40s http=%-4s %s" % (r["path"], r["http"], r["why"]))
    short = [r for r in rows if not r.get("why") and r.get("len", 0) < 1200]
    print("（覆盖天花板）SSR 正文 <1200 字的正常页 %d 条——本仓多为客户端"
          "渲染屏，不判红" % len(short))
    if as_json:
        with open(as_json, "w", encoding="utf-8") as f:
            json.dump(rows, f, ensure_ascii=False, indent=1)
    if hard:
        sys.exit(1)


if __name__ == "__main__":
    main()
