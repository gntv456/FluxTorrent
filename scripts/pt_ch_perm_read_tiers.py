# -*- coding: utf-8 -*-
"""CH 闸门（批六十八）：三张「谁有什么权力」的读口没有档位，
且 `settings.view` 是目录里有、代码零消费的悬空键。

修前（现网镜像 api:aa1017b）实测，探针是 class 12 的普通成员、
只在用户级覆盖了 `staff.panel`（低于一切职员档）：
  P-249 `GET /admin/settings` —— 注释写着「sysop（99）只读+可写；
        administrator 只读」，代码只判 `staff()`；**密文脱敏本身是对的**
        （secret 项回空串），漏的是档位：全站配置明文（module_* 开关、
        限流阈值、追踪与促销参数）任何低档职员整份读走。
        而目录里早就有 `settings.view` 这把键（授 class/90）——**代码零消费**，
        所以「能不能关掉这个读口」今天无法表达。
  P-250 `GET /admin/user-permissions?user_id=` —— 返回**任意指定用户**的
        生效权限键清单（`authz::user_perm_keys`）；同文件的写口要
        `user.manage` 级键，读口却只判 `staff()` ⇒ 低档职员可枚举
        站长/主管的能力面，这是「找最弱目标、规划提权路径」的现成地图。
  P-251 `GET /admin/user-roles?user_id=` —— 同上，返回任意用户的角色分配。

顺带修栅栏自身（这是本批能挖到上面三条的前提）：
  ① 入口助手在 staff_http 叫 `require_auth()`，只认 `staff()` 的探测器
     会把整个域判成「0 条洞」（批六十六记过一次）；
  ② 守卫可能落在**跨文件助手**里：`POST /admin/increment-bulk` 的
     `require_perm` 在 `increment_targets::increment_bulk_validate` 内，
     `GET /admin/grants` 的在 `can_view` 内 ⇒ 整块扫描会把它们误报成洞；
  ③ 用 `authz::can()` + SQL 行归属做授权也算收口：
     `POST /staff/promo-intents`（非管理组只能给**自己的**已过审种子提单，
     站点级范围 `!staff → Forbidden`）、`.../cancel`（`WHERE ... AND
     ($3 OR requested_by=$2)`）都是合法形态。
  ⇒ 本批把「一跳助手 + authz::can」并进判据，并断言两条误报被剔除。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_ch_perm_read_tiers.py
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
from pt_audit_lib import (dig,   # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TAG = int(time.time()) % 100000
NAME = "chh%d" % TAG
AID = [0]

DIRS = ("admin_http", "admin_p2_http", "admin_p3_http", "staff_http")
ENTRY = re.compile(r"staff\(|require_auth\(")
GUARD = re.compile(r"require_(any_)?perm|authz::can|require_\w*(manage|view|"
                   r"handle|grant|assign)\w*\(")
FNDEF = re.compile(r"(?:pub(?:\(\w+\))?\s+)?(?:async\s+)?fn (\w+)\s*[(<]")
CALL = re.compile(r"(?:super|crate)::[\w:]*\b(\w+)\(|\b(\w+)\(")
# 入口助手与标准库调用不算「守卫助手」——否则每个块都因调了 staff() 被判绿
BUILTIN = {"staff", "require_auth", "ok", "some", "none", "format", "json",
           "vec", "chunks", "map", "filter", "collect", "begin", "commit",
           "bind", "execute", "fetch_all", "fetch_one", "fetch_optional"}


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


def dtexts(d):
    base = os.path.join(REPO, "apps", "api", "src", d)
    out = {}
    for fn in sorted(os.listdir(base)):
        if fn.endswith(".rs"):
            out[fn] = strip_comments(
                io.open(os.path.join(base, fn), encoding="utf-8").read())
    return out


def fn_bodies(ts):
    bodies = {}
    for t in ts.values():
        for m in FNDEF.finditer(t):
            i = t.find("{", m.end())
            if i < 0:
                continue
            depth, j = 0, i
            while j < len(t):
                if t[j] == "{":
                    depth += 1
                elif t[j] == "}":
                    depth -= 1
                    if depth == 0:
                        break
                j += 1
            bodies[m.group(1)] = bodies.get(m.group(1), "") + t[i:j + 1]
    return bodies


def coarse(d):
    """返回 (真洞, 守卫在助手里) 两组路由。一跳：块内调用的助手体里有守卫。"""
    ts, holes, helped = dtexts(d), [], []
    bodies = fn_bodies(ts)
    for t in ts.values():
        for b in t.split(chr(10) + "#["):
            m = re.match(r'(get|post|put|delete)\("([^"]+)"\)', b)
            if not m or not ENTRY.search(b) or GUARD.search(b):
                continue
            names = [a or c for a, c in CALL.findall(b)]
            hit = [n for n in names if n not in BUILTIN and n in bodies
                   and GUARD.search(bodies[n])]
            (helped if hit else holes).append(m.group(2))
    return holes, helped


def cleanup():
    if AID[0]:
        psql("DELETE FROM user_permissions WHERE user_id = %d" % AID[0])
        psql("DELETE FROM user_roles WHERE user_id = %d" % AID[0])
        if psql("SELECT status::text FROM users WHERE id = %d" % AID[0]) != "2":
            call("POST", "/admin/users/status", {"user_id": AID[0],
                                                 "status": 2}, token=ROOT)
        call("DELETE", "/admin/users/%d" % AID[0], None, token=ROOT)


atexit.register(cleanup)

RP0 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CH0 前提：授权表 %d 行；`settings.view`/`user.view` 都在目录里"
   % RP0,
   RP0 > 0 and psql("SELECT count(*) FROM permissions WHERE key IN "
                    "('settings.view','user.view')") == "2", "RP=%d" % RP0)

clear_register_rl()
codes = dig(call("POST", "/admin/invites", {"user_id": 1, "count": 1},
              token=ROOT)[1], "data", "codes") or []
cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
if codes and len(nums) >= 2:
    call("POST", "/auth/register", {
        "username": NAME, "email": "%s@ch.local" % NAME,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    AID[0] = int(psql("SELECT id FROM users WHERE username = '%s'" % NAME) or 0)
psql("UPDATE users SET class_id = 12 WHERE id = %d" % (AID[0] or 0))
call("PUT", "/admin/user-permissions",
     {"user_id": AID[0], "permission_key": "staff.panel", "granted": True},
     token=ROOT)
PT = login(NAME, "password123") if AID[0] else None
ok("CH1 探针就绪：uid=%s（class 12 + 只覆盖 staff.panel）" % AID[0],
   bool(PT) and AID[0] > 0, "actor=%s" % AID[0])


def grant(key, val=True):
    return call("PUT", "/admin/user-permissions",
                {"user_id": AID[0], "permission_key": key, "granted": val},
                token=ROOT)[0]


# ===== CH2 P-250 能力矩阵读口 =====
s_up, b_up = call("GET", "/admin/user-permissions?user_id=1", None, token=PT)
txt_up = str(b_up)[:120]
ok("CH2 P-250：低档探针读**站长（user_id=1）的生效权限清单**必须被挡住——"
   "这条给的是「谁有什么权力」的地图，同文件的写口却要管理级键。"
   "修后必须 403；现网 http=%s 内容=%s" % (s_up, txt_up),
   s_up == 403, "up=%s 内容=%s" % (s_up, txt_up))

# ===== CH3 P-251 角色分配读口 =====
s_ur, b_ur = call("GET", "/admin/user-roles?user_id=1", None, token=PT)
ok("CH3 P-251：任意用户的角色分配（`/admin/user-roles?user_id=`）同样要"
   "档位；现网 http=%s 内容=%s" % (s_ur, str(b_ur)[:96]),
   s_ur == 403, "ur=%s" % s_ur)

# ===== CH4 P-249 站点设定读口 + 悬空键 =====
s_st, b_st = call("GET", "/admin/settings", None, token=PT)
n_set = len((((b_st or {}).get("data") or {}).get("settings")) or [])
# P-249 本批**不接**，且这里不冒充绿：承载文件 admin_http/site.rs 是存量超限
# 文件（行数=基线，只许瘦不许胖），而站长正在其中做排版在途改动 ⇒ 加守卫就破
# 行数门、拆文件又要覆盖他们的 hunks。改为**显式钉桩**：断言这条洞仍在口径 B
# 清单里（等下一批把 site.rs 按域拆小后接上，届时这条钉桩会红，提醒删桩）。
pend = "/admin/settings" in coarse("admin_http")[0]
print("[WARN] CH4 P-249 已知未收口：`GET /admin/settings` 对只持 staff.panel 的"
      "低档探针仍是 http=%s（读到 %d 条配置）；`settings.view` 键在目录里但"
      "消费点=%d 处（悬空键，批六十的 implemented 对账同族）"
      % (s_st, n_set, code("admin_http/site.rs").count("perm::SETTINGS_VIEW")))
ok("CH4 P-249 钉桩：`/admin/settings` 必须仍被栅栏记为未收口（不许静默漂成"
   "「已处理」）；下一批拆小 site.rs 后接 `settings.view` 并删掉本桩",
   pend and s_st == 200, "在册=%s http=%s" % (pend, s_st))

# ===== CH5 绿证：授予 user.view 后两张矩阵读口必须读得到（不是整口封死）=====
grant("user.view")
s_up2, b_up2 = call("GET", "/admin/user-permissions?user_id=1", None,
                    token=PT)
s_ur2, _ = call("GET", "/admin/user-roles?user_id=1", None, token=PT)
ok("CH5 授权 user.view 后两口回业务层（200）：%s/%s" % (s_up2, s_ur2),
   s_up2 == 200 and s_ur2 == 200, "up=%s ur=%s" % (s_up2, s_ur2))

# ===== CH6 档位：读他人数据按批六十一的分档（92）=====
for key, floor in (("user.view", 92), ("settings.view", 90)):
    fl = psql("SELECT min(role_key::integer) FROM role_permissions WHERE "
              "role_type='class' AND granted AND permission_key='%s' "
              "AND role_key ~ '^[0-9]+$'" % key)
    ok("CH6 %s 最低档=%s（要 >=%d，档位是站长可改的一行授予，代码只认键）"
       % (key, fl or "-", floor),
       fl not in ("", "None") and int(fl) >= floor, "档=%s" % fl)

# ===== CH7 栅栏修正：一跳助手 + authz::can 认定 =====
h_all, hp_all = [], []
for d in DIRS:
    hh, hp = coarse(d)
    h_all += hh
    hp_all += hp
ok("CH7a 修正后的口径 B 真洞=%d、守卫在助手里（不算洞）=%d；本批两条读口必须"
   "已消失（仍在册=%s）" % (len(h_all), len(hp_all),
      [r for r in ("/admin/user-permissions", "/admin/user-roles")
       if r in h_all] or "无"),
   not [r for r in ("/admin/user-permissions", "/admin/user-roles")
        if r in h_all],
   "真洞=%d 助手守卫=%d" % (len(h_all), len(hp_all)))
ok("CH7b 两条历史误报必须被一跳解析剔掉：`/admin/increment-bulk`（守卫在 "
   "increment_bulk_validate 内）与 `/admin/grants`（守卫在 can_view 内）"
   "——现判为洞=%s" % [r for r in ("/admin/increment-bulk", "/admin/grants")
                       if r in h_all],
   not [r for r in ("/admin/increment-bulk", "/admin/grants")
        if r in h_all], "助手守卫清单=%s" % sorted(set(hp_all)))

# ===== CH8 收尾 =====
cleanup()
RP1 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CH8 真实授权表一条都没改（%d→%d）；探针成员清零；探针用户级覆盖清空"
   % (RP0, RP1, ),
   RP1 == RP0
   and psql("SELECT count(*) FROM users WHERE username LIKE 'chh%%' "
            "AND status < 2") == "0"
   and psql("SELECT count(*) FROM user_permissions WHERE user_id = %d"
            % (AID[0] or 0)) == "0",
   "RP=%s" % RP1)

summary()
