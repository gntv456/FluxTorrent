# -*- coding: utf-8 -*-
"""批九闸门 F：种子列表的搜索 / 筛选 / 排序 / 翻页 / 缓存面。

列表是 PT 用户待得最久的地方，参数 30 多个，每个 facet 都必须「可判别」：
先造一套属性矩阵已知的探针种，再逐条断言精确 id 集合。同一 URL 在成员与站长
两桶里也不许互串。非法值类容错的判据都是「等于不加该参数」并配合法对照。

拆分（500 行门禁，本文件原 511 行）：G8 注入与 LIKE 转义 / G9 视角桶 /
G10 参数容错与错误信封 已搬去 pt_audit_af2_inject_envelope.py，那边自带一套
三颗的最小语料，两支可各自独立跑（不许并发：同名 afprobe 前缀会互相数到）。
"""
import datetime
import glob
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
    # 静默失败的 psql 会让「语料属性」停在默认值上，后续每条 facet 断言
    # 都变成对空集/全集的巧合判断（本闸门第一跑就是这么被骗过去的）
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
            "长度足够过常规校验", "small_descr": "af"}
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
    user = "af%s%d" % (tag, int(time.time()) % 100000)
    pw = "Gate#123456"
    s, r = call("POST", "/admin/adduser", {"username": user, "password": pw,
                                           "email": "%s@af.local" % user},
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


# ---------- 语料 ----------
DAY = datetime.date.today()


def dstr(off_days):
    return (DAY - datetime.timedelta(days=off_days)).isoformat()


print("== G0 造语料：5 颗属性矩阵已知的探针种 ==")
# 静态防呆：Rust 字符串行尾写成两个反斜杠**不会续行**，SQL 里会留下一个
# 真反斜杠——3ac09f0a 之前的 area=4 就是这样现网全 500（静态/真库判据全绿，
# 只有打路由才看得见）。单反斜杠才是续行。
_BAD_BS = []
for _p in glob.glob(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                 "..", "apps", "**", "*.rs"),
                    recursive=True):
    with open(_p, encoding="utf-8") as _f:
        for _i, _line in enumerate(_f, 1):
            if _line.rstrip("\r\n").endswith("\\\\"):
                _BAD_BS.append("%s:%d" % (_p, _i))
ok("G0s Rust 源无「双反斜杠续行」（SQL 里不许留真反斜杠）",
   not _BAD_BS, str(_BAD_BS[:5]))
MTOK, MID, MNAME = member_user("m")
ok("G0 成员探针号可准备", MTOK is not None and MID, MNAME)
AIR = q("SELECT id FROM section_dict WHERE kind = 'air_season' "
        "ORDER BY sort LIMIT 1")
SPEC = [
    ("afprobe xinghe 1080P", 5, 2, 3, 1 << 30, dstr(2), True, False,
     "tt1111111", "[3,10]", "{air:%s}" % AIR),
    ("afprobe xinghe 720P", 0, 0, 0, 2 << 30, dstr(10), False, True,
     None, "[10]", None),
    ("afprobe laoyouji S01", 9, 7, 100, 5 << 30, dstr(30), True, False,
     None, "[3]", None),
    ("afprobe boundary 2GB", 1, 0, 1, 2 << 30, dstr(2), False, False,
     None, None, None),
    ("afprobe pct%10 x", 3, 0, 0, 1 << 30, dstr(2), False, False,
     None, None, None),
]
IDS = {}
for (nm, sd, lc, cm, sz, cd, offl, anon, imdb, tags, sec) in SPEC:
    st, r = upload(ROOT, nm, tags=tags,
                   sections=json.dumps({"air_season": int(AIR)})
                   if sec else None,
                   imdb=imdb, anonymous="1" if anon else "0")
    tid = (r.get("data") or {}).get("id")
    if not tid:
        ok("G0 语料可上传（%s）" % nm, False, brief(r))
        continue
    IDS[nm] = tid
    CLEAN += ["DELETE FROM %s WHERE torrent_id = %d" % (t, tid)
              for t in ("tags", "torrent_sections", "comments", "snatches",
                        "bookmarks", "promotions")]
    CLEAN.append("DELETE FROM torrents WHERE id = %d" % tid)
    exec_sql("UPDATE torrents SET size = %d, seeders = %d, leechers = %d, "
             "times_completed = %d, created_at = timestamp '%s 12:00:00', "
             "official_tag = %s, anonymous = %s WHERE id = %d"
             % (sz, sd, lc, cm, cd, "TRUE" if offl else "FALSE",
                "TRUE" if anon else "FALSE", tid))
