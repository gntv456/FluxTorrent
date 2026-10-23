# -*- coding: utf-8 -*-
"""AH 家族共用夹具（从 pt_audit_ah_hr.py 拆出，500 行门禁）。

两边都需要的只有工具，不需要判据：psql 包装（出错就抛，不把 SQL 写错读成
「查到 0 条」）、站点设定写入、发一颗探针种、等 worker 真跑完一轮
（认 job_status.last_finished_at 前移，不认手动触发成功——面板与定时抢同一把
advisory 锁，抢不到是良态）、建探针成员、造 snatch。

`ROOT`/`CLEAN` 随工具一起进模块：CLEAN 是同一个列表对象，调用方
`for stmt in reversed(CLEAN)` 的收尾照旧能收到 snatch/upload 追加的条目。
两支闸门（违规产生 / 处罚与免罪）各自独立跑，不许并发（探针号前缀同名）。
"""
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_form import BOUNDARY, make_torrent, multipart  # noqa: E402
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402

import json
import urllib.error
import urllib.request

ROOT = login()
CLEAN = []
RUN = int(time.time())
ST = ("hr_days", "hr_hours", "hr_violation_limit", "enable_hr",
      "cancel_hr", "hr_prewarn_hours", "hr_scan_lookback_hours")


def q(sql_text):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    # psql 出错时 stdout 是空串、退出码非 0。静默返回空串会让判据变成
    # "rows="（看着像「查到了 0 条」），其实根本没查成——
    # 这里把 stderr 打出来，判据红时能一眼看出是 SQL 写错还是数据不对。
    if p.returncode:
        print("   [psql-err] %s" % (p.stderr or "").strip()[:200])
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


def upload(tok, name):
    body = multipart({"name": name, "category_id": "1",
                      "descr": "H&R 探针正文，长度足够过常规校验",
                      "small_descr": "ah"},
                     make_torrent(name), filename="ah.torrent")
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


def run_job(job, wait_s=170):
    """确认「改完数据之后 worker 真的跑过一轮该任务」。

    hr_enforce / hr_punish 的 cadence 是 60s（catalog.rs），面板入队
    （/admin/jobs/run）与定时触发抢同一把 advisory 锁，谁先到谁跑——
    手动那一条经常以「锁未抢到」失败，这是良态竞争，不是缺陷。
    判据因此钉在 `job_status.last_started_at` 前移且 last_ok=true 上：
    这证明的是 worker 本体（同一份 SQL）在本步骤之后执行过，
    至于是定时还是手动认领，与断言无关。
    """
    t0 = q("SELECT to_char(now(), "
           "'YYYY-MM-DD\"T\"HH24:MI:SS.US')")
    call("POST", "/admin/jobs/run", {"job": job}, token=ROOT)
    deadline = time.time() + wait_s
    while time.time() < deadline:
        row = q("SELECT COALESCE(to_char(last_started_at, "
                "'YYYY-MM-DD\"T\"HH24:MI:SS.US'),'-')||'|'||"
                "COALESCE(to_char(last_finished_at, "
                "'YYYY-MM-DD\"T\"HH24:MI:SS.US'),'-')||'|'||"
                "COALESCE(last_ok::text,'-')||'|'||"
                "LEFT(COALESCE(last_result,''),60) FROM job_status "
                "WHERE job = '%s'" % job)
        if row.count("|") >= 3:
            started, finished, okv, res = row.split("|", 3)
            # 必须等 last_finished_at 前移：只看 last_started_at 会在任务刚
            # 启动、SQL 还没跑完时就返回（G5 自动恢复曾因此假红）。
            if finished not in ("-", "") and finished > t0:
                return (okv == "true"), "started=%s finished=%s %s" % (
                    started, finished, res)
        time.sleep(4)
    return None, "等待 worker 本轮执行超时（%ss）" % wait_s


def member(tag):
    """普通成员：adduser + 先改初始密码（避开登录限流与弱口令门）"""
    user = "%s%d" % (tag, int(time.time()) % 100000)
    pw = "Gate#123456"
    call("POST", "/admin/adduser", {"username": user, "password": pw,
                                    "email": "%s@ah.local" % user},
         token=ROOT)
    uid = q("SELECT id::text FROM users WHERE username = '%s'" % user)
    t0 = login(user, pw)
    call("POST", "/me/password/change",
         {"old_password": pw, "new_password": pw + "x"}, token=t0)
    return user, int(uid), login(user, pw + "x")


def snatch(uid, tid, completed_ago_days, seeded, downloaded):
    # 完成时点取 2 天前：既要落在 hr_scan_lookback_hours（本闸门设 72）之内
    # ——ZT81 增量窗口按 `completed_at > now() - lookback` 过滤，正好压 72h
    # 边沿会因为 now() 前进而永久出局；又要晚于 hr_days=1 的期限，好让
    # 「逾期未达标」一轮就成立。
    exec_sql("INSERT INTO snatches (user_id, torrent_id, downloaded, "
             "seeded_seconds, completed_at, progress, last_seen_at) VALUES "
             "(%d, %d, %d, %d, now() - interval '%d day', 100, now())"
             % (uid, tid, downloaded, seeded, completed_ago_days))
    CLEAN.append("DELETE FROM snatches WHERE user_id = %d AND torrent_id = %d"
                 % (uid, tid))
