# -*- coding: utf-8 -*-
"""常驻闸门：后台「行为奖励 → 发主题 / 回帖奖励」必须真生效（P-456）。

站长视角：/admin/settings 的「行为奖励」里 `starttopic`（发主题奖励，现值
2）与 `makepost`（回帖奖励，现值 1）改得动、保存成功、回显也对——代码却按
字面量发钱（topic_create 恒 +2、post_write 恒 +1）。配对的另一半更危险：
删主题/删单帖按常数收回，额度一旦可改，常数就同时犯两种错（调大后删老帖
多扣；楼主首帖本没有回帖流水仍被扣 1）。
修法（a85d5fe1）：发放读面板（`repo::community::forum_reward`，归一并夹
[0, 1_000_000]），收回回查当年那笔发放（幂等键 forum-topic:{uid}:{tid} /
forum-reply:{uid}:{pid}，走 spark_ledger 的 idx_spark_idem）。

判据分层：R0 正控（FIX^ 确是常数形状）/ R1 接线在册 / R2 金额实参位不再是
字面量 / R2b 收回前有 owed>0 门槛 / R3 纯函数单测 / R4 源码字面 SQL 在事务
夹具上 A/B（ROLLBACK，残留回读）/ R5 现网面板值与最近流水对账 / R6 幂等键
两处同源。界面层默认只报读数（现网二进制不含本批修复）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_forum_reward.py
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
FR = "apps/api/src/repo/community/forum_reward.rs"
TD = "apps/api/src/repo/community/topic_delete.rs"
TC = "apps/api/src/community_http/topic_create.rs"
PW = "apps/api/src/community_http/post_write.rs"
PE = "apps/api/src/community_http/post_edit.rs"
PTD = "apps/api/src/community_http/post_topic_delete.rs"
FIX = "a85d5fe1"
PSQL = ["docker", "exec", "-i", "flux-postgres",
        "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-v", "ON_ERROR_STOP=1", "-F", "|",
        "-f", "-"]
# 金额实参位允许的来源：面板额度变量、回查额、流水汇总值
ARG_OK = ("topic_award", "reply_award", "owed", "n as i64")


def read(rel, ref=None):
    if ref:
        return subprocess.run(["git", "show", "%s:%s" % (ref, rel)],
                              capture_output=True, text=True,
                              encoding="utf-8", errors="replace").stdout
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def psql(sql):
    p = subprocess.run(PSQL, input=sql, capture_output=True, text=True,
                       encoding="utf-8", errors="replace", cwd=ROOT)
    if p.returncode != 0:
        raise RuntimeError("psql 失败：%s" % ((p.stderr or "")[:240]))
    return (p.stdout or "").strip()


def rust_sql(rel, marker, ref=None):
    """取源码里那段字面 SQL（不手抄），拼回 Rust 的 ``\\`` 续行。"""
    src = read(rel, ref)
    i = src.index(marker)
    lit = src[src.index('"', i) + 1:src.index('",', i)]
    return re.sub(r"\\\s*\n\s*", "", lit)


def args_at(rel, fn):
    """抓 fn(...) 调用的实参列表（只到第一个顶层右括号为止）。"""
    src = read(rel)
    out = []
    for m in re.finditer(r"%s\(" % fn, src):
        seg = src[m.end():m.end() + 420]
        depth, cur, arr = 0, "", []
        for ch in seg:
            if ch in "([{":
                depth += 1
            elif ch in ")]}":
                if depth == 0:
                    break
                depth -= 1
            if ch == "," and depth == 0:
                arr.append(cur.strip())
                cur = ""
                continue
            cur += ch
        arr.append(cur.strip())
        if len(arr) >= 3:
            out.append(arr[2])
    return out


def main():
    # ---------- R0 正控 ----------
    n_old = len(re.findall(
        r'earn_spark\(&state\.repo\.db, auth\.id, [12], "forum"',
        read(TC, FIX + "^") + read(PW, FIX + "^")))
    lit_claw = len(re.findall(r"^\s{12}[12],$",
                              read(PE, FIX + "^") + read(PTD, FIX + "^"),
                              re.M))
    ok("R0 正控：FIX^ 发放点 %d 处常数额、收回点 %d 处常数实参，且 forum_"
       "reward.rs 当时不存在" % (n_old, lit_claw),
       n_old == 2 and lit_claw == 2 and not read(FR, FIX + "^").strip(),
       "旧形状数不对＝这条尺看的已经不是这批修复")
    # ---------- R1 接线在册 ----------
    ok("R1 模块在册：mod 声明 + 四个调用点各接到对的函数",
       "pub mod forum_reward;" in read("apps/api/src/repo/community/mod.rs")
       and "forum_reward::topic_reward" in read(TC)
       and "forum_reward::reply_reward" in read(PW)
       and read(PE).count("forum_reward::granted_by_idem") == 1
       and read(PTD).count("forum_reward::granted_by_idem") == 1
       and "forum_reward::reply_idem(author_id, pid)" in read(PE)
       and "forum_reward::topic_idem(op_id, tid)" in read(PTD),
       "接线被搬走＝面板又变成摆设")
    # ---------- R2 金额实参位 ----------
    earn = args_at(TC, "earn_spark") + args_at(PW, "earn_spark")
    claw = args_at(PE, "clawback_spark") + args_at(PTD, "clawback_spark")
    lit = [a for a in earn + claw if re.fullmatch(r"-?\d+", a or "")]
    off = [a for a in earn + claw if a not in ARG_OK]
    ok("R2 金额实参位：发放 %s / 收回 %s（字面量 %s / 认不出的 %s）"
       % (earn, claw, lit or "无", off or "无"),
       bool(earn) and bool(claw) and not lit and not off,
       "出现字面量＝发放或收回又绕开面板/流水；认不出＝口径搬家，逐条定性")
    ok("R2b 收回前有 owed > 0 门槛（两处都有）",
       read(PE).count("if owed > 0 {") == 1
       and read(PTD).count("if owed > 0 {") == 1,
       "缺门槛＝无发放记录也被送去 clawback 并打出「收回失败」假告警")
    # ---------- R3 纯函数 ----------
    p = subprocess.run(["cargo", "test", "--bin", "flux-api",
                        "forum_reward::"], capture_output=True, text=True,
                       encoding="utf-8", errors="replace",
                       cwd=os.path.join(ROOT, "apps", "api"))
    tail = [ln for ln in (p.stdout or "").split("\n") if "test result" in ln]
    ok("R3 面板值归一的纯函数单测通过（%s）" % (tail or "无输出"),
       p.returncode == 0 and "0 failed" in " ".join(tail),
       "边界（负数/小数/超长位数）不再有守卫")
    # ---------- R4 源码字面 SQL 的夹具 A/B ----------
    new_debt = rust_sql(TD, "pub async fn reply_reward_debt")
    old_debt = rust_sql(TD, "pub async fn reply_reward_debt", FIX + "^")
    owed_sql = rust_sql(FR, "pub async fn granted_by_idem")
    base = psql("SELECT max(id) FROM spark_ledger; SELECT max(id) FROM "
                "topics; SELECT max(id) FROM posts;").split("\n")
    sl, tp, ps = (base + ["0", "0", "0"])[:3]
    fx_tid = "(SELECT tid FROM fx)"
    fx_op = "(SELECT op FROM fx)"
    debt = lambda s: (s.replace("$1", fx_tid).replace("$2", fx_op))  # noqa
    owed_a = owed_sql.replace(
        "$1", "'forum-topic:' || %s || ':' || %s" % (fx_op, fx_tid))
    owed_b = owed_sql.replace("$1", "'forum-topic:0:0'")
    fixture = r"""
