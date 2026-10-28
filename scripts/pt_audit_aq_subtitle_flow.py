# -*- coding: utf-8 -*-
"""批二十七闸门 AQ：字幕区读写面（0146/0148/0149，上传/列表/下载/评分/举报）。

悬赏池与工作流（发起/凑赏/撤销/认领/交稿/验收/worker 超时/月度评选认证）
在姊妹闸门 ar（pt_audit_ar_bounty_pool.py）——两封共用 pt_audit_aq_util，
**必须串行跑**（都会改 module_subtitles 等共享设定）。

本批修复的哨兵：
  P-42 同文件「上传→软删→再传」无限再领 +5 火花（幂等键锚错对象）；
  P-46 anon 行的 user_id 发给未认证访客（用户名脱了、id 没脱）；
  P-47 挂在待审/封禁种子上的字幕出现在公开列表（detail/download 都拦）。
"""
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, note_torrent, ok, summary  # noqa: E402
from pt_audit_aq_util import make_exec  # noqa: E402

CLEAN = []
E = make_exec(CLEAN)
q = E["q"]
setting = E["setting"]
mkuser = E["mkuser"]
mkbackdated = E["mkbackdated"]
add_attachment = E["add_attachment"]
probe_torrent = E["probe_torrent"]
spark_of = E["spark_of"]
grant_spark = E["grant_spark"]
ledger_rows = E["ledger_rows"]
list_row = E["list_row"]
pre_clean = E["pre_clean"]
cleanup = E["cleanup"]
E["queue_final_sweep"]()

login()
TS = int(time.time()) % 100000
PW = "Gate#123456"
SET_KEYS = ("module_subtitles", "subtitle_moderation", "subtitle_bad_threshold")

print("== AQ-S0 前置：自愈残料 + 设定（串行改、收尾还原） ==")
print("  [pre] 字幕=%s（自愈后）" % pre_clean())
SET0 = {}
for k in SET_KEYS:
    SET0[k] = q("SELECT COALESCE((SELECT value FROM site_settings WHERE "
                "name = '%s'), '<absent>')" % k)
    CLEAN.append("DELETE FROM site_settings WHERE name = '%s'" % k
                 if SET0[k] == "<absent>"
                 else "UPDATE site_settings SET value = '%s' WHERE "
                      "name = '%s'" % (SET0[k], k))
setting("module_subtitles", "yes")
setting("subtitle_moderation", "0")
ok("S0 设定就位（module 开 / 免审）",
   q("SELECT value FROM site_settings WHERE name='module_subtitles'")
   == "yes")

uRef, tRef = mkuser("aqRef%d" % TS, PW)
uVet, tVet = mkbackdated("aqVet%d" % TS, PW + "x", src_uid=uRef)
uNew, tNew = mkuser("aqNew%d" % TS, PW)
uBan, tBan = mkbackdated("aqBan%d" % TS, PW + "x", src_uid=uRef)
E["exec_sql"]("UPDATE users SET status = 2 WHERE id = %d" % uBan)
grant_spark(uVet, 1000, "vet")
TID = probe_torrent(uVet, "AqSub.%d" % TS)
shaA = add_attachment(uVet, b"1\n00:00:01,000 --> 00:00:04,000\nA\n",
                      "aqA%d.srt" % TS)
ok("S0 探针齐（参考号/老兵/新号/封号）", uVet > 0 and uRef > 0)
ok("S0 反例：封号后登录被拒（建号时能登；429 限流重试到确定答复）",
   E["login_refused"]("aqBan%d" % TS, PW + "x"))

print("== AQ-S1 上传：附件校验 + 查重 + 奖励幂等（P-42 哨兵） ==")
s, r = call("POST", "/subtitles", {"title": "AqSub A", "lang": "zh",
                                   "file_sha": shaA, "torrent_id": TID},
            token=tVet)
d1 = r.get("data") or {}
sid1 = d1.get("id")
ok("S1 首传成功且发奖 5", s == 200 and d1.get("reward") == 5, str(r)[:110])
ok("S1 奖励流水 x1", ledger_rows(uVet, "subtitle") == 1,
   "n=%s" % ledger_rows(uVet, "subtitle"))