A1 = IDS.get(SPEC[0][0])
A2 = IDS.get(SPEC[1][0])
A3 = IDS.get(SPEC[2][0])
A4 = IDS.get(SPEC[3][0])
A5 = IDS.get(SPEC[4][0])
ok("G0 五颗语料齐（缺一颗后续都不判别）",
   all([A1, A2, A3, A4, A5]), str(IDS))
if not all([A1, A2, A3, A4, A5]):
    sys.exit("语料不全，闸门终止")
# 体检：属性真的落库了吗。size 停在上传解析出的 4096、或 created_at 仍是今天，
# 都说明后面的数值/日期 facet 是在对一堆默认值做「巧合判断」。
N_AF = q("SELECT count(*) FROM torrents WHERE name LIKE 'afprobe%'")
STUB = q("SELECT count(*) FROM torrents WHERE name LIKE 'afprobe%' "
         "AND size = 4096")
TODAY = q("SELECT count(*) FROM torrents WHERE name LIKE 'afprobe%' "
          "AND created_at::date = current_date")
ok("G0 语料属性真的落库（size 与 created_at 都没停在默认值）",
   N_AF == "5" and STUB == "0" and TODAY == "0",
   "n=%s size停4096=%s 日期停今天=%s" % (N_AF, STUB, TODAY))
# A3 归成员（发布者维度与 mine 要用成员视角）
exec_sql("UPDATE torrents SET owner_id = %s WHERE id = %d" % (MID, A3))
# A3 挂一条生效中的免费促销
exec_sql("INSERT INTO promotions(scope,kind,torrent_id,starts_at,ends_at) "
         "VALUES ('torrent','free',%d,now() - interval '1 hour',"
         "now() + interval '1 day')" % A3)
# A2 两条评论（sort=comments 用）
for i in (1, 2):
    call("POST", "/torrents/%s/comments" % A2,
         {"body": "af 评论 %d，内容足够长" % i}, token=MTOK or ROOT)
# 成员与语料的做种关系：A1 在作种 / A2 在下载 / A3 已完成 / A4 未完整有流量
exec_sql("INSERT INTO snatches(user_id,torrent_id,seeding,leeching,"
         "uploaded,downloaded,progress) VALUES "
         "(%s,%d,true,false,0,0,10000),(%s,%d,false,true,0,1024,5000),"
         "(%s,%d,false,false,0,4096,10000),(%s,%d,false,false,0,2048,3000)"
         % (MID, A1, MID, A2, MID, A3, MID, A4))
q("UPDATE snatches SET completed_at = now() WHERE user_id = %s "
  "AND torrent_id = %d" % (MID, A3))
call("PUT", "/torrents/%s/bookmark" % A4, {"on": True}, token=MTOK)

CORPUS = {A1, A2, A3, A4, A5}


LAST_ERR = [""]


def sel(params, tok=None, full=False):
    base = "limit=100&alive=0"
    if "search" not in params and "exclude" not in params:
        base += "&search=afprobe"
    # params 与基础串同名键（limit/alive 的容错用例）不许被基础值顶掉，
    # 也不许重复出现——重复键在旧反序列化下是 400，会污染所有断言
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


