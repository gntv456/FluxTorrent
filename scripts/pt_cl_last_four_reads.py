# -*- coding: utf-8 -*-
"""CL 闸门（批七十二）：职员域队列收尾四条——台账读口接同域已有的键。

现网（修前，镜像 `api:aa1020a`）实测，探针 class 12、只覆盖 `staff.panel`：
  P-271 `GET /admin/deny-reasons`  驳回理由字典（管理组日常话术库），
        同文件三条写口都要 `settings.manage`(99)
  P-272 `GET /admin/menu-items`    前台导航菜单项（站点结构地图），
        同文件写口要 `settings.manage`(99)
  P-273 `GET /admin/avatar-frames` 头像框字典（含价格/上架标记），
        同文件三条写口都要 `prop.manage`(93)
  P-274 `GET /admin/shop-items`    商店道具表（价格、库存配额 stock_quota、
        上下架），同文件三条写口都要 `prop.manage`(93)
  ⇒ 一律「台账与操作同一把键」，零新增键、零迁移。

队列现状：口径 B 从 35 一路降到 12，本批收掉 4 条 ⇒ 8 条，其中
五条是**故意不接**（在 CK/CJ/CH/CI 里各钉了桩，接上后桩会转红提醒删）：
`/admin/settings`（site.rs 行数=基线且站长在排版）、`/admin/section-modes`
（sections.rs 325 行超基线）、`/admin/claims` 与 `/admin/claims/release`
（该域无语义吻合的键，不猜）、`/admin/overview`（只有聚合计数，待定档）。
另三条（`/admin/agentrules`、`/admin/promo-kinds`、`admin_p3_http/roles.rs`
的 `/admin/roles-dict`）承载文件正被站长的在途改动占着 ⇒ 不覆盖他们的 hunks，
等其提交后再接。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_cl_last_four_reads.py
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
NAME = "cll%d" % TAG
AID = [0]
DIRS = ("admin_http", "admin_p2_http", "admin_p3_http", "staff_http")
PINS = ("/admin/settings", "/admin/section-modes", "/admin/claims",
        "/admin/claims/release", "/admin/overview")
ITEMS = (
    ("GET", "/admin/deny-reasons", "settings.manage", "SETTINGS_MANAGE",
     "admin_http/deny.rs", 4),
    ("GET", "/admin/menu-items", "settings.manage", "SETTINGS_MANAGE",
     "admin_p2_http/menus.rs", 2),
    ("GET", "/admin/avatar-frames", "prop.manage", "PROP_MANAGE",
     "admin_p3_http/frames.rs", 4),
    ("GET", "/admin/shop-items", "prop.manage", "PROP_MANAGE",
     "admin_p3_http/shop.rs", 4),
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


def coarse(d):
    base = os.path.join(REPO, "apps", "api", "src", d)
    ts = {fn: code(os.path.join(d, fn)) for fn in sorted(os.listdir(base))
          if fn.endswith(".rs")}
    FN = re.compile(r"(?:pub(?:\(\w+\))?\s+)?(?:async\s+)?fn (\w+)\s*[(<]")
    CALL = re.compile(r"(?:super|crate)::[\w:]*\b(\w+)\(|\b(\w+)\(")
    BUILTIN = {"staff", "require_auth", "ok", "some", "none", "format", "json",
               "vec", "chunks", "map", "filter", "collect", "begin", "commit",
               "bind", "execute", "fetch_all", "fetch_one", "fetch_optional"}
    ENTRY = re.compile(r"staff\(|require_auth\(")
    GUARD = re.compile(r"require_(any_)?perm|authz::can|require_\w*(manage|"
                       r"view|handle|grant|assign)\w*\(")
    bodies = {}
    for t in ts.values():
        for m in FN.finditer(t):
            i = t.find("{", m.end())
            if i < 0:
                continue
            dep, j = 0, i
            while j < len(t):
                if t[j] == "{":
                    dep += 1
                elif t[j] == "}":
                    dep -= 1
                    if dep == 0:
                        break
                j += 1
            bodies[m.group(1)] = bodies.get(m.group(1), "") + t[i:j + 1]
    out = []
    for t in ts.values():
        for b in t.split(chr(10) + "#["):
            m = re.match(r'(get|post|put|delete)\("([^"]+)"\)', b)
            if not m or not ENTRY.search(b) or GUARD.search(b):
                continue
            if m.group(2).startswith("/donate"):
                continue
            names = [a or c for a, c in CALL.findall(b)]
            if [x for x in names if x not in BUILTIN and x in bodies
                    and GUARD.search(bodies[x])]:
                continue
            out.append(m.group(2))
    return out


def cleanup():
    if AID[0]:
        psql("DELETE FROM user_permissions WHERE user_id = %d" % AID[0])
        if psql("SELECT status::text FROM users WHERE id = %d" % AID[0]) != "2":
            call("POST", "/admin/users/status", {"user_id": AID[0],
                                                 "status": 2}, token=ROOT)
        call("DELETE", "/admin/users/%d" % AID[0], None, token=ROOT)


atexit.register(cleanup)

RP0 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CL0 前提：授权表 %d 行，`settings.manage`/`prop.manage` 都在目录里" % RP0,
   RP0 > 0 and psql("SELECT count(*) FROM permissions WHERE key IN "
                    "('settings.manage','prop.manage')") == "2", "RP=%d" % RP0)

clear_register_rl()
codes = (call("POST", "/admin/invites", {"user_id": 1, "count": 1},
              token=ROOT)[1] or {}).get("data") or {}.get("codes") or []
cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
if codes and len(nums) >= 2:
    call("POST", "/auth/register", {
        "username": NAME, "email": "%s@cl.local" % NAME,
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
ok("CL1 探针就绪：uid=%s（class 12 + 只覆盖 staff.panel）" % AID[0],
   bool(PT) and AID[0] > 0, "actor=%s" % AID[0])

for method, route, key, const, f, _floor in ITEMS:
    s, b = call(method, route, None, token=PT)
    d = (b or {}).get("data")
    n = len(d if isinstance(d, list) else ((d or {}).get("rows") or []))
    ok("CL2 %s：低档探针（只 staff.panel）读这条台账，同文件写口要 %s ⇒ 必须同"
       "档；现网 http=%s（%d 条）" % (route, key, s, n), s == 403,
       "%s=%s(%d 条)" % (route, s, n))

for key in ("settings.manage", "prop.manage"):
    call("PUT", "/admin/user-permissions",
         {"user_id": AID[0], "permission_key": key, "granted": True},
         token=ROOT)
got = [call(method, route, None, token=PT)[0]
       for method, route, _k, _c, _f, _fl in ITEMS]
ok("CL3 正控：授权两把键后四条口都离开权限层（非 403）：%s" % got,
   all(c != 403 for c in got), "%s" % got)

for const, f, floor in (("SETTINGS_MANAGE", "admin_http/deny.rs", 4),
                        ("SETTINGS_MANAGE", "admin_p2_http/menus.rs", 2),
                        ("PROP_MANAGE", "admin_p3_http/frames.rs", 4),
                        ("PROP_MANAGE", "admin_p3_http/shop.rs", 4)):
    refs = code(f).count("perm::%s" % const)
    ok("CL4 %s 在 %s 的引用面=%d 处（要 >=%d）" % (const, f, refs, floor),
       refs >= floor, "引用=%d" % refs)

holes = sum((coarse(d) for d in DIRS), [])
still = [r for r in PINS if r in holes]
done = [r for _m, r, _k, _c, _f, _fl in ITEMS if r in holes]
ok("CL5 队列=%d 条：本批四条应已离开（仍在册=%s），五条故意未接的钉桩必须在册"
   "（=%s，须 5 条）" % (len(holes), done or "无", still),
   not done and len(still) == 5 and len(holes) <= 8,
   "队列=%d 钉桩=%s" % (len(holes), still))

cleanup()
RP1 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CL6 真实授权表一条都没改（%d→%d）；探针成员与其用户级覆盖清零" % (RP0, RP1),
   RP1 == RP0 and psql("SELECT count(*) FROM users WHERE username LIKE "
                       "'cll%%' AND status < 2") == "0"
   and psql("SELECT count(*) FROM user_permissions WHERE user_id = %d"
            % (AID[0] or 0)) == "0", "RP=%s" % RP1)

summary()
