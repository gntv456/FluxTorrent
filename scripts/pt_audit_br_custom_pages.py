# -*- coding: utf-8 -*-
"""闸门 BR：自定义页（/admin/custom-pages 4 条 + /public-pages + /pages/{slug}
+ /help-docs）与前台菜单入口这一条「站长建页 → 成员点开」的链。

选题依据：这一族在常驻闸门里零命中（`grep -rl custom-pages scripts/pt_audit_*.py`
= 0），而它正是本项目「通用建站」口径的核心面：`custom_pages` 有 7 行在册、
`menu_items` 是前台导航的来源，`app/(main)/p/[slug]/page.tsx` 与 `sitemap.ts`
都真在消费 ⇒ 面板与前台都活着，只是没人验过。

读码点出的四个怀疑，本闸门逐条打（红/绿以实测为准）：
  1. `validate_body` 用 `title.len() > 100`（**字节**）判长，而文案写
     「标题需 1-100 字符」⇒ 34 个汉字（102 字节）的中文页名被自拒；
     `norm_doc_group` 同样按字节（>60），21 个汉字的分组名也被拒。
     与 P-23（`name.len()>32`）、P-196（pattern 按字节）同族。
  2. `admin_p2_http/menus_public.rs:48` 那段 NOT EXISTS 只挡「页面挂了模块且模块
     关闭」一种死链，**没挡 `visible=false` 与页面已删除**两种——而该文件自己的
     注释说的是「免得给用户一个点进去会 404 的地址」。
  3. 三条审计 `custom_page_add/update/delete` 都传 `ref=None` ⇒「谁删了哪个页面」
     在审计里读不出对象（`/admin/audit` 的 detail 列是空的）。
  4. `/pages/{slug}` 的 404 用 `DomainError::NotFound(0)` ⇒ 信封文案里带着一个
     不存在的资源号 0。

判据纪律：每条「应拒」都配一条同形正控（ASCII 100/101、大写 slug），免得把修复
写成取消限流；菜单死链那组先自证「页面可见时条目确实在菜单里」，否则「不含」
可能是前提没成立。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_br_custom_pages.py
"""
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
SLUG = "brp%d" % TAG
URL = "/p/%s" % SLUG
CN34 = "帮" * 34          # 34 字符 = 102 字节
CN21 = "帮" * 21          # 21 字符 = 63 字节
A100 = "a" * 100
A101 = "a" * 101


def psql(q):
    import subprocess
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:50], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


def page(title=None, body="<p>br body</p>", visible=True, group=None,
         module=None, slug=SLUG, sort=10, doc_sort=5):
    """除 title 外都按调用处给：title 默认 ASCII，好让「分组名/消毒/菜单」那几组
    判据各自只动一个变量（上一版默认中文标题，V2a/V3a 全被标题检查带崩成级联红）。
    CN34 只在 V1a 显式用。"""
    return {"slug": slug, "title": title or ("BR probe %d" % TAG),
            "body": body, "visible": visible, "sort": sort,
            "module_key": module, "doc_group": group, "doc_sort": doc_sort}


def pub_pages():
    d = (call("GET", "/public-pages", None, token=ROOT)[1] or {}).get("data")
    return [x.get("slug") for x in (d or [])]


def help_docs():
    d = (call("GET", "/help-docs", None, token=ROOT)[1] or {}).get("data")
    return [x.get("slug") for x in (d or [])]


def menu_rows(loc):
    d = (call("GET", "/menu-items?location=%s" % loc, None,
              token=ROOT)[1] or {}).get("data")
    out = []
    for x in (d or []):
        if isinstance(x, dict):
            out.append((x.get("url"), x.get("label")))
    return out


def menu_urls(loc="topbar"):
    return [u for u, _ in menu_rows(loc)]


def sv_set(visible):
    """改可见性：只动 visible 这一个变量（title/group 与 V2/V3 用的同一份）"""
    s, _ = call("PUT", "/admin/custom-pages/%d" % PID[0],
                page(visible=visible, group=CN21), token=ROOT)
    return s


