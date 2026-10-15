# -*- coding: utf-8 -*-
"""CF 闸门（批六十六）：考核批量分配的审计记错对象、全站薪资明细只判粗闸。

两条现网（修前）实测：
  P-244 `POST /admin/jixiao/assign-batch`（**界面真正在调的那条**：
        `apps/web/components/admin-jixiao.tsx:91`）一次给多名用户登记某考核
        岗位，审计却写 `audit(Some(auth.id), "jixiao.assign_batch",
        Some(body.type_id))` —— 第三个参数是**审计对象**，于是账上落的是
        `ref={"id":岗位 id}`：**改了谁、改的哪一期、跳过了谁全查不到**。
        「谁在什么时候被登记了考核」是发薪纠纷时唯一要问的三件事。
  P-245 `GET /admin/jixiao/overview` 与 `GET /admin/jixiao/payroll` 只判
        `staff()`，返回**全员 username + amount + bonus + 结算时间**。
        `staff.panel` 挂在 class/90 且 class 键是阈值（`id>=role_key`）⇒
        91 发布员、92 版主都拿得到全站的钱袋子明细。

判据纪律：
  - 探针授权一律走 `user_permissions` 覆盖，真实档位表只读不改；
  - 批量登记用**历史期 1999-01**：worker 的 jixiao 结算只处理上一期
    （`apps/worker/src/jobs/jixiao.rs:10`）⇒ 探针行永远不会被结算成真的发钱，
    收尾按 user_id 删自己造的行；
  - 读口用**只覆盖 staff.panel 的第二支探针**打 ⇒ 403/200 只反映档位；
  - 每条收紧判据都配一层正控（CF4：授予新键后必须 200 且读得到聚合），
    否则「整口封死」会被读成绿；
  - CF6 把批六十五的栅栏口径 B（只认 `require_perm`）**扩到 admin_p2_http /
    admin_p3_http / staff_http** —— 本批两条洞正是 CE9 只扫 admin_http 时
    结构性漏检的那一类。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_cf_exam_audit_tiers.py
"""
import atexit  # noqa: E402
import io
import json
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
ACT = "cfa%d" % TAG       # 只拿 staff.panel 的探针（打读口）
TGT = "cft%d" % TAG       # 被登记考核的成员
HIST = "1999-01"
AID = [0]
TID_U = [0]


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
    """去掉 `//` 行注释。批六十五踩过：补丁注释里写着旧形状的字面量，
    静态判据整文件 count 会把**说明文字**当成残留（假红）。同理，栅栏若
    不剥注释，注释里提一句 `require_perm` 就能把真洞洗白。"""
    return chr(10).join(ln.split("//")[0] for ln in t.splitlines())


def code(f):
    return strip_comments(src(f))


def jc_count():
    return int(psql("SELECT count(*) FROM jixiao_claims") or -1)


def cleanup():
    if TID_U[0]:
        psql("DELETE FROM jixiao_claims WHERE user_id = %d" % TID_U[0])
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
JC0 = jc_count()
EXAM_TYPE = psql("SELECT id FROM jixiao_types WHERE base_pay > 0 "
                 "ORDER BY id LIMIT 1")
ok("CF0 前提：基线已取（jixiao_claims %d 行、授权表 %d 行），考核岗位目录有"
   "可用岗位 id=%s" % (JC0, RP0, EXAM_TYPE or "-"),
   JC0 >= 0 and RP0 > 0 and EXAM_TYPE not in ("", "None"),
   "JC=%d RP=%d 岗位=%s" % (JC0, RP0, EXAM_TYPE))

# ===== 两支探针 =====
clear_register_rl()
codes = (call("POST", "/admin/invites", {"user_id": 1, "count": 2},
              token=ROOT)[1] or {}).get("data") or {}.get("codes") or []


