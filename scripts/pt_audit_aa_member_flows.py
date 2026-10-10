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
ok("A3 正控：自建认领行出现在缺省视图",
   bool(pres_tid) and int(pres_tid) in ids0,
   "pres_tid=%s ids0=%s" % (pres_tid, ids0[:6]))
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

try:
    summary()
finally:
    cleanup()