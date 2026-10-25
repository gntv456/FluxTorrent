# -*- coding: utf-8 -*-
"""闸门 SX：站长画像·治理深水面（2026-10-25 十轮）。

覆盖作弊规则集治理（草稿/激活/影子/回测）、等级编辑与影响预览、
考核豁免/恢复、分区词表、促销价目档、用户杂项（rename/2fa/grant）。

本轮零真缺陷——本闸门把这批口的**守卫语义**钉住（空覆盖拒/同版
回测拒/staff 档拒/幽灵 claim 404/坏时长拒），防后续改动回退。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_sx_governance.py
"""
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOT = login()
TAG = int(time.time()) % 100000


def sql(q):
    import subprocess
    return subprocess.run(
        ["docker", "exec", "flux-postgres", "sh", "-c",
         'psql -U $POSTGRES_USER -d $POSTGRES_DB -tAc "%s"' % q],
        capture_output=True, text=True).stdout.strip()


# ============ S1 作弊规则集治理 ============
s, r = call("GET", "/admin/cheat/rulesets", token=ROOT)
rs = r["data"]["rulesets"] if s == 200 else []
active = [x for x in rs if x.get("active")]
cur = active[0].get("version") if active else None
ok("SX1 规则集列表(含 active)", cur is not None, "cur=%s" % cur)

s, r = call("POST", "/admin/cheat/rulesets", token=ROOT,
            body={"overrides": {}, "note": "gate"})
ok("SX2 空覆盖拒", s == 400, "s=%s" % s)

s, r = call("POST", "/admin/cheat/rulesets", token=ROOT,
            body={"overrides": {"risk_absurd_vouch_gb": "1"},
                  "note": "sxgate%d" % TAG})
ok("SX3 建草稿", s == 200, str(r)[:100])
draft_v = r.get("data", {}).get("version") if s == 200 else None

s, r = call("POST", "/admin/cheat/backtest", token=ROOT,
            body={"from_ver": 1, "to_ver": 1})
ok("SX4 同版本回测拒", s == 400, "s=%s" % s)

if draft_v and cur and draft_v != cur:
    s, r = call("POST", "/admin/cheat/backtest", token=ROOT,
                body={"from_ver": cur, "to_ver": draft_v, "window_days": 7})
    ok("SX5 排回测", s == 200, str(r)[:100])
    # 激活→切回（还原现场）
    s, r = call("POST", "/admin/cheat/rulesets/activate", token=ROOT,
                body={"version": draft_v})
    ok("SX6 激活回显 shadow 状态", s == 200 and "shadow_mode" in str(r),
       str(r)[:110])
    call("POST", "/admin/cheat/rulesets/activate", token=ROOT,
         body={"version": cur})
    s, r = call("GET", "/admin/cheat/rulesets", token=ROOT)
    ok("SX7 切回原版", any(x.get("active") and x.get("version") == cur
                          for x in r["data"]["rulesets"]), "")

s, r = call("POST", "/admin/cheat/rulesets/activate", token=ROOT,
            body={"version": 9999})
ok("SX8 激活不存在 404", s == 404, "s=%s" % s)

# ============ S2 等级编辑 ============
s, r = call("POST", "/admin/classes/3/impact-preview", token=ROOT,
            body={"min_uploaded": 53687091200, "min_download_count": 5,
                  "min_seed_hours": 240, "min_account_age_days": 30,
                  "demotable": True})
ok("SX9 影响预览", s == 200, "s=%s" % s)

s, r = call("PUT", "/admin/classes/99", token=ROOT,
            body={"name": "x", "min_uploaded": 0, "min_download_count": 0,
                  "min_seed_hours": 0, "min_account_age_days": 0})
ok("SX10 staff 档编辑拒", s == 400, "s=%s" % s)

# ============ S3 考核豁免/恢复（种子 claim 走完即删） ============
seed = sql(
    "INSERT INTO task_claims (task_id, user_id, status) VALUES "
    "((SELECT min(id) FROM tasks WHERE kind='onboard'), 1, 0) RETURNING id"
).split("\n")[0].strip()
ok("SX11 种子考核 claim", seed.isdigit(), seed)
if seed.isdigit():
    cid = int(seed)
    s, r = call("POST", "/admin/exam-users/%d/exempt" % cid, token=ROOT,
                body={})
    ok("SX12 豁免", s == 200, str(r)[:90])
    s, r = call("POST", "/admin/exam-users/%d/exempt" % cid, token=ROOT,
                body={})
    ok("SX13 重复豁免拒", s == 404, "s=%s" % s)
    s, r = call("POST", "/admin/exam-users/%d/recover" % cid, token=ROOT,
                body={})
    ok("SX14 恢复", s == 200, str(r)[:90])
    sql("DELETE FROM task_claims WHERE id = %d" % cid)

s, r = call("POST", "/admin/exam-users/999999/exempt", token=ROOT, body={})
ok("SX15 幽灵 claim 404", s == 404, "s=%s" % s)

# ============ S4 分区词表 / 价目档 ============
s, r = call("GET", "/admin/section-dict", token=ROOT)
rows = r["data"] if s == 200 else []
ok("SX16 分区词表", len(rows) > 0, "n=%d" % len(rows))
first = rows[0]
orig_name = first["name"]
s, r = call("PUT", "/admin/section-dict/%d" % first["id"], token=ROOT,
            body={"kind": first["kind"], "name": orig_name + "sxg",
                  "sort": first.get("sort"), "mode_id": first.get("mode_id")})
ok("SX17 词表改名", s == 200, str(r)[:90])
call("PUT", "/admin/section-dict/%d" % first["id"], token=ROOT,
     body={"kind": first["kind"], "name": orig_name,
           "sort": first.get("sort"), "mode_id": first.get("mode_id")})
ok("SX18 词表还原", sql(
    "SELECT name FROM section_dict WHERE id = %d" % first["id"]) == orig_name,
   "")

s, r = call("POST", "/admin/promo-kinds/tiers", token=ROOT,
            body={"kind": "free", "hours": 47, "price": 123})
ok("SX19 建价目档", s == 200, str(r)[:90])
tid = sql("SELECT id FROM promo_kind_tiers WHERE kind='free' AND hours=47")
if tid.isdigit():
    s, r = call("DELETE", "/admin/promo-kinds/tiers/%s" % tid, token=ROOT)
    ok("SX20 删价目档", s == 200, str(r)[:90])
s, r = call("POST", "/admin/promo-kinds/tiers", token=ROOT,
            body={"kind": "free", "hours": 0, "price": 1})
ok("SX21 坏时长拒", s == 400, "s=%s" % s)

summary()
