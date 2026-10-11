# -*- coding: utf-8 -*-
"""批二十五闸门 AN：组队契约端到端（0102/0103 社交层，worker 真跑）。

此前只有开发期一次性实测（_doc/契约失败流转与信誉.md 第 4 节），
无回归守卫。本闸门把三条流转 + 一条模块回归全部钉住：

  S1 到期前达标 → settle：按贡献占比分账（余数落末位）、信誉 +20、
     荣誉落库、resurrections→done、重跑零双发（幂等键）；
  S2 中途退出 → leave：队长不可退、退出扣 15、退出者不再分账、
     且在期限未到时 expire 不得抢跑；
  S3 到期未达标 → expire：CAS 判失败、信誉 -5、resurrection→expired、
     不连坐（不扣钱）、与「恰在期限达标」互斥（S1 已结算队伍不被 expire）。
  S5 P-38 回归哨兵：模块关着（module_social=no）settle/expire 仍必须
     真跑——契约收尾不受模块开关拦；同日建队 API 被拒（关模块拦的是
     「办新契约」，不是「收尾旧契约」）。

判据纪律：每条「没有」都配正控（该有的出口必须先为真），
出账断言把「余额增量」与「流水金额」两处一起钉。收尾原样还原
module_social（测前 no）并删净探针数据（rescue_honor 的 team_id 是
ON DELETE SET NULL——删队伍不会带走荣誉行，必须显式清理）。
"""
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOT = login()
CLEAN = []
TS = int(time.time()) % 100000
PW = "Gate#123456"
# 探针小队（首名为队长）
G1 = ["anA%d" % TS, "anB%d" % TS, "anC%d" % TS]   # S1 达标结算
G2 = ["anD%d" % TS, "anE%d" % TS]                  # S2 退出
G3 = ["anF%d" % TS, "anG%d" % TS]                  # S3 到期失败
G4 = ["anH%d" % TS, "anI%d" % TS]                  # S5 模块关着也要收尾


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


def setting(name, value):
    exec_sql("INSERT INTO site_settings (name, value) VALUES ('%s', '%s') "
             "ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value"
             % (name, value))


def uid_of(name):
    return int(q("SELECT id::text FROM users WHERE username = '%s'" % name))


def mkuser(name):
    """建探针成员（管理组建号 → 改临时密码 → 可登录 token）。"""
    call("POST", "/admin/adduser", {"username": name, "password": PW,
                                    "email": "%s@an.local" % name},
         token=ROOT)
    u = uid_of(name)
    tok = login(name, PW)
    call("POST", "/me/password/change", {"old_password": PW,
                                         "new_password": PW + "x"},
         token=tok)
    CLEAN.append("UPDATE users SET status = 2 WHERE id = %d" % u)
    # 结算会让探针达标成就（rescue_1 等）——若闸门慢跑撞上整点
    # achievement_grant，ua 行会留下（流水/私信由各 CLEAN 覆盖，ua 单独列）
    CLEAN.append("DELETE FROM user_achievements WHERE user_id = %d" % u)
    return u, login(name, PW + "x")


def run_job(job, wait_s=170):
    """入队并等 worker 本体真跑一轮（判据钉 last_started_at 前移）。"""
    t0 = q("SELECT to_char(now(), 'YYYY-MM-DD\"T\"HH24:MI:SS.US')")
    call("POST", "/admin/jobs/run", {"job": job}, token=ROOT)
    deadline = time.time() + wait_s
    while time.time() < deadline:
        row = q("SELECT to_char(last_started_at, "
                "'YYYY-MM-DD\"T\"HH24:MI:SS.US')||'|'||"
                "COALESCE(last_ok::text,'-')||'|'||"
                "LEFT(COALESCE(last_result,''),60) FROM job_status "
                "WHERE job = '%s'" % job)
        if "|" in row:
            started, okv, res = row.split("|", 2)
            if started > t0:
                return okv == "true", "started=%s %s" % (started, res)
        time.sleep(4)
    return None, "等 worker 本轮执行超时（%ss）" % wait_s


