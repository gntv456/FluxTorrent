# -*- coding: utf-8 -*-
"""闸门 AA：成员日常链路（H&R 总览 / 保种区四档视图 / 我的种子 / 求种板 / 字幕在身单上限）。

由来：2026-10-10 资深用户视角实测。六条断链里五条是「一行数据形态不合预期就炸整页」
或「筛选器选中即失效」——单测与类型检查都不会红，只有按真实动线打一遍才暴露。
前置行自己造、造完删，不依赖库里恰好有数据。

跑法：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_aa_member_flows.py
需要本地栈（flux-api / flux-postgres）。FLUX_API_BASE 可指向侧容器。
"""
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (call, clear_register_rl, login, ok,
                         summary)  # noqa: E402

DB = "fluxtorrent"


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         DB, "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip())
    return r.stdout.strip()


CLEANUP = []


def cleanup():
    for stmt in reversed(CLEANUP):
        try:
            psql(stmt)
        except BaseException as e:  # noqa: BLE001
            print("   ! 收尾失败: %s -> %s" % (stmt[:64], str(e)[:80]))


ROOT = login()

# ------------------------------------------------------ 前置：造现场条件

# 1) 一条「快照在、流水没」的 H&R 记录。义务存 hr_snapshots，上传/下载量
#    却读时从 snatches 现取（LEFT JOIN），清号/复活作业会把流水抽走。
pair = psql("SELECT s.user_id || ' ' || s.torrent_id FROM hr_snapshots s "
            "LEFT JOIN snatches sn ON sn.user_id = s.user_id "
            "AND sn.torrent_id = s.torrent_id WHERE sn.user_id IS NULL LIMIT 1")
if pair:
    print("（现场天然存在无流水快照 %s，直接当判据）" % pair)
else:
    cand = psql("SELECT u.id || ' ' || t.id FROM users u, torrents t "
                "WHERE u.id <> 1 AND t.approval_status = 1 "
                "AND NOT EXISTS (SELECT 1 FROM snatches sn "
                "WHERE sn.user_id = u.id AND sn.torrent_id = t.id) "
                "AND NOT EXISTS (SELECT 1 FROM hr_snapshots h "
                "WHERE h.user_id = u.id AND h.torrent_id = t.id) LIMIT 1")
    uid = tid = ""
    if cand:
        uid, tid = cand.split()
        psql("INSERT INTO hr_snapshots (user_id, torrent_id, required_seconds, "
             "deadline, seeded_seconds, status) VALUES (%s, %s, 432000, "
             "now() + interval '5 day', 0, 'open')" % (uid, tid))
        CLEANUP.append("DELETE FROM hr_snapshots WHERE user_id = %s "
                       "AND torrent_id = %s" % (uid, tid))
        print("（已造无流水快照 uid=%s tid=%s）" % (uid, tid))

# 2) 一条「已认领保种但无下载流水」的记录：/preserve/claim 不要求先下载，
#    所以这是正常动线可达状态；而「我的种子·保种中」把 snatch 派生列按非
#    Option 解码。
member = psql("SELECT id FROM users WHERE id <> 1 "
              "ORDER BY class_id, id LIMIT 1")
pres_tid = psql("SELECT t.id FROM torrents t WHERE t.approval_status = 1 "
                "AND NOT EXISTS (SELECT 1 FROM seed_preserve sp "
                "WHERE sp.torrent_id = t.id) LIMIT 1")
if member and pres_tid:
    psql("INSERT INTO seed_preserve (torrent_id, claimed_by, claimed_at) "
         "VALUES (%s, %s, now()) ON CONFLICT (torrent_id) DO UPDATE SET "
         "claimed_by = EXCLUDED.claimed_by, claimed_at = now(), "
         "exited_at = NULL" % (pres_tid, member))
    CLEANUP.append("DELETE FROM seed_preserve WHERE torrent_id = %s" % pres_tid)
