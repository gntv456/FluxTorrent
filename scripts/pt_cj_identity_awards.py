# -*- coding: utf-8 -*-
"""CJ 闸门（批七十）：身份变更史与字幕评选域的台账/写口比同域写口还宽。

现网（修前）实测，探针 class 12、只覆盖 `staff.panel`（低于一切职员档）：
  P-256 `GET /admin/rename-logs` —— `username_change_logs`：**每个成员的旧名→新名
        + 操作者 + 时间**。这是身份变更史（PII，也是社工/冒充素材），只判 `staff()`。
  P-257 `GET /admin/modify-logs` —— `user_modify_logs`：uid + username + **谁改了
        什么（content）** = 管理组对成员的处置记录整页。
  P-258 `GET /admin/subtitles/awards/candidates` —— 评选候选池：username + 字幕标题
        + score + rating + downloads。同文件 `POST .../awards/grant` 要
        `subtitles.grant`(95)。
  P-259 `POST /admin/subtitles/awards/build` —— **这是写口**（重建候选池），却只判
        `staff()` ⇒ 91 发布员能把评选候选池重刷一遍（改掉谁在池子里 = 改掉谁能拿奖）。
  P-260 `GET /admin/subtitles/certs` —— 认证字幕组名单（username + tier + reason +
        授予时间）。同文件的盖章/撕章要 `subtitles.grant`(95)。
  P-261 `GET /admin/tracker-urls` —— tracker 地址表（url / with_passkey / 默认 /
        优先级）。同文件写口要 `tracker.manage`。
  ⇒ 一律「台账与操作同一把键」，**零新增键**。

另一条是栅栏自身的账：`staff_http` 里的 `/donate/state|topup|order-status|order`
**不是职员口**——代码全程以 `auth.id` 为主体（读自己的捐赠台账、下自己的单），
是会员自助口。把它们算进「职员粗闸」会把队列灌水并掩盖真洞，CJ5 专门把这条
分类钉住。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_cj_identity_awards.py
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
NAME = "cjj%d" % TAG
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

# (方法, 路由, 需要的键, 承载文件)
ITEMS = (
    ("GET", "/admin/rename-logs", "user.view", "admin_p3_http/rename_logs.rs"),
    ("GET", "/admin/modify-logs", "user.view", "admin_p3_http/rename_logs.rs"),
    ("GET", "/admin/subtitles/awards/candidates", "subtitles.grant",
     "admin_http/subtitle_awards.rs"),
    ("POST", "/admin/subtitles/awards/build", "subtitles.grant",
     "admin_http/subtitle_awards.rs"),
    ("GET", "/admin/subtitles/certs", "subtitles.grant",
     "admin_http/subtitle_certs.rs"),
    ("GET", "/admin/tracker-urls", "tracker.manage",
     "admin_p3_http/tracker_urls.rs"),
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
    ts = {fn: chr(10).join(l.split("//")[0] for l in io.open(
        os.path.join(base, fn), encoding="utf-8").read().splitlines())
        for fn in sorted(os.listdir(base)) if fn.endswith(".rs")}
    bodies, holes, helped = {}, [], []
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
                continue  # 会员自助口（全程以 auth.id 为主体），不入职员队列
            holes.append(m.group(2))
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
KEYS = ("user.view", "subtitles.grant", "tracker.manage")
ok("CJ0 前提：授权表 %d 行，%s 三把键都在目录里" % (RP0, "/".join(KEYS)),
   RP0 > 0 and psql("SELECT count(*) FROM permissions WHERE key IN "
                    "('user.view','subtitles.grant','tracker.manage')") == "3",
   "RP=%d" % RP0)

clear_register_rl()
codes = (call("POST", "/admin/invites", {"user_id": 1, "count": 1},
              token=ROOT)[1] or {}).get("data") or {}.get("codes") or []
cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
if codes and len(nums) >= 2:
    call("POST", "/auth/register", {
        "username": NAME, "email": "%s@cj.local" % NAME,
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
ok("CJ1 探针就绪：uid=%s（class 12 + 只覆盖 staff.panel）" % AID[0],
   bool(PT) and AID[0] > 0, "actor=%s" % AID[0])

# ===== CJ2 六条口：档位必须挡住（修前全放行）=====
for method, route, key, f in ITEMS:
    body = {} if method == "GET" else {"period": "1999-01"}
    s, b = call(method, route, body, token=PT)
    d = (b or {}).get("data")
    n = len(d if isinstance(d, list) else ((d or {}).get("rows") or []))
    ok("CJ2 %s %s：低档探针（只 staff.panel）打这条，而同域写口要 %s ⇒ 必须"
       "同档；现网 http=%s（%d 条）" % (method, route, key, s, n),
       s == 403, "%s %s=%s(%d 条)" % (method, route, s, n))

# ===== CJ3 正控：授予对应键后必须回业务层 =====
for k in KEYS:
    grant(k)
got = []
for method, route, key, f in ITEMS:
    body = {} if method == "GET" else {"period": "1999-01"}
    got.append(call(method, route, body, token=PT)[0])
ok("CJ3 授权三把键后六条口全部离开权限层（非 403）：%s" % got,
   all(c != 403 for c in got), "%s" % got)

# ===== CJ4 静态：守卫真的接进了承载文件 =====
for const, files, floor in (("USER_VIEW", ("admin_p3_http/rename_logs.rs",), 2),
                            ("SUBTITLES_GRANT",
                             ("admin_http/subtitle_awards.rs",
                              "admin_http/subtitle_certs.rs"), 5),
                            ("TRACKER_MANAGE",
                             ("admin_p3_http/tracker_urls.rs",), 4)):
    refs = sum(code(f).count("perm::%s" % const) for f in files)
    ok("CJ4 %s 引用面=%d 处（本批读口/写口接进来后应 >=%d）" % (const, refs,
       floor), refs >= floor, "引用=%d" % refs)

# ===== CJ5 栅栏分类：会员自助口不进取职员队列 =====
staff_holes = sum((coarse(d)[0] for d in DIRS), [])
print("[WARN] CJ5 `/donate/state|topup|order-status|order` 四条是**会员自助口**"
      "（代码全程以 `auth.id` 为主体：读自己的捐赠台账、下自己的单），不是职员面"
      "——按职员粗闸数它们就是灌水。")
ok("CJ5 这四条不得计入职员队列：队列里现在含=%s"
   % [r for r in MEMBER_SELF if r in staff_holes],
   not [r for r in MEMBER_SELF if r in staff_holes],
   "队列=%d 条" % len(staff_holes))

# ===== CJ6 队列下降 + 钉桩仍在 =====
pend = [r for r in ("/admin/claims", "/admin/claims/release",
                    "/admin/settings") if r in staff_holes]
ok("CJ6a 本批六条必须已离开队列（剩余=%d 条）；三条已知未收口仍要在册（=%s）"
   % (len(staff_holes), pend or "无"),
   len(staff_holes) <= 24 and len(pend) == 3,
   "剩=%d 钉桩=%s" % (len(staff_holes), pend))
ok("CJ6b 六条本批口不得再出现在队列里：%s"
   % [r for _m, r, _k, _f in ITEMS if r in staff_holes],
   not [r for _m, r, _k, _f in ITEMS if r in staff_holes], "已在册=%s"
   % [r for _m, r, _k, _f in ITEMS if r in staff_holes])

# ===== CJ8 界面读者：本批新加的 403 在四个面板上怎么落地 =====
def wsrc(rel):
    return io.open(os.path.join(REPO, rel), encoding="utf-8").read()


AW = wsrc(os.path.join("apps", "web", "components",
                       "admin-subtitles-awards.tsx"))
swallow = ("} catch {" + chr(10) + "      setRows([]);") in AW
print("[WARN] CJ8 界面读者核对：`admin-userlogs.tsx`（改名/处置日志）与 "
      "`admin-trackers.tsx` 的 catch 都把 ApiError.message 弹出来（403 显示"
      "「无权限」，不假成功）；但 `admin-subtitles-awards.tsx` 的 load() 是裸 "
      "`catch { setRows([]); }` ⇒ 403 会被渲染成「本期没有候选人」（把失败读成"
      "空数据）。该文件站长正在改（工作树 M），本批不覆盖他们的 hunks；修法是一"
      "行：同 catch 里补 setMsg(e instanceof ApiError ? e.message : t.loadFail)。")
ok("CJ8 「403 被读成空列表」这条必须记为已知未修（仍吞=%s），修好后删本桩"
   % swallow, swallow is True, "吞掉=%s" % swallow)

cleanup()
RP1 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CJ7 真实授权表一条都没改（%d→%d）；探针成员与其用户级覆盖清零" % (RP0, RP1),
   RP1 == RP0 and psql("SELECT count(*) FROM users WHERE username LIKE "
                        "'cjj%%' AND status < 2") == "0"
   and psql("SELECT count(*) FROM user_permissions WHERE user_id = %d"
            % (AID[0] or 0)) == "0", "RP=%s" % RP1)

summary()