def reg(name, idx):
    """验证码是一次性的 ⇒ 每个探针各取一张题（批六十五踩过的那条）。"""
    cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(codes) <= idx or len(nums) < 2:
        return 0
    call("POST", "/auth/register", {
        "username": name, "email": "%s@cf.local" % name,
        "password": "password123", "invite_code": codes[idx]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    return int(psql("SELECT id FROM users WHERE username = '%s'" % name) or 0)


AID[0] = reg(ACT, 0)
TID_U[0] = reg(TGT, 1)
psql("UPDATE users SET class_id = 12 WHERE id = %d" % (AID[0] or 0))
AT = login(ACT, "password123") if AID[0] else None


def grant(key, who=None, val=True):
    return call("PUT", "/admin/user-permissions",
                {"user_id": who or AID[0], "permission_key": key,
                 "granted": val}, token=ROOT)[0]


grant("staff.panel")
ok("CF1 两支探针就绪：读口探针 uid=%s（class 12 + 只覆盖 staff.panel，低于"
   "90 档）、被登记成员 uid=%s" % (AID[0], TID_U[0]),
   bool(AT) and AID[0] > 0 and TID_U[0] > 0,
   "actor=%s target=%s" % (AID[0], TID_U[0]))

# ===== CF2 P-245 薪资读口只判粗闸 =====
s_ov, b_ov = call("GET", "/admin/jixiao/overview?period=%s" % HIST, None,
                  token=AT)
s_pr, b_pr = call("GET", "/admin/jixiao/payroll?period=%s" % HIST, None,
                  token=AT)


def leak_shape(body):
    """从响应里取「成员明细/发薪行」第一条，证明泄漏的是**人+钱**而不只是计数。"""
    d = (body or {}).get("data") or {}
    for k in ("members", "list"):
        rows = d.get(k) or []
        if rows and isinstance(rows[0], dict):
            r0 = rows[0]
            return "%s=%s amount=%s" % (
                "username" if "username" in r0 else "?",
                r0.get("username"), r0.get("amount"))
    if d.get("types"):
        return "types=%d（聚合计数）" % len(d["types"])
    return "-"


LV, LP = leak_shape(b_ov), leak_shape(b_pr)
ok("CF2 P-245：只有 staff.panel 的低档探针读 `/admin/jixiao/overview` 与 "
   "`/admin/jixiao/payroll`（**全员 username + amount**）——修后必须 403；"
   "现网读数：overview=%s 内容=%s；payroll=%s 内容=%s"
   % (s_ov, LV, s_pr, LP),
   s_ov == 403 and s_pr == 403,
   "ov=%s pr=%s 内容=%s / %s" % (s_ov, s_pr, LV, LP))

# ===== CF3 P-244 批量登记的审计对象记成岗位 =====
grant("exam.manage")
s_ab, b_ab = call("POST", "/admin/jixiao/assign-batch",
                  {"type_id": int(EXAM_TYPE), "user_ids": [TID_U[0]],
                   "period": HIST}, token=AT)
JC1 = jc_count()
A_REF = psql("SELECT coalesce((SELECT ref::text FROM audit_log "
             "WHERE action = 'jixiao.assign_batch' ORDER BY id DESC "
             "LIMIT 1), '-')")
ok("CF3 P-244 前提：批量登记本身要走通（正控，否则 CF4 的「审计没痕迹」会被"
   "读成修好）：http=%s 响应=%s，jixiao_claims %d→%d"
   % (s_ab, str(b_ab)[:64], JC0, JC1),
   s_ab == 200 and JC1 == JC0 + 1, "ab=%s JC=%d" % (s_ab, JC1))
ok("CF4 P-244：`jixiao.assign_batch` 的审计必须能回答「改了谁、哪一期」——"
   "`ref` 现在记的是**岗位 id**（audit 的第三参数是审计对象），既没有成员也"
   "没有期次；修后要求同一条审计里能查到被登记的成员与 period。现网 ref=%s"
   % A_REF[:110],
   A_REF != "-" and str(TID_U[0]) in A_REF and HIST in A_REF,
   "ref=%s" % A_REF[:120])

# ===== CF4b 泄漏面取证（登记过成员之后再读，才看得到「人 + 钱」的形状）=====
grant("exam.manage", val=False)   # 用户级覆盖可以无损收回
s_ov3, b_ov3 = call("GET", "/admin/jixiao/overview?period=%s" % HIST, None,
                    token=AT)
LV3 = leak_shape(b_ov3)
ok("CF4b P-245：库里有了登记行之后再读 overview，泄漏的是**具体成员的姓名与"
   "金额**（不只是聚合计数）——修后必须 403；现网 http=%s 内容=%s"
   % (s_ov3, LV3), s_ov3 == 403, "ov=%s 内容=%s" % (s_ov3, LV3))

# ===== CF5 判据形状：写口不许再拿 type_id 当审计对象 =====
# 引用面要**两个文件一起数**：本批把读口拆进了 jixiao_reads.rs，
# 判据若硬编码写口的文件名，拆完就找不到 `perm::` 了（第一跑的两条
# 「引用=0/1 处」就是这么来的假红）。
JO = code("admin_p3_http/jixiao_ops.rs")
JORS = JO + code("admin_p3_http/jixiao_reads.rs")
ok("CF5 P-244：`jixiao_ops.rs` 的 `audit(...)` 必须换成 `audit_detail` 且"
   " ref 带 `{period, user_ids/assigned, skipped_dup}`（一次多名，`ref` 的"
   "单 id 装不下）——现网仍在用 `audit(` 记 type_id=%s"
   % ('"jixiao.assign_batch"' in JO and
      'audit(Some(auth.id), "jixiao.assign_batch"' in JO),
   'audit(Some(auth.id), "jixiao.assign_batch"' not in JO
   and "jixiao.assign_batch" in JO,
   "仍用旧形状=%s" % ('audit(Some(auth.id), "jixiao.assign_batch"' in JO))

# ===== CF6 绿证：授权后两口回业务层（不是被一刀切封死）=====
grant("exam.manage")
grant("jixiao.payroll.view")
s_ov2, b_ov2 = call("GET", "/admin/jixiao/overview?period=%s" % HIST, None,
                    token=AT)
s_pr2, _ = call("GET", "/admin/jixiao/payroll?period=%s" % HIST, None,
                token=AT)
D2 = (b_ov2 or {}).get("data") or {}
ok("CF6 授权后两口回业务层：overview 200 且 `types/members` 键在、payroll "
   "200：%s/%s（types=%d）" % (s_ov2, s_pr2, len(D2.get("types") or [])),
   s_ov2 == 200 and "types" in json.dumps(D2)[:4000] and s_pr2 == 200,
   "ov=%s pr=%s" % (s_ov2, s_pr2))

# ===== CF7 新键三件齐 + overview 复用已有键 =====
KEY, CONST, FLOOR = "jixiao.payroll.view", "JIXIAO_PAYROLL_VIEW", 95
has = psql("SELECT count(*) FROM permissions WHERE key = '%s'" % KEY)
fl = psql("SELECT min(role_key::integer) FROM role_permissions WHERE "
          "role_type='class' AND granted AND permission_key='%s' "
          "AND role_key ~ '^[0-9]+$'" % KEY)
impl = psql("SELECT implemented::int FROM permissions WHERE key = '%s'" % KEY)
ok("CF7a %s：目录=%s implemented=%s 常量=%s 引用=%d 处 最低档=%s（要 >=%d）"
   % (KEY, has == "1", impl, CONST in code("authz/perm.rs"),
      JORS.count("perm::%s" % CONST), fl or "-", FLOOR),
   has == "1" and impl == "1" and CONST in code("authz/perm.rs")
   and JORS.count("perm::%s" % CONST) >= 1
   and fl not in ("", "None") and int(fl) >= FLOOR,
   "目录=%s impl=%s 档=%s" % (has, impl, fl))
ok("CF7b overview 不新设键、复用 `exam.manage`（目录里已有且授 class/93）："
   "引用=%d 处（批量分配 + 总览）最低档=%s"
   % (JORS.count("perm::EXAM_MANAGE"),
      psql("SELECT min(role_key::integer) FROM role_permissions WHERE "
           "role_type='class' AND granted AND permission_key='exam.manage' "
           "AND role_key ~ '^[0-9]+$'") or "-"),
   JORS.count("perm::EXAM_MANAGE") >= 2,
   "引用=%d 处" % JORS.count("perm::EXAM_MANAGE"))


# ===== CF8 栅栏口径 B 扩面（本批只取证，不动别人域）=====
# 入口助手在四个域里不叫同一个名字：admin_* 用 `staff()`，staff_http 用
# `require_auth()`。第一版只认 `staff(` ⇒ staff_http 报 0 条是**假干净**
# （探测器压根没进门），这条账必须写进判据。收口标志：`require_perm` 或
# 域内自建的 `require_*_(manage|view|handle|grant|assign)` 助手。
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
        t = strip_comments(
            io.open(os.path.join(base, fn), encoding="utf-8").read())
        for b in t.split(chr(10) + "#["):
            m = re.match(r'(get|post|put|delete)\("([^"]+)"\)', b)
            if not m or not ENTRY.search(b):
                continue
            if GUARD.search(b):
                continue
            out.append(m.group(2))
    return out


ALLC = {}
for d in ("admin_http", "admin_p2_http", "admin_p3_http", "staff_http"):
    ALLC[d] = coarse(d)
tot = sum(len(v) for v in ALLC.values())
ok("CF8 栅栏口径 B（只认 require_perm）扩到四个职员域：admin_http=%d "
   "admin_p2_http=%d admin_p3_http=%d staff_http=%d 合计=%d（本批收 2 条 ⇒ "
   "admin_p3_http 的 jixiao 两口应已消失）"
   % (len(ALLC["admin_http"]), len(ALLC["admin_p2_http"]),
      len(ALLC["admin_p3_http"]), len(ALLC["staff_http"]), tot),
   "/admin/jixiao/overview" not in ALLC["admin_p3_http"]
   and "/admin/jixiao/payroll" not in ALLC["admin_p3_http"],
   "p3 剩余=%s" % sorted(ALLC["admin_p3_http"]))

# ===== CF9 收尾 =====
cleanup()
RP1 = int(psql("SELECT count(*) FROM role_permissions") or -1)
ok("CF9 真实授权表一条都没改（%d→%d）；jixiao_claims 回 %d→%d；探针成员清零；"
   "库里不留 period=%s 的行"
   % (RP0, RP1, JC0, jc_count(), HIST),
   RP1 == RP0 and jc_count() == JC0
   and psql("SELECT count(*) FROM jixiao_claims WHERE period = '%s'" % HIST)
   == "0"
   and psql("SELECT count(*) FROM users WHERE (username LIKE 'cfa%%' OR "
            "username LIKE 'cft%%') AND status < 2") == "0",
   "JC=%s RP=%s" % (jc_count(), RP1))

summary()
