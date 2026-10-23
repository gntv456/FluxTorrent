# -*- coding: utf-8 -*-
"""批九闸门 F 的第二支（af2）：列表口的注入面 / 视角桶 / 参数容错信封。

从 pt_audit_af_search.py 拆出（500 行门禁）：facet/排序/翻页那支仍在那边，
这一支只管「同一只 GET /torrents 在恶意输入、另一种视角、非法参数下会不会
说谎」。语料自带一套最小的三颗（具名 / 匿名 / 名称里真含 %），不复用另一支
的上传矩阵——两支各自能独立跑，红账才分得清是谁的前提坏了。

判据纪律（[[feedback-verification-visible-layer]]）：每条「不该泄露/不该
当通配」都配一条正控（该有名字的必须有、真含 % 的必须搜得到），防止拿空集
当通过。

不许与 af_search 并发：两支都用 `afprobe` 前缀造语料，af 那边有一条
`name LIKE 'afprobe%%'` 计数为 5 的体检判据，撞在一起会红在对方的前提上
（活体闸门本来也不许并发，见 [[feedback-live-probe-rows-vs-worker-tick]]）。
"""
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


def exec_sql(sql_text, tuples_only=False):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent",
         "-tAc" if tuples_only else "-c", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    # 静默失败的 psql 会让「语料属性」停在默认值上，后续每条断言都变成
    # 对空集/全集的巧合判断（af 第一跑就是这么被骗过去的）
    if p.returncode:
        raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:220])
    return p.stdout.strip()


def q(sql_text):
    return exec_sql(sql_text, True)


def brief(resp):
    return "msg=%s code=%s" % (str(resp.get("message"))[:56],
                               resp.get("code"))


def upload(tok, name, **fields):
    base = {"name": name, "category_id": "1", "descr": "列表探针正文，"
            "长度足够过常规校验", "small_descr": "af2"}
    base.update(fields)
    req = urllib.request.Request(BASE + "/torrents?category_id=1",
                                 method="POST", data=multipart(
                                     base, make_torrent(name)))
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(req, timeout=40)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"raw": raw[:200].decode("utf-8", "replace")}


def member_user(tag):
    """后台建号 + 降到 class 1 + 改初始密码（成员视角要真成员）。"""
    user = "af2%s%d" % (tag, int(time.time()) % 100000)
    pw = "Gate#123456"
    s, r = call("POST", "/admin/adduser", {"username": user, "password": pw,
                                           "email": "%s@af2.local" % user},
                token=ROOT)
    if s != 200:
        return None, None, "adduser http=%s %s" % (s, brief(r))
    uid = q("SELECT id FROM users WHERE username = '%s'" % user)
    CLEAN.append("UPDATE users SET status = 2 WHERE id = %s" % uid)
    exec_sql("UPDATE users SET class_id = 1 WHERE id = %s" % uid)
    t0 = login(user, pw)
    s, r = call("POST", "/me/password/change",
                {"old_password": pw, "new_password": pw + "x"}, token=t0)
    if s != 200:
        return None, uid, "改密失败 http=%s %s" % (s, brief(r))
    return login(user, pw + "x"), uid, user


print("== G0 语料：具名 / 匿名 / 名称含 % 各一颗 ==")
MTOK, MID, MNAME = member_user("m")
ok("G0 成员探针号可准备", MTOK is not None and MID, MNAME)
SPEC = [
    ("afprobe xinghe 1080P", False, None),
    ("afprobe laoyouji S01", True, None),
    ("afprobe pct%10 x", False, None),
]
IDS = {}
for (nm, anon, _x) in SPEC:
    st, r = upload(ROOT, nm, anonymous="1" if anon else "0")
    tid = (r.get("data") or {}).get("id")
    if not tid:
        ok("G0 语料可上传（%s）" % nm, False, brief(r))
        continue
    IDS[nm] = tid
    CLEAN += ["DELETE FROM %s WHERE torrent_id = %d" % (t, tid)
              for t in ("tags", "torrent_sections", "comments", "snatches",
                        "bookmarks", "promotions")]
    CLEAN.append("DELETE FROM torrents WHERE id = %d" % tid)