def probe_torrent(owner_id, tag):
    """直接建一颗探针种（本闸门不打 tracker，纯链上数据）。"""
    name = "AnTeam.%s.%d" % (tag, TS)
    tid = int(q(
        "INSERT INTO torrents (info_hash, name, size, owner_id, "
        "category_id, approval_status) VALUES (md5(random()::text), "
        "'%s', 1024, %d, 1, 1) RETURNING id" % (name, owner_id)))
    CLEAN.append("DELETE FROM torrents WHERE id = %d" % tid)
    return tid


def snatch_touch(u, tid, seed_secs=1000, seeding=True):
    """造/改探针 snatches 行（契约贡献口径=该行 seeded_seconds 增量）。"""
    exec_sql(
        "INSERT INTO snatches (user_id, torrent_id, downloaded, "
        "seeded_seconds, seeding, completed_at) "
        "VALUES (%d, %d, 1024, %d, %s, now()) "
        "ON CONFLICT (user_id, torrent_id) DO UPDATE SET "
        "seeded_seconds = EXCLUDED.seeded_seconds, seeding = EXCLUDED.seeding"
        % (u, tid, seed_secs, "TRUE" if seeding else "FALSE"))
    CLEAN.append("DELETE FROM snatches WHERE user_id = %d AND torrent_id = %d"
                 % (u, tid))


def cleanup():
    for stmt in reversed(CLEAN):
        try:
            exec_sql(stmt)
        except Exception as e:  # noqa: BLE001
            print("  [cleanup-skip] %s: %s" % (stmt[:60], str(e)[:120]))


print("== S0 前置：module_social 临时开 + 探针成员/种子 ==")
MOD0 = q("SELECT COALESCE((SELECT value FROM site_settings "
         "WHERE name = 'module_social'), '<absent>')")
setting("module_social", "yes")
if MOD0 == "<absent>":
    CLEAN.append("DELETE FROM site_settings WHERE name = 'module_social'")
else:
    CLEAN.append("UPDATE site_settings SET value = '%s' "
                 "WHERE name = 'module_social'" % MOD0)
ok("S0 module_social 已开（收尾还原 %s）" % MOD0,
   q("SELECT value FROM site_settings WHERE name = 'module_social'") == "yes")

# 门槛压低到 1 小时：缺省 240h=864000s 增量，探针造不出也等不起；
# 这里测的是「达标→结算」链路，不是门槛本身（建队时按现值写进任务）。
RH0 = q("SELECT COALESCE((SELECT value::text FROM site_settings WHERE "
        "name = 'resurrection_hours'), '<absent>')")
setting("resurrection_hours", "1")
if RH0 == "<absent>":
    CLEAN.append("DELETE FROM site_settings WHERE name = 'resurrection_hours'")
else:
    CLEAN.append("UPDATE site_settings SET value = '%s' "
                 "WHERE name = 'resurrection_hours'" % RH0)

users = {}
for name in G1 + G2 + G3:
    u, tok = mkuser(name)
    users[name] = (u, tok)
uA, tA = users[G1[0]]
uB, tB = users[G1[1]]
uC, tC = users[G1[2]]
uD, tD = users[G2[0]]
uE, tE = users[G2[1]]
uF, tF = users[G3[0]]
uG, tG = users[G3[1]]

T1 = probe_torrent(uD, "s1")   # owner 是探针自己人；队长不能用自己发布的种
T2 = probe_torrent(uF, "s2")
T3 = probe_torrent(uD, "s3")   # T3 owner=uD：与 T1 同 owner，不同种，OK

for u, tid in ((uA, T1), (uB, T1), (uC, T1),
               (uD, T2), (uE, T2), (uF, T3), (uG, T3)):
    snatch_touch(u, tid)

RH = q("SELECT COALESCE((SELECT value::int FROM site_settings WHERE "
       "name = 'resurrection_hours'), 240)")
