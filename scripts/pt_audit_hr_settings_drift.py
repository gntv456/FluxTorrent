# -*- coding: utf-8 -*-
"""常驻闸门：H&R 三把参数不得停在「审计探针值」上（现网漂移 25 天的引信）。

现场（本批判定的取证）：`site_settings` 里
    hr_hours=1（09-25 起）hr_days=1（10-08 起）cancel_hr=100（10-05 起）
    hr_scan_lookback_hours=72（10-02 起）
这四个值恰好等于 `pt_audit_ah_hr.py` 的探针签名 SIG（该文件 G0 段自己写着
「10-18/10-19 两次崩溃实测：……一直留在现网」）。后果不是「多罚人」而是**反向失效**：
  * H&R 义务＝1 小时 / 考察 1 天 ⇒ 近乎无约束（0025 播种 120 小时、0311 播种 14 天）；
  * 销 H&R 价＝100 火花而不是 10000（0034 播 20000 → 0278 改 10000）⇒ 便宜 100 倍；
  * 扫描回看 72 小时而不是 6 小时。
根因是闸门的复原逻辑：崩溃后「原值」快照本身就是污染值，G0 只 [WARN] 一句
「请人工核对迁移默认值」，然后照旧把污染值当原值写回去——**污染被永久固化**。

本闸门不做修复脚本，做**引信**：
  D0 逐把键与「迁移里最后一次写入值」对账（迁移目录现场解析，不写死副本）
  D1 任何一把落在探针签名上 ⇒ 直接红（即使 D0 解析失败也兜得住）
  D2 正控：解析器必须认得那四把键与它们的出厂值（认不出＝尺子瞎，不许当「站干净」）
  D3 检测器承重：在**一个事务内**把 hr_hours 改成探针值、跑判定、当场 ROLLBACK，
     回滚后再读一次确认真的没落库（内存里的规矩：红线不许留下写痕）
  D4 复现本批现场：事务内把**四把键一起**摆成探针值 ⇒ D1 必须四把全点名，
     回滚后证明现网没被改动（「引信有效」要测出来，不是推断）
  D5 残留棘轮：AH 崩溃留下的 ahm 探针号（status<2 ⇒ 公开成员目录当在册会员显示，
     且每人还挂着一条未解除的 hr_warn_states）只许降不许增。本闸门**不删账号**，
     产品侧有自己的 purge 口，处理与否交站长
跑法：python -u scripts/pt_audit_hr_settings_drift.py
"""
import io
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MIG = os.path.join(ROOT, "apps", "api", "migrations")
KEYS = ["hr_days", "hr_hours", "cancel_hr", "hr_scan_lookback_hours"]
PROBE = {"hr_days": "1", "hr_hours": "1", "cancel_hr": "100",
         "hr_scan_lookback_hours": "72"}
# 2026-10-20 实测残留 3 个在册 ahm 探针号（崩溃收尾没跑完）。清完后把这里改小，
# 数量涨回去 = 当场红。
RESIDUE_CEIL = 3
INS = re.compile(r"\('%s',\s*'([^']+)'")
UPD = re.compile(r"UPDATE site_settings SET value\s*=\s*'([^']+)'"
                 r"[^;]*?WHERE name\s*=\s*'%s'")


def seeds_from_migrations():
    """按文件名顺序扫迁移，取每把键「最后一次被写入」的字面值。

    必须**按语句**判定：`('hr_hours', 'number', …)` 是 0039 给 settings_meta
    登记的行，形状和 site_settings 的播种行一模一样——不区分语句归属就会把
    类型名当出厂值（第一版就这么读出了 hr_hours='number'）。
    """
    seen = {}
    for f in sorted(n for n in os.listdir(MIG) if n.endswith(".sql")):
        t = io.open(os.path.join(MIG, f), encoding="utf-8",
                    errors="replace").read()
        t = re.sub(r"--[^\n]*", "", t)                 # 去行注释
        for stmt in t.split(";"):
            one = " ".join(stmt.split())
            if "INSERT INTO site_settings" in one:
                for k in KEYS:
                    m = re.search(INS.pattern % re.escape(k), one)
                    if m:
                        seen.setdefault(k, []).append((f, m.group(1)))
            elif "UPDATE site_settings" in one:
                for k in KEYS:
                    m = re.search(UPD.pattern % re.escape(k), one)
                    if m:
                        seen.setdefault(k, []).append((f, m.group(1)))
    return {k: v[-1] for k, v in seen.items()}


def live_values():
    got = psql_stdin("SELECT name || '=' || value FROM site_settings "
                     "WHERE name IN (%s) ORDER BY name"
                     % ",".join("'%s'" % k for k in KEYS))
    d = {}
    for line in got.split("\n"):
        if "=" in line:
            k, v = line.split("=", 1)
            d[k.strip()] = v.strip()
    return d


