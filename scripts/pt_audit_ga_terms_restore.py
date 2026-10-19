# -*- coding: utf-8 -*-
"""GA 闸门：术语治理的版本快照与回灌（批一百六十八 P-402）。

盯的是「失败不许留痕、成功必须真回灌」这一对：
  · 旧实现 HTTP 层两步 autocommit——先 `push_version` 记一版，再
    `restore_terms` 回灌。回灌一失败（快照损坏/口径漂移/插入报错），版本史
    里就多一条 `rollback / restore -> vN`，而表体一行没动。实测（修前）：
    灌一条坏快照后 `term_versions` 从 1 涨到 2。
  · 版本列表是站长判断「能回滚到哪一版」的唯一依据：多一条没发生过的记录，
    就会有人照着它回滚到一段从未存在过的形态。

正控同样重要：只把写入砍掉也算「失败不留痕」，所以必须同时证明有效快照
真的会回灌（rows=1、表体变成那一版的形态）并留下回滚前的一版。

用法：`python -u scripts/pt_audit_ga_terms_restore.py`（只跑静态：GA_LIVE=0）
"""
import io
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import call, login, ok, summary  # noqa: E402

LIVE = os.environ.get("GA_LIVE", "1") != "0"
MOD = "apps/api/src/repo/terms_gov/mod.rs"
VER = "apps/api/src/repo/terms_gov/versions.rs"
HANDLER = "apps/api/src/staff_http/terms_gov.rs"
A = "ga402-term-a"
B = "ga402-term-b"


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def cut(s):
    return "\n".join(
        ln for ln in s.split("\n") if not ln.strip().startswith("//"))


def sql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        raise RuntimeError("psql 失败: " + (r.stderr or "")[:300])
    return (r.stdout or "").strip()


def dat(p):
    return p.get("data") or {}


ver = cut(rd(VER))
body = ver.split("fn restore_marked")[1][:1500] if "fn restore_marked" in ver \
    else ""
hd = cut(rd(HANDLER))
ok("GA1 回灌走单事务：restore_marked 内既记版本又改表，且只 begin 一次",
   "fn restore_marked" in ver and "push_version_tx" in body
   and "DELETE FROM site_terms" in body and body.count(".begin()") == 1
   and ".commit()" in body,
   "两步各自 autocommit＝回灌失败也留下一条没发生过的版本记录")
ok("GA2 旧的 `restore_terms` 不得复活，HTTP 层也不许再自己 push 一版",
   "restore_terms" not in ver and "push_version(" not in hd
   and "restore_marked" in hd,
   " handler 先 push 再 restore 就是这个洞的本体")
ok("GA3 坏快照必须先验格式再进事务（否则白占一版）",
   body and body.index("版本快照格式不正确") < body.index(".begin()"),
   "格式检查放在事务里＝校验失败也要先把版本行回滚掉，语义含混")
ok("GA4 版本块已按域拆出且 mod.rs 重新导出（引用面没断）",
   "pub use versions::" in cut(rd(MOD))
   and "restore_marked" in rd(MOD),
   "拆分后忘了 re-export，调用方会一路编译错")

if LIVE:
    ROOT_TOK = login("root")
    ok("GA-L0 root 登录可用", bool(ROOT_TOK), "卡在 PoW/限流则活体段作废")
    if ROOT_TOK:
        START_V = int(sql("SELECT count(*) FROM term_versions") or 0)
        CLEAN = ["DELETE FROM site_terms WHERE canonical IN ('%s','%s')"
                 % (A, B),
                 "DELETE FROM term_versions WHERE created_at > now() "
                 "- interval '15 minutes' AND (note LIKE '%%ga402%%' "
                 "OR snapshot::text LIKE '%%ga402%%' OR source='rollback')"]
        try:
            def put(canon, repl):
                return call("POST", "/admin/terms", {
                    "canonical": canon, "replacement": repl,
                    "enabled": True, "descr": "ga402"}, token=ROOT_TOK)

            put(A, "甲词")
            put(B, "乙词")
            v_now = sql("SELECT max(version) FROM term_versions")
            s, r = call("POST", "/admin/terms/versions/%s/restore" % v_now,
                        {}, ROOT_TOK)
            d = dat(r)
            tbl = sql("SELECT coalesce(string_agg(canonical,','),'-') "
                      "FROM site_terms WHERE canonical LIKE 'ga402%'")
            ok("GA-L1 正控：有效快照真回灌（rows=%s 表体=%s）"
               % (d.get("rows"), tbl),
               s == 200 and d.get("rows") == 1 and tbl == A,
               "砍掉写入也能让『失败不留痕』变绿，所以正控必须打真回灌")
            ok("GA-L2 回滚前那版确实记下了（saved_current_as=%s）"
               % d.get("saved_current_as"),
               int(d.get("saved_current_as") or 0) > int(v_now),
               "回滚不留当前态＝「回滚 → 再回滚」变单向操作")
            # 负控：把探针自己的版本行改成坏快照，失败必须零写入
            bad = sql("SELECT max(version) FROM term_versions")
            v_before = sql("SELECT count(*) FROM term_versions")
            sql("UPDATE term_versions SET snapshot='{}'::jsonb "
                "WHERE version=%s" % bad)
            s, r = call("POST", "/admin/terms/versions/%s/restore" % bad,
                        {}, ROOT_TOK)
            v_after = sql("SELECT count(*) FROM term_versions")
            ok("GA-L3 负控：坏快照被拒且不留版本行（http=%s，%s→%s）"
               % (s, v_before, v_after),
               s == 400 and r.get("code") == 1002 and v_after == v_before,
               "这就是 P-402 的本体：拒了却留下一条 rollback 假记录")
        finally:
            for stmt in CLEAN:
                sql(stmt)
            ok("GA-L4 残料清零（版本行数回到开局 %d，探针规则 0 条）"
               % START_V,
               int(sql("SELECT count(*) FROM term_versions") or 0)
               == START_V
               and sql("SELECT count(*) FROM site_terms WHERE "
                       "canonical LIKE 'ga402%'") == "0",
               "闸门不许往现网留数据")

summary()
