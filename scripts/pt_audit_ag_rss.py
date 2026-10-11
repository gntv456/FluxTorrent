# -*- coding: utf-8 -*-
"""批十闸门 G：RSS 出口（主订阅源 + 论坛订阅源）。

RSS 是刷流链路的咽喉：passkey 鉴权、限流、促销标记、paid 过滤此前各轮已
加固；本轮补两条判据——① search 的 LIKE 转义与主列表同口径（裸 %/%25
不再当通配退化成「不过滤」，但字面 % 的种名要搜得到）；② 论坛 RSS 的
逐行权限复刻补齐 class>=90 放行（网页能读的版块 RSS 也必须推）。
判据全部对语料/造帖断言精确集合，不认「有返回就行」。
"""
import datetime
import hashlib
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_form import BOUNDARY, make_torrent, multipart  # noqa: E402
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402

ROOT = login()
CLEAN = []


def q(sql_text):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    return p.stdout.strip()


def exec_sql(sql_text):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    if p.returncode:
        raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:220])


def brief(resp):
    if "raw" in resp:
        return "http=%s raw=%s" % (resp.get("status"), resp["raw"][:110])
    return "msg=%s code=%s" % (str(resp.get("message"))[:56],
                               resp.get("code"))


def upload(tok, name):
    body = multipart({"name": name, "category_id": "1",
                      "descr": "RSS 探针正文，长度足够过常规校验",
                      "small_descr": "ag"},
                     make_torrent(name), filename="ag.torrent")
    req = urllib.request.Request(BASE + "/torrents", method="POST",
                                 data=body)
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(req, timeout=40)
        return json.loads(r.read())
    except urllib.error.HTTPError as e:
        return {"status": e.code,
                "raw": e.read()[:200].decode("utf-8", "replace")}


def rss(path_qs, tok=None, pk=None, sub=""):
    """拉 RSS 原文。passkey 在路径里，鉴权中间件本就放行 /rss；带 token
    只是为了保持与站内其它探针同构。sub 是 /rss/ 下的子路径（forum/）。
    返回 (status, text, content_type, cache_control)。"""
    url = BASE + path_qs if pk is None else \
        BASE.replace("/api/v1", "") + "/api/v1/rss/" + sub + pk + (
            "?" + path_qs if path_qs else "")
    rq = urllib.request.Request(url)
    if tok:
        rq.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(rq, timeout=30)
        return r.status, r.read().decode("utf-8", "replace"), \
            r.headers.get("Content-Type"), r.headers.get("Cache-Control")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace"), \
            e.headers.get("Content-Type"), e.headers.get("Cache-Control")


def items_of(text):
    return text.count("<item>")


print("== G0 语料与 passkey ==")
PK = q("SELECT passkey FROM users WHERE class_id = 99 AND status < 2 "
       "ORDER BY id LIMIT 1")
ok("G0 站长 passkey 可取（32 位小写 hex）",
   len(PK) == 32 and all(c in "0123456789abcdef" for c in PK),
   "pk=%s…" % PK[:8])
NAMES = ["agrss xinghe 1080P %d" % int(time.time()),
         "agrss pct%%50 x %d" % int(time.time())]
IDS = {}
for nm in NAMES:
    r = upload(ROOT, nm)
    tid = (r.get("data") or {}).get("id")
    if not tid:
        ok("G0 语料可上传（%s）" % nm, False, brief(r))
        continue
    IDS[nm] = tid
    CLEAN.append("DELETE FROM tags WHERE torrent_id = %d" % tid)
    CLEAN.append("DELETE FROM torrent_sections WHERE torrent_id = %d" % tid)
    CLEAN.append("DELETE FROM comments WHERE torrent_id = %d" % tid)
    CLEAN.append("DELETE FROM snatches WHERE torrent_id = %d" % tid)
    CLEAN.append("DELETE FROM bookmarks WHERE torrent_id = %d" % tid)
    CLEAN.append("DELETE FROM promotions WHERE torrent_id = %d" % tid)
    CLEAN.append("DELETE FROM torrents WHERE id = %d" % tid)
ok("G0 两颗语料齐（缺一颗后续不判别）",
   all(t in IDS for t in NAMES), str(IDS))
if len(IDS) < 2:
    sys.exit("语料不全，闸门终止")
A1 = str(IDS[NAMES[0]])
A2 = str(IDS[NAMES[1]])

print("== G1 基线：内容类型 / 频道名 / 语料可见 ==")
s, body, ct, _cc = rss("", ROOT, PK)
ok("G1 RSS 200 且 content-type 为 rss+xml",
   s == 200 and "rss+xml" in (ct or ""), "http=%s ct=%s" % (s, ct))