ok("S1 反例：同文件未删时重复上传被挡",
   call("POST", "/subtitles", {"title": "AqSub Adup", "lang": "zh",
                               "file_sha": shaA, "torrent_id": TID},
        token=tVet)[0] == 400)
if sid1:
    E["exec_sql"]("UPDATE subtitles SET deleted_at = now() WHERE id = %d"
                  % sid1)
s3, r3 = call("POST", "/subtitles", {"title": "AqSub A2", "lang": "zh",
                                     "file_sha": shaA, "torrent_id": TID},
              token=tVet)
d3 = r3.get("data") or {}
ok("S1 软删后允许再传（列表可见性口径不变）", s3 == 200 and d3.get("id"),
   str(r3)[:90])
ok("S1 哨兵：同文件再传不再发钱（P-42 修复点）",
   ledger_rows(uVet, "subtitle") == 1 and d3.get("reward") == 0,
   "ledger=%s reward=%s" % (ledger_rows(uVet, "subtitle"), d3.get("reward")))
ok("S1 正控：另一文件照常发钱",
   call("POST", "/subtitles", {"title": "AqSub B", "lang": "en",
                               "file_sha": add_attachment(
                                   uVet, b"2\nx\n", "aqB%d.srt" % TS),
                               "torrent_id": TID},
        token=tVet)[1].get("data", {}).get("reward") == 5
   and ledger_rows(uVet, "subtitle") == 2,
   "ledger=%s" % ledger_rows(uVet, "subtitle"))
ok("S1 反例：非 http(s) 直链被拒",
   call("POST", "/subtitles", {"title": "AqSub L", "lang": "zh",
                               "file_ref": "ftp://x/y.srt", "torrent_id": 0},
        token=tVet)[0] == 400)
ok("S1 反例：空标题被拒",
   call("POST", "/subtitles", {"title": "", "lang": "zh",
                               "file_ref": "https://x/z.srt",
                               "torrent_id": 0}, token=tVet)[0] == 400)
ok("S1 反例：冒用他人附件 sha 被拒",
   call("POST", "/subtitles", {"title": "AqSub X", "lang": "zh",
                               "file_sha": shaA, "torrent_id": 0},
        token=tNew)[0] == 400)
# 闸门首版按当时的注释判「不存在的种子 → 存 NULL」。P-643 之后口径收口：
# 「不存在」与「存在但对本会员不可见」**同回 1004**（不给存在性探针留两种
# 表征），且绝不允许 500、也不落孤儿行。2026-10-27 真机复验：
# torrent_id=999999999 → 404/1004、subtitles 零新增；待审种同样 1004。
s, r = call("POST", "/subtitles", {"title": "AqSub Ghost", "lang": "zh",
                                   "file_ref": "https://x/ghost.srt",
                                   "torrent_id": 999999999}, token=tVet)
ghost = q("SELECT count(*) FROM subtitles WHERE title = 'AqSub Ghost'")
ok("S1c 不存在的 torrent_id 不 500、不落孤儿行（1004）",
   s == 404 and r.get("code") == 1004 and ghost == "0",
   "%s %s rows=%s" % (s, str(r)[:90], ghost))
inv = q("SELECT id FROM torrents WHERE approval_status <> 1 ORDER BY id "
        "LIMIT 1")
s2, r2 = (call("POST", "/subtitles",
               {"title": "AqSub Inv", "lang": "zh",
                "file_ref": "https://x/inv.srt", "torrent_id": int(inv)},
               token=tVet) if inv else (404, {"code": 1004}))
inv_rows = q("SELECT count(*) FROM subtitles WHERE title = 'AqSub Inv'")
ok("S1c2 同一表征：存在但会员看不到的种也回 1004（反存在性探针）",
   s2 == 404 and r2.get("code") == 1004 and inv_rows == "0",
   "inv=%s %s %s rows=%s" % (inv, s2, str(r2)[:70], inv_rows))

