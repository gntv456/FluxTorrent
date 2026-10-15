# -*- coding: utf-8 -*-
"""EV 常驻闸门：等级自动降级的地板必须是 1，不能把成员降到 class 0（P-325）

现网取证（批一百三十九，事务内改字典+改档位后跑代码里那段原 CTE）：
一名 LV5、sh=0、LV1 门槛抬到 1h 的探针，旧实现算出的 new_class = **0**。
原因不是 COALESCE 写错，而是它根本到不了：字典里 LV0（Peasant）四列门槛
全为 0 ⇒ 任何用户都「满足 LV0」⇒ INNER JOIN 永远有行 ⇒ max 取到 0。
0 档在站内是死角：现网 0 人在 class 0，注册给 1，achv_dormant 的 dormant
降级写 1，subtitles_govern 判 class_id >= 1，onboarding 的 demotable 开关
挂的也是 class 1 —— 被降到 0 的人既掉出权限门又成为全站唯一一档。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADJ = "apps/worker/src/jobs/class_adj.rs"
PREV = "9b00d2a0"  # 修前参照提交（本批修复的父提交，别写 HEAD）


def sh(args):
    return subprocess.run(args, cwd=ROOT, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")


def blob(rev):
    p = sh(["git", "show", "%s:%s" % (rev, ADJ)])
    if p.returncode != 0:
        FAILURES.append("EV 取文失败 %s / %s" % (rev, (p.stderr or "")[:120]))
        return ""
    return p.stdout or ""


def cur():
    p = os.path.join(ROOT, ADJ)
    if not os.path.exists(p):
        FAILURES.append("EV 文件缺失 %s" % ADJ)
        return ""
    return open(p, encoding="utf-8", errors="replace").read()


def demote_block(src):
    i = src.find("let demoted")
    seg = src[i:] if i >= 0 else ""
    j = seg.find("target AS (")
    k = seg.find("UPDATE users", j) if j >= 0 else -1
    return seg[j:k] if j >= 0 and k > j else ""


old = demote_block(blob(PREV))
print("== EV1 正控：修前形态必须「INNER JOIN 且没有 class_id >= 1」 ==")
ok("EV1a 修前能取到降级段（空串=参照物失效）", len(old) > 40,
   "取到 %d 字符" % len(old))
ok("EV1b 修前是 INNER JOIN（COALESCE 保底不可达）",
   re.search(r"stats s JOIN class_rules r ON", old) is not None)
ok("EV1c 修前没有把 0 档挡在候选之外", "class_id >= 1" not in old)

now = demote_block(cur())
print("== EV2 现网体：LEFT JOIN + 排除 0 档，地板回到 1 ==")
ok("EV2a 降级段能取到", len(now) > 40, "取到 %d 字符" % len(now))
ok("EV2b 改成 LEFT JOIN（只剩 0 档时是空组→NULL→保底）",
   re.search(r"stats s LEFT JOIN class_rules r ON", now) is not None)
ok("EV2c 候选里排除 class 0（保底才不会是 Peasant）",
   "r.class_id >= 1" in now)
ok("EV2d 保底的 COALESCE(…, 1) 仍在", re.search(r"COALESCE\(max\(r\.class_"
                                                r"id\), 1\)", now) is not None)

def cur_or(rel):
    p = os.path.join(ROOT, rel)
    if not os.path.exists(p):
        FAILURES.append("EV 参照文件缺失 %s" % rel)
        return ""
    return open(p, encoding="utf-8", errors="replace").read()


print("== EV3 地板语义与站内别处一致（谁在把 1 当地板） ==")
ok("EV3a achv_dormant 的 dormant 降级仍写 class_id = 1",
   "SET class_id = 1" in cur_or("apps/worker/src/jobs/achv_dormant.rs"))
ok("EV3b subtitles_govern 判的是 class_id >= 1（降到 0 就掉出权限门）",
   "class_id >= 1" in cur_or("apps/api/src/content_http/"
                             "subtitles_govern.rs"))
ok("EV3c onboarding 的 demotable 开关挂在 class 1",
   "class_id = 1" in cur_or("apps/api/src/ops_http/onboarding.rs"))


print("== EV4 现网读数：class 0 应当仍是空档 ==")
q = sh(["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
        "-d", "fluxtorrent", "-tAc",
        "SELECT count(*) FROM users WHERE class_id = 0"])
if q.returncode != 0:
    print("   WARN 库不可达：EV4 本轮不判（静态各条照常承重）")
else:
    n = (q.stdout or "").strip()
    ok("EV4 class_id = 0 的人数为 0（一旦长出 Peasant 就是这条洞放行过）",
       n == "0", "实读 %s 人" % n)

summary()
