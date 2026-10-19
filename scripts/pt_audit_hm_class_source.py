# -*- coding: utf-8 -*-
"""闸门 HM：等级门槛必须单一来源（class_rules），死列记账（批一百八十六）。

本批走查「会员升到哪一档」这条链的四读一写，实测结论是**同源的**（写得很规范）：
  · 引擎  `worker/jobs/class_adj.rs`：升级与降级两条 SQL 都
      `JOIN class_rules r ON s.uploaded >= r.min_uploaded AND s.dl >= … AND
       s.sh >= … AND s.age >= …`（13 档真门槛，如 class 2 = 53.7GB / 241 次 /
       240 做种小时 / 28 天）；
  · 站长预览 `admin_p3_http/classes_preview.rs`：同一张 class_rules，还把
      「未保存的草稿」用 UNION 注进第 $1 档 ⇒ 预览即所见即所得；
  · 会员公开页 `gaps_http/classes_public.rs`：`user_classes LEFT JOIN class_rules`
      + COALESCE（贵宾 20 这类只存在于 user_classes 的任命档没有规则行）；
  · `me_settings.rs` / `profile.rs` 亦同。

但同一次走查里查出**四列纯死配置**（`user_classes` 上填了值、全仓零读者）：
  min_ratio（0/21 有值）、min_age_days（0/21）、pod_level（20/21 有值）、
  privileges（21/21 有值）——面板 `classes_admin.rs` 只能改 name，
  所以这四列既没人读、也没入口能改，属结构性遗留（NexusPHP 老列）。
  这类「看起来是配置、其实是尸体」的东西最会骗到接手的人，所以记账而非删列。

判据：
  HM1 三处读数（引擎 / 预览 / 公开页）都必须以 class_rules 为门槛来源，
      且**不得**从 user_classes 读任何 `min_*` 门槛；
  HM2 降级那条 SQL 必须有 `r.class_id >= 1` 的地板（P-325 的回归钉：字典 LV0
      门槛全零时，LEFT JOIN + 排除 0 档才不会把人降到 Peasant 以下）；
  HM3 承重自证：把「从 user_classes 读 min_ratio 当门槛」的旧形状喂同一把尺必须被抓；
  HM4 死列在册清单：只许缩——谁接线（或确认删除）就划掉一条；
  HM5 活体（只读）：class_rules 的 class_id 必须都能在 user_classes 找到，
      否则引擎会把人升/降成一个不存在的档。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hm_class_source.py`
      HM_LIVE=0 跳过 HM5。
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

LIVE = os.environ.get("HM_LIVE", "1") != "0"
ENGINE = "apps/worker/src/jobs/class_adj.rs"
PREVIEW = "apps/api/src/admin_p3_http/classes_preview.rs"
PUBLIC = "apps/api/src/gaps_http/classes_public.rs"
ADMIN = "apps/api/src/admin_p3_http/classes_admin.rs"
DEAD_COLS = ["min_age_days", "min_ratio", "pod_level", "privileges"]
OLD_BAD = """
    let needs: (i64, f64) = sqlx::query_as(
        "SELECT min_uploaded, min_ratio FROM user_classes WHERE id = $1",
    )