\set ON_ERROR_STOP on
BEGIN;
CREATE TEMP TABLE fx ON COMMIT DROP AS
SELECT %s + 1 AS tid, %s + 1 AS p0, %s + 1 AS l0,
       (SELECT forum_id FROM topics WHERE forum_id IS NOT NULL LIMIT 1) AS fid,
       (SELECT min(id) FROM users) AS op,
       (SELECT id FROM users ORDER BY id OFFSET 1 LIMIT 1) AS a,
       (SELECT id FROM users ORDER BY id OFFSET 2 LIMIT 1) AS b;
INSERT INTO topics (id, forum_id, title, user_id)
SELECT tid, fid, 'p456-gate-topic', op FROM fx;
INSERT INTO posts (id, topic_id, user_id, body)
SELECT p0, tid, op, 'p456-gate-op-first' FROM fx
UNION ALL SELECT p0+1, tid, a, 'p456-gate-a-1' FROM fx
UNION ALL SELECT p0+2, tid, a, 'p456-gate-a-2' FROM fx
UNION ALL SELECT p0+3, tid, b, 'p456-gate-b-none' FROM fx;
INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key,
                          balance_after)
SELECT l0, op, 2, 'forum', 'forum-topic:' || op || ':' || tid, -1 FROM fx
UNION ALL SELECT l0+1, a, 3, 'forum', 'forum-reply:' || a || ':' || (p0+1), -1
FROM fx
UNION ALL SELECT l0+2, a, 3, 'forum', 'forum-reply:' || a || ':' || (p0+2), -1
FROM fx;
SELECT 'A|' || (t.*)::text FROM (%s) t ORDER BY 1;
SELECT 'B|' || (t.*)::text FROM (%s) t ORDER BY 1;
SELECT 'C|' || COALESCE((%s), 0);
SELECT 'D|' || COALESCE((%s), 0);
ROLLBACK;
SELECT 'E|' || count(*) FROM topics WHERE title = 'p456-gate-topic';
SELECT 'F|' || count(*) FROM posts WHERE body LIKE 'p456-gate-%%';
SELECT 'G|' || count(*) FROM spark_ledger WHERE id > %s;
""" % (tp, ps, sl, debt(new_debt), debt(old_debt), owed_a, owed_b, sl)
    got, err = {}, None
    try:
        for ln in psql(fixture).split("\n"):
            if "|" in ln:
                k, v = ln.split("|", 1)
                got.setdefault(k, []).append(v.strip())
    except Exception as ex:  # noqa: BLE001
        err = str(ex)[:240]

    def rows(key):
        """解析 `(uid,额)` 元组文本（列名两版不同，按位置取）。"""
        out = []
        for v in got.get(key, []):
            m = re.match(r"\((\d+),\s*(-?\d+)\)", v)
            out.append("%s|%s" % (m.group(1), m.group(2)) if m
                       else "解析失败:%s" % v[:60])
        return sorted(out)

    a_new, a_old = rows("A"), rows("B")
    ok("R4 SQL A/B：旧口径=%s（按条数）→ 新口径=%s（按当年发放额），"
       "发帖回查=%s、查不到=%s"
       % (a_old, a_new, got.get("C"), got.get("D")),
       not err and a_new == ["16|6", "17|0"] and a_old == ["16|2", "17|1"]
       and got.get("C") == ["2"] and got.get("D") == ["0"],
       "新口径不等于发放额＝收回侧没跟着面板走；err=%s" % (err or "无"))
    ok("R4b 夹具零残留（topics/posts/ledger 回读 %s/%s/%s）"
       % (got.get("E"), got.get("F"), got.get("G")),
       all(got.get(k) == ["0"] for k in ("E", "F", "G")),
       "残留会弄脏演示站的论坛列表与流水")
    # ---------- R5 现网对账（当天行为不变的账面） ----------
    one = ("SELECT (SELECT value FROM site_settings "
           "WHERE name='starttopic') || '|' || (SELECT value FROM "
           "site_settings WHERE name='makepost') || '|' || "
           "(SELECT COALESCE(string_agg(amount::text, ','), '无') FROM "
           "(SELECT amount FROM spark_ledger WHERE idempotency_key LIKE "
           "'forum-topic:%' ORDER BY id DESC LIMIT 3) t) || '|' || "
           "(SELECT COALESCE(string_agg(amount::text, ','), '无') FROM "
           "(SELECT amount FROM spark_ledger WHERE idempotency_key LIKE "
           "'forum-reply:%' ORDER BY id DESC LIMIT 3) t);")
    st, mp, top_amt, rep_amt = (psql(one).split("|") + ["?"] * 4)[:4]
    ok("R5 现网读数：面板 starttopic=%s / makepost=%s，最近发帖流水=%s、"
       "回帖=%s（一致＝接线当天零行为变化）" % (st, mp, top_amt, rep_amt),
       top_amt.split(",")[0] == st and rep_amt.split(",")[0] == mp,
       "不一致＝站长改过面板——正是这批修复要让改动生效的场景")
    # ---------- R6 幂等键两处同源 ----------
    key = psql("SELECT idempotency_key FROM spark_ledger WHERE "
               "idempotency_key LIKE 'forum-topic:%' ORDER BY id DESC "
               "LIMIT 1;")
    shape = re.findall(r'format!\("forum-(topic|reply):\{(\w+)\}:\{(\w+)\}"',
                       read(FR))
    ok("R6 幂等键同源：源码 format! 形状 %d 个，流水里的真实键 %s"
       % (len(shape), key), len(shape) == 2
       and re.fullmatch(r"forum-topic:\d+:\d+", key or "") is not None,
       "只改一边＝收回读不到旧流水，白拿火花")
    print("   [L] 界面层未验：现网 api 仍按常数发放。升硬需站方重建后，把 "
          "starttopic 改成非 2 再发一帖，流水增量应等于面板值（闸门不代改"
          "站长设置，故不自动跑）")
    if "--selftest" in sys.argv:
        selftest()
    summary()


def flip(rel, a, b):
    raw = io.open(os.path.join(ROOT, rel), encoding="utf-8", newline="").read()
    crlf = "\r\n" in raw
    oa = a.replace("\n", "\r\n") if crlf else a
    ob = b.replace("\n", "\r\n") if crlf else b
    if raw.count(oa) != 1 and raw.count(ob) != 1:
        raise AssertionError("翻转锚点不唯一：%s" % rel)
    io.open(os.path.join(ROOT, rel), "w", encoding="utf-8",
            newline="").write(raw.replace(oa, ob, 1) if raw.count(oa) == 1
                              else raw.replace(ob, oa, 1))


def selftest():
    """翻回常数 / 撤掉门槛 ⇒ R2、R2b 必须红；finally 按字节还原并复跑正向。

    这两个文件里还留着站方的在途 hunk，所以还原是否干净只能比**字节**，
    不能比 `git diff --numstat`（那会把别人的改动算成我没还原）。
    """
    pairs = [(TC, 'earn_spark(&state.repo.db, auth.id, topic_award, '
                   '"forum", &idem)',
              'earn_spark(&state.repo.db, auth.id, 2, "forum", &idem)'),
             (PE, "        if owed > 0 {", "        if true {")]
    snaps = {}
    try:
        for rel, _a, _b in pairs:
            snaps[rel] = io.open(os.path.join(ROOT, rel), "rb").read()
        for rel, a, b in pairs:
            flip(rel, a, b)
        args = args_at(TC, "earn_spark") + args_at(PE, "clawback_spark")
        lit = [a for a in args if re.fullmatch(r"-?\d+", a or "")]
        gate = read(PE).count("if owed > 0 {")
        ok("S1 负控：翻回常数后 R2 抓到字面量 %s、R2b 门槛掉到 %d"
           % (lit, gate), bool(lit) and gate == 0,
           "翻红了没抓到＝判据不承重")
    finally:
        for rel, blob in snaps.items():
            io.open(os.path.join(ROOT, rel), "wb").write(blob)
    back = all(io.open(os.path.join(ROOT, rel), "rb").read() == blob
               for rel, blob in snaps.items())
    args = args_at(TC, "earn_spark") + args_at(PE, "clawback_spark")
    ok("S2 负控还原：两文件按字节回到翻转前（%s）且正向复跑无字面量（%s）"
       % (back, args), back and not [a for a in args
                                     if re.fullmatch(r"-?\d+", a or "")],
       "还原不干净＝把翻转态留在了工作树")


if __name__ == "__main__":
    main()
