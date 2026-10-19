# -*- coding: utf-8 -*-
"""GB 闸门：反作弊「回收虚报上传量」的流水与快照对账（批一百六十九 P-403）。

洞的形状（仓里早有定例，见 `admin_http/user_ops.rs` 的 P0 错账注释：
「流水必须记实际发生的差额而非请求 delta」）：
  扣减侧 `uploaded = GREATEST(uploaded - 请求值, 0)` 会被夹到 0，
  而 traffic_ledger 写的是**请求值** ⇒
    ① sum(ledger) 与 users.uploaded 永久撕裂；
    ② 申诉推翻按同一枚请求值回补 ⇒ 虚报数大于现有上传量的案件被推翻后，
       会员白得差额（分享率、等级、成就全读这一列）。
实测（旧镜像 :8080，探针号 uploaded=100、reclaim_up=500）：
  快照 100→0（Δ=-100），流水合计 -500 ⇒ **撕裂 400 字节**。

判据同时钉反向解除只读 `reclaim_applied`（缺键的历史案件仍按请求值走，
因为那些案的扣减侧本来就是请求值，账目自洽，不许凭空改口径）。

用法：`python -u scripts/pt_audit_gb_cheat_reclaim.py`
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

SANCT = "apps/api/src/http/cheat_case/sanction.rs"
REV = "apps/api/src/repo/cheat_reverse.rs"
# P-403 收尾：回收的正向执行下沉到 repo（sanction.rs 曾长到 312 行破 300 门禁），
# 所以 GB1 的读向要跟着换文件——**判据跟着实现走，不许反过来让实现迁就判据**。


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def cut(s):
    return "\n".join(
        ln for ln in s.split("\n") if not ln.strip().startswith("//"))


def one(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (r.stdout or "").strip()


san = cut(rd(SANCT))
rev = cut(rd(REV))
# 下沉后：正向执行的 SQL 在 rev 里，san 里只剩一次调用
CALL = "crate::repo::cheat_reverse::apply_reclaim_tx"
# 锚点只能取**码体**：这段的说明文字在 `//` 注释里，而 `cut()` 已把注释整行
# 剥掉——按注释关键词去 split 会得到空串，判据恒红（仪器自伤，本轮实测）。
READ = "SELECT uploaded FROM users WHERE id = $1 FOR UPDATE"
# 下沉后这两处都在 repo 侧：索引必须建在同一个文件里，否则恒红（本轮就红过一次）
i_read = rev.find(READ)
i_ledger = rev.find("INSERT INTO traffic_ledger")
ok("GB1 回收前先锁行读实际上传量，流水绑的是实际值（下沉后的 repo 侧）",
   0 <= i_read < i_ledger and ".bind(-applied)" in rev
   and ".bind(-reclaim_up)" not in rev and "applied = requested.min(" in rev,
   "写请求值＝sum(ledger) 与快照永久撕裂（对照 user_ops 的 P0 错账定例）")
ok("GB1b handler 侧只调一次下沉原语，且把返回值写进案件 JSON",
   CALL in san and "let reclaim_applied = " in san,
   "编排层自己再拼一份 SQL＝第二把尺，正反对账迟早漂移")
ok("GB2 案件 JSON 同时留请求值与实际值（前者读数、后者回补）",
   '"reclaim_up": reclaim_up' in san and '"reclaim_applied": reclaim_applied'
   in san, "只留一个数就没法两边对账，也没法判断历史案件该按哪个口径")
ap_pos = rev.find('get("reclaim_applied")')
up_pos = rev.find('get("reclaim_up")')
ok("GB3 反向解除优先读 applied（缺键历史案件才回落请求值）",
   0 <= ap_pos < up_pos and "or_else" in rev[:up_pos + 200],
   "按请求值回补＝推翻一次白得一批上传量")
OLD = ('        .bind(uid)\n        .bind(-reclaim_up)\n'
       '        .bind(format!("cheat reclaim case #{id}"))')
ok("GB4 承重自证：旧的「流水绑请求值」写法必须判红",
   ".bind(-reclaim_up)" in OLD, "打不中说明 GB1 的认法已经瞎了")

# ---------- 现网台账：新案件不许再落「只有请求值」的旧形状 ----------
cases = one("SELECT count(*) FROM reports WHERE ref_type='cheat' "
            "AND coalesce((sanction->>'reclaim_up')::bigint,0) > 0")
old_shape = one("SELECT count(*) FROM reports WHERE ref_type='cheat' "
                "AND coalesce((sanction->>'reclaim_up')::bigint,0) > 0 "
                "AND sanction->>'reclaim_applied' IS NULL")
print("   [读数] 现网带 reclaim 的作弊案 %s 件；回收流水行 %s 条；"
      "旧形状（只有请求值）%s 件" % (
          cases, one("SELECT count(*) FROM traffic_ledger WHERE "
                     "reason LIKE 'cheat reclaim %'"), old_shape))
ok("GB5 带回收的作弊案必须写明实际扣减数（旧形状只许是历史已结案件）",
   one("SELECT count(*) FROM reports WHERE ref_type='cheat' AND "
       "coalesce((sanction->>'reclaim_up')::bigint,0) > 0 AND "
       "sanction->>'reclaim_applied' IS NULL AND status <> 1") == "0",
   "未结案件缺 reclaim_applied＝有人绕过 sanction 口写了 JSON，"
   "反向解除会退回按请求值回补的老洞")

summary()
