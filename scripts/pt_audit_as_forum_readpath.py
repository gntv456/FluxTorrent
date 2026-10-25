# -*- coding: utf-8 -*-
"""闸门 AS：论坛读路径与视频内嵌口径（2026-10-11 批二十八实测固化）。

钉住五件事：
  S1 protected 版块的「2 楼起隐藏正文」不许被 /forums/search 击穿（P-51）——
     详情返回 `……` 而搜索返回原文，等于同一批正文换了个出口；
  S2 分区下架（forum_categories.visible=false）要三个分发口同时生效：
     列表 / 搜索 / 关注流 / RSS（P-52）——只藏列表页等于没藏；
  S3 无权读取（403）不得涨主题热度，views+1 必须在判权之后（P-53）；
  S4 post_video_max 与前端渲染同口径：裸 URL 自动内嵌要计数、编辑口也要过闸
     （P-54：旧版只数 `!video(`，且只有发帖/回帖查，编辑不查）；
  S5 内嵌规则真的能命中（P-55：0189 把正则里的 `\\.` 写成 `\\\\.`，四条内置
     规则自上线以来永不命中），且保存侧拦住这种「能编译、永不命中」的形态；
     命中之外还要断言**生成物**（P-56：bilibili 模板取 $1，而 1 组是可选的
     `(www\\.)?`，播放器加载的是 `bvid=www.` 这个不存在的稿件）。

假绿自查（写这条闸门时真实踩过）：搜索/RSS 的「送不出去」不能拿空数组当判据
——SQL 拼坏让接口 500 时 data 也是空，四条负控一起绿、四条正控一起灭。
所以 search() 非 200/非列表直接炸，S2 还加了「可见时三出口都送得出」的地基
与「恢复可见后立刻恢复」的反控。

自建数据自收尾：探针分区/版块跑完删除，探针号置 status=2（audit_log 防篡改
链不允许真删号），site_settings 的先读后写、finally 原值写回。
共享设置只在本站内串行探针，不与其它闸门并行。

跑法：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_as_forum_readpath.py
"""
import atexit
import os
import re
import subprocess
import sys
import time
from urllib.parse import quote
sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
CLEAN = []
BV = "https://www.bilibili.com/video/BV1xx411c7mD"
NON = "https://example.com/watch?v=deadbeef"


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip()[:200])
    return r.stdout.strip()


def exec_sql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   ! SQL 失败: %s" % (r.stderr or "").strip()[:150])
    return r.returncode


def cleanup():
    """可重复执行：中途崩了 atexit 仍会把共享设置还原。"""
    stmts = list(reversed(CLEAN))
    del CLEAN[:]
    for stmt in stmts:
        exec_sql(stmt)


atexit.register(cleanup)


def login_backoff(user, tries=10):
    for _ in range(tries):
        s, r = call("POST", "/auth/login",
                    {"username": user, "password": "password123"})
        tok = (r.get("data") or {}).get("token")
        if tok:
            return tok
        time.sleep(8 if r.get("code") == 1015 or s == 429 else 3)
    return None


