# -*- coding: utf-8 -*-
"""闸门 DD：HTML 注水点的消毒与权限面（公告 + 自定义页 + 字幕规则条）。

批一百五十六（P-341）实测的缺陷本体：
  `staff_http/custom_pages_public.rs` 的模块注释写着「与公告同防线 / 出站
  ammonia 消毒」，但实现只有 `ammonia::Builder::default().clean()` ——
  默认白名单不含 iframe/video/source，于是站长贴进「帮助页/规则页」的视频
  在**后台预览看得到、访客端整块消失**（活体实测：188 字符进、11 字符出），
  而公告那条线允许内嵌并按 video_embed_rules 校验 src。同一件产品两种尺。
  修法：两个出口共用 `http::home_news::html_clean`（默认白名单 +
  iframe/video/source + src/poster 过 enabled 的 embed_origin 集合，
  规则为空时一律剥 src，fail-closed）。

三处注水点各自的定性（都验过，别再当洞报）：
  1) 快讯弹窗 home-news-modal.tsx ← /home 公告 ← announcements（写侧
     news.manage=class>=90 或 announce.publish=role:liaison）→ 有消毒；
  2) /p/[slug] ← custom_pages（写侧 custompages.manage=class>=98，即站长层，
     且内容包/站型包写不到这两张表）→ 本批补齐同一把尺；
  3) 字幕规则条 subtitle-board.tsx:143 ← i18n 开发者文案 t.rules，
     不是用户数据 → boldRule 的 <b> 包装无害。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dd_html_sinks.py
"""
import io
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SLUG = "dd-probe-%d" % (int(time.time()) % 100000)
TITLE = "DD探针页勿动"
NEWS_TITLE = "DD探针公告勿动"


def read(rel):
    return io.open(os.path.join(ROOTDIR, rel), encoding="utf-8",
                   errors="replace").read()


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:170])
    return (r.stdout or "").strip()


# ---------- DD1 两个出口必须共用同一把尺 ----------
pages = read("apps/api/src/staff_http/custom_pages_public.rs")
news = read("apps/api/src/http/home_news.rs")
ok("DD1 自定义页出口走 html_clean（不许退回 ammonia 默认集直洗）",
   "html_clean(" in pages and "ammonia::Builder::default()" not in pages,
   "pages 里 html_clean=%s" % ("html_clean(" in pages))
ok("DD1b 公告出口仍走同一把尺", "html_clean(" in news and
   "pub(crate) fn html_clean" in news,
   "html_clean 不是 pub(crate) 就是第二把尺的开端")
ok("DD1c 内嵌源白名单只有一个出处（embed_src_allowed 别处不再抄一份）",
   len(re.findall(r"fn embed_src_allowed", news)) == 1)

# ---------- DD2 注水点台账（棘轮：新增必须写理由） ----------
SINKS = {
    "apps/web/app/layout.tsx": "<script>/<style>：JSON-LD 与主题令牌引导，"
                               "内容由服务端常量拼，非用户数据",
    "apps/web/app/(main)/p/[slug]/page.tsx": "自定义页正文（本批起与公告同尺）",
    "apps/web/components/home-news-modal.tsx": "公告正文（ammonia 白名单消毒）",
    "apps/web/components/subtitle-board.tsx": "字幕规则条：i18n 开发者文案",
}
SINK_RX = re.compile(r"dangerouslySetInnerHTML\s*=\s*\{")
found = {}
for dp, dn, fn in os.walk(os.path.join(ROOTDIR, "apps", "web")):
    dn[:] = [d for d in dn if d not in (".next", "node_modules")]
    for f in fn:
        if not f.endswith((".tsx", ".ts")):
            continue
        full = os.path.join(dp, f)
        txt = io.open(full, encoding="utf-8", errors="replace").read()
        # 只数真实的 JSX 属性用法：论坛那两处组件的注释写着「不使用
        # dangerouslySetInnerHTML」，按字符串计数会把它们记成注水点（本轮实测）
        n = len(SINK_RX.findall(txt))
        if n:
            rel = os.path.relpath(full, ROOTDIR).replace("\\", "/")
            found[rel] = n
extra = sorted(set(found) - set(SINKS))
gone = sorted(set(SINKS) - set(found))
for rel, n in sorted(found.items()):
    print("   注水点 %-52s %d 处  —— %s" % (rel, n, SINKS.get(rel, "未登记")))
ok("DD2 HTML 注水点集合不变（新 %d、消失 %d，共 %d 个文件）"
   % (len(extra), len(gone), len(found)), not extra and not gone,
   "新增未登记=%s｜已消失=%s" % ("、".join(extra), "、".join(gone)))

# ---------- DD3 单测在位（承重力交给 Rust 侧的纯函数断言） ----------
TESTS = ("html_clean_keeps_layout_tags_and_allowlisted_embeds",
         "html_clean_strips_scripts_and_unlisted_embed_origins",
         "html_clean_fails_closed_when_no_origins")
missing_tests = [t for t in TESTS if "fn " + t not in news]
ok("DD3 消毒器三条单测在位（缺 %d）" % len(missing_tests), not missing_tests,
   "、".join(missing_tests))

# ---------- DD4 活体：自定义页的内嵌与消毒 ----------
PAYLOAD = ('<h2>DD 探针</h2>'
            '<iframe src="https://player.bilibili.com/player.py?bvid=BV1"'
            ' allowfullscreen></iframe>'
            '<video src="/api/v1/attachments/a.mp4" controls'
            ' poster="/api/v1/attachments/p.jpg"></video>'
            '<iframe src="https://evil.example/e"></iframe>'
            '<script>alert(1)</script><img src=x onerror=alert(2)>'
            '<a href="javascript:alert(3)">js</a>')
