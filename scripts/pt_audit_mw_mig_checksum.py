# -*- coding: utf-8 -*-
"""MW：迁移台账 `_sqlx_migrations` 与工作树文件逐条对账（只读，不改数据）。

为什么需要这把尺：sqlx 只按**版本号**记账，同一号不可能落两个文件，也不允许
「库里记着、代码里没有」。三种坏形状都在这条线上炸：

  1. 同号两个文件（11-02 实测：站方 3288effb 的 `0449_bg_url_or_path.sql` 与
     已记账的 `0449_official_team_rules.sql` 撞号）——台账 checksum 精确等于
     后者，前者必然 `MigrateError::VersionMismatch(449)` ⇒ api 启动即退出；
     干净安装则在第二次 apply 撞 `_sqlx_migrations` 主键。
  2. 已应用的迁移事后被改内容（P-442 同一形状）——存量站永远拿不到新增部分，
     且 checksum 一变，下一次用新镜像启动就 VersionMismatch。
  3. 台账超前于文件（库里 456 已记、工作树/镜像只到 455）——sqlx 的
     `validate_applied_migrations` 直接 `VersionMissing` ⇒ 崩溃循环。

判据不写死号：读台账、读目录，两边对上就算绿。负控在末尾用纯函数造一对
必不匹配的输入，确认这把尺真的会红（而不是恒绿）。
"""
import hashlib
import os
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MIG = os.path.join(ROOT, "apps", "api", "migrations")


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAF", "|", "-v", "ON_ERROR_STOP=1",
         "-c", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        raise SystemExit("psql failed: %s" % (r.stderr or "")[:300])
    return (r.stdout or "").strip()


def tree_versions():
    """工作树里的迁移：版本号 → [文件名]（同号多个 = 坏形状 1）。"""
    out = {}
    for fn in sorted(os.listdir(MIG)):
        if not fn.endswith(".sql") or not fn[:4].isdigit():
            continue
        out.setdefault(int(fn[:4]), []).append(fn)
    return out


def file_sha384(name):
    with open(os.path.join(MIG, name), "rb") as fh:
        return hashlib.sha384(fh.read()).hexdigest()


def recon(rows, files, digest):
    """纯判据：台账 (version, checksum) × 文件表 → 三类问题列表。

    rows: [(version:int, checksum:str), ...]（调用方按序给）
    files: {version: [文件名]}
    digest: version -> 该号文件的 sha384（同号多文件时返回 None）
    """
    dup = ["%d：%d 个同号文件（%s）" % (v, len(fs), ", ".join(fs))
           for v, fs in files.items() if len(fs) > 1]
    mismatch, ghost = [], []
    for v, ck in rows:
        if v not in files:
            ghost.append("%d：台账在册、工作树没有该号"
                         "（sqlx validate 会 VersionMissing）" % v)
            continue
        if len(files[v]) > 1:
            continue  # 已进 dup，这里不重复报
        got = digest(v)
        if got != ck:
            mismatch.append("%d：%s checksum 台账 %s… ≠ 树 %s…"
                            % (v, files[v][0], ck[:12], got[:12]))
    return dup, mismatch, ghost


files = tree_versions()
ledger = []
for ln in psql("SELECT version::text || '|' || encode(checksum,'hex') "
               "FROM _sqlx_migrations ORDER BY version").splitlines():
    if "|" in ln:
        v, ck = ln.split("|")
        ledger.append((int(v), ck))
digest = lambda v: file_sha384(files[v][0])  # noqa: E731
dup, mismatch, ghost = recon(ledger, files, digest)

tree_max = max(files) if files else 0
led_max = max(v for v, _ in ledger) if ledger else 0
pending = sorted(v for v in files if v > led_max and len(files[v]) == 1)
holes = sorted(v for v in range(led_max + 1, tree_max) if v not in files)

print("台账 %d 条（最大 %d）；工作树 %d 个文件（最大 %d）"
      % (len(ledger), led_max, sum(len(x) for x in files.values()), tree_max))
if dup:
    print("[FAIL] MW1 同号迁移 %d 组（api 启动即 VersionMismatch / 主键冲突）："
          % len(dup))
    for d in dup:
        print("   " + d)
if mismatch:
    print("[FAIL] MW2 checksum 不符 %d 条（已应用的迁移被改过内容，"
          "存量站拿不到改动，且新镜像启动即崩）：" % len(mismatch))
    for m in mismatch[:10]:
        print("   " + m)
if ghost:
    print("[FAIL] MW3 台账超前 %d 条（库里记着、代码没有）：" % len(ghost))
    for g in ghost[:10]:
        print("   " + g)
    print("   ↑ 判据只对工作树核；若只是现网镜像比库旧，走 "
          "scripts/update.sh 的迁移守卫核对镜像内容")
if holes:
    print("[FAIL] MW4 台账上界与树最大号之间真空 %s" % holes)
if pending:
    print("[NOTE] MW5 %d 个号尚未应用（%s）——迁移只在 api 启动时跑，"
          "随镜像重建自动执行，别手工写库（手工写会让台账超前于旧镜像）"
          % (len(pending), ",".join(str(v) for v in pending)))

# ---- 负控：纯函数必须能报出「改过一个字节」的那条 ----
one = {456: ["0456_a.sql"]}
ck_ok = file_sha384(files[list(files)[-1]][0]) if files else "x"
d_bad = lambda _v: "0" * 96  # noqa: E731
dup_t, mis_t, ghost_t = recon([(456, ck_ok)], one, d_bad)
if len(mis_t) != 1 or len(ghost_t) != 0:
    print("[FAIL] MW-负控：同一号换内容没有被报出来（mis=%d ghost=%d）"
          % (len(mis_t), len(ghost_t)))
    raise SystemExit(1)
_, _, ghost_t2 = recon([(9999, ck_ok)], one, d_bad)
if len(ghost_t2) != 1:
    print("[FAIL] MW-负控：台账超前（号 9999 不在树里）没有被报出来")
    raise SystemExit(1)
# 3288effb 当时的真实形状：同号两个文件（内容不同）
dup_t2, _, _ = recon([], {449: ["0449_a.sql", "0449_b.sql"]}, d_bad)
if len(dup_t2) != 1:
    print("[FAIL] MW-负控：同号两个文件没有被报出来（dup=%d）" % len(dup_t2))
    raise SystemExit(1)
print("MW6 负控成立：换内容 → MW2 命中 1 条；号缺失 → MW3 命中 1 条；"
      "同号两文件 → MW1 命中 1 条")

if not dup and not mismatch and not ghost and not holes:
    print("== ALL PASS（同号 0、checksum 不符 0、台账超前 0、真空 0、"
          "待应用 %d）==" % len(pending))
    raise SystemExit(0)
raise SystemExit(1)