else:
    member = pres_tid = ""

# 3) 一条自建求种（不依赖库里恰好有人求种）
stamp = int(time.time()) % 1000000
s, r = call("POST", "/requests",
            {"title": "AA闸门求种%d" % stamp, "descr": "闸门探针",
             "bounty": 0}, token=ROOT)
my_req = (r.get("data") or {}).get("id")
if my_req:
    CLEANUP.append("DELETE FROM requests WHERE id = %s" % my_req)

# ------------------------------------------------------------- 断言

print("A1 后台 H&R 总览：无流水快照不得炸整页")
s, r = call("GET", "/admin/hr/records?per_page=100", token=ROOT)
ok("A1 /admin/hr/records 200", s == 200 and r.get("code") == 0,
   "http=%s %s" % (s, str(r.get("message"))[:150]))
rows = ((r.get("data") or {}).get("rows")) or []
nulls = [x for x in rows if x.get("uploaded") is None]
ok("A1 正控：判据可判别（确有 uploaded=null 的行）", bool(nulls),
   "rows=%d null_flow=%d" % (len(rows), len(nulls)))

print("A8 种子导出：无主种子（owner_id IS NULL）不该让整页 500")
if int(psql("SELECT count(*) FROM torrents WHERE owner_id IS NULL") or 0) == 0:
    row0 = psql("SELECT id || ' ' || owner_id FROM torrents "
                "WHERE owner_id IS NOT NULL ORDER BY id LIMIT 1")
    if row0:
        oid0, oowner = row0.split()
        psql("UPDATE torrents SET owner_id = NULL WHERE id = %s" % oid0)
        CLEANUP.append("UPDATE torrents SET owner_id = %s WHERE id = %s"
                       % (oowner, oid0))
        print("（已把种子 %s 置为无主当判据）" % oid0)
per = int((call("GET", "/admin/export/torrents?page=1",
                token=ROOT)[1].get("data") or {}).get("per") or 50)
null_id = psql("SELECT min(id) FROM torrents WHERE owner_id IS NULL")
page = 1
if null_id:
    pos = int(psql("SELECT count(*) FROM torrents "
                   "WHERE id < %s" % null_id) or 0)
    page = pos // per + 1
s, r = call("GET", "/admin/export/torrents?page=%d" % page, token=ROOT)
d8 = r.get("data") or {}
ok("A8 /admin/export/torrents 200", s == 200 and r.get("code") == 0,
   "http=%s page=%d %s" % (s, page, str(r.get("message"))[:120]))
ok("A8 正控：该页确有 owner_id=null 的行",
   any(x.get("owner_id") is None for x in d8.get("items") or []),
   "page=%d items=%d" % (page, len(d8.get("items") or [])))

print("A2 保种区四档视图：信封同构（前端按 {items,total,stats} 解构）")
KEYS = {"items", "total", "stats", "page", "per_page"}
for st in ["?status=current", "?status=active", "?status=grace",
          "?status=expired"]:
    s, r = call("GET", "/preserve" + st, token=ROOT)
    d = r.get("data")
    shape = set(d.keys()) if isinstance(d, dict) else set()
    ok("A2 status=%s 信封同构" % (st.replace("&status=", "") or "缺省"),
       s == 200 and shape >= KEYS, "http=%s keys=%s" % (s, sorted(shape)))

print("A3 保种区分页与计数：page=1 即第一页，计数与视图同源")
d0 = call("GET", "/preserve", token=ROOT)[1].get("data") or {}
d1 = call("GET", "/preserve?page=1", token=ROOT)[1].get("data") or {}
ids0 = [x["torrent_id"] for x in d0.get("items") or []]
ids1 = [x["torrent_id"] for x in d1.get("items") or []]
ok("A3 page=1 与缺省同集且非空", bool(ids0) and ids0 == ids1,
   "default=%d page1=%d" % (len(ids0), len(ids1)))