def fetch_home():
    import urllib.error
    import urllib.request
    try:
        with urllib.request.urlopen("http://localhost:3000/", timeout=25) as r:
            return r.read().decode("utf-8", "replace")
    except Exception as e:  # noqa: BLE001
        print("   首页抓取失败（V5d 会判红并写明原因）：%s" % str(e)[:70])
        return ""


PID = [None]
MIDS = []
NAV0 = None


def cleanup():
    for m in MIDS:
        call("DELETE", "/admin/menu-items/%d" % m, None, token=ROOT)
    if PID[0]:
        call("POST", "/admin/custom-pages/%d/delete" % PID[0], {}, token=ROOT)
    if NAV0 is None:
        return
    # nav.custom_enabled 按库层原值复原（不依赖被验代码），缺行则删掉探针插的那行
    if NAV0 == "<缺行>":
        psql("DELETE FROM site_settings WHERE name = 'nav.custom_enabled'")
    else:
        psql("UPDATE site_settings SET value = '%s' WHERE name = "
             "'nav.custom_enabled'" % NAV0)


import atexit  # noqa: E402

atexit.register(cleanup)

T0 = psql("SELECT to_char(now(), 'YYYY-MM-DD HH24:MI:SS.US')")
CP0 = int(psql("SELECT count(*) FROM custom_pages") or -1)
MI0 = int(psql("SELECT count(*) FROM menu_items") or -1)
ok("R0a 基线：custom_pages=%d 行、menu_items=%d 行" % (CP0, MI0),
   CP0 >= 0 and MI0 >= 0, "CP=%s MI=%s" % (CP0, MI0))

# ===== V1 标题/分组名的字节 vs 字符 =====
s, b = call("POST", "/admin/custom-pages", page(title=CN34), token=ROOT)
pid = ((b or {}).get("data") or {}).get("id")
ok("V1a 34 个汉字的页名（102 字节，字符数 34 ≤100）必须能建页——限长文案写的"
   "是「1-100 字符」（修前若按字节判长：http=%s msg=%s）"
   % (s, str((b or {}).get("message", ""))[:40]),
   s == 200 and pid, "http=%s id=%s" % (s, pid))
if not pid:
    # 前置红不许带崩下游：用 ASCII 标题把探针页建出来，让 V2~V6 仍有前提可测
    s, b = call("POST", "/admin/custom-pages",
                page(title="BR probe %d" % TAG), token=ROOT)
    pid = ((b or {}).get("data") or {}).get("id")
PID[0] = pid
if not PID[0]:
    ok("V1z 探针页造不出来（上游前提整体不成立，本轮判据不可信）", False,
       "POST=%s %s" % (s, str((b or {}).get("message", ""))[:40]))
    summary()
    sys.exit(1)
s2, b2 = call("POST", "/admin/custom-pages",
              page(title=A100, slug="brqa%d" % TAG), token=ROOT)
qida = ((b2 or {}).get("data") or {}).get("id")
ok("V1b 正控：100 个 ASCII 字符的标题应放行（同形边界内）", s2 == 200,
   "http=%s" % s2)
s3, b3 = call("POST", "/admin/custom-pages",
              page(title=A101, slug="brqb%d" % TAG), token=ROOT)
ok("V1c 正控：101 个字符仍须被拒（不许把修复写成取消限长）",
   s3 == 400, "http=%s msg=%s" % (s3, str((b3 or {}).get("message", ""))[:30]))
s4, b4 = call("POST", "/admin/custom-pages",
              page(slug="BRUp%d" % TAG, title="大写 slug 探针"), token=ROOT)
ok("V1d 正控：slug 含大写必须 400（slug 规则不动）", s4 == 400,
   "http=%s" % s4)
for x in (qida, ((b4 or {}).get("data") or {}).get("id")):
    if x:
        call("POST", "/admin/custom-pages/%d/delete" % x, {}, token=ROOT)

# ===== V2 帮助中心分组名（同一族） =====
s5, b5 = call("PUT", "/admin/custom-pages/%d" % PID[0],
              page(group=CN21), token=ROOT)
ok("V2a 21 个汉字的帮助分组名（63 字节 / 21 字符）应能保存——超长应报错而非"
   "静默丢组（修前若按字节判长：http=%s msg=%s）"
   % (s5, str((b5 or {}).get("message", ""))[:36]),
   s5 == 200, "http=%s" % s5)
