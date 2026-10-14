# -*- coding: utf-8 -*-
"""CY 闸门（批八十一 P-288）：三个页面不许把故障演成空板面。

红证（10-14，同一份源码起两个 dev 实例，`API_SERVER_URL` 一个指活口、
一个指死端口 127.0.0.1:9，带合法会话各打一遍）：

    死上游修前：/friends    → 「暂无好友」+「黑名单为空」+「暂无候选」，
                              加友/拉黑表单照常可点（一次演三个假空）
              /textbooks   → 只剩页面外壳，列表区既无内容也无提示
              /magic-pool  → 站免池面板显示「进度 0」——经济面板把
                            「读不到」报成「一分没募」
    死上游修后：三页均为 ErrorPanel（「加载失败 fetch failed」+ 重试）
    活上游：三页照常出内容、无面板 ⇒ 没有反向误伤

    /magic-pool  → 站免池显示「进度 0」——经济面板把「读不到」
页面按「确实没有」渲染。本批解除四个帮手的吞错（getFriends/getOffers/
getTextbooks/getPool）并让三页接上错误面板。

判据两层：源码形状（锁死「不许再把 catch 换回空值」）+ 现网健康态
（防反向误伤）。request_id 只在后端回了信封时才有；网络层失败

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_cy_swallow_to_empty.py
"""
import io  # noqa: E402
import os  # noqa: E402
import re  # noqa: E402
import subprocess  # noqa: E402
import sys  # noqa: E402
import urllib.error  # noqa: E402
import urllib.request  # noqa: E402

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import login, ok, summary  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WEB = os.environ.get("FLUX_WEB_BASE", "http://127.0.0.1:3000")
ROOT = login()

HELPERS = ("getFriends", "getOffers", "getTextbooks", "getPool")
PAGES = ("friends", "textbooks", "magic-pool")


def src(rel):
    return io.open(os.path.join(REPO, rel), encoding="utf-8",
                   errors="replace").read().replace("\r\n", "\n")


def strip(s):
    return "\n".join(l.split("//")[0] for l in s.splitlines())


def get(p):
    req = urllib.request.Request(WEB + p)
    req.add_header("Cookie", "flux_token=%s; flux.session=1" % ROOT)
    try:
        r = urllib.request.urlopen(req, timeout=90)
        return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")


dc = strip(src(os.path.join("apps", "web", "lib", "data-content.ts")))
print("== CY1 四个帮手不得再把异常吞成空值 ==")
for name in HELPERS:
    i = dc.find("export async function %s(" % name)
    seg = dc[i:dc.find("\n}", i) + 2] if i >= 0 else ""
    ok("CY1 %s 体内没有 catch→[]/null（段长 %d）" % (name, len(seg)),
       bool(seg) and "catch" not in seg,
       seg[:120].replace("\n", " "))

print("== CY2 三个页面都接上了错误面板（且 normalizeError 取自 lib）==")
for slug in PAGES:
    rel = os.path.join("apps", "web", "app", "(main)", slug, "page.tsx")
    s = src(rel.replace(os.sep, "/"))
    b = strip(s)
    ok("CY2 /%s：try/catch + ErrorPanel + normalizeError 从 lib/render-error 取"
       % slug,
       "catch (e)" in b and "ErrorPanel" in b
       and 'from "@/lib/render-error"' in b
       and "import { ErrorPanel, normalizeError }" not in s,
       "catch=%s panel=%s lib=%s" % ("catch (e)" in b, "ErrorPanel" in b,
                                     'from "@/lib/render-error"' in b))

print("== CY3 现网健康态：三页都不得出现错误面板（防反向误伤）==")
for slug in PAGES:
    st, h = get("/" + slug)
    ok("CY3 /%s 健康时没有错误面板 http=%s" % (slug, st),
       st == 200 and "ui-err__msg" not in h,
       "http=%s ui-err=%d" % (st, h.count("ui-err")))

print("== CY4 全仓守卫：服务端页面不许从 client 模块取 normalizeError ==")
r = subprocess.run(["git", "grep", "-n", "--",
                    'from "@/components/ui/error-panel"', "apps/web"],
                   cwd=REPO, stdout=subprocess.PIPE,
                   universal_newlines=True, encoding="utf-8")
bad = [x for x in (r.stdout or "").splitlines() if "normalizeError" in x]
ok("CY4 零命中（批七十七的教训固化成常驻判据）", not bad, str(bad[:2]))

print("== CY5 同族余量记账（还剩几处 catch→空值，逐个判后才收）==")
n = len(re.findall(r"catch \{\s*\n\s*return (?:\[\]|null);", dc))
print("   WARN `lib/data-content.ts` 仍有 %d 处 catch→空值（其消费页面是否"
      "该报错要逐个判；本批收掉的是有活页面消费的四个）" % n)
ok("CY5 余量只能减不能增（本批前 12 → 现 %d）" % n, n <= 8)

sys.exit(summary())