"""


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip(src):
    return "\n".join(
        ln for ln in src.split("\n")
        if not ln.strip().startswith("//") and not ln.strip().startswith("///"))


def uc_threshold_reads(text):
    """只看 SQL 字面量：门槛列被 user_classes 的别名限定（或该句根本没 join
    class_rules 却从 user_classes 读）才算越源。别名限定到 class_rules 是正常的。"""
    bad = []
    for lit in re.finditer(r'"((?:[^"\\]|\\.){6,}?)"', text, re.S):
        sql = lit.group(1)
        if not re.search(r"\bselect\b|\bupdate\b|\bdelete\b|\binsert\b",
                         sql, re.I):
            continue
        has_uc = re.search(r"\buser_classes\b", sql, re.I) is not None
        has_cr = re.search(r"\bclass_rules\b", sql, re.I) is not None
        uc_alias = {a.lower() for a in
                    re.findall(r"\buser_classes\s+([a-z]\w*)", sql, re.I)}
        for m in re.finditer(r"(?:(\w+)\.)?\bmin_[a-z_]+\b", sql, re.I):
            qual = (m.group(1) or "").lower()
            if qual and qual in uc_alias:
                bad.append(m.group(0))
            elif not qual and has_uc and not has_cr:
                bad.append(m.group(0))
    return bad


eng = strip(rd(ENGINE))
pre = strip(rd(PREVIEW))
pub = strip(rd(PUBLIC))
adm = strip(rd(ADMIN))

ok("HM1a 引擎以 class_rules 为门槛源",
   eng.count("JOIN class_rules") + eng.count("FROM class_rules") >= 2
   and not uc_threshold_reads(eng),
   "class_rules 引用=%d 越源读法=%s"
   % (eng.count("class_rules"), uc_threshold_reads(eng)))
ok("HM1b 站长预览以 class_rules 为门槛源",
   "FROM class_rules" in pre and not uc_threshold_reads(pre),
   "引用=%d" % pre.count("class_rules"))
ok("HM1c 会员公开页以 class_rules 为门槛源（LEFT JOIN + COALESCE 兜任命档）",
   re.search(r"LEFT\s+JOIN\s+class_rules", pub) is not None
   and not uc_threshold_reads(pub),
   "引用=%d" % pub.count("class_rules"))
ok("HM1d 管理端不得改门槛列（当前只改 name，门槛走 class_rules 维护）",
   "UPDATE user_classes SET name" in adm
   and not re.search(r"UPDATE user_classes SET[^\"\n]*min_", adm),
   "越权改门槛语句=0")
ok("HM2 降级 SQL 带 class_id >= 1 地板（P-325 回归钉）",
   re.search(r"r\.class_id >= 1", eng) is not None,
   "缺地板就会把人降到 0 档 Peasant 之下")
ok("HM3 承重：从 user_classes 读门槛的旧形状被抓",
   len(uc_threshold_reads(OLD_BAD)) >= 1,
   "命中=%s" % uc_threshold_reads(OLD_BAD))

alive = []
for col in DEAD_COLS:
    hits = 0
    for rel in (ENGINE, PREVIEW, PUBLIC, ADMIN):
        hits += len(re.findall(r"\b%s\b" % col, strip(rd(rel))))
    src_hits = subprocess.run(
        ["grep", "-rl", col, os.path.join(ROOT, "apps", "api", "src"),
         os.path.join(ROOT, "apps", "worker", "src")],
        capture_output=True, text=True, encoding="utf-8", errors="replace"
    ).stdout.strip()
    n = len([l for l in src_hits.split("\n") if l.strip()])
    if n == 0:
        alive.append(col)
ok("HM4 死列在册清单（只许缩）", sorted(alive) == sorted(DEAD_COLS),
   "仍零读者=%s（接线一个就划掉一个）" % (alive,))

if LIVE:
    def psql(q):
        r = subprocess.run(
            ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
             "-d", "fluxtorrent", "-q", "-tA"], input=q,
            capture_output=True, text=True, encoding="utf-8", errors="replace")
        err = (r.stderr or "")
        if "ERROR" in err:
            raise RuntimeError("psql: " + err[:300])
        return (r.stdout or "").strip()

    orphan_rules = psql(
        "SELECT count(*) FROM class_rules r WHERE NOT EXISTS "
        "(SELECT 1 FROM user_classes c WHERE c.id = r.class_id)")
    ok("HM5 class_rules 的档位都能在 user_classes 找到",
       int(orphan_rules) == 0, "越界规则行=%s" % orphan_rules)
    users_bad = psql(
        "SELECT count(*) FROM users u JOIN user_classes c ON c.id = u.class_id "
        "LEFT JOIN class_rules r ON r.class_id = u.class_id "
        "WHERE u.status < 2 AND u.class_id >= 1 AND r.class_id IS NULL")
    print("  [参考] 有档位但无规则行的在册会员数=%s（任命档，属正常）"
          % users_bad)
else:
    print("[HM5] HM_LIVE=0 跳过活体两条")

summary()