ok("G1 语料两颗都在 feed 里", A1 in body and A2 in body,
   "items=%d" % items_of(body))
ok("G1 cache-control 带 no-store（enclosure 含 passkey 不可被中间层缓存）",
   _cc is not None and "no-store" in _cc.lower(), "cc=%r" % _cc)

print("== G2 search 的 LIKE 转义（本轮修复判据）==")
s, body, _, _cc = rss("search=" + urllib.parse.quote("%"), ROOT, PK)
ok("G2 裸 % 不当通配（命中数不得等于全站，应为 0 或仅字面 % 的 A2）",
   s == 200 and items_of(body) <= 1,
   "items=%d" % items_of(body))
s, body, _, _cc = rss("search=" + urllib.parse.quote("agrss pct%50"), ROOT, PK)
ok("G2 字面 % 的种名搜得到（转义没把合法用例打死）",
   s == 200 and A2 in body, "items=%d" % items_of(body))
s, body, _, _cc = rss("search=" + urllib.parse.quote("agrss"), ROOT, PK)
ok("G2 正常关键词命中两颗", s == 200 and A1 in body and A2 in body,
   "items=%d" % items_of(body))
s, body, _, _cc = rss("search=" + urllib.parse.quote(
    "x' OR 1=1;--"), ROOT, PK)
ok("G2 注入串安全返回空集", s == 200 and items_of(body) == 0,
   "items=%d" % items_of(body))

print("== G3 形状与限流闸 ==")
s, body, _, _cc = rss("", ROOT, "!!!!short")
ok("G3 非法 passkey 形状 400", s == 400, "http=%s" % s)
s, body, _, _cc = rss("", ROOT, "0" * 32)
ok("G3 合法形状但不存在的 passkey 404", s == 404, "http=%s" % s)

print("== G4 论坛 RSS：权限口径（本轮修复判据）==")
BOARD = q("SELECT COALESCE((SELECT min(id) FROM forums WHERE "
          "name LIKE '公告%'), 1)")
TT = "agrss 论坛探针 %d" % int(time.time())
s, r = call("POST", "/forums/topics",
            {"forum_id": int(BOARD), "title": TT,
             "body": "RSS 探针正文，长度足够"},
            token=ROOT)
tid = (r.get("data") or {}).get("id") or (r.get("data") or {}).get(
    "topic_id")
ok("G4 探针帖可建", s == 200 and tid, brief(r) + " tid=%s" % tid)
if tid:
    CLEAN.append("DELETE FROM topics WHERE id = %s" % tid)
    CLEAN.append("DELETE FROM posts WHERE topic_id = %s" % tid)
s, body, _, _cc = rss("", ROOT, PK, sub="forum/")
ok("G4 论坛 RSS 200 且探针帖可见（缺省=公告版）",
   s == 200 and TT in body, "http=%s" % s)

# 高门槛版块 + class 90 职员口径
exec_sql("INSERT INTO forums (name, descr, min_class, minclassread, "
         "minclasswrite, minclasscreate, protected, sort) VALUES "
         "('agrss 门槛版', '探针', 1, 95, 95, 95, FALSE, 999)")
FID = q("SELECT id FROM forums WHERE name = 'agrss 门槛版'")
CLEAN.append("DELETE FROM forums WHERE id = %s" % FID)
TT2 = "agrss 门槛版探针 %d" % int(time.time())
s, r = call("POST", "/forums/topics",
            {"forum_id": int(FID), "title": TT2,
             "body": "RSS 探针正文，长度足够"}, token=ROOT)
tid2 = (r.get("data") or {}).get("id") or (r.get("data") or {}).get(
    "topic_id")
ok("G4 门槛版探针帖可建", s == 200 and tid2, brief(r))
if tid2:
    CLEAN.append("DELETE FROM topics WHERE id = %s" % tid2)
    CLEAN.append("DELETE FROM posts WHERE topic_id = %s" % tid2)
    s, body, _, _cc = rss("forums=%s" % FID, ROOT, PK, sub="forum/")
    ok("G4 站长（class 99）门槛版帖子在 RSS 里",
       s == 200 and TT2 in body, "items=%d" % items_of(body))

