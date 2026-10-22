# -*- coding: utf-8 -*-
"""KM 保种结算幂等键：认领人 + 认领轮次（P-559，2026-10-22 资深用户深测）。

缺陷本体：旧键 `preserve:<种号>:<档>` 把同一枚种的历次认领挤进同一条阶梯，
而「已发次数」按当前认领人算。种子「凑够人数出区 → 又死回区」（class_adj
的 ON CONFLICT 把 claimed_by 清零）或认领人被删号（user_del 置 NULL）后换人
接手，新一任数到自己 0 条 ⇒ 档号回到 1，撞键判据却按整枚种算 ⇒ 上一任的
`preserve:<种>:1` 早已在册 ⇒ 新认领人每天都被洗掉，一天都领不到且无报错。

本闸门直接**从源码里抠出那条 SQL**（不是抄一份），在事务内造夹具跑真语句：
  KM0 抠取自证：SQL 非空、含 preserve_reward、含 cyc 轮次片段
  KM1 真语句在「第二任认领人」夹具下必须发出 1 条，键含认领人 + 轮次 + :1
  KM2 同事务再跑一次必须 0 条（当日不双发）
  KM3 负控：同一夹具喂旧键形 SQL 必须 0 条（证明判据承重、不是恒绿）
  KM4 缺 claimed_at 的认领不参与结算（防 idempotency_key 拼成 NULL 绕过触发器）
  KM5 回滚后残留回读：本轮专属夹具件一个都不许留在库里

只读性质：全部语句跑在一个 `BEGIN … ROLLBACK` 会话里；KM5 在事务外回读。
"""
import io
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "apps", "worker", "src", "jobs", "preserve.rs")

FAIL = []
PASS = 0


def ok(name, cond, detail=""):
    global PASS
    if cond:
        PASS += 1
        print("[PASS] %s%s" % (name, ("  " + detail) if detail else ""))
    else:
        FAIL.append(name)
        print("[FAIL] %s  %s" % (name, detail))


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-v", "ON_ERROR_STOP=1", "-tA", "-F", "|"],
        input=sql, capture_output=True, text=True, encoding="utf-8",
        errors="replace")
    err = (r.stderr or "").strip()
    if r.returncode != 0 or "ERROR" in err:
        raise RuntimeError("psql 失败：%s" % err[:400])
    return (r.stdout or "").strip().splitlines()


def source_sql():
    """抠出 preserve_settle 的 r#\"…\"# 原文；找不到返回空串（KM0 会红）。"""
    s = io.open(SRC, encoding="utf-8", errors="replace").read()
    i = s.find("pub async fn preserve_settle")
    if i < 0:
        return ""
    m = re.search(r'r#"(.*?)"#', s[i:], re.S)
    return m.group(1) if m else ""


OLD_SQL = """WITH due AS (
    SELECT sp.claimed_by AS user_id, sp.torrent_id,
           (SELECT count(*) FROM spark_ledger l
            WHERE l.kind = 'preserve_reward'
              AND l.idempotency_key LIKE 'preserve:' || sp.torrent_id || ':%'
              AND l.user_id = sp.claimed_by) + 1 AS day_index,
           COALESCE((SELECT max(l.created_at) FROM spark_ledger l
            WHERE l.kind = 'preserve_reward'
              AND l.idempotency_key LIKE 'preserve:' || sp.torrent_id || ':%'
              AND l.user_id = sp.claimed_by), sp.claimed_at) AS last_paid_at
    FROM seed_preserve sp
    JOIN snatches s ON s.torrent_id = sp.torrent_id
      AND s.user_id = sp.claimed_by AND s.seeding
    WHERE sp.claimed_by IS NOT NULL AND sp.exited_at IS NULL
      AND s.last_port > 0 AND NOT COALESCE(s.connectable = 0, false)
      AND now() - COALESCE((SELECT max(l.created_at) FROM spark_ledger l
            WHERE l.kind = 'preserve_reward'
              AND l.idempotency_key LIKE 'preserve:' || sp.torrent_id || ':%'
              AND l.user_id = sp.claimed_by), sp.claimed_at) >= interval '1 day'
),
fresh AS (
    SELECT d.* FROM due d
    WHERE d.day_index >= 1 AND NOT EXISTS (
        SELECT 1 FROM spark_ledger l
        WHERE l.idempotency_key =
              'preserve:' || d.torrent_id || ':' || d.day_index
    )
)
INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id,
                          idempotency_key)
SELECT nextval('spark_ledger_id_seq'), user_id, 100, 'preserve_reward',
       'torrent', torrent_id, 'preserve:' || torrent_id || ':' || day_index
FROM fresh
WHERE torrent_id = __TID__;"""


def fixture(tid, claim_at):
    """夹具：种 tid 有保种行、认领人 root 在种、上一任已按旧键领 3 档。"""
    return base_fixture(tid, claim_at) + history(tid)


def base_fixture(tid, claim_at):
    """种 tid 的保种行 + root 在种（可重复调用，ON CONFLICT 覆盖）。"""
    return """
INSERT INTO snatches (user_id, torrent_id, uploaded, downloaded,
    seeded_seconds, leeching, seeding, hr_flag, last_up, last_down,
    last_seen_at, agent, progress, last_port, total_announces,
    near_cap_announces, reset_count, connectable)
VALUES (1, %d, 0, 0, 0, false, true, false, 0, 0, now(),
        'gate-km', 10000, 51413, 1, 0, 0, 1)
ON CONFLICT (user_id, torrent_id) DO UPDATE SET seeding = true,
    last_seen_at = now(), last_port = 51413, connectable = 1;
INSERT INTO seed_preserve (torrent_id, claimed_by, claimed_at,
    exited_at, last_settle_at)
VALUES (%d, 1, %s, NULL, %s)
ON CONFLICT (torrent_id) DO UPDATE SET claimed_by = 1,
    claimed_at = EXCLUDED.claimed_at, exited_at = NULL,
    last_settle_at = EXCLUDED.last_settle_at;
""" % (tid, tid, claim_at, claim_at)