ok("S0 探针齐（7 成员/3 种，required_hours=%s）" % RH,
   len(users) == 7 and int(RH) > 0)

print("== S1 到期前达标：settle 分账 + 荣誉 + 信誉 + 幂等 ==")
s, r = call("POST", "/social/team/create", {"torrent_id": T1}, token=tA)
d = r.get("data") or {}
TEAM1 = d.get("team_id")
ok("S1 建队成功（认领复活任务、队长基线快照）",
   s == 200 and TEAM1, "http=%s %s" % (s, str(r)[:100]))
if TEAM1:
    CLEAN.append("DELETE FROM resurrections WHERE team_id = %d" % TEAM1)
    CLEAN.append("DELETE FROM rescue_honor WHERE team_id = %d" % TEAM1)
    CLEAN.append("DELETE FROM social_reputation WHERE uid IN (%d,%d,%d)"
                 % (uA, uB, uC))
    CLEAN.append("DELETE FROM messages WHERE receiver_id IN (%d,%d,%d)"
                 % (uA, uB, uC))
    CLEAN.append("DELETE FROM spark_ledger WHERE user_id IN (%d,%d,%d)"
                 % (uA, uB, uC))
    CLEAN.append("DELETE FROM social_team WHERE id = %d" % TEAM1)

s2, r2 = call("POST", "/social/team/create", {"torrent_id": T1}, token=tB)
ok("S1 同种重复建队被拒（torrent_id 唯一）", s2 == 400, str(r2)[:90])
s2, r2 = call("POST", "/social/team/create", {"torrent_id": T3}, token=tD)
d2 = r2.get("data") or {}
TEAM3 = d2.get("team_id")
ok("S1 反例：不能拯救自己发布的种", T3 and s2 == 400, str(r2)[:90])
if TEAM3:
    CLEAN.append("DELETE FROM resurrections WHERE team_id = %d" % TEAM3)
    CLEAN.append("DELETE FROM social_team WHERE id = %d" % TEAM3)

s, r = call("POST", "/social/team/join", {"team_id": TEAM1}, token=tB)
ok("S1 成员加入成功", s == 200, "http=%s %s" % (s, str(r)[:90]))
s, r = call("POST", "/social/team/join", {"team_id": TEAM1}, token=tB)
ok("S1 重复加入被拒（已在这支队伍中）",
   s == 400 and "已在" in (r.get("message") or ""), str(r)[:90])
s, r = call("POST", "/social/team/join", {"team_id": 999999999}, token=tB)
ok("S1 加入不存在的队伍 → 404 信封", s == 404, "http=%s" % s)

# 基线（队长/成员均在 1000s 起步）——先核基线快照，再抬到贡献值
BASE_B = q("SELECT seed_seconds_begin::text FROM social_team_member "
           "WHERE team_id = %d AND uid = %d" % (TEAM1, uB))
ok("S1 加入时基线快照=加入前累计值", BASE_B == "1000", "base=%s" % BASE_B)

s, r = call("POST", "/social/team/join", {"team_id": TEAM1}, token=tC)
ok("S1 第三成员加入成功", s == 200, str(r)[:80])
exec_sql("UPDATE social_team SET team_size_max = 2 WHERE id = %d" % TEAM1)
s, r = call("POST", "/social/team/join", {"team_id": TEAM1}, token=tD)
ok("S1 满员拒绝（上限 2/现 3 人）",
   s == 400 and "满" in (r.get("message") or ""), str(r)[:90])
exec_sql("UPDATE social_team SET team_size_max = 5 WHERE id = %d" % TEAM1)

# 贡献：A=+7000s、B=+2000s、C 退出后不加（退出者不再分账）。
# 阈值处设 1 小时=3600s，故增量必须给够（首轮 900s 时被合法挡下——
# 这不是缺陷，是本闸门自己的前提写错，留记）。
snatch_touch(uA, T1, 8000)
snatch_touch(uB, T1, 3000)
snatch_touch(uC, T1, 1000)
s, r = call("POST", "/social/team/leave", {"team_id": TEAM1}, token=tC)
ok("S1 成员中途退出成功（信誉 -15，先记着）",
   s == 200 and (r.get("data") or {}).get("reputation_delta") == -15,
   str(r)[:90])

