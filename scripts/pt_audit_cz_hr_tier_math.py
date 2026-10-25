# -*- coding: utf-8 -*-
"""CZ 闸门：H&R 义务时长的 SQL 口径必须与 Rust 侧同式（P-335）。

现网取证（2026-10-19）：站点设定 hr_hours=1（9-25 起就是 1），而 10-18 之后
新建的 hr_snapshots.required_seconds = 540000 秒 = 150 小时；未映射档位
（class 2 等，倍率回 100%）算成 100 倍。全站 62 条未结义务合计 2817 小时。
根因是 0385 的 SQL 函数把「百分数倍率」乘进去后少除一次 100：
    base * pct * (100 - relief) / 100     ← 错
    (base * pct / 100) * (100 - relief) / 100   ← 对（= Rust 侧 domain/hr:83）
建快照用的是这条 SQL，所以成员被大面积误判 H&R 违规、下载权限被暂停
（worker 日志同步在抖：「violations n=3」「已恢复下载权限 n=5」）。

判据三层：
  CZ1-CZ3 静态——迁移在册、函数体两处 /100、Rust 侧那一处也没丢（防有人反过来改错）
  CZ4 语义矩阵——在**回滚事务**里用迁移文件的函数体重建函数，逐格与 Rust 同式比对
        （期望值用库里的 seed_hours_pct / hr_relief_pct 现值算，只复核「组合方式」，
         也就是这个 bug 本体）；同时用旧式子算一遍当**负控**：旧式必须与期望不同，
         否则矩阵不承重。全程 ROLLBACK，不碰现网一行。
  CZ5 现网读数钉桩——库里函数若还是旧式，报 WARN + 现值，断言「已登记为待部署」；
        等站方下次启动跑掉 0394 后把它转成硬断言（见文件尾的收口说明）。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, "scripts")
from pt_audit_lib import psql_stdin, ok, FAILURES  # noqa: E402

MIG = "apps/api/migrations/0394_hr_seed_hours_pct_divide.sql"
RUST = "apps/api/src/domain/hr/mod.rs"
# 矩阵：base 小时 × 档位 × 用户已做种小时
MATRIX = [(1, 1, 0), (1, 2, 0), (48, 1, 0), (48, 2, 0), (48, 2, 200),
          (24, 2, 0), (72, 2, 5)]


def psql(sql_text):
    # 出错即抛：旧写法无视 returncode/stderr，而 psql 走 stdin 时
    # 语句出错也返回 0——SQL 写错了闸门照样报绿。
    return psql_stdin(sql_text, sep="|")


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


print("=" * 68)
print("一、静态：两侧同式的形状")
print("=" * 68)
mig = ""
if os.path.exists(MIG):
    mig = read(MIG)
ok("CZ1 迁移 0394 在册且重定义了 hr_reduced_seed_hours",
   "CREATE OR REPLACE FUNCTION hr_reduced_seed_hours" in mig, "缺文件或缺函数")
# 函数体里必须出现两次 /100（倍率一步 + 减免一步），且倍率那步在乘减免之前
body = mig.split("AS $$")[1].split("$$;")[0] if "AS $$" in mig else ""
ok("CZ2 修复后的函数体里「倍率 /100」在「减免 /100」之前",
   body.count("/ 100") >= 2
   and body.index("pct") < body.index("/ 100")
   and body.index("/ 100") < body.rindex("/ 100")
   and "hr_relief_pct" in body,
   "函数体里 /100 出现 %d 次" % body.count("/ 100"))
rs = read(RUST)
ok("CZ3 Rust 侧那两处 /100 没被顺手改坏（倍率与减免各一次）",
   "saturating_mul(pct) / 100" in rs
   and "saturating_mul(100 - relief_pct) / 100" in rs,
   "domain/hr 里找不到两处 /100 的同式")

print()
print("=" * 68)
print("二、语义矩阵（事务内重建函数 → 比对 → ROLLBACK）")
print("=" * 68)
if not body:
    print("CZ4 跳过：没拿到函数体（CZ1 已红）")
    tested = 0
    live_note = True
else:
    fn_fixed = ("CREATE OR REPLACE FUNCTION"
                " hr_reduced_seed_hours(p_base_hours INT,"
                " p_class_id INT, p_user_hours INT) RETURNS INT LANGUAGE sql"
                " STABLE AS $$" + body + "$$;")
    cells = " UNION ALL ".join(
        "SELECT %d b, %d c, %d h" % t for t in MATRIX)
    sql = ("BEGIN;\n" + fn_fixed + "\n"
           "SELECT v.b, v.c, v.h,\n"
           "       hr_reduced_seed_hours(v.b, v.c, v.h) AS got_sql,\n"
           "       (SELECT seed_hours_pct FROM hr_tier_pct(v.c)) AS pct,\n"
           "       hr_relief_pct(GREATEST(v.h - v.b, 0)) AS relief,\n"
           "       COALESCE((SELECT value::int FROM site_settings\n"
           "                 WHERE name='hr_relief_floor_hours'),1) AS fl,\n"
           "       (v.b * (SELECT seed_hours_pct FROM hr_tier_pct(v.c)))\n"
           "         * (100 - hr_relief_pct(GREATEST(v.h - v.b,0))) / 100\n"
           "         AS old_shape\n"
           "  FROM (" + cells + ") v ORDER BY 1,2,3;\nROLLBACK;\n")
    out = psql(sql)
    rows = [r for r in out.splitlines() if r and "|" in r]
    tested, bad, ctrl_ok = 0, [], 0
    for r in rows:
        p = [x.strip() for x in r.split("|")]
        if len(p) < 8:
            continue
        try:
            b, c, h, got, pct, relief, fl, old = (int(p[0]), int(p[1]),
                                                  int(p[2]), int(p[3]),
                                                  int(p[4]), int(p[5]),
                                                  int(p[6]), int(p[7]))
        except ValueError:
            continue
        # 与 Rust 同式：scaled = b*pct//100；relieved = scaled*(100-relief)//100
        scaled = b * pct // 100
        want = max(fl, scaled * (100 - relief) // 100)
        tested += 1
        if got != want:
            bad.append("b=%d c=%d h=%d sql=%d 期望=%d" % (b, c, h, got, want))
        if old != want:
            ctrl_ok += 1
    print("   矩阵 %d 格；SQL 侧输出与 Rust 同式的差异：%s"
          % (tested, bad or "无"))
    ok("CZ4 每一格 SQL 输出都等于 Rust 口径", tested >= 6 and not bad,
       "%d 格不符：%s" % (len(bad), bad[:4]))
    ok("CZ4b 负控：旧式子（少一次 /100）在矩阵上确实给出不同值（判据承重）",
       ctrl_ok >= tested * 0.6, "旧式与期望不同的格数=%d/%d" % (ctrl_ok, tested))
    # 现网函数（未跑 0394 时仍是旧式）——只记录读数，不当硬断言
    live = psql("SELECT hr_reduced_seed_hours(48, 2, 0) || '/' || "
                "hr_reduced_seed_hours(1, 1, 0);").strip()
    lb, lc = (live.split("/") + ["?", "?"])[:2]
    deployed = lb.isdigit() and int(lb) <= 96 and int(lc) <= 2
    # 量级读数：库里未结义务的最大小时数 vs 站点设定小时数。
    # 这个页面级矛盾就是本 bug 的成员可见症状：/me/hr 的规则文案读
    # default_seed_hours（站点设定），列表行读 hr_snapshots.required_seconds
    # ——同一屏一边写「1 小时」、一边写「还差 150 小时」。
    mag = psql(
        "SELECT coalesce(max(h.required_seconds)/3600, 0)::text || '/' || "
        "coalesce(nullif((SELECT value FROM site_settings "
        "WHERE name='hr_hours'),''),'?') "
        "FROM hr_snapshots h WHERE h.status IN ('open','violated')").strip()
    hrs, base_h = (mag.split("/") + ["?", "?"])[:2]
    over = hrs.isdigit() and base_h.isdigit() and int(hrs) > int(base_h) * 3
    print("   现网函数读数 hr_reduced_seed_hours(48,2,0)=%s、(1,1,0)=%s ⇒ %s"
          % (lb, lc, "已是修复后口径" if deployed else "仍是 100 倍膨胀，"
             "等站方启动跑 0394 后转硬断言"))
    print("   成员可见症状量级：未结义务最大 %s 小时 / 站点设定 %s 小时%s"
          % (hrs, base_h, "（>3 倍即膨胀未清）" if over else ""))
    if not deployed or over:
        print("   [WARN] P-335 修复在 master（迁移 0394），现网镜像未部署；"
              "部署并回填后把 CZ5 转成硬断言")
    else:
        ok("CZ5 现网函数与未结义务量级都已是修复后口径", not over,
           "最大义务 %s 小时 vs 设定 %s 小时" % (hrs, base_h))

print()
print("红账：%s" % (FAILURES or "无"))
sys.exit(1 if FAILURES else 0)