v2a = s5 == 200
ok("V2b 分组真生效：该页出现在 /help-docs 目录里" + ("" if v2a else
   "（级联：V2a 的分组没存上，本条不可测，判红不判绿）"),
   (SLUG in help_docs()) if v2a else False,
   "V2a=%s help-docs 含=%s" % (v2a, SLUG in help_docs()))

# ===== V3 公开展示口 =====
s6, b6 = call("PUT", "/admin/custom-pages/%d" % PID[0],
              page(group=CN21,
                   body="<p>ok</p><script>alert(1)</script>"
                        '<a href="javascript:alert(2)">x</a>'),
              token=ROOT)
sd, bd = call("GET", "/pages/%s" % SLUG, None, token=None)
body_html = str(((bd or {}).get("data") or {}).get("body") or "")
ok("V3a 出站消毒（正控）：/pages 回 body 里不许有 <script> 与 javascript:",
   s6 == 200 and "<script" not in body_html.lower()
   and "javascript:" not in body_html.lower(),
   "http=%s 残留 script=%s js 协议=%s"
   % (sd, "<script" in body_html.lower(),
      "javascript:" in body_html.lower()))
ok("V3b 可见页应同时出现在 /public-pages（sitemap 的来源）",
   SLUG in pub_pages(), "public-pages 含=%s" % (SLUG in pub_pages()))
s7, b7 = call("GET", "/pages/br-nope-%d" % TAG, None, token=None)
msg7 = str((b7 or {}).get("message") or "")
ok("V4 不存在的 slug：404 信封文案里不许出现假资源号 0（NotFound(0) 会让提示"
   "写成「资源 0 不存在」）——实测 message=%s" % msg7[:44],
   s7 == 404 and " 0" not in msg7, "http=%s message=%s" % (s7, msg7[:44]))

# ===== V5 菜单入口的死链与前台渲染（本族最重的一支） =====
# nav.custom_enabled 关着时前台菜单接口一律返回空（staff-tools-menu.tsx:53 有开关，
# 属正当设计），所以这一组判据必须先把开关临时打开，收尾按库层原值复原
# （复原不依赖被验代码）。
NAV0 = psql("SELECT coalesce(value, '<缺行>') FROM site_settings "
            "WHERE name = 'nav.custom_enabled'")
psql("INSERT INTO site_settings (name, value) VALUES ('nav.custom_enabled', "
     "'yes') ON CONFLICT (name) DO UPDATE SET value = 'yes'")
NAV1 = psql("SELECT value FROM site_settings WHERE name = "
            "'nav.custom_enabled'")
ok("V5z 前提：临时打开 nav.custom_enabled（原值=%s，写入后回读=%s；收尾按库层"
   "复原）" % (NAV0, NAV1), NAV1 == "yes", "写后回读=%s" % NAV1)
LB_TOP = "BR顶栏%d" % TAG
LB_SIDE = "BR侧栏%d" % TAG
for loc, lb in (("topbar", LB_TOP), ("sidebar", LB_SIDE)):
    s, b = call("POST", "/admin/menu-items",
                {"location": loc, "label": lb, "url": URL, "enabled": True},
                token=ROOT)
    if ((b or {}).get("data") or {}).get("id"):
        MIDS.append(((b or {}).get("data") or {}).get("id"))
    ok("V5a-%s 前提自证：页面可见时，指向 /p/<slug> 的 %s 条目能建能读"
       % (loc[:3], loc), s == 200 and lb in [x[1] for x in menu_rows(loc)],
       "http=%s" % s)
html = fetch_home()
# 正控（真判据）：API 侧这条 topbar 条目读得到——它证明「建页 → 建菜单 →
# 前台接口返回」三段是通的；首页 HTML 里有没有它只作读数（layout.tsx:143 把
# topbar 条目用来替换主菜单一级项，匿名首页是否渲染不属本判据）
ok("V5d 正控：可见页面的菜单条目能从公开接口读到（topbar 位置）",
   URL in menu_urls("topbar") and LB_TOP in [x[1] for x in menu_rows("topbar")],
   "HTML 长度=%d 含顶栏文字=%s（读数，不断言）" % (len(html),
                                                LB_TOP in html))
