# -*- coding: utf-8 -*-
"""做种收益「页面预估 vs 实际到账」偏差巡检（只读，不写库）。

跑法：
    python scripts/audit_seeding_est_parity.py
    python scripts/audit_seeding_est_parity.py --hours 24 --threshold 0.15

为什么需要它：`/me/spark` 的 hourly_estimate（apps/api/src/economy_http/
seeding_est.rs）与 worker 的 seeding_reward（apps/worker/src/jobs/
seeding.rs）是两段独立 SQL，历史上漂移过一次 —— 旧预估漏了盒子倍率与
发布者加成、又多算了 port=0 的幽灵行，实测偏高 16.9%，0488 核对时修正。
任一侧改动没同步，用户看到的就是「页面说 +97、流水只到 +77」。

口径说明（重要）：预估算的是**当前**状态，流水是**结算当时**的状态。
一小时内 seeded_seconds 增长、last_seen_at 刷新、档位边界跨越都会造成
合理差异，所以阈值默认 25% —— 抓的是系统性漂移，不是逐颗精度。
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys

PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"),
      "-t", "-A", "-F", "|", "-v", "ON_ERROR_STOP=1", "-c"]

# 与 seeding_est.rs 的 EST_SQL 同源，只是从「单用户」扩成「本批用户」。
# 改那边必须同步改这里，否则巡检跑的是另一套口径、结果无意义。
SQL = r"""
WITH recent AS (
    SELECT user_id, sum(amount) AS amount, count(*) AS rows
    FROM spark_ledger
    WHERE kind = 'seeding_reward'
      AND created_at > now() - (:hours * interval '1 hour')
    GROUP BY user_id
),
p AS MATERIALIZED (
    SELECT *, seeding_official_mult() AS official_mult
    FROM seeding_params()
),
per_torrent AS (
    SELECT u.id AS user_id, donor_privileged(u.id) AS donor,
           seeding_torrent_bonus(
               t.size, t.seeders,
               (EXTRACT(EPOCH FROM (now() - t.created_at))
                / 86400.0)::double precision,
               t.times_completed,
               (GREATEST(s.seeded_seconds, 0) / 3600.0)::double precision,
               p.vol_base, p.rarity_k, p.rarity_exp, p.scale)
           * COALESCE(sb.bonus_factor, 1.0)
           * (CASE WHEN t.official_tag
                   THEN p.official_mult ELSE 1.0 END)
           * seeding_publisher_mult(
                 t.owner_id = u.id,
                 (EXTRACT(EPOCH FROM (now() - t.created_at))
                  / 86400.0)::double precision) AS b
    FROM users u
    JOIN snatches s ON s.user_id = u.id AND s.seeding
    LEFT JOIN seedbox_effective sb ON sb.user_id = u.id
    JOIN torrents t ON t.id = s.torrent_id
      AND s.last_port > 0
      AND NOT COALESCE(s.connectable = 0, false)
    CROSS JOIN p
    WHERE u.id IN (SELECT user_id FROM recent)
      AND u.status < 2
      AND NOT COALESCE(u.cheat_banned_until > now(), false)
      AND s.last_seen_at > now() - (7200 * interval '1 second')
      AND NOT EXISTS (
          SELECT 1 FROM cheat_events c
          WHERE c.user_id = u.id AND c.resolved_at IS NULL
            AND NOT c.false_positive
            AND (c.agent LIKE 'ghost:%' OR c.agent LIKE 'speed:%'
                 OR c.agent LIKE 'reset:%' OR c.agent LIKE 'corr:%')
          LIMIT 1
      )
),
per_user AS (
    SELECT user_id, donor, sum(b) AS bonus_raw, count(*) AS n
    FROM per_torrent GROUP BY user_id, donor
),
est AS (
    SELECT user_id,
           CASE WHEN n = 0 THEN 0::bigint
                ELSE seeding_hourly(bonus_raw, p.base, p.cap, p.curve_k,
                                    p.donor_mult, donor) END AS estimate
    FROM per_user CROSS JOIN p
)
SELECT r.user_id,
       r.amount,
       COALESCE(e.estimate, 0) AS estimate,
       r.rows
