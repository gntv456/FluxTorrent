# -*- coding: utf-8 -*-
"""CG 闸门（批六十七）：同一荣誉域里，改期与台账读口没跟授予同档。

现网（修前）实测的三条：
  P-246 `POST /admin/users/{id}/medal/{medal_id}/redate` —— 同域的
        `POST /admin/users/{id}/medal/{medal_id}`（授予）与
        `POST /admin/user-medals/delete`（回收）都判
        `require_perm(medal.manage)`（授 class/93），**只有改期这一条不判**：
        它只判 `staff()` + `ensure_outranks`（等级护栏，不是能力档——
        批六十五立的第七段病形）。而它写的正是「别人的荣誉有效期」，
        `days=null` 还能把限时勋章**改成永久**。
  P-247 `GET /admin/user-medals` —— 全站荣誉持有台账（username + 勋章名 +
        granted_at/expires_at + 是否过期），只判 `staff()` ⇒ 91 发布员整页
        读走「谁有什么勋章、哪个已过期」。同页对应的删除口却要 medal.manage。
  P-248 同文件 `total` 的 `SELECT count(*) … .unwrap_or(0)` —— 查询失败
        当成 0：面板写「共 0 条」而列表里明明有行，翻页器也算不出页数。
        这条造不出失败，落静态形状判据（红证取修前命中数）。

判据纪律（承 CE/CF 三条教训）：
  - 探针授权一律 `user_permissions` 覆盖，真实档位表只读不改；
  - **先用合法路径自建荣誉行**（探针被授予 medal.manage 后给自己人授章），
    再撤掉那把键、只用 `staff.panel` 去打改期口 ⇒ 红的是「档位」而不是
    「业务校验」；行只落在探针账号上，绝不碰真实成员（库里原有 2 行）；
  - 每条收紧判据配一层正控（CG3 改期真的生效、CG6 读口授权后真的读得到），
    否则「整口封死」会被读成绿；
  - 静态扫描**先去 `//` 注释**；引用面按**同域全部文件**合计（拆文件会
    抽走按单文件名硬编码的判据地基）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_cg_medal_tiers.py
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
ACT, TGT = "cga%d" % TAG, "cgt%d" % TAG
AID = [0]
TID_U = [0]
MEDAL = [0]


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


def strip_comments(t):
    return chr(10).join(ln.split("//")[0] for ln in t.splitlines())


def code(f):
    return strip_comments(src(f))


def expiry():
    return psql("SELECT coalesce(expires_at::text, 'NULL') FROM user_medals "
                "WHERE user_id = %d AND medal_id = %d" % (TID_U[0], MEDAL[0]))


def cleanup():
    if TID_U[0] or AID[0]:
        psql("DELETE FROM user_medals WHERE user_id IN (%d, %d)"
             % (AID[0] or 0, TID_U[0] or 0))
    if AID[0]:
        psql("DELETE FROM user_permissions WHERE user_id = %d" % AID[0])
        psql("DELETE FROM user_roles WHERE user_id = %d" % AID[0])
    for who in (AID[0], TID_U[0]):
        if who and psql("SELECT status::text FROM users WHERE id = %d"
                        % who) != "2":
            call("POST", "/admin/users/status", {"user_id": who, "status": 2},
                 token=ROOT)
    for who in (AID[0], TID_U[0]):
        if who:
            call("DELETE", "/admin/users/%d" % who, None, token=ROOT)


atexit.register(cleanup)

RP0 = int(psql("SELECT count(*) FROM role_permissions") or -1)
UM0 = int(psql("SELECT count(*) FROM user_medals") or -1)
MEDAL[0] = int(psql("SELECT id FROM medals ORDER BY id LIMIT 1") or 0)
ok("CG0 前提：基线已取（user_medals %d 行、授权表 %d 行），勋章字典可用 id=%s"
   % (UM0, RP0, MEDAL[0] or "-"), UM0 >= 0 and RP0 > 0 and MEDAL[0] > 0,
   "UM=%d RP=%d 章=%s" % (UM0, RP0, MEDAL[0]))

clear_register_rl()
codes = (call("POST", "/admin/invites", {"user_id": 1, "count": 2},
              token=ROOT)[1] or {}).get("data", {}).get("codes") or []


def reg(name, idx):
    cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(codes) <= idx or len(nums) < 2:
        return 0
    call("POST", "/auth/register", {
        "username": name, "email": "%s@cg.local" % name,
        "password": "password123", "invite_code": codes[idx]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    return int(psql("SELECT id FROM users WHERE username = '%s'" % name) or 0)


AID[0] = reg(ACT, 0)
TID_U[0] = reg(TGT, 1)
psql("UPDATE users SET class_id = 12 WHERE id = %d" % (AID[0] or 0))
AT = login(ACT, "password123") if AID[0] else None


def grant(key, val=True):
    return call("PUT", "/admin/user-permissions",
                {"user_id": AID[0], "permission_key": key, "granted": val},
                token=ROOT)[0]


grant("staff.panel")
ok("CG1 探针就绪：职员侧 uid=%s（class 12 + 覆盖 staff.panel，低于 90 档）、"
   "对象侧 uid=%s" % (AID[0], TID_U[0]),
   bool(AT) and AID[0] > 0 and TID_U[0] > 0,
   "actor=%s target=%s" % (AID[0], TID_U[0]))

# ===== 先用合法路径建一行荣誉（授予口自己有档位，是正当用法）=====
grant("medal.manage")
s_gm, _ = call("POST", "/admin/users/%d/medal/%d" % (TID_U[0], MEDAL[0]),
               {"days": 30}, token=AT)
E0 = expiry()
ok("CG2 前提（正控的底座）：被授予 medal.manage 后授章 200，且库里真有一行"
   "限时荣誉：http=%s expires_at=%s" % (s_gm, E0),
   s_gm == 200 and E0 not in ("", "NULL", "-"), "gm=%s E=%s" % (s_gm, E0))

# ===== CG3 P-246 改期口的档位 =====
grant("medal.manage", val=False)   # 撤键：只留 staff.panel
RD = "/admin/users/%d/medal/%d/redate" % (TID_U[0], MEDAL[0])
s_rd, b_rd = call("POST", RD, {"days": None}, token=AT)
E1 = expiry()
UM1 = int(psql("SELECT count(*) FROM user_medals") or -1)
ok("CG3 P-246：只有 staff.panel 的低档职员把**别人的限时勋章改成永久**"
   "（days=null）必须被档位挡住且 `expires_at` 一字未改——修前实测 200 且 "
   "expires_at 变 NULL；现网 http=%s 有效期 %s→%s，行数 %d→%d"
   % (s_rd, E0, E1, UM0 + 1, UM1),
   s_rd == 403 and E1 == E0,
   "%s E=%s UM=%d" % (str(b_rd)[:48], E1, UM1))
s_bad, _ = call("POST", RD, {"days": 99999}, token=AT)
ok("CG3b P-246 边界：即使被档位挡住，值域校验的形状也要在（days=99999 越界"
   "应 400/403，不能 200）：%s" % s_bad, s_bad in (400, 403), "%s" % s_bad)

# ===== CG4 P-247 荣誉台账读口 =====
s_l, b_l = call("GET", "/admin/user-medals?uid=%d&per_page=20" % TID_U[0],
                None, token=AT)
D = (b_l or {}).get("data") or {}
R0 = (D.get("rows") or [{}])[0]
LEAK = "username=%s medal=%s expired=%s" % (R0.get("username"),
                                            R0.get("medal_name"),
                                            R0.get("expired"))
ok("CG4 P-247：只 staff.panel 读全站荣誉台账 `/admin/user-medals`（返回每人"
   "的勋章名、授予/到期时间与是否过期）——修后必须 403；现网 http=%s 内容=%s"
   % (s_l, LEAK), s_l == 403, "list=%s 内容=%s" % (s_l, LEAK))

# ===== CG5 P-248 total 的 fail-open（静态形状）=====
MU = code("admin_p3_http/medal_users.rs")
ok("CG5 P-248：`/admin/user-medals` 的 total 查询不许 `.unwrap_or(0)`"
   "（失败当成 0 ⇒ 面板「共 0 条」与列表里的行自相矛盾，翻页器算不出页数；"
   "修前命中 1 处，按**去注释后**的形状扫）——残留=%d 处"
   % MU.count(".unwrap_or(0)"), MU.count(".unwrap_or(0)") == 0,
   "残留=%d" % MU.count(".unwrap_or(0)"))

# ===== CG6 绿证：授予 medal.manage 后改期与台账都回业务层 =====
grant("medal.manage")
s_rd2, _ = call("POST", RD, {"days": 7}, token=AT)
E2 = expiry()
s_l2, b_l2 = call("GET", "/admin/user-medals?uid=%d&per_page=20" % TID_U[0],
                  None, token=AT)
D2 = (b_l2 or {}).get("data") or {}
n2 = len(D2.get("rows") or [])
ok("CG6 授权 medal.manage 后：改期 200 且**有效期真的变了**（%s→%s）、"
   "台账 200 且 total 与 rows 数一致（total=%s rows=%d）"
   % (E1, E2, D2.get("total"), n2),
   s_rd2 == 200 and E2 != E1 and E2 not in ("", "-") and s_l2 == 200
   and int(D2.get("total") or -1) == n2,
   "redate=%s E=%s list=%s total=%s/%s" % (s_rd2, E2, s_l2, D2.get("total"),
                                           n2))

# ===== CG7 同域档位一致（引用面按同域全部文件合计）=====
FILES = ("admin_http/user_grant_medal.rs", "admin_http/user_revoke.rs",
         "admin_p3_http/medal_users.rs")
refs = sum(code(f).count("perm::MEDAL_MANAGE") for f in FILES)
fl = psql("SELECT min(role_key::integer) FROM role_permissions WHERE "
          "role_type='class' AND granted AND permission_key='medal.manage' "
          "AND role_key ~ '^[0-9]+$'")
ok("CG7 荣誉域三口（授予 / 改期 / 台账+回收）共用一把 `medal.manage`："
   "引用面=%d 处（本批新增 2 处，要 >=4），最低档=%s（现 93）" % (refs, fl),
   refs >= 4 and fl not in ("", "None") and int(fl) >= 92,
   "引用=%d 档=%s" % (refs, fl))


# ===== CG8 栅栏：四域口径 B，本批两条口必须消失 =====
ENTRY = re.compile(r"staff\(|require_auth\(")
GUARD = re.compile(r"require_(any_)?perm|require_\w*(manage|view|handle|"
                   r"grant|assign)")


def coarse(dirname):
    out = []
    base = os.path.join(REPO, "apps", "api", "src", dirname)
    if not os.path.isdir(base):
        return out
    for fn in sorted(os.listdir(base)):
        if not fn.endswith(".rs"):
            continue
        t = strip_comments(io.open(os.path.join(base, fn),
                                   encoding="utf-8").read())
        for b in t.split(chr(10) + "#["):
            m = re.match(r'(get|post|put|delete)\("([^"]+)"\)', b)
            if not m or not ENTRY.search(b):
                continue
            if GUARD.search(b):
                continue
            out.append(m.group(2))
    return out


ALL = {}
for d in ("admin_http", "admin_p2_http", "admin_p3_http", "staff_http"):
    ALL[d] = coarse(d)
tot = sum(len(v) for v in ALL.values())
want = ["/admin/users/{id}/medal/{medal_id}/redate", "/admin/user-medals"]
still = [p for p in want if p in sum(ALL.values(), [])]
ok("CG8 栅栏口径 B（四域）：本批两条口必须从清单消失（仍在册=%s）；"
   "总数 admin_http=%d p2=%d p3=%d staff=%d 合计=%d（批六十六收 2 后是 44）"
   % (still or "无", len(ALL["admin_http"]), len(ALL["admin_p2_http"]),
      len(ALL["admin_p3_http"]), len(ALL["staff_http"]), tot),
   not still and tot <= 42, "仍在册=%s 合计=%d" % (still, tot))

# ===== CG9 收尾 =====
cleanup()
RP1 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CG9 真实授权表零改动（%d→%d）；user_medals 回基线（%d→%d）；探针成员清零"
   % (RP0, RP1, UM0, int(psql("SELECT count(*) FROM user_medals") or -1)),
   RP1 == RP0 and int(psql("SELECT count(*) FROM user_medals") or -1) == UM0
   and psql("SELECT count(*) FROM users WHERE (username LIKE 'cga%%' OR "
            "username LIKE 'cgt%%') AND status < 2") == "0",
   "UM=%s RP=%s" % (psql("SELECT count(*) FROM user_medals"), RP1))

summary()