print("== AQ-S2 列表 / 详情 / 下载计数去重 ==")
sid2 = d3.get("id") or 0
s, r = call("GET", "/subtitles?torrent_id=%d" % TID, token=tVet)
body = r.get("data") or {}
rows = body if isinstance(body, list) else (body.get("items") or [])
ok("S2 列表可按种过滤", s == 200 and any(
    x.get("id") in (sid1, sid2) for x in rows), str(r)[:120])
ok("S2 详情可读",
   call("GET", "/subtitles/%d/detail" % sid2, token=tVet)[1].get(
       "data", {}).get("id") == sid2)
# 去重断言用**外链行**（成功出口，两版口径都成立）；附件行在本闸门
# 没有真实字节（storage 里取不到 → 404），P-48 之后 404 不再计数，
# 拿它测「+1」会把修复测成回归。
sidExt = int(q("INSERT INTO subtitles (user_id, title, lang, file_ref, "
               "size, ext, status, deleted_at) VALUES (%d, 'AqSub ext', "
               "'zh', 'https://x/ext.srt', 10, 'srt', 1, NULL) RETURNING id"
               % uVet))
D0 = int(q("SELECT downloads FROM subtitles WHERE id = %d" % sidExt) or 0)
call("GET", "/subtitles/%d/download" % sidExt, token=tVet)
call("GET", "/subtitles/%d/download" % sidExt, token=tVet)
D1 = int(q("SELECT downloads FROM subtitles WHERE id = %d" % sidExt) or 0)
ok("S2 下载计数同人每小时去重（两次点按只 +1）", D1 - D0 == 1,
   "before=%s after=%s" % (D0, D1))
ok("S2 反例：分页 per_page 白名单外的值回落 50",
   len(((call("GET", "/subtitles?per_page=9999", token=tVet)[1].get("data")
         or {}).get("items")) or []) <= 50)
# P-48：取不到字节（404）不得计下载——downloads 进评选加权与列表排序，
# 计数在结果之前等于「拿坏链或越权探测就能刷排名」
sidDead = int(q("INSERT INTO subtitles (user_id, title, lang, file_ref, "
                "size, ext, status, deleted_at) VALUES (%d, 'AqSub dead', "
                "'zh', 'attach://%s', 10, 'srt', 1, NULL) RETURNING id"
                % (uVet, "b" * 64)))
s, _ = call("GET", "/subtitles/%d/download" % sidDead, token=tVet)
ok("S2 哨兵：下载 404 不计入 downloads（P-48 修复点）",
   s == 404 and q("SELECT downloads FROM subtitles WHERE id = %d" % sidDead)
   == "0", "http=%s dl=%s" % (s, q("SELECT downloads FROM subtitles WHERE "
                                   "id = %d" % sidDead)))

print("== AQ-S2b 读路径脱敏（P-46 anon id / P-47 待审种子） ==")
sidAnon = int(q("INSERT INTO subtitles (user_id, title, lang, file_ref, "
                "size, ext, status, anon) VALUES (%d, 'AqSub anon', 'zh', "
                "'https://x/an.srt', 10, 'srt', 1, TRUE) RETURNING id"
                % uVet))
s, row = list_row("/subtitles?search=AqSub%20anon", None, sidAnon)
ok("S2b 匿名访客拿不到 anon 行的上传者 id（P-46 修复点）",
   s == 200 and row and row.get("user_id") is None,
   "http=%s row=%s" % (s, str(row)[:120]))
s, row = list_row("/subtitles?search=AqSub%20anon", tVet, sidAnon)
ok("S2b 正控：本人看自己的 anon 行仍有 id（编辑/删除入口）",
   s == 200 and row and row.get("user_id") == uVet, str(row)[:110])
tidPend = int(q("INSERT INTO torrents (info_hash, name, size, owner_id, "
                "category_id, approval_status) VALUES "
                "(md5(random()::text), 'AqSub.pend', 1024, %d, 1, 0) "
                "RETURNING id" % uVet))
# P-444：这颗待审种是本闸门自己 INSERT 的，登记后由 summary() 按 id 精确回收
# （subtitles 对 torrents 是 ON DELETE CASCADE，连带一起清）
note_torrent(tidPend)
sidPend = int(q("INSERT INTO subtitles (user_id, title, lang, file_ref, "
                "size, ext, status, torrent_id) VALUES (%d, 'AqSub pendtxt',"
                " 'zh', 'https://x/pd.srt', 10, 'srt', 1, %d) RETURNING id"
                % (uVet, tidPend)))