FROM recent r LEFT JOIN est e ON e.user_id = r.user_id
ORDER BY abs(COALESCE(e.estimate, 0) - r.amount) DESC
"""


def psql(sql: str) -> str:
    r = subprocess.run(PG + [sql], capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    if r.returncode != 0:
        print("psql 失败：\n" + r.stderr[:800], file=sys.stderr)
        sys.exit(2)
    return r.stdout.strip()


def pct(part: int, whole: int) -> float:
    if whole == 0:
        return 0.0
    return abs(part - whole) / float(whole)


def quantile(values: list[float], q: float) -> float:
    if not values:
        return 0.0
    s = sorted(values)
    idx = min(len(s) - 1, int(round(q * (len(s) - 1))))
    return s[idx]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--hours", type=int, default=6,
                    help="回看小时数（默认 6）")
    ap.add_argument("--threshold", type=float, default=0.25,
                    help="偏差告警阈值，0.25 = 25%%（默认）")
    ap.add_argument("--top", type=int, default=10,
                    help="明细最多列几个用户（默认 10）")
    a = ap.parse_args()

    rows = []
    for line in psql(SQL.replace(":hours", str(a.hours))).splitlines():
        parts = line.split("|")
        if len(parts) != 4:
            continue
        rows.append((int(parts[0]), int(parts[1]), int(parts[2]),
                     int(parts[3])))

    title = "做种收益预估 vs 到账偏差巡检"
    print("== %s（最近 %d 小时）==" % (title, a.hours))
    if not rows:
        print("窗口内没有 seeding_reward 流水 —— 结算没跑，或本地库无数据。")
        return
    print("样本用户 %d · 流水行 %d" % (len(rows),
                                   sum(r[3] for r in rows)))

    # 只对有实际到账的用户算相对偏差：到账 0 时相对值无意义（分母为 0），
    # 单独统计「页面有预估但一分没到」——那是最严重的口径不一致。
    devs = [pct(est, amt) for _, amt, est, _ in rows if amt > 0]
    ghost = [(uid, est) for uid, amt, est, _ in rows
             if amt == 0 and est > 0]

    if devs:
        print("偏差分布：P50 %.1f%% · P90 %.1f%% · P99 %.1f%% · max %.1f%%"
              % (quantile(devs, 0.50) * 100, quantile(devs, 0.90) * 100,
                 quantile(devs, 0.99) * 100, max(devs) * 100))

    bad = [(uid, amt, est, pct(est, amt)) for uid, amt, est, _ in rows
           if amt > 0 and pct(est, amt) > a.threshold]
    bad.sort(key=lambda x: -x[3])
    if bad:
        print("\n超阈值（>%.0f%%）用户 %d 个：" % (a.threshold * 100, len(bad)))
        print("  %-8s %-8s %-8s %s" % ("uid", "预估", "实到", "偏差"))
        for uid, amt, est, d in bad[:a.top]:
            print("  %-8d %-8d %-8d %+.1f%%" % (uid, est, amt,
                                                 (est - amt) / amt * 100))
        if len(bad) > a.top:
            print("  ...（其余 %d 个略）" % (len(bad) - a.top))

    if ghost:
        print("\n页面有预估、实际未到账 %d 个（最严重的口径不一致）："
              % len(ghost))
        for uid, est in ghost[:a.top]:
            print("  uid=%d 预估 %d 实到 0" % (uid, est))

    print("\n--- 结论 ---")
    if bad or ghost:
        print("发现口径漂移。先查两侧 SQL 是否有一处被单独改过：")
        print("  apps/api/src/economy_http/seeding_est.rs  (EST_SQL)")
        print("  apps/worker/src/jobs/seeding.rs             (seeding_reward)")
    else:
        print("未发现超阈值偏差，两侧口径一致。")
        print("注意：预估算当前状态、流水算结算当时，"
              "小幅差异属正常漂移。")


if __name__ == "__main__":
    main()