def main():
    seeds = seeds_from_migrations()
    ok("D2 正控：迁移解析必须认得全部四把键",
       all(k in seeds for k in KEYS), str(sorted(seeds)))
    exp = {k: seeds[k][1] for k in KEYS if k in seeds}
    print("   [D2] 迁移里的出厂值：%s"
          % ", ".join("%s=%s(%s)" % (k, exp.get(k, "?"), seeds[k][0][:12])
                      for k in KEYS))
    live = live_values()
    ok("D0a 四把键都在 site_settings 里（没被删行）",
       all(k in live for k in KEYS), str(sorted(live)))
    drift = [k for k in KEYS if k in live and k in exp and live[k] != exp[k]]
    ok("D0 逐把键 = 迁移最后一次写入值（漂移=%s）" % (drift or "无"),
       not drift, "现网=%s 应有=%s" % (live, exp))
    onprobe = [k for k in KEYS if live.get(k) == PROBE.get(k)]
    ok("D1 任何一把都不许停在审计探针值上（命中=%s）" % (onprobe or "无"),
       not onprobe, "探针签名=%s" % PROBE)

    # D3：事务内投毒 ⇒ 判据必须当场看见；随后 ROLLBACK 并证明没落库。
    # 读回要带标记：psql 把 ROLLBACK 也打到 stdout，「取最后一行」会取到它。
    tx = ("BEGIN; UPDATE site_settings SET value='%s' WHERE name='hr_hours'; "
          "SELECT 'MARK=' || value FROM site_settings WHERE name='hr_hours'; "
          "ROLLBACK;" % PROBE["hr_hours"])
    inside = psql_stdin(tx)
    marked = [ln.split("MARK=")[1].strip() for ln in inside.split("\n")
              if "MARK=" in ln]
    after = psql_stdin("SELECT value FROM site_settings WHERE name='hr_hours'")
    ok("D3a 检测器看得见投毒（事务内读到探针值 1）",
       marked == ["1"], inside.replace("\n", " ")[:90])
    ok("D3b 回滚后没落库（仍=%s，期望 %s）" % (after, exp.get("hr_hours")),
       after == exp.get("hr_hours"), "")

    # D4：把本批真实遇到过的「四把键同时停在探针值」在事务里复现一次，
    # 证明 D1 当时会红——否则「引信有效」只是我的推断，不是测出来的。
    poison_tx = ["BEGIN;"]
    for k in KEYS:
        poison_tx.append("UPDATE site_settings SET value='%s' WHERE name='%s';"
                         % (PROBE[k], k))
    poison_tx.append("SELECT 'MARK=' || name || '=' || value "
                     "FROM site_settings WHERE name IN (%s) ORDER BY name;"
                     % ",".join("'%s'" % k for k in KEYS))
    poison_tx.append("ROLLBACK;")
    snap = psql_stdin("\n".join(poison_tx))
    pv = {}
    for ln in snap.split("\n"):
        if "MARK=" in ln:
            k, v = ln.split("MARK=")[1].split("=")
            pv[k.strip()] = v.strip()
    hit = [k for k in KEYS if pv.get(k) == PROBE[k]]
    ok("D4 复现本批现场：四把键投毒后 D1 必须点名（命中=%s）" % hit,
       len(hit) == len(KEYS), snap.replace("\n", " ")[:120])
    now = live_values()
    ok("D4b 复现后现网没被动过（%s）" % now,
       all(now.get(k) == exp.get(k) for k in KEYS), "")

    # D5 残留棘轮：AH 崩溃留下的探针号——它们 status<2，会被公开成员目录
    # （user_directory 的 VISIBLE 只挡 `deleted-%`，挡不住 `ahm%`）当成在册会员，
    # 且每人还挂着一条没解开的 hr_warn_states（同批已停用的 25 个探针号是 0 条，
    # 说明正常收尾会先解除警告状态再停用）。
    # 本闸门**不替站长删账号**（产品侧有自己的 purge 口），只把现状钉成上界：
    # 残留只许降不许增，降了要顺手把 RESIDUE_CEIL 改小。
    res = psql_stdin("SELECT count(*) FROM users WHERE username LIKE 'ahm%' "
                     "AND status < 2")
    openw = psql_stdin("SELECT count(*) FROM hr_warn_states WHERE user_id IN "
                       "(SELECT id FROM users WHERE username LIKE 'ahm%' "
                       "AND status < 2)")
    ids = psql_stdin("SELECT string_agg(id::text, ',') FROM users "
                     "WHERE username LIKE 'ahm%' AND status < 2")
    ok("D5 AH 探针残留只许降不许增（在册=%s，上界=%d；未解除的警告行=%s）"
       % (res, RESIDUE_CEIL, openw), int(res or 0) <= RESIDUE_CEIL,
       "探针号=%s" % ids)
    print("   [D5] 待站长处理：ahm%% 在册探针号=%s（id=%s），"
          "其未解除 hr_warn_states=%s 条；同批已停用的 25 个残留是 0 条"
          % (res, ids, openw))
    summary()


if __name__ == "__main__":
    main()
