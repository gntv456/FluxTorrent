# -*- coding: utf-8 -*-
"""闸门 BT：职员授权族（/admin/permission-matrix、/admin/user-permissions、
/admin/user-roles 一族 8 条路）。这族只在 legacy 脚本里出现过（admin_e2e.py、
e2e_p3_round8.py），常驻 pt_audit_* 里零命中——而它决定「谁能干什么」。

三个怀疑（红证以实测为准，读码推断不立 P）：
  - **P-206 半包授权**：`permission_matrix_update`（perms.rs:118 起）在 for 循环里
    逐条 autocommit，而 `role_type` 与「权限不存在」两条校验也写在循环内 ⇒
    第 N 条非法时前面 N-1 条**已经生效**。勾选一批是常态（给某职务配 8 项），
    打错一个权限名 = 前 7 项落库 + 整批 400 + 重试再叠一层；
    撤销方向同理（半摘 ⇒ 职员当场丢权）。
  - **P-207 junk 配置**：写侧校验了 `permissions.key` 存在，却**不校验 role_key**
    （class 侧可以是任意文本、role 侧可以是没有这个职务的键）⇒ 配置写得进、
    生效点找不到；而矩阵读侧 `SELECT DISTINCT role_key FROM role_permissions`
    会把这个假角色列成「可配角色」，界面上多出一条谁都不对应的行。
  - **P-208 审计不带对象**：`user.role_grant` / `user.role_revoke` 只 ref=user_id，
    `user.permission_set` 也只 ref=user_id ⇒「谁被授予/摘掉了**哪个职务**、
    加了**哪条权限**」读不出来。同文件的 `permission_matrix.update` 早就把
    items 快照进了 extra（注释里写着「三轮遗留」修过），这三处没跟上——
    与 P-202/P-204 同族（记了 ≠ 读得到）。

判据纪律：半包用「合法项在前、非法项在后」的两项批次打，并**逐条回读库里行数**
（不是只看返回码）；用户级三处需要一个 class 低于站长的活账号 ⇒ 自建探针成员
（邀请码 + 注册），收尾作废邀请并墓碑账号；所有写操作只动探针自己造的行，
基线行数（role_permissions / user_roles / user_permissions）收尾复读。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bt_perms.py
"""
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
TAG = int(time.time()) % 100000
RK = "btjunk%d" % TAG                  # 不存在的 role_key
PAIR = ("role", "liaison", "seed.stats.view")
BADKEY = "bt.no_such_permission_%d" % TAG
UID = [0]


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:50], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


def rp_cnt():
    return psql("SELECT count(*) FROM role_permissions")


def rp_pair():
    return psql("SELECT count(*) FROM role_permissions WHERE role_type='%s' "
                "AND role_key='%s' AND permission_key='%s'" % PAIR)


def audit_rows(action_prefix):
    d = (call("GET", "/admin/audit?q=%s&limit=60" % action_prefix, None,
              token=ROOT)[1] or {}).get("data") or []
    return [r for r in d if str(r.get("created_at") or "").replace(
        "T", " ")[:19] >= T0[:19]]


def cleanup():
    psql("DELETE FROM role_permissions WHERE role_key = '%s' OR "
         "(role_type='%s' AND role_key='%s' AND permission_key='%s')"
         % (RK, PAIR[0], PAIR[1], PAIR[2]))
    psql("DELETE FROM user_roles WHERE user_id = %d" % (UID[0] or 0))
    psql("DELETE FROM user_permissions WHERE user_id = %d" % (UID[0] or 0))
    if UID[0]:
        if psql("SELECT status::text FROM users WHERE id = %d" % UID[0]) != "2":
            call("POST", "/admin/users/status", {"user_id": UID[0],
                                                 "status": 2}, token=ROOT)
        call("DELETE", "/admin/users/%d" % UID[0], None, token=ROOT)


import atexit  # noqa: E402

atexit.register(cleanup)

# 现网存量：role_permissions 里指向不存在等级档/职务的 role_key（P-209 的实例，
# 数据修复待站长拍板，故 T2b 只拦「本轮新出现的」）
JUNK0 = sorted({x for x in psql(
    "SELECT DISTINCT role_key FROM role_permissions rp WHERE NOT EXISTS ("
    "SELECT 1 FROM user_classes uc WHERE uc.id::text = rp.role_key) "
    "AND NOT EXISTS (SELECT 1 FROM roles r WHERE r.key = rp.role_key)"
    "").split() if x})


