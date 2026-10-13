# -*- coding: utf-8 -*-
"""CA 闸门（批六十一）：职员读口的权限档位——作弊名单与授权矩阵。

先记着 `role_permissions` 的 class 语义（`authz.rs:96-98`）：
`role_type='class'` 的 `role_key` 不是「等于这一档」，而是
**「class_id >= role_key 的所有档」**。所以 `class/90` 那一行把
`staff.panel` 发给了 91 发布员、92 论坛版主、93 总版主、94 管理员……
（现网 `user_classes` 里根本没有 id=90 的档，别把它当无效行。）

两个读口只判 `staff()`（= 只要 staff.panel）：
  - `GET /admin/cheaters`：返回「疑似作弊会话」名单，含每个用户近 1 小时
    上传增量与折算速度、种子名、user_id —— 这是管理组的调查材料；
  - `GET /admin/permission-matrix`：返回全量授权图（哪个档/职务能干什么）。
而它们的**写口**要求的是 `settings.manage`（现网仅 class/99）⇒ 读比写宽三档。

修法照 0285 的先例（那批把「审种」从 staff.panel 里拆出 torrent.review）：
新增 `cheaters.view` 并只授予 class/95（主管及以上），矩阵读口对齐写口。

判据纪律：行为红用**探针成员**（注册口拿邀请码 + 算术验证码建，密码已知），
再在用户级覆盖授予 staff.panel ⇒ 不用改动任何真实档位的授权；
CA5/CA6 同时钉「目录有键 ⇔ 代码有消费点」与「最低可读档位 ≥ 95」；
CA7 是正控：撤销用户级授权后仍要 403（粗闸不能被拆掉）；
收尾用 atexit 兜住（墓碑探针、清 user_permissions/user_roles），
`role_permissions` 一条都不动 ⇒ 收尾要复算基线。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ca_staff_view.py
"""
import atexit  # noqa: E402
import io
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TAG = int(time.time()) % 100000
NAME = "cam%d" % TAG
UID = [0]


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def src(f):
    return io.open(os.path.join(REPO, "apps", "api", "src", f),
                   encoding="utf-8").read()


def floor_of(keys):
    """给定权限键集合，算出「最低可读者」的档位阈值（class 是 ≥ 语义）。"""
    got = psql("SELECT min(role_key::integer) FROM role_permissions "
               "WHERE role_type='class' AND granted AND permission_key "
               "IN (%s) AND role_key ~ '^[0-9]+$'"
               % ",".join("'%s'" % k for k in keys))
    return int(got) if got not in ("", "None") else None


def cleanup():
    if UID[0]:
        psql("DELETE FROM user_permissions WHERE user_id = %d" % UID[0])
        psql("DELETE FROM user_roles WHERE user_id = %d" % UID[0])
        if psql("SELECT status::text FROM users WHERE id = %d"
                % UID[0]) != "2":
            call("POST", "/admin/users/status", {"user_id": UID[0],
                                                 "status": 2}, token=ROOT)
        call("DELETE", "/admin/users/%d" % UID[0], None, token=ROOT)


atexit.register(cleanup)

RP0 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CA0a 前提：root 读两口感都 200（正控，别把门修死）",
   call("GET", "/admin/cheaters", None, token=ROOT)[0] == 200
   and call("GET", "/admin/permission-matrix", None, token=ROOT)[0] == 200,
   "staff.panel 阈值=%s settings.manage 阈值=%s"
   % (floor_of(["staff.panel"]), floor_of(["settings.manage"])))
ok("CA0b 前提：现网 class/90 是「≥90 全部档」的阈值语义（91 发布员等都在内）"
   "——所以 staff.panel 的实际最低可读档 = %s" % floor_of(["staff.panel"]),
   floor_of(["staff.panel"]) is not None
   and psql("SELECT count(*) FROM user_classes WHERE id = 90") == "0",
   "floor=%s 且库里无 id=90 档" % floor_of(["staff.panel"]))

# ===== CA1 探针职员：用户级授予 staff.panel（不动任何真实档位） =====
clear_register_rl()
codes = (call("POST", "/admin/invites", {"user_id": 1, "count": 1},
              token=ROOT)[1] or {}).get("data", {}).get("codes") or []
cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
if codes and len(nums) >= 2:
    call("POST", "/auth/register", {
        "username": NAME, "email": "%s@ca.local" % NAME,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    UID[0] = int(psql("SELECT id FROM users WHERE username = '%s'" % NAME) or 0)
ok("CA1 探针职员就绪（uid=%s，等级低于职员档，仅靠用户级覆盖拿 staff.panel）"
   % UID[0], UID[0] > 0, "uid=%s class=%s" % (
       UID[0], psql("SELECT class_id FROM users WHERE id = %d" % UID[0])))
PT = login(NAME, "password123") if UID[0] else None
if PT:
    call("PUT", "/admin/user-permissions", {"user_id": UID[0],
                                            "permission_key": "staff.panel",
                                            "granted": True}, token=ROOT)
def my_perms(tok):
    return ((call("GET", "/me/perms", None, token=tok)[1] or {}).get("data")
            or {}).get("perms") or []


ok("CA1b 前提：用户级覆盖确实生效——探针的 /me/perms 里有"
   " staff.panel（`user_can` 第一段就读 user_permissions，所以覆盖"
   "优先于档位阈值，不用改任何真实授权）",
   bool(PT) and "staff.panel" in my_perms(PT),
   "perms=%s" % str(my_perms(PT))[:60])

# ===== CA2/CA3 行为红：低档职员能读调查材料与授权图 =====
s_c, b_c = call("GET", "/admin/cheaters", None, token=PT)
ok("CA2 P-231：只有 staff.panel 的职员读 /admin/cheaters 必须被拒"
   "（名单含每人 1 小时上传增量与折算速度，是管理组调查材料；现网 "
   "staff.panel 由 class/90 发出 ⇒ 91 发布员/92 版主都可读）："
   "http=%s 行数=%s" % (s_c, len((b_c or {}).get("data") or [])),
   s_c == 403, str(b_c)[:60])
s_m, b_m = call("GET", "/admin/permission-matrix", None, token=PT)
ok("CA3 P-232：矩阵读口要与其写口同档（写口要 settings.manage=仅 class/99，"
   "读口却只判 staff.panel ⇒ 读比写宽三档，任何职员能拿走「谁能干什么」"
   "的完整地图）：http=%s 权限条数=%s"
   % (s_m, len(((b_m or {}).get("data") or {}).get("permissions") or [])),
   s_m == 403, str(b_m)[:50])

# ===== CA4 目录与代码要一起落地 =====
has_key = psql("SELECT count(*) FROM permissions WHERE key = "
               "'cheaters.view'") == "1"
code_hit = "CHEATERS_VIEW" in src("authz/perm.rs")
site_hit = src("admin_http/site.rs").count("perm::CHEATERS_VIEW")
ok("CA4 新增能力必须「目录有键 + 常量 + 消费点」三件齐（0285 的先例；"
   "只加目录不加代码 = 又造一个装饰开关）：目录=%s 常量=%s 引用=%d 处"
   % (has_key, code_hit, site_hit),
   has_key and code_hit and site_hit >= 1,
   "granted_to=%s" % psql("SELECT coalesce(string_agg(role_type||'/'||"
                          "role_key,'+'),'-') FROM (SELECT role_type,role_key "
                          "FROM role_permissions WHERE permission_key="
                          "'cheaters.view' ORDER BY 1) t"))

# ===== CA5 门槛：读作弊名单的最低档必须 >= 95 =====
floor = floor_of(["staff.panel"])
gate = floor_of(["cheaters.view"])
eff = gate if gate is not None else floor
ok("CA5 读「疑似作弊名单」的最低档位必须 ≥95（主管及以上；照 0285 对 "
   "user.passkey.reveal 的口径）。现算：cheaters.view 阈值=%s，"
   "缺键时退到 staff.panel 阈值=%s ⇒ 实际=%s"
   % (gate, floor, eff), eff is not None and eff >= 95,
   "实际最低档=%s" % eff)

# ===== CA6 正控：撤销用户级授权后，职员门仍在（别把粗闸拆掉） =====
call("PUT", "/admin/user-permissions", {"user_id": UID[0],
                                        "permission_key": "staff.panel",
                                        "granted": False}, token=ROOT)
s_np, _ = call("GET", "/admin/cheaters", None, token=PT)
s_nm, _ = call("GET", "/admin/permission-matrix", None, token=PT)
ok("CA6 正控：把 staff.panel 覆盖改成 explicit false 后两个口都 403"
   "（收紧不是取消职员门；也顺便证明用户级覆盖是"
   "权威源，不是只做加法）：cheaters=%s matrix=%s covered=%s"
   % (s_np, s_nm, "staff.panel" in my_perms(PT)),
   s_np == 403 and s_nm == 403 and "staff.panel" not in my_perms(PT),
   "cheaters=%s matrix=%s" % (s_np, s_nm))

# ===== CA8 栅栏：粗闸（只判 staff()）的职员读口不许再长 =====
def coarse_routes(dirname):
    """列 admin_http 下「只判 staff()、后面没有 require_perm」的路由。"""
    out = []
    base = os.path.join(REPO, "apps", "api", "src", dirname)
    sep = chr(10) + "#["          # 按路由属性切块，免写正则转义
    for fn in sorted(os.listdir(base)):
        if not fn.endswith(".rs"):
            continue
        t = io.open(os.path.join(base, fn), encoding="utf-8").read()
        for b in t.split(sep):
            m = re.match(r'(get|post|put|delete)\("([^"]+)"\)', b)
            if not m or "let _auth = staff(" not in b:
                continue
            tail = b.split("let _auth = staff(")[1][:400]
            if "require_perm" not in tail:
                out.append(m.group(2))
    return out


COARSE = coarse_routes("admin_http")
ok("CA6b 栅栏：只判 staff()（无专属权限位）的后台读口今天在册 %d 条，"
   "本批把 cheaters 与 permission-matrix 收掉之后不得再增长；"
   "其中 /admin/users、/admin/login-logs（含用户名+IP）、"
   "/admin/spark-logs（含 balance_after）敏感度最高，逐条补权限位列待拍板"
   "｜在册：%s" % (len(COARSE), ", ".join(COARSE)),
   len(COARSE) <= 15 and "/admin/cheaters" not in COARSE
   and "/admin/permission-matrix" not in COARSE,
   "在册 %d 条" % len(COARSE))

# ===== CA7 收尾：只动探针，授权表回基线 =====
cleanup()
RP1 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CA7a role_permissions 一条都没被本批改到（%s→%s）" % (RP0, RP1),
   RP1 == RP0, "RP=%s" % RP1)
ok("CA7b 探针已墓碑，其用户级覆盖/职务清零，ca 前缀无残留",
   psql("SELECT count(*) FROM users WHERE status < 2 AND username = '%s'"
        % NAME) == "0"
   and (not UID[0] or psql("SELECT count(*) FROM user_permissions WHERE "
                           "user_id = %d" % UID[0]) == "0")
   and psql("SELECT count(*) FROM users WHERE username LIKE 'cam%%' "
            "AND status < 2") == "0",
   "live=%s" % psql("SELECT count(*) FROM users WHERE username LIKE 'cam%%' "
                    "AND status < 2"))

summary()