BAL0 = {u: int(q("SELECT spark_balance::text FROM users WHERE id = %d" % u))
        for u in (uA, uB)}
jok, jinfo = run_job("social_team_settle")
ok("S1 settle 本轮真跑", jok is True, jinfo)
ST = q("SELECT status::text||'|'||COALESCE(fail_reason,'')||'|'||"
       "COALESCE(settled_at::text,'') FROM social_team WHERE id = %d" % TEAM1)
ok("S1 队伍 status=2 且 settled_at 已置", ST.startswith("2|"), ST)
RS = q("SELECT status FROM resurrections WHERE team_id = %d" % TEAM1)
ok("S1 resurrection→done", RS == "done", RS)

# 分账口径：total=9000、reward=5000 → A=5000*7000/9000=3888、
# B=5000*2000/9000=1111、余数 1 落排序末位（贡献最少者 B）
ROW = q("SELECT m.uid::text||'|'||m.contributed_sec::text||'|'||"
        "m.settled_amount::text||'|'||m.join_status::text "
        "FROM social_team_member m WHERE m.team_id = %d ORDER BY m.uid" % TEAM1)
rows = [x.split("|") for x in ROW.splitlines()]
by_uid = {int(x[0]): x for x in rows}
ok("S1 贡献回填与分账金额（余数落末位、总额恰=5000）",
   by_uid.get(uA, ["", "", ""])[1:3] == ["7000", "3888"]
   and by_uid.get(uB, ["", "", ""])[1:3] == ["2000", "1112"]
   and by_uid.get(uC, ["", "", ""])[1:3] == ["0", "0"],
   ROW)
ok("S1 退出者不计入分账（join_status=3，settled_amount=0）",
   by_uid.get(uC, ["", "", "", ""])[2:4] == ["0", "3"], ROW)
ok("S1 在册成员 join_status=4（完成）",
   by_uid.get(uA, ["", "", "", ""])[3] == "4"
   and by_uid.get(uB, ["", "", "", ""])[3] == "4", ROW)

# 出账两处一起钉：流水金额 + 余额增量
LED = q("SELECT user_id::text||'='||amount::text FROM spark_ledger "
        "WHERE idempotency_key LIKE 'social:team:%d:%%' ORDER BY user_id"
        % TEAM1)
ok("S1 spark_ledger 两笔（3888/1112）",
   LED == "%d=3888\n%d=1112" % (uA, uB), LED or "<empty>")
BAL1 = {u: int(q("SELECT spark_balance::text FROM users WHERE id = %d" % u))
        for u in (uA, uB)}
ok("S1 余额增量=分账金额",
   BAL1[uA] - BAL0[uA] == 3888 and BAL1[uB] - BAL0[uB] == 1112,
   "A:+%d B:+%d" % (BAL1[uA] - BAL0[uA], BAL1[uB] - BAL0[uB]))

RP = q("SELECT uid::text||'='||score::text||'/'||fulfilled_count::text "
       "FROM social_reputation WHERE uid IN (%d,%d) ORDER BY uid" % (uA, uB))
ok("S1 信誉 +20、fulfilled_count=1（仅结算时在册者）",
   RP == "%d=1020/1\n%d=1020/1" % (uA, uB), RP or "<empty>")
HC = q("SELECT count(*)::text FROM rescue_honor WHERE team_id = %d" % TEAM1)
TOT = q("SELECT total_sec::text FROM rescue_honor WHERE team_id = %d" % TEAM1)
ok("S1 拯救荣誉落库（total_sec=9000=两名在册者贡献和）",
   HC == "1" and TOT == "9000",
   "n=%s total=%s" % (HC, TOT))
