# -*- coding: utf-8 -*-
"""CI 闸门（批六十九）：台账读口比它自己的写口还宽——五张嘴两把锁。

现网（修前）实测，探针 class 12、只覆盖 `staff.panel`（低于一切职员档）：
  P-252 `GET /admin/user-props`（谁买了什么道具，含 username + 价格）
        —— 同文件 `DELETE /admin/user-props/{order_id}` 要 `prop.manage`(93)。
  P-253 `GET /admin/shop-orders`（全站商店流水：user_id / username / item /
        price / 时间）—— 同上，道具域管理级键。
  P-254 `GET /admin/sticky-promos`（置顶促销位内容与生效区间）
        —— 同文件 POST/PUT/DELETE 三条**全部**要 `settings.manage`(99)。
  P-255 `GET /admin/message-templates` 与 `POST /admin/message-templates/preview`
        （站内信模板正文与预览）—— 同域 PUT 要 `settings.manage`(99)。
  ⇒ 修法一律「台账与操作同一把键」（批六十七定的口径），**零新增权限键**。

边界与纪律：
  - 公开口 `/sticky-promos`（前台首页顶栏用）**不在本批管辖范围**，CI5 专门
    验它无 token 仍 200 —— 收档收错文件会把公开页一起关掉；
  - 探针授权只走 `user_permissions` 覆盖，真实档位表只读不改；
  - 每条收紧判据配一层正控（CI3：授予对应键后必须 200）；
  - 静态判据先去 `//` 注释；引用面按同域全部文件合计（批六十六那条账）；
  - `GET /admin/claims` 与 `POST /admin/claims/release`（把**别人的保种记录**
    标成退出：`UPDATE seed_preserve SET exited_at=now()`）也只判 `staff()`，
    但该域没有语义吻合的现成键（`hr.view`/`hr.exempt`/`hr.pardon` 都不是
    「放掉申领」）⇒ 本批**不猜键**，改为显式钉桩 + WARN，列待拍板。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_ci_ledger_read_tiers.py
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
NAME = "cii%d" % TAG
AID = [0]
DIRS = ("admin_http", "admin_p2_http", "admin_p3_http", "staff_http")
ENTRY = re.compile(r"staff\(|require_auth\(")
GUARD = re.compile(r"require_(any_)?perm|authz::can|require_\w*(manage|view|"
                   r"handle|grant|assign)\w*\(")
FNDEF = re.compile(r"(?:pub(?:\(\w+\))?\s+)?(?:async\s+)?fn (\w+)\s*[(<]")
CALLRE = re.compile(r"(?:super|crate)::[\w:]*\b(\w+)\(|\b(\w+)\(")
BUILTIN = {"staff", "require_auth", "ok", "some", "none", "format", "json",
           "vec", "chunks", "map", "filter", "collect", "begin", "commit",
           "bind", "execute", "fetch_all", "fetch_one", "fetch_optional"}

# (路由, 方法, 需要的键, 取证字段名, 承载文件)
READS = (
    ("/admin/user-props", "GET", "prop.manage", "username",
     "admin_p3_http/user_props.rs"),
    ("/admin/shop-orders", "GET", "prop.manage", "username",
     "admin_p3_http/shop_orders_admin.rs"),
    ("/admin/sticky-promos", "GET", "settings.manage", "title",
     "admin_p2_http/promos.rs"),
    ("/admin/message-templates", "GET", "settings.manage", "body",
     "admin_p2_http/templates.rs"),
)


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


def code(f):
    return chr(10).join(l.split("//")[0] for l in src(f).splitlines())


def dtexts(d):
    base = os.path.join(REPO, "apps", "api", "src", d)
    return {fn: chr(10).join(l.split("//")[0] for l in io.open(
        os.path.join(base, fn), encoding="utf-8").read().splitlines())
        for fn in sorted(os.listdir(base)) if fn.endswith(".rs")}


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
    ts, bodies, holes, helped = dtexts(d), {}, [], []
    bodies = fn_bodies(ts)
    for t in ts.values():
        for b in t.split(chr(10) + "#["):
            m = re.match(r'(get|post|put|delete)\("([^"]+)"\)', b)
            if not m or not ENTRY.search(b) or GUARD.search(b):
                continue
            names = [a or c for a, c in CALLRE.findall(b)]
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
ok("CI0 前提：授权表 %d 行，`prop.manage`/`settings.manage` 都在目录里" % RP0,
   RP0 > 0 and psql("SELECT count(*) FROM permissions WHERE key IN "
                    "('prop.manage','settings.manage')") == "2", "RP=%d" % RP0)

clear_register_rl()
codes = (call("POST", "/admin/invites", {"user_id": 1, "count": 1},
              token=ROOT)[1] or {}).get("data", {}).get("codes") or []
cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
if codes and len(nums) >= 2:
    call("POST", "/auth/register", {
        "username": NAME, "email": "%s@ci.local" % NAME,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    AID[0] = int(psql("SELECT id FROM users WHERE username = '%s'" % NAME) or 0)
psql("UPDATE users SET class_id = 12 WHERE id = %d" % (AID[0] or 0))


def grant(key, val=True):
    return call("PUT", "/admin/user-permissions",
                {"user_id": AID[0], "permission_key": key, "granted": val},
                token=ROOT)[0]


grant("staff.panel")
PT = login(NAME, "password123") if AID[0] else None
ok("CI1 探针就绪：uid=%s（class 12 + 只覆盖 staff.panel）" % AID[0],
   bool(PT) and AID[0] > 0, "actor=%s" % AID[0])

# ===== CI2 台账读口的档位（逐条，红证带「读到几条」）=====
for route, method, key, field, f in READS:
    s, b = call(method, route, None, token=PT)
    d = (b or {}).get("data")
    n = len(d if isinstance(d, list) else ((d or {}).get("rows") or []))
    ok("CI2 %s：低档探针（只 staff.panel）读台账，而同文件的**写**口要 %s "
       "⇒ 台账必须与操作同档；现网 http=%s 读到 %d 条"
       % (route, key, s, n), s == 403, "%s=%s(%d 条)" % (route, s, n))

# ===== CI2b 第五口：模板预览（POST 但不落库，同样要档位）=====
s_pv, _ = call("POST", "/admin/message-templates/preview",
               {"scene_key": "no_such_scene", "vars": {}}, token=PT)
ok("CI2b `POST /admin/message-templates/preview`（同域 PUT 要 settings.manage）"
   "：现网 http=%s —— 400 表示**没被档位挡住**、直接进了业务层报「场景不存在」"
   % s_pv, s_pv == 403, "preview=%s" % s_pv)

# ===== CI3 正控：授予对应键后必须回业务层 =====
grant("prop.manage")
grant("settings.manage")
codes6 = [call("GET", route, None, token=PT)[0]
          for route, _m, _k, _f, _x in READS]
s_pv2 = call("POST", "/admin/message-templates/preview",
             {"scene_key": "no_such_scene", "vars": {}}, token=PT)[0]
codes6.append(s_pv2)
ok("CI3 授权 prop.manage + settings.manage 后五口回业务层（非 403）：%s"
   % codes6, all(c != 403 for c in codes6), "%s" % codes6)

# ===== CI4 静态：守卫真的接进了承载文件（按同域合计）=====
for const, files, floor in (("PROP_MANAGE", ("admin_p3_http/user_props.rs",
                                            "admin_p3_http/shop_orders_admin.rs"), 2),
                            ("SETTINGS_MANAGE", ("admin_p2_http/promos.rs",
                                                 "admin_p2_http/"
                                                 "templates.rs"), 4)):
    refs = sum(code(f).count("perm::%s" % const) for f in files)
    ok("CI4 %s 引用面=%d 处（读口接进来后应 >=%d，写口原本就有）"
       % (const, refs, floor), refs >= floor, "引用=%d" % refs)

# ===== CI5 公开页不许被误伤：`/sticky-promos`（前台顶栏）仍要能读 =====
s_pub, b_pub = call("GET", "/sticky-promos", None, token=None)
ok("CI5 公开口 `/sticky-promos`（无 token）不能被本批收档连带关掉：http=%s"
   % s_pub, s_pub == 200, "pub=%s" % s_pub)

# ===== CI6 钉桩：claims 族本批不猜键，必须仍在册（不许静默漂成已处理）=====
holes = sum((coarse(d)[0] for d in DIRS), [])
pend = [r for r in ("/admin/claims", "/admin/claims/release") if r in holes]
print("[WARN] CI6 已知未收口：`/admin/claims`（保种申领队列）与 "
      "`POST /admin/claims/release`（把**别人的**保种记录标成退出 "
      "`UPDATE seed_preserve SET exited_at=now()`）仍只判 staff()；该域没有"
      "语义吻合的现成键（hr.view/hr.exempt/hr.pardon 都不是「放掉申领」），"
      "本批不猜键 ⇒ 列待拍板")
ok("CI6 钉桩：claims 两条必须仍被栅栏记为未收口（在册=%s）" % (pend or "无"),
   len(pend) == 2, "在册=%s" % pend)

# ===== CI7 收尾 =====
h_all, hp_all = [], []
for d in DIRS:
    hh, hp = coarse(d)
    h_all += hh
    hp_all += hp
ok("CI7 修正后口径 B：真洞=%d、守卫在助手里=%d；本批五口已从清单消失"
   "（仍在册=%s）" % (len(h_all), len(hp_all),
      [r for r, *_ in READS if r in h_all] or "无"),
   not [r for r, *_ in READS if r in h_all], "真洞=%d" % len(h_all))
cleanup()
RP1 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CI8 真实授权表一条都没改（%d→%d）；探针成员与其用户级覆盖清零" % (RP0, RP1),
   RP1 == RP0 and psql("SELECT count(*) FROM users WHERE username LIKE "
                       "'cii%%' AND status < 2") == "0"
   and psql("SELECT count(*) FROM user_permissions WHERE user_id = %d"
            % (AID[0] or 0)) == "0", "RP=%s" % RP1)

summary()
