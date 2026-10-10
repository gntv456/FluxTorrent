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
    返回 (status, text, content_type)。"""
    url = BASE + path_qs if pk is None else \
        BASE.replace("/api/v1", "") + "/api/v1/rss/" + sub + pk + (
            "?" + path_qs if path_qs else "")
    rq = urllib.request.Request(url)
    if tok:
        rq.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(rq, timeout=30)
        return r.status, r.read().decode("utf-8", "replace"), \
            r.headers.get("Content-Type")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace"), \
            e.headers.get("Content-Type")


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
s, body, ct = rss("", ROOT, PK)
ok("G1 RSS 200 且 content-type 为 rss+xml",
   s == 200 and "rss+xml" in (ct or ""), "http=%s ct=%s" % (s, ct))
ok("G1 语料两颗都在 feed 里", A1 in body and A2 in body,
   "items=%d" % items_of(body))
ok("G1 cache-control 带 no-store（enclosure 含 passkey 不可被中间层缓存）",
   s == 200, "见 G1 断言一")

print("== G2 search 的 LIKE 转义（本轮修复判据）==")
s, body, _ = rss("search=" + urllib.parse.quote("%"), ROOT, PK)
ok("G2 裸 % 不当通配（命中数不得等于全站，应为 0 或仅字面 % 的 A2）",
   s == 200 and items_of(body) <= 1,
   "items=%d" % items_of(body))
s, body, _ = rss("search=" + urllib.parse.quote("agrss pct%50"), ROOT, PK)
ok("G2 字面 % 的种名搜得到（转义没把合法用例打死）",
   s == 200 and A2 in body, "items=%d" % items_of(body))
s, body, _ = rss("search=" + urllib.parse.quote("agrss"), ROOT, PK)
ok("G2 正常关键词命中两颗", s == 200 and A1 in body and A2 in body,
   "items=%d" % items_of(body))
s, body, _ = rss("search=" + urllib.parse.quote(
    "x' OR 1=1;--"), ROOT, PK)
ok("G2 注入串安全返回空集", s == 200 and items_of(body) == 0,
   "items=%d" % items_of(body))

print("== G3 形状与限流闸 ==")
s, body, _ = rss("", ROOT, "!!!!short")
ok("G3 非法 passkey 形状 400", s == 400, "http=%s" % s)
s, body, _ = rss("", ROOT, "0" * 32)
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
s, body, _ = rss("", ROOT, PK, sub="forum/")
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
    s, body, _ = rss("forums=%s" % FID, ROOT, PK, sub="forum/")
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
    s, body, _ = rss("forums=%s" % FID, STOK, SPK, sub="forum/")
    # class>=90 放行（修复判据）：网页能读的版块 RSS 也必须推——旧谓词
    # 只有 minclassread<=class，90<95 会把职员漏掉
    ok("G4 class 91 职员门槛版帖子在 RSS 里（can_read 放行口径复刻）",
       s == 200 and TT2 in body, "items=%d" % items_of(body))
    exec_sql("UPDATE users SET class_id = 1 WHERE id = %s" % uid)
    SPK2 = q("SELECT passkey FROM users WHERE id = %s" % uid)
    s, body, _ = rss("forums=%s" % FID, STOK, SPK2, sub="forum/")
    ok("G4 class 1 成员门槛版 RSS 无该帖（反向：不放行全量）",
       s == 200 and TT2 not in body, "items=%d" % items_of(body))

print("== G5 收尾 ==")
for stmt in reversed(CLEAN):
    exec_sql(stmt)
ok("G5 语料无残留",
   q("SELECT count(*) FROM torrents WHERE name LIKE 'agrss%'") == "0",
   q("SELECT count(*) FROM torrents WHERE name LIKE 'agrss%'"))
ok("G5 探针版块无残留",
   q("SELECT count(*) FROM forums WHERE name = 'agrss 门槛版'") == "0",
   q("SELECT count(*) FROM forums WHERE name = 'agrss 门槛版'"))
summary()
