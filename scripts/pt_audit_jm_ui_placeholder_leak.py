# -*- coding: utf-8 -*-
"""常驻闸门 JM：现网界面上不许露出 `{占位符}`（P-558 同族的部署层护栏）。

为什么要有这条：字典里 508 条文案带 `{n}/{magic}/{slug}` 这类占位，
`fmt()` 的实现是「没喂值就折成空串」（apps/web/i18n/config.ts），
而**根本没走 fmt 的裸渲染**会把花括号原样摊在界面上。普查与渲染层用例量的
都是我这一边的代码；这条闸门量的是**用户实际打开的那个镜像**。

判据口径（三条防呆，缺一就把覆盖当成假绿）：
  * 只扫**可见标记**：先整段剥掉 `<script>`（Next 把 RSC flight payload 写在
    里面，整本字典都在那儿，`{magic}` 必然出现）与 `<style>`，再剥标签；
  * 逐页设可信下限：SSR 可见正文 < `MIN_VISIBLE` 判「这页没渲染出内容」
    ⇒ 记 unreliable 并**照样列出来**，不许读成「干净」。客户端渲染屏
    （/requests 表单、/admin 面板、私信/银行…）天然落在这一档，
    那部分由 apps/web 的 vitest 渲染层用例钉（见 P-558 那三支护栏用例）；
  * 登录只打一次：连续失败第 3 次起要 PoW，拿不到 token 就直接 raise，
    绝不retry 错密码把 root 卡进 PoW/限流。

正控：往抓到的 HTML 里塞一枚假 `{magic}`（只在内存），判据必须报它。

用法：python scripts/pt_audit_jm_ui_placeholder_leak.py
       FLUX_WEB / FLUX_API 可用环境变量改上游。
"""
import html
import json
import os
import re
import sys
import urllib.request as up

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary  # noqa: E402

WEB = os.environ.get("FLUX_WEB", "http://localhost:3000")
API = os.environ.get("FLUX_API", "http://localhost:8080/api/v1")
ROOT_USER = os.environ.get("FLUX_AUDIT_USER", "root")
ROOT_PW = os.environ.get("FLUX_AUDIT_PW", "password123")
MIN_VISIBLE = 1200
# 以「有内容可看」为准：这几页在现网是 SSR 出正文的（首页/列表/论坛/商城…）
PAGES = ["/", "/torrents", "/forums", "/shop", "/top", "/rules", "/games",
         "/users/1", "/torrents?search=%E6%B7%B1%E6%B5%8B"]


def login_token():
    req = up.Request(API + "/auth/login",
                     data=json.dumps({"username": ROOT_USER,
                                      "password": ROOT_PW}).encode(),
                     headers={"content-type": "application/json"})
    with up.urlopen(req, timeout=30) as r:
        sc = [v for k, v in r.getheaders() if k.lower() == "set-cookie"]
        body = json.loads(r.read().decode("utf-8", "replace"))
    for line in sc:
        if line.startswith("flux_token="):
            return body.get("code"), line.split(";", 1)[0].split("=", 1)[1]
    return body.get("code"), ""


def visible(raw):
    s = re.sub(r"<script[^>]*>.*?</script>", " ", raw, flags=re.S | re.I)
    s = re.sub(r"<style[^>]*>.*?</style>", " ", s, flags=re.S | re.I)
    s = re.sub(r"<[^>]+>", " ", s)
    return re.sub(r"\s+", " ", html.unescape(s)).strip()


def fetch(url, token):
    req = up.Request(url, headers={"user-agent": "pt-audit-jm",
                                  "cookie": "flux_token=%s; flux.session=1"
                                  % token})
    with up.urlopen(req, timeout=45) as r:
        return r.status, r.read().decode("utf-8", "replace")


def leaks(txt):
    return sorted(set(re.findall(r"\{[A-Za-z_][A-Za-z0-9_]*\}", txt)))


def main():
    code, tok = login_token()
    ok("JM0 登录拿到 flux_token（code=%s）" % code,
       code == 0 and bool(tok),
       "拿不到会话 ⇒ 每一页都会读成登录页，后面的绿全不算数")
    if not tok:
        summary()
        return

    trusted, unreliable, bad = [], [], []
    for p in PAGES:
        try:
            st, raw = fetch(WEB + p, tok)
        except Exception as e:            # 抓不到就是抓不到，不猜「没问题」
            bad.append((p, str(e)[:80]))
            continue
        txt = visible(raw)
        if st != 200 or len(txt) < MIN_VISIBLE or "用户登录" in txt[:60]:
            unreliable.append((p, st, len(txt)))
            continue
        trusted.append(p)
        hit = leaks(txt)
        ok("JM1 %s 可见正文里不露占位符（len=%d）" % (p, len(txt)),
           not hit, "泄漏=%s 片段=%s" % (hit[:5], txt[:0]))
    ok("JM2 可信页 >=5（其余是客户端渲染屏，交给 vitest 渲染层用例）",
       len(trusted) >= 5, "可信=%s" % trusted)
    print("   [未覆盖] SSR 读不出正文的页（客户端渲染/骨架，共 %d）：%s"
          % (len(unreliable), ", ".join("%s(%s,%d)" % u for u in unreliable)))
    if bad:
        print("   [抓失败] %s" % ", ".join("%s %s" % b for b in bad))
    ok("JM3 没有抓失败的页（网络/上游异常要出声）", not bad, str(bad)[:200])

    # 正控：内存里把首页 HTML 掺一枚 {magic}，判据必须报出来
    try:
        _, raw = fetch(WEB + "/", tok)
        dirty = raw.replace("</body>", "<p>{magic}</p></body>", 1)
        got = leaks(visible(dirty))
        ok("JM4 正控：掺一枚 {magic} 进首页 HTML，判据必须抓到",
           got == ["{magic}"], "实际=%s" % got)
    except Exception as e:
        ok("JM4 正控：掺一枚 {magic} 进首页 HTML，判据必须抓到", False,
           "抓取失败：%s" % str(e)[:80])

    summary()


if __name__ == "__main__":
    main()
