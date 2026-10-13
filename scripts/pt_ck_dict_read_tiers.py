# -*- coding: utf-8 -*-
"""CK 闸门（批七十一）：字典/队列读口接各域已有的键——九张嘴同一批形状。

修前（现网 `api:aa1019b`）实测，探针 class 12、只覆盖 `staff.panel`：
  P-262 `GET /admin/reports`       举报队列（举报人 + 对象 + 理由全文），
        同文件 claim/release/resolve 三口都要 `reports.handle`(92)
  P-263 `GET /admin/roles`         角色字典（含每个角色的权限键），写口要
        `roles.manage`(99)
  P-264 `GET /admin/jixiao-types`  考核岗位表（含 base_pay 基准薪），写口要
        `exam.manage`(93)
  P-265 `GET /admin/medals`        勋章字典（含 duration_days 等），写口要
        `medal.manage`(93)
  P-266 `GET /admin/section-dict` + P-267 `GET /admin/section-modes`
        站型分区字典/模式表，同文件写口要 `categories.manage`(93)
  P-268 `GET /admin/tasks`         队列任务表，同文件写口要 `task.manage`(93)
  P-269 `GET /admin/torrent-ops`   **种子操作日志**（operator + action + detail），
  P-270 `GET /admin/torrents`      全站种子列表 —— 这两条承载文件里没有任何键，
        接目录里已有的 `torrent.manage`（现值 class/90=全部职员 ⇒ 本批接上后
        对真实职员行为不变，但从「硬编码 staff()」变成**可撤销**；要不要抬档
        由站长定）。

纪律（承 CE/CF/CG/CH/CI/CJ）：探针只走 `user_permissions` 覆盖，真实授权表只读；
每条收紧配一层正控（CK3 授权后必须离开权限层）；静态判据先去 `//` 注释；
引用面按同域全部文件合计；已知未收口继续钉桩（CK5），别把没修的说成修了。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_ck_dict_read_tiers.py
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
NAME = "ckk%d" % TAG
AID = [0]
DIRS = ("admin_http", "admin_p2_http", "admin_p3_http", "staff_http")
MEMBER_SELF = ("/donate/state", "/donate/topup", "/donate/order-status",
               "/donate/order")
ENTRY = re.compile(r"staff\(|require_auth\(")
GUARD = re.compile(r"require_(any_)?perm|authz::can|require_\w*(manage|view|"
                   r"handle|grant|assign)\w*\(")
FNDEF = re.compile(r"(?:pub(?:\(\w+\))?\s+)?(?:async\s+)?fn (\w+)\s*[(<]")
CALLRE = re.compile(r"(?:super|crate)::[\w:]*\b(\w+)\(|\b(\w+)\(")
BUILTIN = {"staff", "require_auth", "ok", "some", "none", "format", "json",
           "vec", "chunks", "map", "filter", "collect", "begin", "commit",
           "bind", "execute", "fetch_all", "fetch_one", "fetch_optional"}
ITEMS = (
    ("GET", "/admin/reports", "reports.handle", "REPORTS_HANDLE",
     "admin_http/reports.rs"),
    ("GET", "/admin/roles", "roles.manage", "ROLES_MANAGE",
     "admin_http/roles.rs"),
    ("GET", "/admin/jixiao-types", "exam.manage", "EXAM_MANAGE",
     "admin_p3_http/jixiao_types.rs"),
    ("GET", "/admin/medals", "medal.manage", "MEDAL_MANAGE",
     "admin_p3_http/medals.rs"),
    ("GET", "/admin/section-dict", "categories.manage", "CATEGORIES_MANAGE",
     "admin_p3_http/section_dict.rs"),
    ("GET", "/admin/tasks", "task.manage", "TASK_MANAGE",
     "admin_p3_http/tasks.rs"),
    ("GET", "/admin/torrent-ops", "torrent.manage", "TORRENT_MANAGE",
     "admin_http/torrent_ops.rs"),
    ("GET", "/admin/torrents", "torrent.manage", "TORRENT_MANAGE",
     "admin_http/torrents.rs"),
)
PINS = ("/admin/claims", "/admin/claims/release", "/admin/settings",
        "/admin/overview", "/admin/section-modes")


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
    ts = {fn: chr(10).join(l.split("//")[0] for l in io.open(
        os.path.join(base, fn), encoding="utf-8").read().splitlines())
        for fn in sorted(os.listdir(base)) if fn.endswith(".rs")}
    bodies, holes = {}, []
    for t in ts.values():
        for m in FNDEF.finditer(t):
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
    for t in ts.values():
        for b in t.split(chr(10) + "#["):
            m = re.match(r'(get|post|put|delete)\("([^"]+)"\)', b)
            if not m or not ENTRY.search(b) or GUARD.search(b):
                continue
            names = [a or c for a, c in CALLRE.findall(b)]
            if [x for x in names if x not in BUILTIN and x in bodies
                    and GUARD.search(bodies[x])]:
                continue
            if m.group(2) in MEMBER_SELF:
                continue
            holes.append(m.group(2))
    return holes


def cleanup():
    if AID[0]:
        psql("DELETE FROM user_permissions WHERE user_id = %d" % AID[0])
        if psql("SELECT status::text FROM users WHERE id = %d" % AID[0]) != "2":
            call("POST", "/admin/users/status", {"user_id": AID[0],
                                                 "status": 2}, token=ROOT)
        call("DELETE", "/admin/users/%d" % AID[0], None, token=ROOT)


atexit.register(cleanup)

RP0 = int(psql("SELECT count(*) FROM role_permissions") or -1)
CAT = psql("SELECT string_agg(key, ',') FROM (SELECT key FROM permissions "
           "WHERE key IN ('reports.handle','roles.manage','exam.manage',"
           "'medal.manage','categories.manage','task.manage',"
           "'torrent.manage') ORDER BY key) t")
ok("CK0 前提：授权表 %d 行，本批七把键都在目录里（=%s）" % (RP0, CAT),
   RP0 > 0 and CAT.count(",") == 6, "键=%s" % CAT)

clear_register_rl()
codes = (call("POST", "/admin/invites", {"user_id": 1, "count": 1},
              token=ROOT)[1] or {}).get("data", {}).get("codes") or []
cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
if codes and len(nums) >= 2:
    call("POST", "/auth/register", {
        "username": NAME, "email": "%s@ck.local" % NAME,
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
ok("CK1 探针就绪：uid=%s（class 12 + 只覆盖 staff.panel）" % AID[0],
   bool(PT) and AID[0] > 0, "actor=%s" % AID[0])

# ===== CK2 九条读口：修后必须 403 =====
for method, route, key, const, f in ITEMS:
    s, b = call(method, route + "?page=1&per_page=5", None, token=PT)
    d = (b or {}).get("data")
    n = len(d if isinstance(d, list) else ((d or {}).get("rows") or
                                           (d or {}).get("items") or []))
    ok("CK2 %s：低档探针（只 staff.panel）读这条，而同域写口要 %s ⇒ 台账与操作"
       "同档；现网 http=%s（%d 条）" % (route, key, s, n), s == 403,
       "%s=%s(%d 条)" % (route, s, n))

# ===== CK3 正控：授予七把键后必须离开权限层 =====
for key in ("reports.handle", "roles.manage", "exam.manage", "medal.manage",
            "categories.manage", "task.manage", "torrent.manage"):
    call("PUT", "/admin/user-permissions",
         {"user_id": AID[0], "permission_key": key, "granted": True},
         token=ROOT)
got = [call("GET", route + "?page=1&per_page=5", None, token=PT)[0]
       for _m, route, _k, _c, _f in ITEMS]
ok("CK3 授权后八条口全部离开权限层（非 403）：%s" % got,
   all(c != 403 for c in got), "%s" % got)

# ===== CK4 静态引用面（按同域文件合计）=====
for const, files, floor in (
        ("REPORTS_HANDLE", ("admin_http/reports.rs",), 3),
        ("ROLES_MANAGE", ("admin_http/roles.rs",), 2),
        ("EXAM_MANAGE", ("admin_p3_http/jixiao_types.rs",), 2),
        ("MEDAL_MANAGE", ("admin_p3_http/medals.rs",), 2),
        ("CATEGORIES_MANAGE", ("admin_p3_http/section_dict.rs",
                             "admin_p3_http/sections.rs"), 4),
        ("TASK_MANAGE", ("admin_p3_http/tasks.rs",), 2),
        ("TORRENT_MANAGE", ("admin_http/torrent_ops.rs",
                            "admin_http/torrents.rs"), 2)):
    refs = sum(code(f).count("perm::%s" % const) for f in files)
    ok("CK4 %s 引用面=%d 处（>= %d）" % (const, refs, floor), refs >= floor,
       "引用=%d" % refs)

# ===== CK5 钉桩：四条已知未收口必须仍在册 =====
holes = sum((coarse(d) for d in DIRS), [])
on = [r for r in PINS if r in holes]
print("[WARN] CK5 已知未收口=%s；`/admin/settings` 因承载文件 site.rs 存量"
      "超限且站长在其中排版、claims 两条因该域无语义吻合的键、`/admin/overview` "
      "因只有聚合计数（低敏待定档）、`/admin/section-modes` 因承载文件 sections.rs "
      "是基线跟踪文件（只许瘦不许胖，加 6 行守卫就破行数门）⇒ 都不接，只钉桩。" % on)
ok("CK5 队列=%d 条（本批八条应已离开）；五条钉桩仍在册=%s（须 5 条）"
   % (len(holes), on), not [r for _m, r, _k, _c, _f in ITEMS if r in holes]
   and len(on) == 5 and len(holes) <= 12,
   "队列=%d 钉桩=%s" % (len(holes), on))

cleanup()
RP1 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CK6 真实授权表一条都没改（%d→%d）；探针成员与其用户级覆盖清零" % (RP0, RP1),
   RP1 == RP0 and psql("SELECT count(*) FROM users WHERE username LIKE "
                        "'ckk%%' AND status < 2") == "0"
   and psql("SELECT count(*) FROM user_permissions WHERE user_id = %d"
            % (AID[0] or 0)) == "0", "RP=%s" % RP1)

summary()