MSG = q("SELECT count(*)::text FROM messages WHERE receiver_id IN (%d,%d) "
        "AND subject = '保种协作完成'" % (uA, uB))
ok("S1 完成站内信两封", MSG == "2", MSG)

jok2, jinfo2 = run_job("social_team_settle")
BAL2 = {u: int(q("SELECT spark_balance::text FROM users WHERE id = %d" % u))
        for u in (uA, uB)}
ok("S1 重跑零双发（余额纹丝不动）",
   jok2 is True and BAL2 == BAL1, "%s ; %s" % (jinfo2, BAL2))

print("== S2 中途退出：队长不可退 / 退出扣 15 / 退出者不再分账 ==")
s, r = call("POST", "/social/team/create", {"torrent_id": T2}, token=tD)
TEAM2 = (r.get("data") or {}).get("team_id")
if TEAM2:
    CLEAN.append("DELETE FROM resurrections WHERE team_id = %d" % TEAM2)
    CLEAN.append("DELETE FROM social_reputation WHERE uid IN (%d,%d)"
                 % (uD, uE))
    CLEAN.append("DELETE FROM messages WHERE receiver_id IN (%d,%d)"
                 % (uD, uE))
    CLEAN.append("DELETE FROM social_team WHERE id = %d" % TEAM2)
ok("S2 建队成功", s == 200 and TEAM2, str(r)[:90])
s, r = call("POST", "/social/team/join", {"team_id": TEAM2}, token=tE)
ok("S2 成员加入", s == 200, str(r)[:90])
s, r = call("POST", "/social/team/leave", {"team_id": TEAM2}, token=tD)
ok("S2 队长不能直接退出", s == 400 and "队长" in (r.get("message") or ""),
   str(r)[:90])
s, r = call("POST", "/social/team/leave", {"team_id": TEAM2}, token=tE)
ok("S2 成员退出 200 且 reputation_delta=-15",
   s == 200 and (r.get("data") or {}).get("reputation_delta") == -15,
   str(r)[:90])
RPV = q("SELECT score::text||'/'||withdrawn_count::text FROM "
        "social_reputation WHERE uid = %d" % uE)
ok("S2 退出者信誉 985、withdrawn_count=1", RPV == "985/1", RPV)
s, r = call("POST", "/social/team/leave", {"team_id": TEAM2}, token=tE)
ok("S2 重复退出被拒（你不在该队伍中——状态 3 不算在册）",
   s == 400, str(r)[:90])
RPV2 = q("SELECT score::text FROM social_reputation WHERE uid = %d" % uE)
ok("S2 重复退出不双扣", RPV2 == "985", RPV2)
jok, jinfo = run_job("social_team_expire")
ST2 = q("SELECT status::text FROM social_team WHERE id = %d" % TEAM2)
ok("S2 期限未到 expire 不抢跑（status 仍 1）",
   jok is True and ST2 == "1", "%s ; status=%s" % (jinfo, ST2))

print("== S3 到期未达标：expire 判失败 / 不连坐 / 与结算互斥 ==")
s, r = call("POST", "/social/team/create", {"torrent_id": T3}, token=tF)
TEAM3 = (r.get("data") or {}).get("team_id")
if TEAM3:
    CLEAN.append("DELETE FROM resurrections WHERE team_id = %d" % TEAM3)
    CLEAN.append("DELETE FROM social_reputation WHERE uid IN (%d,%d)"
                 % (uF, uG))
    CLEAN.append("DELETE FROM messages WHERE receiver_id IN (%d,%d)"
                 % (uF, uG))
    CLEAN.append("DELETE FROM social_team WHERE id = %d" % TEAM3)
ok("S3 建队成功（owner 改为探针同组，非发布者建队）",
   s == 200 and TEAM3, str(r)[:90])