STOK = None
if tid2:
    user = "ag%d" % (int(time.time()) % 100000)
    pw = "Gate#123456"
    s, r = call("POST", "/admin/adduser",
                {"username": user, "password": pw,
                 "email": "%s@ag.local" % user}, token=ROOT)
    uid = q("SELECT id FROM users WHERE username = '%s'" % user)
    CLEAN.append("UPDATE users SET status = 2 WHERE id = %s" % uid)
    # 职员档取 user_classes 实有的 91（发布员）：minclassread=95 下 91<95，
    # 全靠 class>=90 放行（修复判据）——旧谓词只有 minclassread<=class 会漏
    exec_sql("UPDATE users SET class_id = 91 WHERE id = %s" % uid)
    t0 = login(user, pw)
    call("POST", "/me/password/change",
         {"old_password": pw, "new_password": pw + "x"}, token=t0)
    STOK = login(user, pw + "x")
    SPK = q("SELECT passkey FROM users WHERE id = %s" % uid)
    s, body, _, _cc = rss("forums=%s" % FID, STOK, SPK, sub="forum/")
    # class>=90 放行（修复判据）：网页能读的版块 RSS 也必须推——旧谓词
    # 只有 minclassread<=class，90<95 会把职员漏掉
    ok("G4 class 91 职员门槛版帖子在 RSS 里（can_read 放行口径复刻）",
       s == 200 and TT2 in body, "items=%d" % items_of(body))
    exec_sql("UPDATE users SET class_id = 1 WHERE id = %s" % uid)
    SPK2 = q("SELECT passkey FROM users WHERE id = %s" % uid)
    s, body, _, _cc = rss("forums=%s" % FID, STOK, SPK2, sub="forum/")
    ok("G4 class 1 成员门槛版 RSS 无该帖（反向：不放行全量）",
       s == 200 and TT2 not in body, "items=%d" % items_of(body))

print("== G6 NP 惯用参数名（批十一修复判据）==")
# 语料两颗都在分类 1；分类 24 里没有 agrss 语料——若 cat 被静默忽略，
# 两条断言会同为 2 条命中，红在这里
s, body, _, _cc = rss("search=agrss&cat=1", ROOT, PK)
ok("G6 cat=1 生效（语料两颗命中）", s == 200 and items_of(body) == 2,
   "items=%d" % items_of(body))
s, body, _, _cc = rss("search=agrss&cat=24", ROOT, PK)
ok("G6 cat=24 生效（该分类无语料 → 空集）",
   s == 200 and items_of(body) == 0, "items=%d" % items_of(body))
s, body, _, _cc = rss("search=agrss&categories=1", ROOT, PK)
ok("G6 本站名 categories 同时受理（正控不被别名打死）",
   s == 200 and items_of(body) == 2, "items=%d" % items_of(body))
s, body, _, _cc = rss("slot_number=3", ROOT, PK)
ok("G6 slot_number=3 限到 3 条（NP 条数名）",
   s == 200 and items_of(body) == 3, "items=%d" % items_of(body))
# 别名转发链：getrss.php?passkey=…&cat=24 经 302 落到真端点后仍生效
import urllib.request
al = BASE.replace("/api/v1", "") + (
    "/api/v1/compat/nexusphp/getrss.php?passkey=%s&cat=24&search=agrss" % PK)
try:
    r = urllib.request.urlopen(al, timeout=30)
    ab = r.read().decode("utf-8", "replace")
    asc = r.status
except urllib.error.HTTPError as e:
    ab, asc = e.read().decode("utf-8", "replace"), e.code
ok("G6 getrss.php 别名转发后 cat 仍生效（空集）",
   asc == 200 and items_of(ab) == 0, "http=%s items=%d" % (asc, items_of(ab)))

print("== G7 enclosure 端到端（订阅器只会照抄这个地址）==")
import html as _html
import re as _re
s, body, _, _cc = rss("showrows=2", ROOT, PK)
encs = [_html.unescape(x) for x in
        _re.findall(r'<enclosure url="([^"]+)"', body)]
ok("G7 feed 里每条都带 enclosure 下载直链", s == 200 and len(encs) == 2,
   "enclosures=%d" % len(encs))
if encs:
    try:
        rr = urllib.request.urlopen(encs[0], timeout=40)
        data, dst = rr.read(), rr.status
        dct = rr.headers.get("Content-Type")
        dcc = rr.headers.get("Cache-Control")
    except urllib.error.HTTPError as e:
        data, dst = b"", e.code
        dct = e.headers.get("Content-Type")
        dcc = e.headers.get("Cache-Control")
    ok("G7 enclosure 真下得到 .torrent（bencode 起手 + 正确内容类型）",
       dst == 200 and data[:1] == b"d" and "bittorrent" in (dct or ""),
       "http=%s bytes=%d ct=%s" % (dst, len(data), dct))
    ok("G7 下载响应 no-store（.torrent 内嵌本人 passkey）",
       dcc is not None and "no-store" in dcc.lower(), "cc=%r" % dcc)