A1 = IDS.get(SPEC[0][0])
A2 = IDS.get(SPEC[1][0])
A5 = IDS.get(SPEC[2][0])
ok("G0 三颗语料齐（缺一颗后续都不判别）", all([A1, A2, A5]), str(IDS))
if not all([A1, A2, A5]):
    for stmt in reversed(CLEAN):
        exec_sql(stmt)
    sys.exit("语料不全，闸门终止")
CORPUS = {A1, A2, A5}


LAST_ERR = [""]


def sel(params, tok=None, full=False):
    base = "limit=100&alive=0"
    if "search" not in params and "exclude" not in params:
        base += "&search=afprobe"
    # params 与基础串同名键不许被基础值顶掉，也不许重复出现——重复键在旧
    # 反序列化下是 400，会污染所有断言（af 的同一把尺）
    for kv in params.split("&"):
        k = kv.split("=")[0]
        base = "&".join(p for p in base.split("&")
                        if p.split("=")[0] != k)
    url = "/torrents?" + base + (("&" + params) if params else "")
    s, r = call("GET", url, token=tok or ROOT)
    if s != 200:
        LAST_ERR[0] = "http=%s %s" % (s, brief(r))
        return [] if full else frozenset()
    LAST_ERR[0] = ""
    d = r.get("data") or {}
    items = d.get("items") or []
    if full:
        return items
    return frozenset((x or {}).get("id") for x in items) & CORPUS


def eqs(a, b):
    return sorted(a or []) == sorted(b or [])


print("== G8 注入与 LIKE 转义 ==")
bad = urllib.parse.quote("x' OR 1=1;--")
got = sel("search=%s" % bad)
ok("G8 注入串安全返回空集（不炸也不放全表）",
   got == frozenset(), str(got))
got2 = sel("search=%s" % urllib.parse.quote("%"))
ok("G8 裸 % 不当通配：命中数不得等于全站（应为 0 或仅名称含 % 的 A5）",
   got2.issubset({A5}), str(got2))
got3 = sel("search=%s" % urllib.parse.quote("afprobe pct%10"))
ok("G8 名称里真含 % 的种能被搜到（转义没把合法用例打死）",
   eqs(got3, {A5}), str(got3))
# 精确模式同样要转义：旧实现把原串直接塞进 ILIKE，`%`/`_` 仍是通配，
# 「精确搜 %」会命中全站（现网红证：total 与不加筛选相同）。
got4 = sel("search=%s&search_mode=2" % urllib.parse.quote("%"))
ok("G8b 精确模式下裸 % 是字面量（旧行为当通配 ⇒ 命中全集）",
   got4 == frozenset(), str(got4))
got5 = sel("search=%s&search_mode=2" % urllib.parse.quote("afprobe pct%10 x"))
ok("G8b 正控：精确模式能搜到名称里真含 % 的整串",
   eqs(got5, {A5}), str(got5))
got6 = sel("search=%s&search_mode=2" % urllib.parse.quote("afprobe pct_10 x"))
ok("G8b 精确模式下 _ 不当单字符通配（不该命中 A5）",
   got6 == frozenset(), str(got6))

print("== G9 同一 URL 的两个缓存桶不得互串 ==")
# 列表层 SQL 对匿名种一律 CASE WHEN anonymous THEN NULL（list_noclamp_as），
# 站长也不例外——匿名豁免只在详情页 view_anonymous。所以正控不是
# 「站长看匿名种有名字」，而是「具名种在成员视角有名字」。
s, mrows = call("GET", "/torrents?limit=100&alive=0&search=afprobe",
                token=MTOK)
mitems = (mrows.get("data") or {}).get("items") or []


def by_id(items, tid):
    return next((x for x in items if (x or {}).get("id") == tid), {})


m2 = by_id(mitems, A2)
m1 = by_id(mitems, A1)
ok("G9 匿名种在成员视角不透露发布者名",
   not m2.get("owner_name"), "owner_name=%r" % (m2.get("owner_name"),))