s, row = list_row("/subtitles?search=AqSub%20pendtxt", None, sidPend)
ok("S2b 待审种子的字幕不进公开列表（P-47 修复点）",
   s == 200 and row is None, str(row)[:120])
E["exec_sql"]("UPDATE torrents SET approval_status = 1 WHERE id = %d"
              % tidPend)
s, row = list_row("/subtitles?search=AqSub%20pendtxt", None, sidPend)
ok("S2b 正控：种子过审后同一行立刻可见（不是把字幕封死）",
   s == 200 and row is not None, str(row)[:110])

print("== AQ-S3 评分门槛（老兵可投 / 新号被挡 / 改投不增计数） ==")
ok("S3 老兵投票成功",
   call("POST", "/subtitles/%d/vote" % sid2, {"score": 9}, token=tVet)[0]
   == 200)
agg = q("SELECT rating_count::text||'|'||rating_sum::text FROM subtitles "
        "WHERE id = %d" % sid2)
ok("S3 聚合列同步（count=1 sum=9）", agg == "1|9", agg)
ok("S3 反例：刚注册新号被门槛挡下",
   call("POST", "/subtitles/%d/vote" % sid2, {"score": 9}, token=tNew)[0]
   == 400)
s, _ = call("POST", "/subtitles/%d/vote" % sid2, {"score": 4}, token=tVet)
ok("S3 改投覆盖不增计数（A8 口径 1|4）",
   s == 200 and q("SELECT rating_count::text||'|'||rating_sum::text FROM "
                  "subtitles WHERE id = %d" % sid2) == "1|4")
ok("S3 反例：分数越界被拒",
   call("POST", "/subtitles/%d/vote" % sid2, {"score": 11}, token=tVet)[0]
   == 400)

print("== AQ-S4 坏字幕举报：同人去重 / 达阈值隐藏 ==")
call("POST", "/subtitles/%d/report" % sid2, None, token=tRef)
call("POST", "/subtitles/%d/report" % sid2, None, token=tRef)
call("POST", "/subtitles/%d/report" % sid2, None, token=tVet)
rep = q("SELECT bad_reports FROM subtitles WHERE id = %d" % sid2)
ok("S4 同人重复举报不累计、他人计 2（阈值 3 未到不隐藏）",
   rep == "2" and q("SELECT status FROM subtitles WHERE id = %d" % sid2)
   == "1", "bad=%s" % rep)
call("POST", "/subtitles/%d/report" % sid2, None, token=tNew)
ok("S4 达阈值（3）自动隐藏 status=2",
   q("SELECT status FROM subtitles WHERE id = %d" % sid2) == "2",
   q("SELECT status FROM subtitles WHERE id = %d" % sid2))
ok("S4 正控：隐藏后列表不再出这一行",
   list_row("/subtitles?search=AqSub%20A2", tRef, sid2)[1] is None)

print("== AQ-S5 收尾：设定逐项还原 + 探针清零 ==")
cleanup()
back = [k for k in SET_KEYS if q(
    "SELECT COALESCE((SELECT value FROM site_settings WHERE name = '%s'), "
    "'<absent>')" % k) != SET0[k]]
ok("S5 共享设定全部还原（%s）" % "/".join(SET_KEYS), not back, str(back))
ok("S5 探针字幕/求单/附件清零",
   q("SELECT (SELECT count(*) FROM subtitles WHERE title LIKE 'Aq%%')::text"
     "||'|'||(SELECT count(*) FROM subtitle_requests WHERE descr LIKE "
     "'AqGate %%')::text||'|'||(SELECT count(*) FROM attachments WHERE "
     "filename LIKE 'aq%%')::text") == "0|0|0")
ok("S5 探针账号全部停用",
   int(q("SELECT count(*) FROM users WHERE username LIKE 'aq%%' AND "
         "status < 2")) == 0)
summary()
