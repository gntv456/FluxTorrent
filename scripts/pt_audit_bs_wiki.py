# -*- coding: utf-8 -*-
"""闸门 BS：wiki 后台 CRUD + 公开目录/正文 + 版本史（0359 那族，五条管理口 +
两条公开口在常驻闸门里零命中）。

站型前提：`site_settings.module_wiki` 出厂为关（modules.rs:109「新站缺省关，站长
显式打开」），公开两口被网关拦成 4101 ⇒ 本闸门把它**临时打开**（DB 层写，等 30s
模块缓存 TTL 过期后回读，收尾按库层原值复原并复读）。这一步不能省：上一版批四十七
就是因为没开 module_farm 把「线上 404」当成了缺陷。

三个产品洞（红证取自未修版 `api:aa1013v`）：
  - **P-205** `GET /wiki` 把 `Vec<(i64,String,String)>, 元组)` 直接序列化 ⇒ JSON 是
    「数组的数组」，而 web 侧 `app/(main)/wiki/[[...slug]]/page.tsx:19` 声明的是
    `WikiIndexItem={id,slug,title}` 并按字段取 ⇒ 目录页每条的 slug/title 都是
    undefined（点开是空链接）。与批十一「/friends 回元组而 domain-types 声明
    {username,list} ⇒ 好友页永远空」同型。
  - **P-203** `validate_wiki_body` 用 `title.len()`（字节）判长而文案写「1-100 字符」
    ⇒ 34 个汉字的 wiki 页名 `http=400`（批五十三 P-200 的第二遍实现）。
  - **P-204** `wiki_delete` 的审计是 `audit(actor, "wiki_delete", None)`——既没 ref
    也没 slug，而 `wiki_revisions` 随 ON DELETE CASCADE 一起没了 ⇒「谁删了哪一页、
    删之前是什么」在库里彻底无处可查。

同族一次扫：`.len()` 判长的用户写字段共 5 处（wiki/custom_pages 标题、菜单名与链接、
内嵌规则中文名、促销标题、促销种类标签），本批一并改成 `chars().count()`，并用
W6a 的静态判据钉住「这些文件里不许再出现按字节判长的文本校验」、用 W6b 真打一处
（菜单名 17 个汉字 = 51 字节）验行为。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bs_wiki.py
"""
import io
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
SLUG = "bsw%d" % TAG
CN34 = "帮" * 34
CN17 = "帮" * 17          # 17 字符 = 51 字节（菜单名的字节判长靶子）
PID = [None]
MIDS = []
NAV0 = psql_val = None


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:50], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


def wiki_index():
    return (call("GET", "/wiki", None, token=ROOT)[1] or {}).get("data")


def cleanup():
    for m in MIDS:
        call("DELETE", "/admin/menu-items/%d" % m, None, token=ROOT)
    if PID[0]:
        call("POST", "/admin/wiki/%d/delete" % PID[0], {}, token=ROOT)
    psql("DELETE FROM wiki_revisions WHERE page_id NOT IN "
         "(SELECT id FROM wiki_pages)")
    if NAV0 is not None:
        if NAV0 == "<缺行>":
            psql("DELETE FROM site_settings WHERE name = 'module_wiki'")
        else:
            psql("UPDATE site_settings SET value = '%s' "
                 "WHERE name = 'module_wiki'" % NAV0)


import atexit  # noqa: E402

atexit.register(cleanup)

WP0 = int(psql("SELECT count(*) FROM wiki_pages") or -1)
T0 = psql("SELECT to_char(now(), 'YYYY-MM-DD HH24:MI:SS.US')")
NAV0 = psql("SELECT coalesce(value, '<缺行>') FROM site_settings "
            "WHERE name = 'module_wiki'")
psql("INSERT INTO site_settings (name, value) VALUES ('module_wiki', 'yes') "
     "ON CONFLICT (name) DO UPDATE SET value = 'yes'")
ok("W0a 基线：wiki_pages=%d 行、module_wiki 原值=%s（已临时开闸，等 TTL）"
   % (WP0, NAV0), WP0 >= 0, "WP=%s" % WP0)

# 模块缓存 TTL 30s：轮询到 /wiki 不再 4101 为止（前提不成立就别往下判）
opened, code0 = False, None
for _ in range(9):
    time.sleep(8)
    st, body = call("GET", "/wiki", None, token=ROOT)
    code0 = body.get("code") if isinstance(body, dict) else "非 JSON"
    if code0 != 4101:
        opened = True
        break