print("== G1 关键词搜索：范围与模式 ==")
ok("G1 search=afprobe 命中全部 5 颗", eqs(sel(""), CORPUS), str(sel("")))
ok("G1 两词 AND 模糊（afprobe xinghe）→ {A1,A2}",
   eqs(sel("search=afprobe%20xinghe"), {A1, A2}), str(
       sel("search=afprobe%20xinghe")))
ok("G1 search_mode=2 精确整串只命中 1 颗",
   eqs(sel("search=afprobe%20xinghe%201080P&search_mode=2"), {A1}),
   str(sel("search=afprobe%20xinghe%201080P&search_mode=2")))
ok("G1 exclude 排除 laoyouji → 4 颗且不含 A3",
   eqs(sel("exclude=laoyouji"), {A1, A2, A4, A5}), str(
       sel("exclude=laoyouji")))
ok("G1 search_area=3 按发布者命中 A3（正控：不含自己发的 A1）",
   eqs(sel("search_area=3&search=afm"), {A3}), str(
       sel("search_area=3&search=afm")))
_IMDB_HIT = sel("search_area=4&search=tt1111111")
ok("G1 search_area=4 按 IMDb 编号命中 A1（发种填的 IMDb 真能搜到）",
   eqs(_IMDB_HIT, {A1}) and not LAST_ERR[0],
   "err=%s items=%s" % (LAST_ERR[0] or "-", str(_IMDB_HIT)))

print("== G2 数值与日期 facet（含边界与容错）==")
B2 = 2 << 30
ok("G2 size_min 含边界（正好 2GiB 的 A2/A4 在里）",
   eqs(sel("size_min=%d" % B2), {A2, A3, A4}), str(sel("size_min=%d" % B2)))
ok("G2 带单位写法与字节写法同集合（parse_size 一致）",
   eqs(sel("size_min=2GB"), sel("size_min=%d" % B2)),
   str(sel("size_min=2GB")))
ok("G2 size_max 含边界 → {A1,A2,A4,A5}",
   eqs(sel("size_max=%d" % B2), {A1, A2, A4, A5}), str(
       sel("size_max=%d" % B2)))
ok("G2 date_from 只回近三天发的三颗",
   eqs(sel("date_from=%s" % dstr(3)), {A1, A4, A5}), str(
       sel("date_from=%s" % dstr(3))))
ok("G2 date_to 含边界当天 → 更早的两颗",
   eqs(sel("date_to=%s" % dstr(3)), {A2, A3}), str(
       sel("date_to=%s" % dstr(3))))
ok("G2 非法日期必须 400（批七十五 P-279；旧判据「不筛 = 返回全站」正是那个洞，"
   "同时保证不打成 500）",
   sel("date_from=2025-13-45") == frozenset()
   and "http=400" in LAST_ERR[0], LAST_ERR[0])
ok("G3 min_seeders=5 → {A1,A3}", eqs(sel("min_seeders=5"), {A1, A3}),
   str(sel("min_seeders=5")))
ok("G3 max_seeders=1 含边界 → {A2,A4}", eqs(sel("max_seeders=1"), {A2, A4}),
   str(sel("max_seeders=1")))
ok("G3 min_completed=100 → A3", eqs(sel("min_completed=100"), {A3}),
   str(sel("min_completed=100")))

print("== G4 布尔与促销 facet ==")
ok("G4 official=1 → {A1,A3}", eqs(sel("official=1"), {A1, A3}),
   str(sel("official=1")))
ok("G4 official=0 是补集而不是全集", eqs(sel("official=0"), {A2, A4, A5}),
   str(sel("official=0")))
ok("G4 anonymous=1 → A2", eqs(sel("anonymous=1"), {A2}),
   str(sel("anonymous=1")))
ok("G4 anonymous=2 具名 → 4 颗", eqs(sel("anonymous=2"), {A1, A3, A4, A5}),
   str(sel("anonymous=2")))