# 正控必须按视图的真实排序取全集：`/preserve` 是
# `ORDER BY (exited_at IS NULL ? seeders : 0) ASC,
#  COALESCE(exited_at, created_at)
# DESC`（最缺人保种的排前面），所以一条 seeders=1 的行**本来就不在第一页**。
# 旧写法断言「在第一页」，早先只是碰巧成立——批一百九十七扫到 49710 就红了。
def all_pres_ids():
    ids = []
    page = 1
    while page <= 40:
        d = call("GET", "/preserve?page=%d" % page,
                 token=ROOT)[1].get("data") or {}
        got = [x["torrent_id"] for x in d.get("items") or []]
        ids += got
        per = int(d.get("per_page") or 50) or 50
        total = int(d.get("total") or 0)
        if not got or page * per >= total:
            break
        page += 1
    return ids


all_ids = all_pres_ids() if pres_tid else []
ok("A3 正控：自建认领行在保种区视图里查得到（按真实排序翻页取全集）",
   bool(pres_tid) and int(pres_tid) in all_ids,
   "pres_tid=%s 全集=%d 条" % (pres_tid, len(all_ids)))
dE = call("GET", "/preserve?status=expired",
          token=ROOT)[1].get("data") or {}
ok("A3 已移出视图计数不复用在保总数",
   dE.get("total") != d0.get("total") or not d0.get("total"),
   "expired_total=%s in_total=%s" % (dE.get("total"), d0.get("total")))

print("A4 我的种子·保种中：认领人无流水不得 500")
if member and pres_tid:
    s, r = call("GET", "/users/%s/torrentlist" % member, token=ROOT)
    ok("A4 /users/{id}/torrentlist 200", s == 200 and r.get("code") == 0,
       "http=%s %s" % (s, str(r.get("message"))[:150]))
    pres = ((r.get("data") or {}).get("preserved")) or []
    ok("A4 正控：认领的那条在列表里", any(
        p.get("torrent_id") == int(pres_tid) for p in pres),
        "preserved=%d" % len(pres))
else:
    ok("A4 前置（成员/种子）可构造", False,
       "member=%s tid=%s" % (member, pres_tid))

# A4b：SnatchRow 的 approval_status/deny_reason 只有 /me/torrentlist 的 uploads
# 查询会投影，公开资料那 6 条查询不投影——sqlx 对 Option 字段不容忍「列不存在」，
# 所以只要某档有行，整个资料页种子列表就 500。挑一个真有上传记录的用户验。
owner = psql("SELECT t.owner_id FROM torrents t WHERE t.approval_status = 1 "
             "AND t.owner_id IS NOT NULL AND NOT t.anonymous LIMIT 1")
if owner:
    s, r = call("GET", "/users/%s/torrentlist?limit=20" % owner, token=ROOT)
    ups = ((r.get("data") or {}).get("uploads")) or []
    ok("A4b 有上传记录的公开资料列表 200", s == 200 and r.get("code") == 0,
       "uid=%s http=%s %s" % (owner, s, str(r.get("message"))[:120]))
    ok("A4b 正控：uploads 非空（判据可判别）", bool(ups), "uploads=%d" % len(ups))
else:
    ok("A4b 前置（有上传记录的用户）可构造", False, "owner=%s" % owner)

print("A5 求种板：无数据源的计数不得伪装成 0")
items = call("GET", "/requests?finished=all", token=ROOT)[1].get("data") or []
ok("A5 自建求种出现在列表", any(x.get("id") == my_req for x in items),
   "my_req=%s n=%d" % (my_req, len(items)))
ok("A5 comments/bids 为 null（读作「—」而不是「无人应求」）",
   bool(items) and all(x.get("comments") is None and x.get("bids") is None
                       for x in items),
   str([(x.get("comments"), x.get("bids")) for x in items[:3]]))