s, r = call("POST", "/social/team/join", {"team_id": TEAM3}, token=tG)
ok("S3 成员加入", s == 200, str(r)[:90])
# 贡献远低阈值（+10s），把期限拨到过去
snatch_touch(uF, T3, 1010)
snatch_touch(uG, T3, 1010)
BALF0 = int(q("SELECT spark_balance::text FROM users WHERE id = %d" % uF))
exec_sql("UPDATE social_team SET deadline_at = now() - interval '1 hour' "
         "WHERE id = %d" % TEAM3)
jok, jinfo = run_job("social_team_expire")
ok("S3 expire 本轮真跑", jok is True, jinfo)
ST3 = q("SELECT status::text||'|'||COALESCE(fail_reason,'')||'|'||"
        "CASE WHEN settled_at IS NULL THEN 'unsettled' ELSE 'settled' END "
        "FROM social_team WHERE id = %d" % TEAM3)
ok("S3 队伍 status=3 / deadline_exceeded / settled_at 已置",
   ST3 == "3|deadline_exceeded|settled", ST3)
RS3 = q("SELECT status FROM resurrections WHERE team_id = %d" % TEAM3)
ok("S3 resurrection→expired", RS3 == "expired", RS3)
RP3 = q("SELECT uid::text||'='||score::text||'/'||failed_count::text "
        "FROM social_reputation WHERE uid IN (%d,%d) ORDER BY uid" % (uF, uG))
ok("S3 成员信誉 -5、failed_count=1（不连坐=只记事实）",
   RP3 == "%d=995/1\n%d=995/1" % (uF, uG), RP3 or "<empty>")
BALF1 = int(q("SELECT spark_balance::text FROM users WHERE id = %d" % uF))
ok("S3 不连坐：失败不扣钱", BALF1 == BALF0,
   "before=%d after=%d" % (BALF0, BALF1))
MSG3 = q("SELECT count(*)::text FROM messages WHERE receiver_id IN (%d,%d) "
         "AND subject = '保种协作已到期'" % (uF, uG))
ok("S3 到期站内信两封（文案不提羞辱性内容属产品口径，只钉封数）",
   MSG3 == "2", MSG3)
# 已结算的 TEAM1 不会被再碰（CAS 互斥，重跑 settle 已验证零双发）
ST1B = q("SELECT status::text FROM social_team WHERE id = %d" % TEAM1)
ok("S3 与之互斥：S1 已结算队伍状态仍 2", ST1B == "2", ST1B)

print("== S5 P-38 回归：module_social=no 也拦不住契约收尾 ==")
# 现场还原：契约在模块开时建立（既有契约），管理员随后关模块。
# P-38 修复前 job_module 把 settle/expire 挂在 social 上 → 定时与手动
# 两条路径整轮跳过，契约永悬、resurrection 永挂 open（实测现场：手动
# 触发报「模块已关闭（social），未执行」）。本块即该缺陷的回归哨兵。
uH, tH = mkuser(G4[0])
uI, tI = mkuser(G4[1])
T4 = probe_torrent(uD, "s5")
snatch_touch(uH, T4)
snatch_touch(uI, T4)
s, r = call("POST", "/social/team/create", {"torrent_id": T4}, token=tH)
TEAM5 = (r.get("data") or {}).get("team_id")
if TEAM5:
    CLEAN.append("DELETE FROM resurrections WHERE team_id = %d" % TEAM5)
    CLEAN.append("DELETE FROM rescue_honor WHERE team_id = %d" % TEAM5)
    CLEAN.append("DELETE FROM social_reputation WHERE uid IN (%d,%d)"
                 % (uH, uI))
    CLEAN.append("DELETE FROM messages WHERE receiver_id IN (%d,%d)"
                 % (uH, uI))
    CLEAN.append("DELETE FROM spark_ledger WHERE user_id IN (%d,%d)"
                 % (uH, uI))
    CLEAN.append("DELETE FROM social_team WHERE id = %d" % TEAM5)
ok("S5 建队成功（模块开着时建，模拟既有契约）",
   s == 200 and TEAM5, str(r)[:90])