ok("G4 promo=free 命中挂了促销的 A3", eqs(sel("promo=free"), {A3}),
   str(sel("promo=free")))
ok("G4 promo=any 同样命中（促销存在即算）", eqs(sel("promo=any"), {A3}),
   str(sel("promo=any")))
ok("G4 promo=none 是补集", eqs(sel("promo=none"), {A1, A2, A4, A5}),
   str(sel("promo=none")))
ok("G4 未知 promo 值必须 400（批七十四 P-277 收紧；此处旧判据「不筛 = 返回"
   "全站」正是那个洞，2026-10-14 随实现改判据）",
   sel("promo=zzz") == frozenset() and "http=400" in LAST_ERR[0],
   LAST_ERR[0])

print("== G5 标签与维度 ==")
ok("G5 tag_ids 多选 any → {A1,A2,A3}",
   eqs(sel("tag_ids=3,10&tag_mode=any"), {A1, A2, A3}),
   str(sel("tag_ids=3,10&tag_mode=any")))
ok("G5 tag_mode=all 只留同时有两个标的 A1",
   eqs(sel("tag_ids=3,10&tag_mode=all"), {A1}),
   str(sel("tag_ids=3,10&tag_mode=all")))
ok("G5 旧单值键 tag_id 兼容", eqs(sel("tag_id=3"), {A1, A3}),
   str(sel("tag_id=3")))
ok("G5 不存在的标签 id → 空集（不放行全部）",
   sel("tag_ids=999999") == frozenset(), str(sel("tag_ids=999999")))
ok("G5 维度筛选 sec_air_season 命中挂了值的 A1",
   eqs(sel("sec_air_season=%s" % AIR), {A1}), str(
       sel("sec_air_season=%s" % AIR)))
ok("G5 组合：分类 + 标签 + 存活（AND 语义正确）",
   eqs(sel("category_id=1&tag_ids=3&alive=1"), {A1, A3}),
   str(sel("category_id=1&tag_ids=3&alive=1")))

print("== G6 存活与「我相关」视角 ==")
ok("G6 alive=1 只回活种 → 去掉 0 做种的 A2",
   eqs(sel("alive=1&search=afprobe"), {A1, A3, A4, A5}),
   str(sel("alive=1&search=afprobe")))
ok("G6 alive=2 只回断种 → A2", eqs(sel("alive=2&search=afprobe"), {A2}),
   str(sel("alive=2&search=afprobe")))
ok("G6 include_dead=1 与 alive=0 同集合（两条路一致）",
   eqs(sel("include_dead=1"), CORPUS), str(sel("include_dead=1")))
ok("G6 mine=1 成员只看到自己发的 A3",
   eqs(sel("mine=1", tok=MTOK), {A3}), str(sel("mine=1", tok=MTOK)))
ok("G6 bookmarked=1 只看到自己书签的 A4",
   eqs(sel("bookmarked=1", tok=MTOK), {A4}), str(
       sel("bookmarked=1", tok=MTOK)))
for label, want in (("seeding", {A1}), ("leeching", {A2}),
                    ("completed", {A3}),
                    # incomplete = 有下载量但没 completed_at：A2(1024) 和 A4(2048)
                    ("incomplete", {A2, A4})):
    ok("G6 status=%s 按查看者自己的记录命中" % label,
       eqs(sel("status=%s" % label, tok=MTOK), want),
       str(sel("status=%s" % label, tok=MTOK)))
ok("G6 status=notseeding 是「我没有在作种」的四颗（含从未下过的）",
   eqs(sel("status=notseeding", tok=MTOK), {A2, A3, A4, A5}),
   str(sel("status=notseeding", tok=MTOK)))
ok("G6 正控：站长视角无做种记录 → status=seeding 应为空集"
   "（viewer_sql 恒空的老形状只能这样钉）",
   sel("status=seeding") == frozenset(), str(sel("status=seeding")))