def real_role(k):
    if psql("SELECT count(*) FROM roles WHERE key = '%s'" % k) != "0":
        return True
    return psql("SELECT count(*) FROM user_classes WHERE "
                "id::text = '%s'" % k) != "0"
                       "id::text = '%s'" % k) != "0"


RP0 = int(rp_cnt() or -1)
T0 = psql("SELECT to_char(now(), 'YYYY-MM-DD HH24:MI:SS.US')")
ok("T0a 基线：role_permissions=%s 行（收尾要回到这个数）" % RP0, RP0 >= 0,
   "RP=%s" % RP0)

s, b = call("GET", "/admin/permission-matrix", None, token=ROOT)
d = (b or {}).get("data") or {}
ok("T0b 前提：矩阵读口 200 且 permissions/roles/grants 三段齐",
   s == 200 and all(k in d for k in ("permissions", "roles", "grants")),
   "http=%s keys=%s" % (s, sorted(d.keys())))

# ===== T1 半包授权（P-206） =====
psql("DELETE FROM role_permissions WHERE role_type='%s' AND role_key='%s' "
     "AND permission_key='%s'" % PAIR)
c0 = int(rp_pair())
s1, b1 = call("PUT", "/admin/permission-matrix", {"items": [
    {"role_type": PAIR[0], "role_key": PAIR[1],
     "permission_key": PAIR[2], "granted": True},
    {"role_type": PAIR[0], "role_key": PAIR[1],
     "permission_key": BADKEY, "granted": True}]}, token=ROOT)
c1 = int(rp_pair())
ok("T1a 一批两条（第 1 条合法、第 2 条权限名不存在）必须整批拒绝且**一条都不落**"
   "——现在校验写在循环里、每条 autocommit（实测 http=%s msg=%s 探针行=%d→%d）"
   % (s1, str((b1 or {}).get("message", ""))[:28], c0, c1),
   s1 == 400 and c1 == c0, "探针行 %d→%d" % (c0, c1))

# 撤销方向的半包：先留一行，再打「撤销它 + 非法项」
psql("INSERT INTO role_permissions (role_type, role_key, permission_key, "
     "granted) VALUES ('%s','%s','%s', true) ON CONFLICT DO NOTHING" % PAIR)
before = int(rp_pair())
s2, b2 = call("PUT", "/admin/permission-matrix", {"items": [
    {"role_type": PAIR[0], "role_key": PAIR[1],
     "permission_key": PAIR[2], "granted": False},
    {"role_type": PAIR[0], "role_key": PAIR[1],
     "permission_key": BADKEY, "granted": False}]}, token=ROOT)
after = int(rp_pair())
ok("T1b 撤销方向同判据：整批 400 时**已摘掉的那条要回滚**（半摘=职员当场丢权）"
   "（实测 http=%s 行数 %d→%d）" % (s2, before, after),
   s2 == 400 and after == before, "行数 %d→%d" % (before, after))

# ===== T2 junk role_key（P-207） =====
s3, b3 = call("PUT", "/admin/permission-matrix", {"items": [
    {"role_type": "role", "role_key": RK,
     "permission_key": "seed.stats.view", "granted": True}]}, token=ROOT)
junk = int(psql("SELECT count(*) FROM role_permissions WHERE role_key = '%s'"
                % RK))
ok("T2a 不存在的 role_key 要当场拒（%s）：实测 http=%s msg=%s 库里 junk=%d 行"
   % ("现在只校验 permissions.key 不校验 role_key", s3,
      str((b3 or {}).get("message", ""))[:26], junk),
   s3 == 400 and junk == 0, "http=%s junk=%d" % (s3, junk))
_, bm = call("GET", "/admin/permission-matrix", None, token=ROOT)
dd = (bm or {}).get("data")
roles_list = dd.get("roles", []) if isinstance(dd, dict) else []
roles_seen = [r.get("role_key") for r in roles_list if isinstance(r, dict)]
bad_now = [k for k in roles_seen if not real_role(k) and k not in JUNK0]
ok("T2b 正控：矩阵读回的「可配角色」每条都要对得上真实对象"
   "（roles.key 或 user_classes.id）。已知存量 junk 键=%s 不计入本轮判据"
   "（P-209 待站长拍板），本轮新出现的对不上=%s" % (JUNK0, bad_now),
   len(roles_seen) > 0 and RK not in roles_seen and not bad_now,
   "角色数=%d 存量=%s 新增对不上=%s" % (len(roles_seen), JUNK0, bad_now))