ok("G9 正控：具名种 A1 在成员视角有发布者名（防恒空桶伪通过）",
   bool(m1.get("owner_name")), "owner_name=%r" % (m1.get("owner_name"),))
s, rrows = call("GET", "/torrents?limit=100&alive=0&search=afprobe",
                token=ROOT)
ritems = (rrows.get("data") or {}).get("items") or []
r1 = by_id(ritems, A1)
ok("G9 站长视角具名种同样有名字（两桶都活着）",
   bool(r1.get("owner_name")), "owner_name=%r" % (r1.get("owner_name"),))
s, r = call("GET", "/torrents?limit=100&approval=3&alive=0", token=MTOK)
its = (r.get("data") or {}).get("items") or []
ok("G9 成员的 approval=3 被吞掉（回收站不外泄）",
   s == 200 and not any((x or {}).get("approval_status") == 3
                        for x in its), "n=%d" % len(its))

print("== G10 参数容错与错误信封（本轮两处修复的判据面）==")
ok("G10 min_seeders=abc 不筛也不炸（宽松反序列化丢非法值）",
   eqs(sel("min_seeders=abc"), CORPUS), str(sel("min_seeders=abc")))
ok("G10 limit=abc 回退默认条数（集合仍等于全集）",
   eqs(sel("limit=abc"), CORPUS), str(sel("limit=abc")))


def probe_url(qs):
    rq = urllib.request.Request(BASE + "/torrents" + qs)
    rq.add_header("Authorization", "Bearer " + ROOT)
    try:
        rr = urllib.request.urlopen(rq, timeout=30)
        body = rr.read()
        return "http=%d ct=%s" % (rr.status,
                                  rr.headers.get("Content-Type"))
    except urllib.error.HTTPError as e:
        body = e.read()
        out = "http=%d ct=%s" % (e.code, e.headers.get("Content-Type"))
        try:
            out += " code=%s" % json.loads(body).get("code")
        except Exception:
            out += " raw=%r" % body[:80]
        return out


# 实测钉死（aa1010m）：重复键在 serde derive 层就整体拒绝（标量与 seq
# 字段一视同仁），「取最后」不可达——最终契约 = 400 + 统一 JSON 信封。
# 判据从「取最后」改成「信封可解」。
dup = probe_url("?limit=100&alive=0&search=afprobe&alive=2")
ok("G10 重复 alive 键 → 400 + JSON 信封 code=1002（不再裸 text/plain）",
   "http=400" in dup and "application/json" in dup
   and "code=1002" in dup, dup)
utf8 = probe_url("?limit=100&search=%FF%FE")
ok("G10 非法 UTF-8 搜索不炸（非 500，回统一信封；actix 宽容解码实测 200）",
   "http=500" not in utf8 and ("http=200" in utf8
                               or "code=" in utf8), utf8)


def probe_lang(lang):
    rq = urllib.request.Request(BASE + "/torrents?limit=2&limit=3")
    rq.add_header("Authorization", "Bearer " + ROOT)
    rq.add_header("Accept-Language", lang)
    try:
        urllib.request.urlopen(rq, timeout=30)
        return "http=200"
    except urllib.error.HTTPError as e:
        return "%d|%s" % (e.code, json.loads(e.read()).get("message") or "")


en = probe_lang("en")
ok("G10 拒单译文真烤进镜像（Accept-Language: en 回英文整句）",
   en.startswith("400|") and "Invalid list filter parameters" in en, en)
zh = probe_lang("zh-CN")
ok("G10 中文拒单同一断言面（zh-CN 回原句）",
   zh.startswith("400|") and "列表筛选参数格式无效" in zh, zh)

print("== G11 收尾 ==")
for stmt in reversed(CLEAN):
    exec_sql(stmt)
ok("G11 语料无残留",
   q("SELECT count(*) FROM torrents WHERE name LIKE 'afprobe%%'") == "0",
   q("SELECT count(*) FROM torrents WHERE name LIKE 'afprobe%%'"))
summary()