ok("G6 正控：站长视角 status=notseeding 应看到全部 5 颗",
   eqs(sel("status=notseeding"), CORPUS), str(sel("status=notseeding")))

print("== G7 排序与翻页 ==")
rows = sel("sort=seeders", full=True)
seq = [x.get("seeders") for x in rows if (x or {}).get("id") in CORPUS]
ok("G7 sort=seeders 单调不增", seq == sorted(seq, reverse=True), str(seq))
BEFORE_FIRST = (sel("", full=True) or [{}])[0].get("id")
# 走后台批量口而不是直改库：批量口会推进列表缓存代际，psql 改完读到的
# 可能还是上一代缓存，判据就分不清「没生效」与「没刷新」。
call("POST", "/admin/torrents/batch",
     {"action": "sticky", "ids": [A2], "pos_state": 1}, token=ROOT)
first = (sel("", full=True) or [{}])[0].get("id")
ok("G7 给 A2 打一级置顶后它排到首位（消费点=列表 sticky_rank）",
   first == A2 and BEFORE_FIRST != A2,
   "before=%s after=%s" % (BEFORE_FIRST, first))
call("POST", "/admin/torrents/batch",
     {"action": "sticky", "ids": [A2], "pos_state": 0}, token=ROOT)
seen, cur, pages = set(), None, 0
while pages < 6:
    p = "limit=2&search=afprobe&alive=0" + ("&cursor=" + urllib.parse.quote(
        str(cur)) if cur else "")
    s, r = call("GET", "/torrents?" + p, token=ROOT)
    d = r.get("data") or {}
    got = [x.get("id") for x in (d.get("items") or [])]
    if s != 200:
        break
    seen |= set(got) & CORPUS
    cur = d.get("next_cursor")
    pages += 1
    if not cur:
        break
ok("G7 游标翻页三页并起来覆盖 5 颗且不重复", len(seen) == 5 and pages >= 2,
   "pages=%s seen=%s" % (pages, len(seen)))


def page_of(cur=None, direction=None):
    # 默认视图只回活种，不带 alive=0 的话死种 A2 永远进不了翻页窗口
    p = "/torrents?limit=2&search=afprobe&alive=0"
    if cur:
        p += "&cursor=%s" % urllib.parse.quote(str(cur))
    if direction:
        p += "&dir=%s" % direction
    s, r = call("GET", p, token=ROOT)
    d = r.get("data") or {}
    return [x.get("id") for x in (d.get("items") or [])], d.get(
        "next_cursor")


p1, c1 = page_of()
p2, c2 = page_of(c1)
p3, _ = page_of(c2)
prev2, _ = page_of(c2, "prev")
prev1, _ = page_of(c1, "prev")
ok("G7 三页互不重叠", not (set(p1) & set(p2)) and not (set(p2) & set(p3)),
   "p1=%s p2=%s p3=%s" % (p1, p2, p3))
# dir=prev 锚定语义（0170，实测钉死）：游标锚在某行 ⇒ 回「以该行结尾的一页」
# （锚行属上一页，cursor.rs tie_inclusive）。客户端从第二页回翻时传的是
# 第一页的末游标 c1 —— 判据用它，不用 c2。
ok("G7 dir=prev 用第一页末游标回翻得到第一页（锚行属上一页）",
   c1 and set(prev1) == set(p1), "p1=%s prev1=%s" % (p1, prev1))
ok("G7 dir=prev 用第二页末游标得到第二页（同一语义的另一面）",
   c2 and set(prev2) == set(p2), "p2=%s prev2=%s" % (p2, prev2))

print("== G11 收尾 ==")
for stmt in reversed(CLEAN):
    exec_sql(stmt)
ok("G10 语料无残留",
   q("SELECT count(*) FROM torrents WHERE name LIKE 'afprobe%%'") == "0",
   q("SELECT count(*) FROM torrents WHERE name LIKE 'afprobe%%'"))
summary()