ok("W0b 前提成立：module_wiki 打开后 /wiki 不再被网关拦（4101 消失；"
   "http=%s 信封 code=%s）" % (st, code0), opened, "code=%s" % code0)

# ===== W1 标题判长（字节 vs 字符） =====
s, b = call("POST", "/admin/wiki", {"slug": SLUG, "title": CN34,
                                    "body": "[b]bs[/b]", "visible": True,
                                    "sort": 5}, token=ROOT)
pid = ((b or {}).get("data") or {}).get("id")
ok("W1a 34 个汉字的 wiki 页名（102 字节 / 34 字符）必须建得起来"
   "（未修版读数：http=%s msg=%s）"
   % (s, str((b or {}).get("message", ""))[:34]),
   s == 200 and pid, "http=%s id=%s" % (s, pid))
if not pid:
    s, b = call("POST", "/admin/wiki", {"slug": SLUG, "title": "BS probe",
                                        "body": "[b]bs[/b]", "visible": True,
                                        "sort": 5}, token=ROOT)
    pid = ((b or {}).get("data") or {}).get("id")
PID[0] = pid
OLD_TITLE = CN34 if s == 200 else "BS probe"
if not PID[0]:
    ok("W1z 探针页造不出来，本轮判据不可信", False, "POST=%s" % s)
    summary()
    sys.exit(1)
s2, b2 = call("POST", "/admin/wiki", {"slug": "bsa%d" % TAG,
                                      "title": "a" * 100, "body": "x",
                                      "visible": True, "sort": 6}, token=ROOT)
s3, _ = call("POST", "/admin/wiki", {"slug": "bsb%d" % TAG, "title": "a" * 101,
                                     "body": "x", "visible": True,
                                     "sort": 6}, token=ROOT)
extra = ((b2 or {}).get("data") or {}).get("id")
if extra:
    call("POST", "/admin/wiki/%d/delete" % extra, {}, token=ROOT)
ok("W1b 正控：100 字符标题放行、101 字符仍拒（http=%s / %s，不许把修复写成"
   "取消限长）" % (s2, s3), s2 == 200 and s3 == 400,
   "100=%s 101=%s" % (s2, s3))

# ===== W2 公开目录与正文的形状（P-205 的主判据） =====
idx = wiki_index() or []
bad = [x for x in idx if not isinstance(x, dict)
       or not all(k in x for k in ("id", "slug", "title"))]
mine = [x for x in idx if isinstance(x, dict) and x.get("slug") == SLUG]
ok("W2a `GET /wiki` 每条必须是 {id,slug,title} 对象——web 侧按字段取值，元组形状"
   "会让目录页每条都读成 undefined（本轮 %d 条，形状不合格的 %d 条；未修版读数="
   "[[id,slug,title],…]" % (len(idx), len(bad)),
   not bad and len(idx) > 0, "样例=%s" % str(idx[:1])[:90])
ok("W2b 探针页能从目录里读到（slug 命中=%d 条）" % len(mine),
   bool(mine), "命中=%d" % len(mine))
s4, bd = call("GET", "/wiki/%s" % SLUG, None, token=ROOT)
d4 = (bd or {}).get("data") or {}
ok("W2c 正文口回对象且带 body/updated_at（BBCode 原文，消毒由前端渲染层负责）",
   s4 == 200 and d4.get("slug") == SLUG and "body" in d4,
   "http=%s keys=%s" % (s4, sorted(d4.keys())))

# ===== W3 版本史：PUT 同事务先存档 =====
call("PUT", "/admin/wiki/%d" % PID[0], {"slug": SLUG, "title": "BS 改名一次",
                                        "body": "[i]v2[/i]", "visible": True,
                                        "sort": 5}, token=ROOT)
rev = ((call("GET", "/admin/wiki/%d/revisions" % PID[0], None,
             token=ROOT)[1] or {}).get("data") or [])
ok("W3a 改一次必须留一版，且存的是**旧**内容（版本史里那条标题要等于改名前的页名）",
   len(rev) >= 1 and any(str(r.get("title")) == OLD_TITLE for r in rev),
   "版本数=%d 标题=%s" % (len(rev), [r.get("title") for r in rev][:3]))