sv = sv_set(False)
hid = URL in menu_urls("topbar")
ok("V5b 页面隐藏（visible=false）后，前台菜单不该再给这条死链——判据原先只挡"
   "「模块关闭」那一支（隐藏那次 PUT http=%s，实测菜单仍含=%s）" % (sv, hid),
   not hid, "菜单=%s" % (menu_urls("topbar")[-3:]))
sv_set(True)
call("POST", "/admin/custom-pages/%d/delete" % PID[0], {}, token=ROOT)
PID[0] = None
ok("V5c 页面删除后，指向它的菜单条目同样不该留在前台（旧判据的 NOT EXISTS 在行"
   "没了的时候反而成立）", URL not in menu_urls("topbar"),
   "菜单=%s" % (menu_urls("topbar")[-3:]))
ok("V5e 读数（P-200 那条）：sidebar 条目 API 给得出、前台无人请求该位置"
   "——只打印不判绿，处置方式待站长拍板", True,
   "sidebar API 含=%s 首页 HTML 含=%s" % (LB_SIDE in [x[1] for x in
        menu_rows("sidebar")], LB_SIDE in html))

# ===== V6 审计留痕：记了要读得到 =====
s9, b9 = call("POST", "/admin/menu-items", {"location": "topbar",
              "label": "BR 二次%d" % TAG, "url": URL}, token=ROOT)
if ((b9 or {}).get("data") or {}).get("id"):
    MIDS.append(((b9 or {}).get("data") or {}).get("id"))
rows = ((call("GET", "/admin/audit?q=custom_page&limit=50", None,
              token=ROOT)[1] or {}).get("data") or [])
acts = sorted({r.get("action") for r in rows})
ok("V6a 三类动作都在册（add/update/delete，动作名与 i18n 词条一致）：%s" % acts,
   {"custom_page_add", "custom_page_update", "custom_page_delete"}
   <= set(acts), "读到=%s" % acts)
mine = [r for r in rows if str(r.get("created_at") or "")
        .replace("T", " ")[:19] >= T0[:19]]
no_ref = [r.get("action") for r in mine
          if not (r.get("detail") or "").strip()
          or r.get("detail") in ("null", "{}")]
ok("V6b 「谁改了哪个页面」要读得出来：本轮新写的三类审计行 detail（ref）"
   "不许为空（历史行的空 ref 不在本判据内；本轮 %d 条）" % len(mine),
   bool(mine) and not no_ref,
   "本轮=%d 条 空 ref=%d 条 %s" % (len(mine), len(no_ref),
                                sorted(set(no_ref))))
hist = psql("SELECT count(*) FROM audit_log WHERE action LIKE "
            "'custom_page%' AND (ref IS NULL OR ref->>'id' IS NULL)")
print("   [读数] 修复前的历史空 ref 行=%s 条（P-202 的账，只数不删）" % hist)

# ===== R9 收尾 =====
cleanup()
CP1 = int(psql("SELECT count(*) FROM custom_pages") or -1)
MI1 = int(psql("SELECT count(*) FROM menu_items") or -1)
ok("R9a 探针页与菜单条目已清：custom_pages %d→%d、menu_items %d→%d"
   % (CP0, CP1, MI0, MI1), CP1 == CP0 and MI1 == MI0,
   "CP=%s MI=%s" % (CP1, MI1))
ok("R9c nav.custom_enabled 已复原（原值=%s，现值=%s）" % (NAV0,
   psql("SELECT coalesce(value, '<缺行>') FROM site_settings WHERE "
        "name = 'nav.custom_enabled'")),
   psql("SELECT coalesce(value, '<缺行>') FROM site_settings WHERE "
        "name = 'nav.custom_enabled'") == NAV0,
   "原值=%s" % NAV0)
ok("R9b 探针 slug 不再出现在任何公开目录里",
   SLUG not in pub_pages() and SLUG not in help_docs()
   and URL not in menu_urls(),
   "pub=%s help=%s menu=%s" % (SLUG in pub_pages(), SLUG in help_docs(),
                               URL in menu_urls()))
summary()
