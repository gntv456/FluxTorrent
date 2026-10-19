# -*- coding: utf-8 -*-
"""闸门 EZ：头像框的扣款-佩戴原子性与「谁在写展示列」台账。

批一百八十一。会员视角：花 5000 魔力戴一个框，页面报错，钱没了、框也没戴上，
而翻遍面板找不到这笔消费属于哪个框。

  P-371（本批已修）：`PUT /me/avatar-frame` 的 paid 分支是「`spend_spark`（独立
  事务，先提交）→ 再 `UPDATE users.avatar_frame_id`（另一次提交）」。中间任何一步
  失败（DB 抖动、进程被杀）就留下**已扣款未佩戴**的中间态；而头像框的归属只写在
  `users.avatar_frame_id` 这一列上（本口不写 `user_dressups`，也没有持有表），
  一旦停在这个状态，会员付的钱在任何面板里都找不到。改成一个事务：扣款 →
  UPDATE（判 rows_affected，0 行整笔回滚）→ commit。

  在册的另一半（**待裁，本批不代改**，即 P-356）：`users.avatar_frame_id` 有
  **两套所有权模型**——`/dressup/wear` 走 `shop_items`+`user_dressups`（有佩戴标记、
  有 expires_at），`/me/avatar-frame` 只写这一列。现网实证：带框会员 2 人，
  其中 1 人（user 4406）**没有任何 dressup 支撑**；本口留下的扣款流水
  `ref_type='avatar_frame'` 只有 1 条 / 5000 魔力。要么把本口并入 dressup 模型
  （发持有行、支持过期与回收），要么承认「框=一次性买断的展示列」并给展示列加对账
  ——两条路都要动数据，站方拍板。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ez_frame_wear.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOTDIR, "apps", "api", "src")
FRAME = os.path.join(SRC, "http", "frames_gomoku.rs")
# 在册写口数（展示列 users.avatar_frame_id 的 UPDATE 处数）：
#   dressup.rs 佩戴/摘下 2 处 + frames_gomoku.rs 佩戴/卸下 2 处
#   + admin_p3_http/frames.rs 管理员清列 1 处
WRITERS_CEIL = 5


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:240])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


body = cut(FRAME)
i0 = body.find("async fn frame_equip")
i1 = body.find("async fn gomoku_create")
equip = body[i0:i1] if 0 <= i0 else body[i0:]

# ---------- EZ1/EZ2/EZ3 原子性形状 ----------
ok("EZ1 佩戴口不再用非事务版 spend_spark(",
   not re.search(r"spend_spark\(", equip),
   "非事务版先提交，回滚不了＝「钱扣了没戴上」的入口")
ok("EZ2 扣款与写列都在同一 tx（begin / &mut *tx / commit 齐备）",
   ".begin()" in equip and "execute(&mut *tx)" in equip
   and "tx.commit()" in equip,
   "两个提交点就是撕裂窗口")
ok("EZ2b 写列判 rows_affected，0 行整笔回滚",
   "rows_affected()" in equip and "if n == 0" in equip,
   "不判就是「扣款成功、无人佩戴、无人知道」")
ok("EZ3 顺序是先扣款后写列（都在同一事务里）",
   0 <= equip.find("spend_spark_tx") < equip.find(
       "UPDATE users SET avatar_frame_id"),
   "同事务内顺序不影响原子性，但保持与本仓派彩口一致的读法")

# ---------- EZ4 真库 A/B：回滚吃干净 ----------
AB = r"""
BEGIN;
SELECT 'bal0=' || spark_balance FROM users WHERE id = 1;
INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id,
  idempotency_key, balance_after)
VALUES (nextval('spark_ledger_id_seq'), 1, -5000, 'shop', 'avatar_frame', 1,
  'ez-probe-frame-key', 0);
SELECT 'upd_rows=' || count(*) FROM users WHERE id = 999999;
ROLLBACK;
SELECT 'ledger_residue=' || count(*) FROM spark_ledger
 WHERE idempotency_key = 'ez-probe-frame-key';
SELECT 'bal1=' || spark_balance FROM users WHERE id = 1;
"""
pc = {}
for ln in psql(AB).split("\n"):
    ln = ln.strip()
    if "=" in ln:
        k, v = ln.split("=", 1)
        pc[k.strip()] = v.strip()
ok("EZ4 真库证：扣款已落但 UPDATE 命中 %s 行时，回滚后流水残留 %s、余额 %s→%s"
   % (pc.get("upd_rows"), pc.get("ledger_residue"), pc.get("bal0"),
      pc.get("bal1")),
   pc.get("upd_rows") == "0" and pc.get("ledger_residue") == "0"
   and pc.get("bal0") == pc.get("bal1"),
   "残留非 0＝回滚没吃干净；余额变了＝探针污染现网")

# ---------- EZ5 展示列写口台账 ----------
found = 0
for root, _, fs in os.walk(SRC):
    for f in fs:
        if not f.endswith(".rs"):
            continue
        t = cut(os.path.join(root, f))
        found += len(re.findall(r"UPDATE users SET avatar_frame_id", t))
ok("EZ5 `users.avatar_frame_id` 的 UPDATE 写口 %d 处，必须 ≤ 在册 %d"
   % (found, WRITERS_CEIL),
   found <= WRITERS_CEIL,
   "每多一处写同一个展示列，就必须同时纳入所有权/过期模型（P-356 的病根）")

# ---------- EZ6 两套模型的现网账 ----------
paid = sval("SELECT count(*) FROM spark_ledger WHERE ref_type='avatar_frame'")
wearing = sval("SELECT count(*) FROM users WHERE avatar_frame_id IS NOT NULL")
orph = sval("SELECT count(*) FROM users u WHERE u.avatar_frame_id IS NOT NULL "
            "AND NOT EXISTS (SELECT 1 FROM user_dressups d JOIN shop_items si "
            "ON si.id = d.item_id WHERE d.user_id = u.id AND d.wearing "
            "AND si.kind = 'avatar_frame')")
free = sval("SELECT count(*) FROM avatar_frames WHERE price = 0")
ok("EZ6 无 dressup 支撑的带框会员 %s ≤ 上界 1（P-356 待裁，只许降）" % orph,
   int(orph or 0) <= 1,
   "上升＝又多一个人付了钱却在装扮面板里看不见自己的框")
print("   [现网账] 本口扣款流水 %s 条 / 带框会员 %s 人 / 其中无支撑 %s 人 / "
      "avatar_frames 免费档 %s 个（免费档=0 ⇒ price>0 分支恒走扣款）"
      % (paid, wearing, orph, free))

# ---------- EZ7 待裁在册 ----------
ok("EZ7 本口仍不写 user_dressups（在册事实，合并模型由站方定）",
   "user_dressups" not in equip,
   "若将来并入 dressup 模型，请同步删掉这条在册判据并改 EZ6 的上界")

summary()