def new_member(tag):
    """注册限流 5/分钟：清掉自己造的计数再按节拍建号。"""
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return None
    code, cid = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None
    user = "as%s%d" % (tag, TAG)
    s, rr = call("POST", "/auth/register", {
        "username": user, "email": "%s@as.local" % user,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    uid = (rr.get("data") or {}).get("user_id")
    if not uid:
        return None
    CLEAN.append("UPDATE users SET status = 2 WHERE id = %s" % uid)
    CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
    tok = login_backoff(user)
    if not tok:
        return None
    return (tok, uid, user)


def mk_board(cid):
    exec_sql("INSERT INTO forums (name, descr, min_class, minclassread, "
             "minclasswrite, minclasscreate, protected, category_id, sort) "
             "VALUES ('AS闸门版%d', '探针版块', 0, 0, 0, 0, FALSE, %s, 900)"
             % (TAG, cid))
    return psql("SELECT id FROM forums WHERE name = 'AS闸门版%d'" % TAG)


def topic(tok, fid, name, body):
    body_json = {"forum_id": int(fid), "title": "AS闸门%s%s" % (name, TAG),
                 "body": body}
    s, r = call("POST", "/forums/topics", body_json, token=tok)
    d = r.get("data") or {}
    return d.get("topic_id") or d.get("id"), s, r


def first_post(tid):
    return psql("SELECT min(id) FROM posts WHERE topic_id = %s" % tid)


def detail(tok, tid):
    return call("GET", "/forums/topics/%s" % tid, token=tok)


def search(tok, kw):
    """搜索返回必须 200 + 列表。把「空数组」直接当成「被权限挡住」是假绿
    机制——本批就撞过一次：format! 拼出的坏 SQL 让接口 500、data 为 null，
    于是四条负控全绿、四条正控全灭。"""
    s, r = call("GET", "/forums/search?q=" + quote(kw), token=tok)
    d = r.get("data")
    if s != 200 or not isinstance(d, list):
        raise SystemExit("搜索接口不可用：http=%s %s" % (s, brief(r)))
    return d


def hit_of(rows, tid, kw):
    """命中该主题的那一行（tid 转 int 再比，接口给的是数字）。"""
    for x in rows:
        if x.get("topic_id") == int(tid):
            return x
    return None


def pmatch(pat, url):
    """与浏览器 new RegExp(pattern, "i") 同口径的命中测试。"""
    try:
        return re.search(pat, url, re.I)
    except (re.error, TypeError):
        return None


def embed_src(rule, url):
    """复刻 forum-video-embed.tsx 的 fillTemplate：$n → 捕获组，再拼
    extra_params。命中与否返回 None/字符串。
    只断言「正则命中」是不够的——模板取错捕获组时正则照样命中，播放器
    却加载一个不存在的 id（P-56：bilibili 模板写 $1，而 1 组是可选的
    (www\\.)?，生成 bvid=www.）。"""
    m = pmatch(rule.get("url_pattern") or "", url)
    if not m:
        return None

    def sub(x):
        n = int(x.group(1))
        try:
            return m.group(n) or ""
        except IndexError:
            return ""

    src = re.sub(r"\$([1-9])", sub, rule.get("embed_template") or "")
    extra = rule.get("extra_params")
    if extra:
        src += "" if extra[:1] in ("&", "?") else "?"
        src += extra
    return src


def brief(x):
    return str(x)[:150]


def rss_body(tok, uid, fid):
    pk = psql("SELECT passkey FROM users WHERE id = %s" % uid)
    s, raw = call("GET", "/rss/forum/%s?forums=%s" % (pk, fid),
                  token=tok, raw=True)
    return s, (raw or b"").decode("utf-8", "replace")


print("== S0 前置：探针分区/版块/成员 ==")
# 上一次中途崩了留下的残料（本闸门独有的名字，不碰别人的数据）
exec_sql("DELETE FROM video_embed_rules WHERE provider ~ '^as(bad)?[0-9]+$'")
exec_sql("DELETE FROM forums WHERE name LIKE 'AS闸门版%'")
exec_sql("DELETE FROM forum_categories WHERE name LIKE 'AS闸门区%'")
exec_sql("INSERT INTO forum_categories (name, sort, visible) "
         "VALUES ('AS闸门区%d', 900, TRUE)" % TAG)
CAT = psql("SELECT id FROM forum_categories WHERE name = 'AS闸门区%d'" % TAG)
FID = mk_board(CAT)
boards = call("GET", "/forums/boards", token=ROOT)[1].get("data") or []
FID2 = boards[0]["id"]
LOW = new_member("lo")
if not (CAT and FID and LOW):
    ok("前置齐备（分区/版块/探针号）", False,
       "CAT=%s FID=%s LOW=%s" % (CAT, FID, bool(LOW)))
    summary()
    cleanup()
    sys.exit(1)
LT, LUID, _ = LOW
CLEAN.append("DELETE FROM follows WHERE user_id = %s" % LUID)
CLEAN.append("DELETE FROM posts WHERE topic_id IN (SELECT id FROM topics "
             "WHERE title LIKE 'AS闸门%%%s')" % TAG)
CLEAN.append("DELETE FROM topics WHERE title LIKE 'AS闸门%%%s'" % TAG)
CLEAN.append("DELETE FROM forums WHERE id = %s" % FID)
CLEAN.append("DELETE FROM forum_categories WHERE id = %s" % CAT)
call("POST", "/follows", {"target_type": "forum", "target_id": int(FID)},
     token=LT)
ok("探针分区/版块/成员就绪", bool(CAT) and bool(FID),
   "CAT=%s FID=%s FID2=%s LOW=%s" % (CAT, FID, FID2, LUID))

# 主题：T1 一楼公开 + 二楼私密；T2 下架分区用；T3 判权前的热度用
T1, s1, r1 = topic(ROOT, FID, "T1", "ASPROT1F%s 一楼公开正文" % TAG)
REPLY = "ASPROTSEC%s 二楼私密回复" % TAG
s, rr = call("POST", "/forums/topics/%s/reply" % T1, {"body": REPLY},
             token=ROOT)
T2, s2, _ = topic(ROOT, FID, "T2", "ASHIDDEN%s 下架分区正文" % TAG)
ok("三楼内容可准备（发主题+回帖）",
   bool(T1) and bool(T2) and s == 200, "s1=%s reply=%s s2=%s" % (s1, s, s2))

print("== S1 protected 版块 × 搜索（P-51）==")
exec_sql("UPDATE forums SET protected = TRUE WHERE id = %s" % FID)
d_low = detail(LT, T1)[1].get("data") or {}
floors = d_low.get("posts") or []
f2 = floors[1] if len(floors) > 1 else {}
ok("S1 地基：详情页对普通成员把 2 楼正文替换成提示",
   f2.get("body") == "……", "二楼=%s" % brief(f2.get("body")))
rows = search(LT, "ASPROTSEC%s" % TAG)
h = hit_of(rows, T1, "ASPROTSEC")
ok("S1 搜索不得把 protected 版块的非一楼正文送出去（连命中都不该有）",
   h is None, "命中=%s" % brief(h))
rows = search(LT, "ASPROT1F%s" % TAG)
ok("S1 正控：一楼正文（本就公开）仍搜得到",
   hit_of(rows, T1, "ASPROT1F") is not None, brief(rows))
rows = search(ROOT, "ASPROTSEC%s" % TAG)
ok("S1 正控：站长在 protected 版块搜得到二楼",
   hit_of(rows, T1, "ASPROTSEC") is not None, brief(rows))
d_root = detail(ROOT, T1)[1].get("data") or {}
rf = (d_root.get("posts") or [{}])[1] if len(d_root.get("posts") or []) > 1 \
    else {}
ok("S1 正控：站长读到的二楼是原文", REPLY.split()[0] in str(rf.get("body")),
   brief(rf.get("body")))
exec_sql("UPDATE forums SET protected = FALSE WHERE id = %s" % FID)

print("== S2 分区下架 × 四个分发口（P-52）==")
# 地基：分区可见时三个出口都送得出这条主题——没有这一步，下面的
# 「送不出去」可能是接口挂了而不是权限挡住了（本批真实踩过）。
feed0 = call("GET", "/forums/feed", token=LT)[1].get("data") or {}
items0 = feed0.get("items") if isinstance(feed0, dict) else feed0
s0, xml0 = rss_body(LT, LUID, FID)
ok("S2 地基：可见时搜索/关注流/RSS 三出口都送得出 T2",
   hit_of(search(LT, "ASHIDDEN%s" % TAG), T2, "ASHIDDEN") is not None
   and any(x.get("topic_id") == int(T2) for x in items0 or [])
   and s0 == 200 and ("AS闸门T2%s" % TAG) in xml0,
   "rss=%s feed=%s" % (s0, brief(items0)))
exec_sql("UPDATE forum_categories SET visible = FALSE WHERE id = %s" % CAT)
lst = call("GET", "/forums", token=LT)[1].get("data") or []
lrows = lst if isinstance(lst, list) else (lst.get("forums") or [])
ok("S2 列表页不含下架版块的版块行",
   not any(x.get("id") == int(FID) for x in lrows), brief(lrows))
rows = search(LT, "ASHIDDEN%s" % TAG)
ok("S2 搜索不送下架分区的内容", hit_of(rows, T2, "ASHIDDEN") is None,
   brief(rows))
feed = call("GET", "/forums/feed", token=LT)[1].get("data") or {}
fitems = feed.get("items") if isinstance(feed, dict) else feed
ok("S2 关注流不送下架分区的主题",
   not any(x.get("topic_id") == int(T2) for x in fitems or []),
   brief(fitems))
s, xml = rss_body(LT, LUID, FID)
ok("S2 RSS 不送下架分区的主题（RSS 出口不在网关里，必须自证）",
   s == 200 and "AS闸门T2%s" % TAG not in xml,
   "http=%s len=%d" % (s, len(xml)))
sr, xmlr = rss_body(ROOT, 1, FID)
ok("S2 正控：站长 RSS 仍推得到下架分区的主题（出口活着，判据按等级）",
   sr == 200 and "AS闸门T2%s" % TAG in xmlr, "http=%s len=%d"
   % (sr, len(xmlr)))
ok("S2 正控：站长仍能搜到下架分区",
   hit_of(search(ROOT, "ASHIDDEN%s" % TAG), T2, "ASHIDDEN") is not None)
exec_sql("UPDATE forum_categories SET visible = TRUE WHERE id = %s" % CAT)
ok("S2 反控：分区恢复可见后普通成员立刻搜得到（过滤器不是永真）",
   hit_of(search(LT, "ASHIDDEN%s" % TAG), T2, "ASHIDDEN") is not None)
feed1 = call("GET", "/forums/feed", token=LT)[1].get("data") or {}
items1 = feed1.get("items") if isinstance(feed1, dict) else feed1
ok("S2 反控：关注流也随之恢复（不是缓存或一次性判定）",
   any(x.get("topic_id") == int(T2) for x in items1 or []),
   brief(items1))

print("== S3 无权读取不涨热度（P-53）==")
exec_sql("UPDATE forums SET minclassread = 90 WHERE id = %s" % FID)
v0 = int(psql("SELECT views FROM topics WHERE id = %s" % T1) or 0)
s, r = detail(LT, T1)
v1 = int(psql("SELECT views FROM topics WHERE id = %s" % T1) or 0)
ok("S3 抬门槛后普通成员直读被拒", s == 403, "http=%s" % s)
ok("S3 403 不得涨热度（views+1 必须在判权之后）", v1 == v0,
   "views %s -> %s" % (v0, v1))
s, _ = detail(LT, 999999999)
ok("S3 反控：不存在的主题仍是 404（403 不是被改成万能拒绝）", s == 404,
   "http=%s" % s)
exec_sql("UPDATE forums SET minclassread = 0 WHERE id = %s" % FID)
v0 = int(psql("SELECT views FROM topics WHERE id = %s" % T1) or 0)
s, _ = detail(LT, T1)
v1 = int(psql("SELECT views FROM topics WHERE id = %s" % T1) or 0)
ok("S3 正控：有权读取仍照常 +1（计数器没被一起关掉）",
   s == 200 and v1 == v0 + 1, "http=%s views %s -> %s" % (s, v0, v1))

print("== S4 每帖视频上限：裸链接与编辑口（P-54）==")
orig_max = psql("SELECT value FROM site_settings WHERE name = "
                "'post_video_max'") or "3"
orig_emb = psql("SELECT value FROM site_settings WHERE name = "
                "'forum_video_embed'") or "yes"
CLEAN.append("UPDATE site_settings SET value = '%s' WHERE name = "
             "'post_video_max'" % orig_max)
CLEAN.append("UPDATE site_settings SET value = '%s' WHERE name = "
             "'forum_video_embed'" % orig_emb)
exec_sql("UPDATE site_settings SET value = '3' WHERE name = "
         "'post_video_max'")
exec_sql("UPDATE site_settings SET value = 'yes' WHERE name = "
         "'forum_video_embed'")


def video_msg(r):
    return str((r or {}).get("message") or "")


_, s, r = topic(ROOT, FID2, "V4x",
               "\n".join("!video(%s)" % BV for _ in range(4)))
ok("S4 地基：4 个显式 !video 被拒（上限确实在管）",
   s == 400 and "视频" in video_msg(r), "http=%s %s" % (s, brief(r)))
_, s, r = topic(ROOT, FID2, "V3bare", "\n".join(BV for _ in range(3)))
ok("S4 合法：3 条白名单裸链接放行", s == 200, "http=%s %s" % (s, brief(r)))
_, s, r = topic(ROOT, FID2, "V10bare", "\n".join(BV for _ in range(10)))
ok("S4 裸链接自动内嵌同样计入上限（旧版可绕过）",
   s == 400 and "视频" in video_msg(r), "http=%s %s" % (s, brief(r)))
_, s, r = topic(ROOT, FID2, "V10non", "\n".join(NON for _ in range(10)))
ok("S4 反控：非白名单裸链接不算视频（贴链接清单不受累）", s == 200,
   "http=%s %s" % (s, brief(r)))
_, s, r = topic(ROOT, FID2, "Vfence",
             "```\n" + "\n".join(BV for _ in range(10)) + "\n```")
ok("S4 反控：代码围栏内的 URL 不算视频", s == 200, "http=%s %s"
   % (s, brief(r)))
_, s, r = topic(ROOT, FID2, "Vbad",
                "\n".join("!video(%s)y)" % BV for _ in range(10)))
ok("S4 反控：!video(URL) 里带右括号的畸形行前端不出播放器，也不该计数",
   s == 200, "http=%s %s" % (s, brief(r)))
ET, s, _ = topic(ROOT, FID2, "Vedit", "!video(%s)" % BV)
pid = first_post(ET) if ET else ""
ok("S4 前置：编辑口的目标帖子拿得到", bool(pid) and pid.isdigit(),
   "ET=%s pid=%s" % (ET, pid))
s, r = call("PUT", "/forums/posts/%s" % pid,
            {"body": "\n".join("!video(%s)" % BV for _ in range(4))},
            token=ROOT)
ok("S4 编辑口也要过上限（旧版改帖不查）",
   s == 400 and "视频" in video_msg(r), "http=%s %s" % (s, brief(r)))
s, r = call("PUT", "/forums/posts/%s" % pid,
            {"body": "\n".join("!video(%s)" % BV for _ in range(2))},
            token=ROOT)
ok("S4 正控：编辑到 2 个视频放行", s == 200, "http=%s %s" % (s, brief(r)))
s, r = call("POST", "/forums/topics/%s/reply" % T1,
            {"body": "\n".join(BV for _ in range(9))}, token=ROOT)
ok("S4 回帖口同样计入", s == 400, "http=%s %s" % (s, brief(r)))

print("== S5 内嵌规则命中面与保存侧形态拦截（P-55）==")
rules = call("GET", "/forums/embed-rules", token=ROOT)[1].get("data") or []
CANON = {
    "bilibili": BV,
    "youtube": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
    "youtube_short": "https://youtu.be/dQw4w9WgXcQ",
    "vimeo": "https://vimeo.com/76979871",
    "self": "/api/v1/attachments/" + "0" * 64,
}
# 一个 provider 可能有多条规则（桌面版/移动版/av 号/Shorts…）：产品口径是
# 「按 sort 顺序逐条试，第一条命中的生效」。此前这里用 {provider: rule} 建
# 字典（后一条覆盖前一条），多规则一进来就取错行——判据变假红：
# 2026-10-20 实测 bilibili/youtube 各红两条（取到的是 av 号/移动版那条，
# 拿它去撞桌面版规范 URL 当然不中），而产品行为是对的（活测能出播放器）。
by_provider = {}
for r in rules:
    by_provider.setdefault(r.get("provider"), []).append(r)
ok("S5 五条内置规则都在（不是被后台删了）",
   all(p in by_provider for p in CANON), brief(list(by_provider)))


def first_hit(provider, url):
    """产品口径：按 sort 顺序取第一条命中的规则。"""
    for r in by_provider.get(provider) or []:
        if pmatch(r.get("url_pattern") or "", url):
            return r
    return None


for p, url in CANON.items():
    hit = first_hit(p, url)
    ok("S5 规则 %s 命中该平台规范 URL" % p, hit is not None,
       "url=%s 候选=%s" % (url, brief([
           (r.get("url_pattern") or "") for r in by_provider.get(p) or []])))
NEED = {"bilibili": "BV1xx411c7mD", "youtube": "dQw4w9WgXcQ",
        "youtube_short": "dQw4w9WgXcQ", "vimeo": "76979871",
        "self": "0" * 64}
for p, url in CANON.items():
    # 取**真命中**的那条规则来渲染（与产品同口径），别再拿任意一条
    rule = first_hit(p, url) or {}
    src = embed_src(rule, url)
    ok("S5 %s 生成的播放器 src 取对捕获组且落在 embed_origin 内" % p,
       bool(src) and src.startswith(rule.get("embed_origin") or "无Origin")
       and NEED[p] in src, "src=%s" % brief(src))
# 反控要覆盖该 provider 的**全部**规则：任何一条能撞上仿冒域都不放过
bad = [r for r in by_provider.get("bilibili") or []
       if pmatch(r.get("url_pattern") or "",
                 "https://evil.com/video/BV1xx411c7mD")]
ok("S5 反控：域名锚定生效，仿冒域不命中", not bad, brief(bad))
dub = psql("SELECT count(*) FROM video_embed_rules WHERE position("
            "E'\\\\\\\\' in url_pattern) > 0")
ok("S5 库里不再有「双写反斜杠」的规则（0189 的写法使四条规则永不命中）",
   dub == "0", "残留 %s 条" % dub)
GOOD_PAT = "^https?://as%d\\.example\\.com/([0-9]+)$" % TAG
BAD_PAT = "^https?://asbad%d\\\\.example\\\\.com/([0-9]+)$" % TAG
s, r = call("POST", "/admin/embed-rules", {
    "provider": "as%d" % TAG, "name_zh": "AS 探针规则",
    "url_pattern": GOOD_PAT,
    "embed_template": "https://as%d.example.com/e/$1" % TAG,
    "embed_origin": "https://as%d.example.com/" % TAG,
    "render_kind": "iframe", "aspect": "16:9", "enabled": True}, token=ROOT)
ok("S5 正控：单反斜杠的正确规则可保存", s == 200, "http=%s %s"
   % (s, brief(r)))
new_id = ((r.get("data") or {}) or {}).get("id")
if new_id:
    CLEAN.append("DELETE FROM video_embed_rules WHERE id = %s" % new_id)
ok("S5 正控：存进去的规则形态没有双写反斜杠", "\\\\" not in GOOD_PAT,
   brief(GOOD_PAT))
s, r = call("POST", "/admin/embed-rules", {
    "provider": "asbad%d" % TAG, "name_zh": "AS 坏形态规则",
    "url_pattern": BAD_PAT,
    "embed_template": "https://asbad%d.example.com/e/$1" % TAG,
    "embed_origin": "https://asbad%d.example.com/" % TAG,
    "render_kind": "iframe", "aspect": "16:9", "enabled": True}, token=ROOT)
ok("S5 保存侧拦下双写反斜杠（0189 那类正则从此进不了库）",
   s == 400 and "反斜杠" in video_msg(r), "http=%s %s" % (s, brief(r)))
bad_id = ((r.get("data") or {}) or {}).get("id")
if bad_id:
    CLEAN.append("DELETE FROM video_embed_rules WHERE id = %s" % bad_id)
_, s, r = topic(ROOT, FID2, "V4bare", "\n".join(BV for _ in range(4)))
ok("S5 联动：4 条白名单裸链接被拒 = 服务端计数确实用上了修好的规则",
   s == 400, "http=%s %s" % (s, brief(r)))

cleanup()
summary()