print("== G9 Torznab：不支持的标识符不得顶掉能用的关键词 ==")
s, r = call("POST", "/me/tokens", {"name": "agtz %d" % int(time.time())},
            token=ROOT)
TZTOK = (r.get("data") or {}).get("token")
ok("G9 可签发 API Token（Torznab 鉴权入口）", s == 200 and TZTOK,
   "http=%s" % s)
TID = None
s2, lr = call("GET", "/me/tokens", token=ROOT)
rows = (lr.get("data") or {})
lst = rows.get("tokens") if isinstance(rows, dict) else rows
# 列表回显是掩码，不能拿 Token 明文去比；按本次签发用的 name 找 id
for it in (lst or []):
    if (it.get("name") or "").startswith("agtz"):
        TID = it.get("id")
CLEAN.append("__revoke_token__")

def tz(qs):
    rq = urllib.request.Request(BASE + "/torznab/search?" + qs)
    rq.add_header("Authorization", "Token " + (TZTOK or ""))
    try:
        return urllib.request.urlopen(rq, timeout=30).read().decode(
            "utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.read().decode("utf-8", "replace")

q_kw = "agrss"
a = tz("t=movie&q=%s" % q_kw).count("<item>")
b = tz("t=movie&q=%s&tmdbid=693113" % q_kw).count("<item>")
ok("G9 仅 q 的片名搜索有结果（正控）", a >= 2, "items=%s" % a)
ok("G9 同时带 tmdbid 不得把结果打成空（本批修复判据）", b == a,
   "只传q=%s 条 / 加 tmdbid=%s 条" % (a, b))
c = tz("t=movie&imdbid=tt0000000").count("<item>")
ok("G9 imdbid 仍走 IMDb 精确区（反向：不是一律忽略标识符）", c == 0,
   "items=%s" % c)
d_only = tz("t=movie&tmdbid=693113").count("<item>")
ok("G9 只给不支持的标识符时回空 feed 而非 50 条未筛（P-21 判据）",
   d_only == 0, "items=%s" % d_only)

print("== G8 /compat/meta 声明的每个口都真实存在（对外契约不虚报）==")
import re as _re2
s, resp = call("GET", "/compat/meta")
meta = (resp.get("data") or {}) if s == 200 else {}
eps = meta.get("endpoints") or {}
def first_tok(v):
    return _re2.split(r"[\s（(]", v.strip(), 1)[0]
gets = {k: first_tok(v) for k, v in eps.items()
        if isinstance(v, str) and v.strip().startswith("/api/v1")}
for k, v in (eps.get("aliases") or {}).items():
    if isinstance(v, str) and v.strip().startswith("/api"):
        gets["alias." + k] = first_tok(v)
missing = []
# {id} 要换成库里真实存在的过审种号：download.php 对不存在的种回 404 是
# 「资源没有」，不是「路由不存在」，拿 1 去探会把正确行为报成虚报
LIVE = q("SELECT COALESCE(max(id), 1)::text FROM torrents "
         "WHERE approval_status = 1")
for k, path in sorted(gets.items()):
    url = (path.replace("{passkey}", PK).replace("{id}", LIVE)
           .replace("{token}", "fxo_none").replace("{anchor}", "1"))
    rq = urllib.request.Request(BASE.replace("/api/v1", "") + url)
    try:
        st = urllib.request.urlopen(rq, timeout=25).status
    except urllib.error.HTTPError as e:
        st = e.code
    except Exception:
        st = 0
    # 401/400/429 都是「口在、只是要凭证」；404/1004 才是虚报
    if st in (0, 404, 405):
        missing.append("%s(%s→%s)" % (k, path, st))
ok("G8 声明的 GET 口全部在册（≥6 条，无 404）",
       len(gets) >= 6 and not missing,
       "checked=%d missing=%s" % (len(gets), missing or "无"))

print("== G5 收尾 ==")
for stmt in reversed(CLEAN):
    if stmt == "__revoke_token__":
        if TID:
            call("POST", "/me/tokens/revoke", {"id": TID}, token=ROOT)
        continue
    exec_sql(stmt)
ok("G5 语料无残留",
   q("SELECT count(*) FROM torrents WHERE name LIKE 'agrss%'") == "0",
   q("SELECT count(*) FROM torrents WHERE name LIKE 'agrss%'"))
ok("G5 探针版块无残留",
   q("SELECT count(*) FROM forums WHERE name = 'agrss 门槛版'") == "0",
   q("SELECT count(*) FROM forums WHERE name = 'agrss 门槛版'"))
summary()
