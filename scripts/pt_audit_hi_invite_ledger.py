# -*- coding: utf-8 -*-
"""闸门 HI：邀请账本的真话不变量（批一百八十，修的是**我自己闸门**的账）。

现象（现网实测）：`invites` 里 **307 行 `status=2`(作废) 却带着 `used_by`**——
产品侧所有作废写口都有 `status = 0` 守卫（user_del.rs:114、user_status.rs:139、
invite_http.rs:240、admin_p3_http/invites.rs:201），造不出这种行；逐行核对后
307 行的邀请人全是 root、被邀请人全是闸门探针号（`deleted-*`/`bfp*`/…），
**零行牵连真实会员** ⇒ 是常驻闸门的收尾 SQL 在污染账本：
它们无守卫地 `UPDATE invites SET status = 2 WHERE id IN (...)`，把**已经被注册使用**
的码也刷成作废。

为什么不是「难看一点而已」：
  · `me_settings.rs:29-30` 与 `profile.rs:171` 按 `status=0/1` 数「待用/已邀请」
    ⇒ 被刷成 2 的码**从「已邀请人数」里消失**，会员看到的数字比真实少；
  · 作废语义（站方口径 = 「这码从没被用、现在收回」）与 `used_by` 指向一个已注册
    账号互相打脸，反小号/信任链复盘时读不出哪句是真。

本批做的事：
  1) 备份 307 行（`_gate_bak_invites_dirty`）→ 只把 `status` 改回 1 →
     用备份表逐列 `IS DISTINCT FROM` 核对「除 status 外零差异」→ 删备份表；
  2) 12 支闸门的收尾 SQL 一律补 `WHERE used_by IS NULL AND ...`（13 条语句）；
  3) 本闸门把两件事钉成常驻判据。

判据：
  HI1 现网不变量：`status=2 AND used_by IS NOT NULL` 必须 0 行；
  HI2 正控（防恒真橡皮章）：在**同一个 psql 会话**里 BEGIN→把一行改成坏形状→
      同会话计数必须 ≥1 → ROLLBACK；
  HI3 普查：scripts 里每条 `UPDATE invites SET status = 2` 都必须带
      `used_by IS NULL` 守卫，违例清单必须为空（新增闸门忘了带守卫就红）；
  HI4 承重自证：同一把尺喂「旧收尾语句」合成码必须报违例。
  HI5 产品侧作废写口都还带着 `status = 0` 守卫（防止有人把站方那道守卫改掉，
      那时站方自己也会造出这种行）。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hi_invite_ledger.py`
      HI_LIVE=0 跳过 HI1/HI2 的库侧查询（HI3-HI5 仍跑）。
"""
import glob
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

LIVE = os.environ.get("HI_LIVE", "1") != "0"
BAD = "status = 2 AND used_by IS NOT NULL"
VOID_RE = re.compile(r"UPDATE invites SET status = 2\b([^;\"']{0,200})")

OLD_GATE_SQL = 'psql("UPDATE invites SET status = 2 WHERE id IN (%s)" % ids)'


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-d", "fluxtorrent", "-q", "-tA"],
        input=q, capture_output=True, text=True, encoding="utf-8",
        errors="replace")
    err = (r.stderr or "")
    if r.returncode != 0 or "ERROR" in err:
        raise RuntimeError("psql: " + (err or "")[:300])
    return (r.stdout or "").strip()


def guard_missing(fragment):
    """作废语句的 WHERE 里没有 used_by IS NULL ⇒ 算违例。"""
    return "used_by IS NULL" not in fragment


violations = []
# 本闸门文件自己排除：它的文档串与 HI4 的合成控制码里必然出现「旧写法」字面，
# 按行扫会把叙述当成待改项（同一把尺先要不吃自己的说明书）
SELF = os.path.basename(__file__)
for p in sorted(glob.glob(os.path.join(ROOT, "scripts", "pt_audit_*.py"))):
    if os.path.basename(p) == SELF:
        continue
    s = io.open(p, encoding="utf-8", errors="replace").read()
    s = "\n".join(ln for ln in s.split("\n")
                  if not ln.strip().startswith("#"))
    for m in VOID_RE.finditer(s):
        if guard_missing(m.group(1)):
            ln = s[:m.start()].count("\n") + 1
            violations.append("%s:%d" % (os.path.basename(p), ln))

ok("HI3 闸门收尾的作废 SQL 全带 used_by IS NULL", violations == [],
   "违例=%s" % (violations[:5],))
ok("HI4 承重：旧写法会被本判据抓住",
   guard_missing(VOID_RE.search(OLD_GATE_SQL).group(1)),
   "合成旧语句判为违例=正确")

prod = {
    "user_del.rs": "apps/api/src/admin_http/user_del.rs",
    "user_status.rs": "apps/api/src/admin_http/user_status.rs",
    "invite_http.rs": "apps/api/src/invite_http.rs",
    "invites.rs": "apps/api/src/admin_p3_http/invites.rs",
}
bad_prod = []
for name, rel in prod.items():
    s = io.open(os.path.join(ROOT, rel), encoding="utf-8", errors="replace")\
        .read()
    s = "\n".join(ln for ln in s.split("\n")
                  if not ln.strip().startswith("//"))
    for m in re.finditer(r"UPDATE invites SET status = [23]\b", s):
        tail = s[m.end():m.end() + 200]
        if "status = 0" not in tail:
            bad_prod.append(name)
ok("HI5 站方作废写口都带 status = 0 守卫", bad_prod == [],
   "失去守卫的文件=%s" % sorted(set(bad_prod)))

if LIVE:
    dirty = psql("SELECT count(*) FROM invites WHERE %s" % BAD)
    ok("HI1 作废行不带 used_by（现网不变量）", int(dirty) == 0,
       "现网坏行=%s" % dirty)
    # HI2 正控：同会话造一行坏形状，判据必须数得到，然后回滚
    probe = psql(
        "BEGIN; UPDATE invites SET status = 2 "
        "WHERE used_by IS NOT NULL AND status = 1 "
        "AND id = (SELECT min(id) FROM invites "
        "WHERE used_by IS NOT NULL AND status = 1); "
        "SELECT count(*) FROM invites WHERE %s; ROLLBACK;" % BAD)
    n_probe = int(probe.splitlines()[-1] or 0)
    ok("HI2 正控：同形状的坏行判据数得到", n_probe >= 1,
       "事务内注入后计数=%s（回滚后应仍为 0）" % n_probe)
    after = psql("SELECT count(*) FROM invites WHERE %s" % BAD)
    ok("HI2b 正控未留痕", int(after) == 0, "回滚后再计数=%s" % after)
else:
    print("[HI1/HI2] HI_LIVE=0 跳过库侧查询")

summary()
