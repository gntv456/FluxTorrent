# -*- coding: utf-8 -*-
"""常驻闸门：死种复活任务不得「列表说能领、点下去必回 400」（P-457）。

形状：`resurrections_torrent_id_key` 是**全表** UNIQUE(torrent_id)——一个资源
一辈子只有一条任务行，`done`/`expired` 这些终态行照样占位；而列表的排除谓词
此前只看 `status = 'open'`。于是「曾被人救活过」或「救过没成」的死种仍然出现
在 /resurrections 的可领列表里（0284 P1-5 起还带 required_hours /
reward_sparks），点下去 claim 的 `ON CONFLICT (torrent_id) DO NOTHING` 吃掉
写入、回 400，文案却写「该种子已有进行中的复活任务」——根本没有进行中的任务。
组队侧（social_http/team.rs）同一句谎话。

这是站方 360384f0（种主打开自己的死种仍显示「认领复活任务」）同族的另一半：
同一形状，换了触发条件。修法（b73c81e6）：列表口径与约束口径对齐（有任何
任务行就不露出），冲突分支回查占用行的真实状态，分五种说真话。

判据分层：
  V0 正控   FIX^ 的列表 SQL 带着 status 限定、冲突分支只有一句谎话
  V1        静态：列表排除谓词不再限定状态；两处冲突分支都有 open/done 臂，
            且 done 臂的文案不含「进行中」（一律剥注释后判）
  V2        SQL 层 A/B：同一份事务夹具上，旧列表 SQL 命中探针种（缺陷本体红证）
            而新列表 SQL 不命中；同款 ON CONFLICT 形状写不进（=点了必失败）
  V3        夹具零残留（ROLLBACK 后回读）
  V4        现网账面：resurrections 状态分布（空表＝本批零行为变化）
跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_resurrection_claim.py
      负控：同加 --selftest
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RS = "apps/api/src/ops_http/resurrections.rs"
TM = "apps/api/src/social_http/team.rs"
FIX = "b73c81e6"
PSQL = ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
        "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-F", "|",
        "-f", "-"]
ANCHOR = "SELECT t.id AS torrent_id"
STATUS_OLD = "AND r.status = 'open'"
PICK = ("SELECT t.id FROM torrents t WHERE t.approval_status = 1 "
        "AND NOT EXISTS (SELECT 1 FROM snatches s WHERE s.torrent_id = t.id) "
        "AND NOT EXISTS (SELECT 1 FROM resurrections r WHERE "
        "r.torrent_id = t.id) ORDER BY t.id DESC LIMIT 1;")


def read(rel, ref=None):
    if ref:
        return subprocess.run(["git", "show", "%s:%s" % (ref, rel)],
                              capture_output=True, text=True,
                              encoding="utf-8", errors="replace").stdout
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read().replace("\r\n", "\n")


def strip_comments(text):
    t = re.sub(r"/\*[\s\S]*?\*/", " ", text)
    return re.sub(r"(?<!:)//[^\n]*", " ", t)


def list_sql(rel, ref=None):
    """从源码切出那段字面列表 SQL（不手抄），拼回 Rust 的续行。"""
    src = read(rel, ref)
    i = src.index(ANCHOR)
    lit = src[src.rindex('"', 0, i) + 1: src.index('",', i)]
    return re.sub(r"\\\s*\n\s*", "", lit)


def psql(sql):
    p = subprocess.run(PSQL, input=sql, capture_output=True, text=True,
                       encoding="utf-8", errors="replace", cwd=ROOT)
    if p.returncode != 0:
        raise RuntimeError("psql 失败：%s" % ((p.stderr or "")[:240]))
    return (p.stdout or "").strip()


def cells_of(text):
    out = {}
    for ln in text.split("\n"):
        if "|" in ln:
            k, v = ln.split("|", 1)
            out[k.strip()] = v.strip()
    return out


def main():
    old_list = list_sql(RS, FIX + "^")
    new_list = list_sql(RS)
    old_src = read(RS, FIX + "^")
    # ---------- V0 正控 ----------
    ok("V0 正控：FIX^ 列表 SQL 带「只排 open」限定、冲突分支只有一句"
       "「已有进行中」且没有 done 臂",
       STATUS_OLD in old_list and old_list.count(STATUS_OLD) == 1
       and old_src.count("该种子已有进行中的复活任务") == 1
       and 'if st == "done"' not in old_src,
       "旧形状不在＝这条尺看的已经不是这批修复")
    # ---------- V1 静态口径 ----------
    body = strip_comments(read(RS))
    team_body = strip_comments(read(TM))
    ok("V1a 列表排除谓词不再限定状态、且「有任何行就排除」在册",
       STATUS_OLD not in new_list
       and "NOT EXISTS (SELECT 1 FROM resurrections r WHERE "
       "r.torrent_id = t.id)" in new_list,
       "列表又回到「看 open」＝终态行还会被当可领露出来")
    names = [a or b for a, b in re.findall(
        r'if st == "(\w+)"|Some\("(\w+)"\)', body + team_body)]
    ok("V1b 两处冲突分支都有 open / done 两条臂（抓到 %s）"
       % sorted(set(names)),
       names.count("open") >= 2 and names.count("done") >= 2,
       "少一条臂＝某种占用又开始被说成「进行中」")
    lies = []
    for src, name in ((body, "claim"), (team_body, "team")):
        for m in re.finditer(r'"done"', src):
            if "进行中" in src[m.end():m.end() + 160]:
                lies.append(name)
    ok("V1c done 臂的文案不含「进行中」（违例 %s）" % (lies or "无"),
       not lies, "说成「进行中」＝用户会等一个永远不会来的空位")
    # ---------- V2/V3 事务夹具 A/B ----------
    tid = psql(PICK).split("\n")[0].strip()
    uids = psql("SELECT string_agg(id::text, ',') FROM (SELECT id FROM "
                "users ORDER BY id LIMIT 3) t;").split(",")
    fixture = """
