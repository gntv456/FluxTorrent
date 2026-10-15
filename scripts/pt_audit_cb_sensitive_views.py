# -*- coding: utf-8 -*-
"""CB 闸门（批六十三）：三条敏感职员读口的专属档位。

批六十一把 `/admin/cheaters` 与授权矩阵收了，并在 CA6b 立了「粗闸读口只减
不增」的栅栏（在册 15 条）。本批按敏感度先收最危险的六条：
  - `GET /admin/users`、`GET /admin/users/{id}`、`GET /admin/users/{id}/snatches`
      ⇒ 新键 `user.view`，授 class/92（版主及以上；发布员 91 收掉）
  - `GET /admin/login-logs`（用户名 + 登录 IP + 成败）
      ⇒ 新键 `loginlogs.view`，授 class/93（总版主及以上）
  - `GET /admin/spark-logs`、`GET /admin/torrent-buys`（魔力流水含 balance_after）
      ⇒ 新键 `sparklog.view`，授 class/95（主管及以上，与 0285 的
         user.passkey.reveal 同档——钱和凭证一样敏感）

判据纪律：
  - 红证要带**内容**，不只带状态码：今天探针（只有 staff.panel）读这几口
    不仅 200，还能数出行数（名单、IP、余额），所以断言同时看 `data` 的长度；
  - 收紧必须配「新键真的接了线」的绿证：CB3/CB4 给用户级授予新键，
    对应口必须重新 200——否则「403 了」也可能只是把门焊死；
  - 探针继续走 `user_permissions` 覆盖（`user_can` 第一段），
    不碰任何真实档位授权；收尾复算 `role_permissions` 条数；
  - 0363 迁移自身会新增授予行，所以基线在迁移后才取（CB0 先读 RP0）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_cb_sensitive_views.py
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
NAME = "cbm%d" % TAG
UID = [0]
LISTS = ["/admin/users", "/admin/login-logs", "/admin/spark-logs",
         "/admin/torrent-buys"]


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


def rows_of(body):
    d = (body or {}).get("data")
    if isinstance(d, list):
        return len(d)
    if isinstance(d, dict):
        for k in ("rows", "items", "list", "data", "logs", "users"):
            if isinstance(d.get(k), list):
                return len(d[k])
    return -1


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


def grant(key, val=True):
    return call("PUT", "/admin/user-permissions", {"user_id": UID[0],
                                                   "permission_key": key,
                                                   "granted": val},
                token=ROOT)[0]


RP0 = int(psql("SELECT count(*) FROM role_permissions") or -1)
s_ok = [call("GET", p, None, token=ROOT)[0] for p in LISTS]
ok("CB0 前提：root 读这四口都能进（%s），且库里确有内容可泄"
   "（users=%s 行、login_events=%s 行、spark_ledger=%s 行）"
   % (s_ok, psql("SELECT count(*) FROM users WHERE status < 2"),
      psql("SELECT count(*) FROM login_events"),
      psql("SELECT count(*) FROM spark_ledger")),
   all(x == 200 for x in s_ok), "http=%s" % s_ok)

# ===== CB1 探针职员（只有 staff.panel）：六条口今天全部敞开 =====
clear_register_rl()
codes = dig(call("POST", "/admin/invites", {"user_id": 1, "count": 1},
              token=ROOT)[1], "data", "codes") or []
cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
if codes and len(nums) >= 2:
    call("POST", "/auth/register", {
        "username": NAME, "email": "%s@cb.local" % NAME,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    UID[0] = int(psql("SELECT id FROM users WHERE username = '%s'" % NAME) or 0)
PT = login(NAME, "password123") if UID[0] else None
grant("staff.panel")
ok("CB1a 探针职员就绪且 /me/perms 里有 staff.panel（uid=%s）" % UID[0],
   bool(PT) and "staff.panel" in (((call("GET", "/me/perms", None,
                                         token=PT)[1] or {}).get("data")
                                   or {}).get("perms") or []),
   "uid=%s" % UID[0])
bad = []
for p in LISTS:
    s, b = call("GET", p, None, token=PT)
    if s == 200:
        bad.append("%s(%s 行)" % (p, rows_of(b)))
s_d, _ = call("GET", "/admin/users/1", None, token=PT)
s_s, _ = call("GET", "/admin/users/1/snatches", None, token=PT)
ok("CB2 P-233：只有 staff.panel 的职员（现网 class/90 = 91 发布员起全部档）"
   "读会员名单/详情/下载流水必须被拒，今天敞开的=%s；detail=%s snatches=%s"
   % (bad[:2], s_d, s_s),
   not bad and s_d == 403 and s_s == 403,
   "敞开 %d 口 %s" % (len(bad), bad))
ok("CB3 P-234：登录日志（用户名 + IP + 成败）不该对总版主以下敞开："
   "今天 http=%s 行数=%s"
   % (("/admin/login-logs" in [x.split("(")[0] for x in bad]), rows_of(
       call("GET", "/admin/login-logs", None, token=PT)[1])),
   "/admin/login-logs" not in [x.split("(")[0] for x in bad],
   "rows=%s" % rows_of(call("GET", "/admin/login-logs", None,
                             token=PT)[1]))
ok("CB4 P-235：魔力流水与种子购买流水（含 balance_after 余额）不该对主管"
   "以下敞开：今天 spark=%s buys=%s"
   % ("/admin/spark-logs" in [x.split("(")[0] for x in bad],
      "/admin/torrent-buys" in [x.split("(")[0] for x in bad]),
   not any(x.startswith("/admin/spark-logs") or
           x.startswith("/admin/torrent-buys") for x in bad),
   "敞开=%s" % bad)

# ===== CB5 三件齐：目录 + 常量 + 消费点 =====
# 键 -> [(文件, 最少引用处数)]：user.view 的引用天然跨两个文件
# （名单在 user_list.rs，详情与下载流水在 user_detail.rs）。
REFS = [("user.view", "admin_http/user_list.rs", 1),
        ("user.view", "admin_http/user_detail.rs", 2),
        ("loginlogs.view", "admin_http/records.rs", 1),
        ("sparklog.view", "admin_http/records.rs", 2)]
KEYS = ["user.view", "loginlogs.view", "sparklog.view"]
miss = []
for key in KEYS:
    const = key.upper().replace(".", "_")
    if psql("SELECT count(*) FROM permissions WHERE key = '%s'"
            % key) != "1":
        miss.append("%s:无目录行" % key)
    if ("pub const %s" % const) not in src("authz/perm.rs"):
        miss.append("%s:无常量" % key)
for key, fn, need in REFS:
    const = key.upper().replace(".", "_")
    got = src(fn).count("perm::%s" % const)
    if got < need:
        miss.append("%s 在 %s 引用 %d 处（要 %d）" % (key, fn, got, need))
ok("CB5 三个新键必须目录 / 常量 / 消费点三件齐（批六十刚修过「有标记没代码」，"
   "本批不许反向往目录里塞没人消费的键）：缺=%s" % miss, not miss,
   "缺=%d 项" % len(miss))
gr_bad = []
for key, floor in (("user.view", 92), ("loginlogs.view", 93),
                   ("sparklog.view", 95)):
    f = psql("SELECT min(role_key::integer) FROM role_permissions WHERE "
             "role_type='class' AND granted AND permission_key='%s' "
             "AND role_key ~ '^[0-9]+$'" % key)
    if f in ("", "None") or int(f) < floor:
        gr_bad.append("%s 最低档=%s（要 >=%d）" % (key, f, floor))
ok("CB6 授予档位按敏感度：user.view>=92、loginlogs.view>=93、"
   "sparklog.view>=95（class 键是阈值语义，min(role_key) 即最低可读者）：%s"
   % gr_bad, not gr_bad, "不符=%d 项" % len(gr_bad))

# ===== CB7 绿证：授予新键后对应口必须重新可用（别把门焊死）=====
res = []
for key, path in (("user.view", "/admin/users"),
                  ("loginlogs.view", "/admin/login-logs"),
                  ("sparklog.view", "/admin/spark-logs")):
    grant(key)
    res.append((key, path, call("GET", path, None, token=PT)[0]))
ok("CB7 用户级授予新键后，对应读口必须重新 200（证明键真的接进了 require_perm"
   "，不是把功能焊死）：%s" % res,
   all(s == 200 for _, _, s in res), str(res))

# ===== CB8 栅栏：CA6b 的粗闸清单应从 15 降到 <=9 =====
def coarse(dirname):
    out = []
    base = os.path.join(REPO, "apps", "api", "src", dirname)
    sep = chr(10) + "#["
    for fn in sorted(os.listdir(base)):
        if not fn.endswith(".rs"):
            continue
        t = io.open(os.path.join(base, fn), encoding="utf-8").read()
        for b in t.split(sep):
            m = re.match(r'(get|post|put|delete)\("([^"]+)"\)', b)
            # 已知盲区（写在这里而不是藏起来）：本探测只认 `let _auth = staff(`
            # 这种「绑定后不使用」的写法。改成 `let auth = staff(` 的口
            # （如 /admin/users/{id} 详情）要靠 require_perm 的窗口判定，
            # 把窗口放宽会误报一批「先 staff() 再 require_perm 隔得较远」的
            # handler（实测 24 条，其中多数其实有档位）。清这个盲区
            # 列下一批，本栅栏先按可核实的口径守。
            if not m or "let _auth = staff(" not in b:
                continue
            if "require_perm" not in b.split("let _auth = staff(")[1][:400]:
                out.append(m.group(2))
    return out


C = coarse("admin_http")
ok("CB8 粗闸读口清单：批六十一在册 15 条，本批收掉 5 条（名单/流水/登录/详情"
   "流水）后应 <= 10；探测器有已知盲区，见上面注释"
   "剩余=%s" % C, len(C) <= 10, "剩 %d 条" % len(C))

# ===== CB9 收尾 =====
cleanup()
RP1 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CB9a role_permissions 运行期一条没改（%d→%d，0363 的授予行已计入基线）"
   % (RP0, RP1), RP1 == RP0, "RP=%d" % RP1)
ok("CB9b 探针已墓碑，覆盖清零，cb 前缀无活号",
   psql("SELECT count(*) FROM users WHERE status < 2 AND username = '%s'"
        % NAME) == "0"
   and psql("SELECT count(*) FROM users WHERE username LIKE 'cbm%%' "
            "AND status < 2") == "0",
   "live=%s" % psql("SELECT count(*) FROM users WHERE username LIKE 'cbm%%' "
                   "AND status < 2"))

summary()