def new_member(tag):
    """管理员发码 → 验证码 → 注册 → 登录，拿真实成员令牌（不借演示号）。"""
    clear_register_rl()
    s, r = call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                token=ROOT)
    codes = ((r.get("data") or {}).get("codes")) or []
    if not codes:
        return None, "发码失败 http=%s" % s
    code, cid = codes[0]["code"], codes[0]["id"]
    CLEANUP.append("DELETE FROM invites WHERE id = %d" % cid)
    s, r = call("GET", "/auth/captcha")
    cap = r.get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None, "验证码题目解析失败 http=%s %r" % (s, cap)
    user = "aa%s%d" % (tag, int(time.time()) % 100000)
    body = {"username": user, "email": "%s@aa.local" % user,
            "password": "password123", "invite_code": code,
            "captcha_id": cap.get("captcha_id"),
            "captcha_answer": int(nums[0]) + int(nums[1]),
            "captcha_token": "", "fields": {}}
    s, r = call("POST", "/auth/register", body)
    if s != 200:
        return None, "注册失败 http=%s %s" % (s, r.get("message"))
    new_id = (r.get("data") or {}).get("user_id")
    if new_id:
        # audit_log 是防篡改链（触发器拒绝删除），所以探针号不能真删——
        # 与站点自身口径一致地置 status=2（已注销），并清掉它的站内信。
        for stmt in ["DELETE FROM messages WHERE receiver_id = %s",
                     "DELETE FROM user_field_values WHERE user_id = %s",
                     "UPDATE users SET status = 2 WHERE id = %s"]:
            CLEANUP.append(stmt % new_id)
    s, r = call("POST", "/auth/login",
                {"username": user, "password": "password123"})
    if s != 200:
        return None, "新成员登录失败 http=%s" % s
    return r["data"]["token"], ""


print("A6 求字幕：在身 3 单上限要真的生效（曾因 SQL 里一个转义引号 fail-open）")
member_tok, why = new_member("sub")
if not member_tok:
    ok("A6 成员可准备", False, why)
else:
    wf = psql("SELECT COALESCE((SELECT value FROM site_settings "
              "WHERE name='subtitle_workflow'),'')")
    psql("INSERT INTO site_settings(name,value) "
         "VALUES('subtitle_workflow','1') "
         "ON CONFLICT (name) DO UPDATE SET value='1'")
    if wf:
        CLEANUP.append("UPDATE site_settings SET value='%s' "
                       "WHERE name='subtitle_workflow'" % wf)
    else:
        CLEANUP.append("DELETE FROM site_settings "
                       "WHERE name='subtitle_workflow'")
    ids = []
    for i in range(4):
        s, r = call("POST", "/subtitles/requests",
                    {"lang": "zh-CN", "descr": "AA闸门单%d" % stamp},
                    token=ROOT)
        rid = (r.get("data") or {}).get("id")
        if rid:
            ids.append(rid)
    CLEANUP.append("DELETE FROM subtitle_requests WHERE descr = "
                   "'AA闸门单%d'" % stamp)
    ok("A6 探针单建满 4 张", len(ids) == 4, "ids=%s" % ids)
    got = []
    for rid in ids:
        s, r = call("POST", "/subtitles/requests/%d/claim" % rid,
                    {"crew": []}, token=member_tok)
        got.append((rid, s, (r.get("message") or "")[:60]))
    ok("A6 前 3 单认领成功", sum(1 for g in got if g[1] == 200) == 3, str(got))
    ok("A6 第 4 单被在身单上限拒绝",
       sum(1 for g in got if g[1] != 200) == 1
       and any("3 单" in g[2] for g in got if g[1] != 200), str(got))

    # A7：用户中心「活跃会话/登录记录」两处读口。login_events.ip 是 inet，
    # 曾经直接投影给 String → sqlx 判类型不合，整卡 500。
    print("A7 会话与登录记录：inet 列要投影成文本，成员刚登录过必有行")
    s, r = call("GET", "/me/sessions", token=member_tok)
    rows = r.get("data") or []
    ok("A7 GET /me/sessions 200", s == 200 and r.get("code") == 0,
       "http=%s %s" % (s, str(r.get("message"))[:140]))
    ok("A7 正控：会话列表非空且 ip 是文本", bool(rows)
       and isinstance(rows[0].get("ip"), str), str(rows)[:160])
    s, r = call("GET", "/me/logins", token=member_tok)
    ok("A7 GET /me/logins 200", s == 200 and r.get("code") == 0,
       "http=%s %s" % (s, str(r.get("message"))[:140]))

