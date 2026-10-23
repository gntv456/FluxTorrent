# -*- coding: utf-8 -*-
"""AN 家族共用夹具（从 pt_audit_an_team_contract.py 拆出，500 行门禁）。

只有工具与开机自清，没有判据：psql 包装（出错就抛）、站点设定写入、建探针
成员/种子、造 snatches、等 worker 本体真跑一轮（认 job_status.last_started_at
前移，面板与定时抢同一把 advisory 锁，抢不到是良态），以及把历史崩溃留下的
`AnTeam.s*` 夹具种在导入时清掉（这批夹具不打 tracker、纯链上数据，漏一次
就永久抬高「过审种缺原始档案」台账）。

`ROOT`/`CLEAN`/`TS`/`G1..G4` 随工具一起进模块：CLEAN 是同一个列表对象，
调用方的 cleanup() 与自加条目照旧互通。两支 AN 闸门不许并发。
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


# 开机自清（批二百零二）：AN 的夹具只在**正常收尾**时删除，崩溃/超时那一跑
# 会把 `AnTeam.s*` 种子与探针号永久留在现网。2026-10-20 实测库里积了 5 个批次
# 共 20 个这样的夹具种，且它们没有 .torrent 存档 ⇒ 直接抬高 GC1
# 「过审种缺原始档案」台账（31 → 11；过审种 180 → 160）。
# 名字模式是本闸门专属（AnTeam.s<序号>.<批次号>），演示数据不会撞上；
# 删除顺序按 `torrents` 的外键实况：snatches 先走（其余多为 ON DELETE CASCADE），
# 且这批夹具都不在 requests/offers/subtitle_requests 等 NO ACTION 引用面上
# （2026-10-20 事务内 dry-run 验证通过并回滚）。
_leak = q("SELECT id FROM torrents WHERE name LIKE 'AnTeam.s%' "
          "AND split_part(name, '.', 1) = 'AnTeam' "
          "AND split_part(name, '.', 3) ~ '^[0-9]+$'")
for _tid in [x for x in _leak.split("\n") if x.strip()]:
    exec_sql("DELETE FROM snatches WHERE torrent_id = %s" % _tid)
    exec_sql("DELETE FROM torrents WHERE id = %s" % _tid)


def probe_torrent(owner_id, tag, seeders=0):
    """直接建一颗探针种（本闸门不打 tracker，纯链上数据）。"""
    name = "AnTeam.%s.%d" % (tag, TS)
    tid = int(q(
        "INSERT INTO torrents (info_hash, name, size, owner_id, "
        "category_id, approval_status, seeders) VALUES "
        "(md5(random()::text), '%s', 1024, %d, 1, 1, %d) RETURNING id"
        % (name, owner_id, seeders)))
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