# ===== T3 正控：一批全合法要真生效、并干净撤销 =====
psql("DELETE FROM role_permissions WHERE role_type='%s' AND role_key='%s' "
     "AND permission_key='%s'" % PAIR)
s4, _ = call("PUT", "/admin/permission-matrix", {"items": [
    {"role_type": PAIR[0], "role_key": PAIR[1],
     "permission_key": PAIR[2], "granted": True}]}, token=ROOT)
got = int(rp_pair())
s5, _ = call("PUT", "/admin/permission-matrix", {"items": [
    {"role_type": PAIR[0], "role_key": PAIR[1],
     "permission_key": PAIR[2], "granted": False}]}, token=ROOT)
gone = int(rp_pair())
ok("T3 正控：全合法的一批 granted=true 要落库（%d 行）、granted=false 要摘干净"
   "（%d 行），两次都 200" % (got, gone),
   s4 == 200 and got == 1 and s5 == 200 and gone == 0,
   "http=%s/%s 行=%d→%d" % (s4, s5, got, gone))
mx_rows = audit_rows("permission_matrix")
ok("T3b 矩阵审计带 items 快照（同文件既有口径，留作回归正控）",
   any("items" in str(r.get("detail")) for r in mx_rows),
   "本轮矩阵审计=%d 条" % len(mx_rows))

# ===== T4 用户级三处审计要带对象（P-208） =====
clear_register_rl()
codes = (call("POST", "/admin/invites", {"user_id": 1, "count": 1},
              token=ROOT)[1] or {}).get("data", {}).get("codes") or []
cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
NAME = "btm%d" % TAG
if codes and len(nums) >= 2:
    call("POST", "/auth/register", {
        "username": NAME, "email": "%s@bt.local" % NAME,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    UID[0] = int(psql("SELECT id FROM users WHERE username = '%s'" % NAME) or 0)
ok("T4a 探针成员就绪（uid=%s，class 低于站长）" % UID[0], UID[0] > 0,
   "uid=%s" % UID[0])
if UID[0]:
    s6, _ = call("POST", "/admin/user-roles", {"user_id": UID[0],
                                               "role_key": "seeder"},
                 token=ROOT)
    s7, _ = call("PUT", "/admin/user-permissions", {"user_id": UID[0],
                                                    "permission_key":
                                                    "seed.stats.view",
                                                    "granted": True},
                 token=ROOT)
    s8, _ = call("DELETE", "/admin/user-roles/%d/seeder" % UID[0], None,
                 token=ROOT)
    g = audit_rows("user.role_grant")
    rv = audit_rows("user.role_revoke")
    ps = audit_rows("user.permission_set")
    miss = []
    if g and not any("seeder" in str(r.get("detail")) for r in g):
        miss.append("role_grant")
    if rv and not any("seeder" in str(r.get("detail")) for r in rv):
        miss.append("role_revoke")
    if ps and not any("seed.stats.view" in str(r.get("detail")) for r in ps):
        miss.append("permission_set")
    ok("T4b 用户级三处审计都要带对象标识（role_key / permission_key）："
       "授予 http=%s、设覆盖 http=%s、撤销 http=%s；缺标识的=%s"
       % (s6, s7, s8, miss),
       s6 == 200 and s8 == 200 and bool(g) and bool(rv) and bool(ps)
       and not miss, "缺=%s 计数 g/rv/ps=%d/%d/%d"
       % (miss, len(g), len(rv), len(ps)))

# ===== T9 收尾 =====
cleanup()
RP1 = int(rp_cnt() or -1)
ok("T9a role_permissions 回到基线（%s→%s），junk 行 0" % (RP0, RP1),
   RP1 == RP0 and psql("SELECT count(*) FROM role_permissions WHERE "
                       "role_key = '%s'" % RK) == "0",
   "RP=%s" % RP1)
ok("T9b 探针成员已墓碑、其授权/覆盖行清零",
   psql("SELECT count(*) FROM users WHERE status < 2 AND username = '%s'"
        % NAME) == "0" and
   psql("SELECT count(*) FROM user_roles WHERE user_id = %d" % UID[0])
   == "0",
   "live=%s" % psql("SELECT count(*) FROM users WHERE status < 2 AND "
                    "username = '%s'" % NAME))
summary()