print("A9 保种区：登录门 + 六格与列表同源 + 今日按站点日")
# P-518：`/preserve` 原先整口不验登录——同级 /resurrections、/torrents、/forums、
# /classes 匿名一律 401(2001)，而它回 200 外加 73 条在保种的名称/大小/做种数/
# 认领人用户名。判据拿同域那条当参照，两条一起看才不孤单。
s_an, r_an = call("GET", "/preserve")
s_ref, r_ref = call("GET", "/resurrections")
ok("A9 匿名打 /preserve 必须与 /resurrections 同形拒登（2001）",
   s_an == 401 and (r_an or {}).get("code") == 2001 and s_ref == 401,
   "preserve=%s/%s resurrections=%s/%s"
   % (s_an, (r_an or {}).get("code"), s_ref, (r_ref or {}).get("code")))
# P-519：六格旧写法直接数 seed_preserve 全表，列表与 total 却按
# approval_status=1 过滤 —— 挂一枚未过审的种进保种区，两边就该分叉。
rej_tid = psql("SELECT t.id FROM torrents t WHERE t.approval_status <> 1 "
               "AND NOT EXISTS (SELECT 1 FROM seed_preserve sp "
               "WHERE sp.torrent_id = t.id) LIMIT 1")
if rej_tid:
    psql("INSERT INTO seed_preserve (torrent_id, claimed_by, claimed_at) "
         "VALUES (%s, NULL, now())" % rej_tid)
    CLEANUP.append("DELETE FROM seed_preserve WHERE torrent_id = %s" % rej_tid)
    d9 = (call("GET", "/preserve", token=ROOT)[1].get("data") or {})
    st9 = d9.get("stats") or {}
    ok("A9b 六格「保种中」与列表 total 同数（被驳回/回收的种不许还计数）",
       str(st9.get("preserving")) == str(d9.get("total")),
       "preserving=%s total=%s 夹具 tid=%s"
       % (st9.get("preserving"), d9.get("total"), rej_tid))
else:
    print("   [WARN] 没有可当夹具的未过审种，A9b 没跑（没跑不等于通过）")
# P-520：「今日新增」旧写法是滚动 now() - interval '1 day'。把夹具挪到
# 「北京昨天 23:00」：它还在滚动 24h 窗口内（北京 0-22 点跑），却已在站点日
# 之前 ⇒ 按站点日口径必须少 1。北京 23-24 点这一小时两种口径同数，属已知窗口。
if pres_tid:
    base = int((((call("GET", "/preserve", token=ROOT)[1].get("data")
                   or {}).get("stats") or {}).get("today_in")) or 0)
    psql("UPDATE seed_preserve SET claimed_at = "
         "(date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') "
         "AT TIME ZONE 'Asia/Shanghai') - interval '1 hour' "
         "WHERE torrent_id = %s" % pres_tid)
    after = int((((call("GET", "/preserve", token=ROOT)[1].get("data")
                    or {}).get("stats") or {}).get("today_in")) or 0)
    ok("A9c 挪到站点日之前认领的种不算「今日新增」（六格按北京日界）",
       after == base - 1, "base=%s after=%s 夹具 tid=%s"
       % (base, after, pres_tid))

try:
    summary()
finally:
    cleanup()