s, r = call("POST", "/social/team/join", {"team_id": TEAM5}, token=tI)
ok("S5 成员加入成功", s == 200, str(r)[:90])
# 贡献合计 5000s > 3600s 阈值（RH=1），分账 4000+1000（整除无余数）
snatch_touch(uH, T4, 5000)
snatch_touch(uI, T4, 2000)

setting("module_social", "no")
ok("S5 module_social 已关（P-38 触发条件）",
   q("SELECT value FROM site_settings WHERE name = 'module_social'") == "no")
sN, rN = call("POST", "/social/team/create", {"torrent_id": T1}, token=tA)
ok("S5 负控：关模块后建队 API 被拒（模块门拦『新契约』仍有效）",
   sN == 400 and "未开放" in (rN.get("message") or ""),
   "http=%s %s" % (sN, str(rN)[:80]))

BALH0 = int(q("SELECT spark_balance::text FROM users WHERE id = %d" % uH))
jok, jinfo = run_job("social_team_settle")
ok("S5 关着模块 settle 仍真跑（P-38 修复点，本块承重）",
   jok is True, jinfo)
ST5 = q("SELECT status::text||'|'||"
        "CASE WHEN settled_at IS NULL THEN 'unsettled' ELSE 'settled' END "
        "FROM social_team WHERE id = %d" % TEAM5)
ok("S5 契约照常收尾（status=2 / settled_at 已置）",
   ST5 == "2|settled", ST5)
LED5 = q("SELECT user_id::text||'='||amount::text FROM spark_ledger "
         "WHERE idempotency_key LIKE 'social:team:%d:%%' ORDER BY user_id"
         % TEAM5)
ok("S5 分账落地（4000/1000）", LED5 == "%d=4000\n%d=1000" % (uH, uI),
   LED5 or "<empty>")
BALH1 = int(q("SELECT spark_balance::text FROM users WHERE id = %d" % uH))
ok("S5 余额增量=分账金额（队长 +4000）", BALH1 - BALH0 == 4000,
   "before=%d after=%d" % (BALH0, BALH1))
RS5 = q("SELECT status FROM resurrections WHERE team_id = %d" % TEAM5)
ok("S5 resurrection→done（不再永挂 open）", RS5 == "done", RS5)
jok, jinfo = run_job("social_team_expire")
ok("S5 关着模块 expire 也照跑（同一修复面）", jok is True, jinfo)

print("== S4 收尾：模块开关原样还原 + 探针数据清零 ==")
cleanup()
ok("S4 module_social 还原为 %s" % MOD0,
   q("SELECT COALESCE((SELECT value FROM site_settings WHERE "
     "name = 'module_social'),'<absent>')") == MOD0)
TEAM_IDS = ",".join(str(t) for t in (TEAM1, TEAM2, TEAM3, TEAM5) if t)
USER_IDS = ",".join([str(v[0]) for v in users.values()] + [str(uH), str(uI)])
Z = q(
    "SELECT (SELECT count(*) FROM social_team WHERE id IN (%s))::text"
    "||'|'||(SELECT count(*) FROM social_team_member WHERE uid IN (%s))::text"
    "||'|'||(SELECT count(*) FROM rescue_honor WHERE team_id IN (%s))::text"
    "||'|'||(SELECT count(*) FROM social_reputation WHERE uid IN (%s))::text"
    "||'|'||(SELECT count(*) FROM messages WHERE receiver_id IN (%s))::text"
    "||'|'||(SELECT count(*) FROM spark_ledger WHERE user_id IN (%s))::text"
    "||'|'||(SELECT count(*) FROM user_achievements "
    "WHERE user_id IN (%s))::text"
    % (TEAM_IDS, USER_IDS, TEAM_IDS, USER_IDS, USER_IDS, USER_IDS, USER_IDS))
ok("S4 残料七数全 0（队伍|成员|荣誉|信誉|私信|流水|成就）",
   Z == "0|0|0|0|0|0|0", Z)

summary()