\\set ON_ERROR_STOP on
BEGIN;
UPDATE torrents SET approval_status = 1, seeders = 0, owner_id = %(owner)s
 WHERE id = %(tid)s;
INSERT INTO resurrections (torrent_id, user_id, required_hours,
                           reward_sparks, status, finished_at)
VALUES (%(tid)s, %(prev)s, 1, 5000, 'done', now() - interval '1 day');
SELECT 'OLDROWS|' || count(*)::text FROM (%(old)s) x;
SELECT 'OLDHIT|' || count(*) FILTER (WHERE x.torrent_id = %(tid)s)::text
FROM (%(old)s) x;
SELECT 'NEWROWS|' || count(*)::text FROM (%(new)s) x;
SELECT 'NEWHIT|' || count(*) FILTER (WHERE x.torrent_id = %(tid)s)::text
FROM (%(new)s) x;
WITH ins AS (
  INSERT INTO resurrections (torrent_id, user_id, required_hours,
                             reward_sparks)
  SELECT %(tid)s, %(mine)s, 1, 5000
  ON CONFLICT (torrent_id) DO NOTHING RETURNING 1)
SELECT 'CLAIM|' || count(*)::text FROM ins;
ROLLBACK;
SELECT 'RESIDUE|' || count(*)::text FROM resurrections;
""" % {"tid": tid, "owner": uids[0], "prev": uids[1], "mine": uids[2],
       "old": old_list, "new": new_list}
    c, err = {}, None
    try:
        c = cells_of(psql(fixture))
    except Exception as ex:  # noqa: BLE001
        err = str(ex)[:240]
    ok("V2 夹具（死种 + 一行 status=done）：旧列表 rows=%s hit=%s、"
       "新列表 rows=%s hit=%s、claim 写入=%s（探针种 id=%s）"
       % (c.get("OLDROWS"), c.get("OLDHIT"), c.get("NEWROWS"),
          c.get("NEWHIT"), c.get("CLAIM"), tid),
       not err and c.get("OLDHIT") == "1" and c.get("NEWHIT") == "0"
       and c.get("CLAIM") == "0",
       "旧列表不 hit＝夹具没造出缺陷条件（红的是尺）；新列表还 hit＝口径"
       "又漏回 open；err=%s" % (err or "无"))
    ok("V3 夹具零残留（resurrections 回读=%s）" % c.get("RESIDUE"),
       c.get("RESIDUE") == "0", "残留会在演示站挂着一个假的复活任务")
    # ---------- V4 现网账面 ----------
    dist = psql("SELECT COALESCE(string_agg(status || ':' || c::text, "
                "', '), '表空') FROM (SELECT status, count(*) c FROM "
                "resurrections GROUP BY status) t;")
    ok("V4 账面自证：现网 resurrections 分布读数取到（=%s；空表＝本批对现网"
       "零行为变化，非空须逐状态重新定性）" % dist, dist not in ("", "?"),
       "读不到分布＝库连接或表名变了")
    if "--selftest" in sys.argv:
        selftest(tid)
    summary()


def selftest(tid):
    """把列表排除谓词翻回「只排 open」⇒ V1a/V2 必然红；按字节还原。"""
    p = os.path.join(ROOT, RS)
    snap = io.open(p, "rb").read()
    txt = snap.decode("utf-8")
    a = "NOT EXISTS (SELECT 1 FROM resurrections r WHERE r.torrent_id = t.id)"
    b = a[:-1] + " AND r.status = 'open')"
    if txt.count(a) != 1:
        ok("S0 负控锚点唯一（命中 %d，应为 1）" % txt.count(a), False,
           "锚点搬家了，先去看列表 SQL 现在长什么样")
        return
    try:
        io.open(p, "wb").write(txt.replace(a, b, 1).encode("utf-8"))
        flipped = list_sql(RS)
        uids = psql("SELECT string_agg(id::text, ',') FROM (SELECT id FROM "
                    "users ORDER BY id LIMIT 3) t;").split(",")
        probe = ("\\set ON_ERROR_STOP on\nBEGIN;\n"
                 "UPDATE torrents SET approval_status = 1, seeders = 0, "
                 "owner_id = %(owner)s WHERE id = %(tid)s;\n"
                 "INSERT INTO resurrections (torrent_id, user_id, "
                 "required_hours, reward_sparks, status, finished_at) VALUES "
                 "(%(tid)s, %(prev)s, 1, 5000, 'done', now());\n"
                 "SELECT 'HIT|' || count(*) FILTER (WHERE x.torrent_id = "
                 "%(tid)s)::text FROM (%(flipped)s) x;\nROLLBACK;"
                 % {"owner": uids[0], "prev": uids[1], "tid": tid,
                    "flipped": flipped})
        hit = cells_of(psql(probe)).get("HIT")
        ok("S1 负控：翻回旧口径后判据抓到 status 限定（抓到=%s，终态行重新"
           "被当可领 hit=%s）" % (STATUS_OLD in flipped, hit),
           STATUS_OLD in flipped and hit == "1", "翻红了没抓到＝判据不承重")
    finally:
        io.open(p, "wb").write(snap)
    back = io.open(p, "rb").read() == snap
    ok("S2 负控还原：按字节回到翻转前（%s）且正向复跑无 status 限定（%s）"
       % (back, STATUS_OLD not in list_sql(RS)),
       back and STATUS_OLD not in list_sql(RS),
       "还原不干净＝把翻转态留在了工作树")


if __name__ == "__main__":
    main()
