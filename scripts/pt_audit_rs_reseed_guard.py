# -*- coding: utf-8 -*-
"""补种请求限频的原子性与通知面（P-626）。

旧版在事务外读 last_reseed 判 15 分钟限频、事务尾才写：两个并发请求
都读到旧值 ⇒ 各发一整批站内信。现场实测同一颗种 2ms 内两条
200/notified=1，收件人收到两封。修法=事务内用带谓词的 UPDATE 抢槽位。

判据纪律：探针只落在「ptc_ 前缀探针名下的死种」，不碰真人收件箱；
收尾删本轮写的信并把 last_reseed 还原（共享设定按原值回填）。
活体闸门不得与同族并发跑：每轮都会重置时间戳抢槽位。
"""
import concurrent.futures
import os
import sys
from threading import Barrier

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, dig, login, ok, psql_stdin, summary,
)

ROOT = login()
ROUNDS = 3


def num(sql):
    out = psql_stdin(sql).strip()
    if not out.isdigit():
        raise RuntimeError("读数不是整数：%r" % out)
    return int(out)


row = psql_stdin(
    "SELECT t.id || '|' || COALESCE(t.last_reseed::text, 'NULL') "
    "FROM torrents t JOIN users u ON u.id = t.owner_id "
    "WHERE t.approval_status = 1 AND t.seeders = 0 "
    "AND u.status < 2 AND u.username LIKE 'ptc\\_%' "
    "AND NOT EXISTS (SELECT 1 FROM snatches s "
    "  JOIN users su ON su.id = s.user_id "
    "  WHERE s.torrent_id = t.id AND s.completed_at IS NOT NULL "
    "    AND su.status < 2 AND s.user_id <> u.id) "
    "ORDER BY t.id DESC LIMIT 1").strip()
ok("RS0 找到探针名下、零外部收件人的死种（不会给真人发信）",
   "|" in row and row.split("|")[0].isdigit(), "候选=%r" % row)
if "|" not in row:
    summary()
TID, ORIG = row.split("|", 1)
if not TID.isdigit():
    summary()
TID = int(TID)


def msg_since(t0):
    return num(
        "SELECT count(*) FROM messages WHERE kind = 'reseed_request' "
        "AND body LIKE '%%/torrent/%d%%' "
        "AND created_at >= '%s'::timestamptz" % (TID, t0))


def cleanup(t0):
    psql_stdin(
        "DELETE FROM messages WHERE kind = 'reseed_request' "
        "AND body LIKE '%%/torrent/%d%%' "
        "AND created_at >= '%s'::timestamptz" % (TID, t0))
    val = "NULL" if ORIG == "NULL" else "'%s'" % ORIG
    psql_stdin("UPDATE torrents SET last_reseed = %s WHERE id = %d"
               % (val, TID))


def pair():
    """两个请求尽量同时发出：barrier 让它们在同一毫秒进入判限频。"""
    b = Barrier(2, timeout=15)

    def one(_):
        b.wait()
        return call("POST", "/torrents/%d/reseed" % TID, {}, token=ROOT)

    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as ex:
        return list(ex.map(one, (0, 1)))


T0 = psql_stdin(
    "SELECT to_char(clock_timestamp(), "
    "'YYYY-MM-DD\"T\"HH24:MI:SS.US')")
rounds, deltas, notified = [], [], []
try:
    for i in range(ROUNDS):
        # 每轮给一个「刚过冷却」的槽位；收尾按 ORIG 回填
        psql_stdin("UPDATE torrents SET last_reseed = NULL WHERE id = %d"
                   % TID)
        before = msg_since(T0)
        rs = pair()
        rounds.append(sorted(s for s, _ in rs))
        deltas.append(msg_since(T0) - before)
        for st, resp in rs:
            if st == 200:
                notified.append(dig(resp, "data", "notified"))
    st, resp = call("POST", "/torrents/%d/reseed" % TID, {}, token=ROOT)
    cool = (st, resp.get("code"))
finally:
    cleanup(T0)

ok("RS1 每轮并发双请求恰好一个放行（另一条被 15 分钟限频拒绝）",
   all(x == [200, 400] for x in rounds), "各轮状态=%s" % rounds)
ok("RS2 每轮最多新增一封补种信（旧版并发时每轮两封）",
   all(d <= 1 for d in deltas), "各轮新增=%s" % deltas)
ok("RS3 放行回执的自报数=1（探针种主真收到且不谎报）",
   bool(notified) and all(n == 1 for n in notified),
   "notified=%s" % notified)
ok("RS4 冷却期内的单发请求被拒（400/1002）",
   cool == (400, 1002), "http=%s code=%s" % cool)
ok("RS5 收尾：last_reseed 按原值还原、本轮残料清零",
   psql_stdin("SELECT COALESCE(last_reseed::text, 'NULL') "
              "FROM torrents WHERE id = %d" % TID).strip() == ORIG
   and msg_since(T0) == 0,
   "now=%s want=%s msgs=%s"
   % (psql_stdin("SELECT COALESCE(last_reseed::text, 'NULL') "
                 "FROM torrents WHERE id = %d" % TID).strip(),
      ORIG, msg_since(T0)))
ok("RS6 目标种仍可变（未被本轮动作改档/下架）",
   psql_stdin("SELECT approval_status || '|' || seeders "
              "FROM torrents WHERE id = %d" % TID).strip() == "1|0",
   psql_stdin("SELECT approval_status || '|' || seeders "
              "FROM torrents WHERE id = %d" % TID).strip())
print("== 目标 %s（%s）" % (TID, BASE))
summary()