# ===== W4 可见性判据一致 =====
call("PUT", "/admin/wiki/%d" % PID[0], {"slug": SLUG, "title": "BS 改名一次",
                                        "body": "[i]v2[/i]", "visible": False,
                                        "sort": 5}, token=ROOT)
hid_in = any(isinstance(x, dict) and x.get("slug") == SLUG
             for x in (wiki_index() or []))
s5, _ = call("GET", "/wiki/%s" % SLUG, None, token=ROOT)
ok("W4 隐藏页：目录里不出现（=%s）且正文口 404（http=%s）——两口判据必须一致"
   % (hid_in, s5), (not hid_in) and s5 == 404,
   "目录含=%s 正文 http=%s" % (hid_in, s5))
call("PUT", "/admin/wiki/%d" % PID[0], {"slug": SLUG, "title": "BS 改名一次",
                                        "body": "[i]v2[/i]", "visible": True,
                                        "sort": 5}, token=ROOT)

# ===== W5 审计留痕（P-204） =====
call("POST", "/admin/wiki/%d/delete" % PID[0], {}, token=ROOT)
PID[0] = None
rows = ((call("GET", "/admin/audit?q=wiki_&limit=50", None,
              token=ROOT)[1] or {}).get("data") or [])
mine5 = [r for r in rows if str(r.get("created_at") or "").replace(
    "T", " ")[:19] >= T0[:19]]
noref = [r.get("action") for r in mine5
         if not (r.get("detail") or "").strip()
         or r.get("detail") in ("null", "{}")]
have_slug = all("slug" in str(r.get("detail"))
             for r in mine5) if mine5 else False
ok("W5a 本轮新写的 wiki 审计行都要带 ref 与 slug（delete 那条尤其——版本史随"
   "CASCADE 一起没了，不留就成了无头账）：本轮 %d 条，空 ref %d 条"
   % (len(mine5), len(noref)), bool(mine5) and not noref and have_slug,
   "空 ref=%s 带 slug=%s" % (sorted(set(noref)), have_slug))

# ===== W6 同族：字节判长的其它站点 =====
FAM = ["apps/api/src/staff_http/wiki.rs",
       "apps/api/src/staff_http/custom_pages.rs",
       "apps/api/src/admin_p2_http/menus_crud.rs",
       "apps/api/src/admin_http/embed_rule_check.rs",
       "apps/api/src/admin_p2_http/promos.rs",
       "apps/api/src/admin_p3_http/promo_kinds.rs"]
ROOT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
hits = []
for f in FAM:
    t = io.open(os.path.join(ROOT_DIR, f), encoding="utf-8").read()
    for expr in ("title.len()", "label.len()", "name_zh.len()",
                 "url.len() >"):
        if expr in t:
            hits.append("%s:%s" % (os.path.basename(f), expr))
ok("W6a 静态守卫：文本字段的校验一律按字符判长（命中=%s）" % hits, not hits,
   "命中=%d 处" % len(hits))
s6, b6 = call("POST", "/admin/menu-items", {"location": "topbar",
              "label": CN17, "url": "/torrents"}, token=ROOT)
mid = ((b6 or {}).get("data") or {}).get("id")
if mid:
    MIDS.append(mid)
ok("W6b 行为验证同族一处：17 个汉字（51 字节 > 50）的菜单名应能建"
   "（未修版读数：http=%s msg=%s）"
   % (s6, str((b6 or {}).get("message", ""))[:30]),
   s6 == 200, "http=%s id=%s" % (s6, mid))

# ===== W9 收尾 =====
cleanup()
WP1 = int(psql("SELECT count(*) FROM wiki_pages") or -1)
NAV1 = psql("SELECT coalesce(value, '<缺行>') FROM site_settings "
            "WHERE name = 'module_wiki'")
ok("W9a 探针页已清：wiki_pages %d→%d，slug=%s 不在库里" % (WP0, WP1, SLUG),
   WP1 == WP0 and psql("SELECT count(*) FROM wiki_pages WHERE slug LIKE "
                       "'bsw%%'") == "0", "WP=%s" % WP1)
ok("W9b module_wiki 已按库层原值复原（%s→%s）" % (NAV0, NAV1), NAV1 == NAV0,
   "现值=%s" % NAV1)
ok("W9c 探针菜单条目已清", psql("SELECT count(*) FROM menu_items WHERE "
                               "label = '%s'" % CN17) == "0",
   "残留=%s" % psql("SELECT count(*) FROM menu_items WHERE label = '%s'"
                  % CN17))
summary()