tok = login()
# 新旧码判别（沿用 DA/DB 那把探针）：只有批一百五十一之后的码认 40 个汉字心愿词
KW40 = "甲" * 40
pr = call("POST", "/wishlist", {"keyword": KW40}, token=tok)
NEWCODE = pr[0] == 200
if NEWCODE:
    call("DELETE", "/wishlist/%s" % ((pr[1] or {}).get("data") or {}).get("id"),
         None, token=tok)
print("   本次跑的是%s二进制（心愿单 40 汉字探针 http=%s）"
      % ("**新批**" if NEWCODE else "现网**旧**", pr[0]))
pid = None
nid = None
try:
    st, res = call("POST", "/admin/custom-pages", {
        "slug": SLUG, "title": TITLE, "body": PAYLOAD, "visible": True,
        "sort": 997, "module_key": "", "doc_group": "", "doc_sort": 0},
        token=tok)
    pid = ((res or {}).get("data") or {}).get("id")
    ok("DD4a 造探针页成功（id=%s http=%s）" % (pid, st), st == 200 and pid,
       str(res)[:120])
    st2, got = call("GET", "/pages/" + SLUG)
    cleaned = ((got or {}).get("data") or {}).get("body") or ""
    print("   读回 http=%s 清洗后=%s" % (st2, cleaned[:190]))
    if NEWCODE:
        ok("DD4b 白名单内嵌源活下来（后台贴的=访客看到的）",
           "player.bilibili.com" in cleaned and "<video" in cleaned
           and 'poster="/api/v1/attachments/p.jpg"' in cleaned,
           "P-341 没生效：出口仍只洗默认白名单 ⇒ 站长贴的视频在访客端消失")
    else:
        print("   WARN DD4b 豁免：旧镜像把 iframe/video 整块吞掉（本批缺陷本体，"
              "此刻实测读出=%s）——豁免不是通过" % cleaned[:110])
    ok("DD4c 白名单外的 src 被剥、脚本/事件属性/伪协议一律杀",
       "evil.example" not in cleaned and "<script" not in cleaned
       and "onerror" not in cleaned and "javascript:" not in cleaned,
       cleaned[:150])
    db_raw = psql("SELECT body FROM custom_pages WHERE slug = '%s'" % SLUG)
    ok("DD4d 正控：库里存的仍是原文（消毒发生在出站，不是写侧偷偷改写）",
       "<script>alert(1)</script>" in db_raw, db_raw[:90])

    # ---------- DD5 活体：公告走的是同一把尺 ----------
    st3, nres = call("POST", "/admin/news", {
        "title": NEWS_TITLE, "body": PAYLOAD, "badge": "探针"}, token=tok)
    nid = ((nres or {}).get("data") or {}).get("id")
    ok("DD5a 造探针公告成功（id=%s http=%s）" % (nid, st3),
       st3 == 200 and nid, str(nres)[:120])
    st4, home = call("GET", "/home", token=tok)
    news_items = ((home or {}).get("data") or {}).get("news") or []
    mine = [n for n in news_items if n.get("id") == nid]
    body5 = (mine[0].get("body") if mine else "") or ""
    print("   /home http=%s 公告条数=%d 探针在不在前 8 条=%s"
          % (st4, len(news_items), bool(mine)))
    ok("DD5b 公告出口也剥净（同一把尺，两处不许一严一松）",
       bool(mine) and "<script" not in body5 and "onerror" not in body5
       and "javascript:" not in body5 and "evil.example" not in body5,
       body5[:150] if mine else "探针没进 /home 前 8 条")
    ok("DD5c 正控：消毒不是拿空数据自证（探针正文里排版标签还在）",
       "<h2" in body5 and "<video" in body5, body5[:120])
finally:
    if pid:
        call("POST", "/admin/custom-pages/%s/delete" % pid, {}, token=tok)
    if nid:
        call("DELETE", "/admin/news/%s" % nid, None, token=tok)
    left_p = psql("SELECT count(*) FROM custom_pages WHERE slug = '%s'" % SLUG)
    left_n = psql("SELECT count(*) FROM announcements WHERE id = %s"
                  % (nid or 0))
    ok("DD6 收尾：探针页/探针公告都不留现网（页=%s 公告=%s）"
       % (left_p, left_n), left_p == "0" and left_n == "0",
       "page=%s news=%s" % (left_p, left_n))

# ---------- DD7 权限面：消毒的前提是「写侧够不着普通会员」 ----------
grants = psql("SELECT permission_key || ' ' || role_type || ':' || "
              "role_key::text || '=' || granted FROM role_permissions "
              "WHERE permission_key IN ('custompages.manage','news.manage',"
              "'announce.publish') ORDER BY 1").replace("\n", "  ")
print("   写侧授予面：%s" % grants)
cp = [g for g in grants.split() if g.startswith("custompages.manage")]
low = [g for g in cp if re.search(r"class:(0|[1-8][0-9]|9[0-7])\b", g)]
ok("DD7 custompages.manage 只授到站长层（class>=98），违例 %d" % len(low),
   not low and bool(cp), " ".join(cp))
pack_hit = []
for rel in ("apps/api/src/settings_http/pack_store.rs",
            "apps/api/src/settings_http/pack_import.rs",
            "apps/api/src/settings_http/pack_assets.rs"):
    if os.path.exists(os.path.join(ROOTDIR, rel)):
        if re.search(r"custom_pages|announcements", read(rel)):
            pack_hit.append(rel)
ok("DD7b 内容包/站型包写不到这两张表（外部包不能带 HTML 进来，命中 %d）"
   % len(pack_hit), not pack_hit, "、".join(pack_hit))

summary()