def history(tid):
    """上一任认领人的旧键流水（只插一次，重插会撞 trg_spark_idem）。"""
    return """
INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id,
    idempotency_key, created_at)
SELECT nextval('spark_ledger_id_seq'),
       (SELECT min(id) FROM users WHERE id <> 1 AND status < 2),
       100, 'preserve_reward', 'torrent', %d,
       'preserve:%d:' || g, now() - make_interval(days => 30 + g)
FROM generate_series(1, 3) g;
""" % (tid, tid)


def run_script(steps):
    return [x for x in psql("\n".join(steps)) if x and not x.startswith("###")]


def main():
    sql = source_sql()
    ok("KM0 从源码抠出结算 SQL", bool(sql) and "preserve_reward" in sql
       and "cyc AS" in sql and "extract(epoch FROM sp.claimed_at)" in sql,
       "%d 字符" % len(sql))
    if not sql:
        return
    tid_rows = [x for x in psql(
        "SELECT 'TID|' || id FROM torrents WHERE approval_status = 1"
        " ORDER BY id LIMIT 1;") if x.startswith("TID|")]
    ok("KM0b 取到一枚过审种做夹具", len(tid_rows) == 1, str(tid_rows))
    if not tid_rows:
        return
    tid = int(tid_rows[0].split("|")[1])
    src = sql.replace("$1", "100").strip().rstrip(";")
    src = src.replace("FROM fresh", "FROM fresh WHERE torrent_id = %d" % tid)
    old = OLD_SQL.replace("__TID__", str(tid))

    # 计数只认本夹具那枚种、近 5 分钟的行；KM2 判「不双发」必须与 KM1
    # 后的读数做差（直接数绝对行数会把 KM1 那行算进 KM2 的账——假红）。
    def recent(tag):
        return ("SELECT '%s|' || count(*) FROM spark_ledger"
                " WHERE kind = 'preserve_reward' AND ref_type = 'torrent'"
                " AND ref_id = %d"
                " AND created_at > now() - interval '5 minutes';" % (tag, tid))

    def wipe():
        return ("DELETE FROM spark_ledger WHERE kind = 'preserve_reward'"
                " AND ref_type = 'torrent' AND ref_id = %d"
                " AND created_at > now() - interval '5 minutes';" % tid)

    tx = ["BEGIN;", recent("N0"), fixture(tid, "now() - interval '3 days'")]
    tx += ["SELECT '### KM1 生产语句：第二任认领人应得的行';", src + ";",
           "SELECT 'ROW|' || user_id || '|' || idempotency_key || '|' ||"
           " amount FROM spark_ledger WHERE kind = 'preserve_reward'"
           " AND ref_type = 'torrent' AND ref_id = %d AND user_id = 1"
           " AND created_at > now() - interval '5 minutes';" % tid,
           recent("N1")]
    tx += ["SELECT '### KM2 当日再跑一次';", src + ";", recent("N2")]
    tx += ["SELECT '### KM3 负控：旧键形同一夹具';", wipe(), old,
           recent("N3")]
    tx += ["SELECT '### KM4 缺 claimed_at 不结算';",
           base_fixture(tid, "NULL"), src + ";", recent("N4")]
    tx += ["ROLLBACK;"]
    out = run_script(tx)

    km1 = [x for x in out if x.startswith("ROW|")]
    key = km1[0].split("|")[2] if km1 else ""
    ok("KM1 第二任认领人拿到第 1 档",
       len(km1) == 1 and key.endswith(":1") and ":1:" in key, str(km1))

    def count_of(tag):
        rows = [x for x in out if x.startswith(tag + "|")]
        return rows[0].split("|")[1] if len(rows) == 1 else None

    base_n, n1, n2, n3, n4 = (count_of(t) for t in
                              ("N0", "N1", "N2", "N3", "N4"))
    # 判据用「与基线做差」：现网若恰有真实结算行，绝对数不为 0 也不算红。
    ok("KM2 同日不双发", None not in (base_n, n1, n2)
       and int(n1) == int(base_n) + 1 and n2 == n1,
       "N0=%s N1=%s N2=%s" % (base_n, n1, n2))
    ok("KM3 负控：旧键形 0 条（判据承重）", n3 == base_n,
       "N0=%s N3=%s" % (base_n, n3))
    ok("KM4 无认领时间不发奖", n4 == base_n, "N0=%s N4=%s" % (base_n, n4))

    # KM5：只认本轮夹具（该种 + 近 5 分钟），不拿全站 preserve 总数当判据
    # ——站点正常结算后全站必然非 0，旧写法会恒红。
    resid = psql(
        "SELECT 'preserve_keys|' || count(*) FROM spark_ledger"
        " WHERE kind = 'preserve_reward' AND ref_type = 'torrent'"
        " AND ref_id = %d AND created_at > now() - interval '5 minutes';"
        % tid)
    ok("KM5 夹具零残留", resid == ["preserve_keys|0"], str(resid))

    print("\n%d 条通过 / %d 条失败" % (PASS, len(FAIL)))
    if FAIL:
        print("失败：" + "、".join(FAIL))
        sys.exit(1)


main()
