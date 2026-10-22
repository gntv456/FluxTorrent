# -*- coding: utf-8 -*-
"""CE 闸门（批六十五）：给成员登记考核金额的口只判粗闸、金额无界、审计无痕。

现网（修前）四条写口都只判 `staff()`，而 `staff.panel` 挂在 class/90、
class 键是「id >= role_key」的阈值语义 ⇒ 91 发布员、92 论坛版主全都够用：
  POST /admin/users/{id}/jixiao    —— **直接动钱**：`amount` 由调用方任意传
       （`body.amount.unwrap_or(0)`，无上限、不拒负数），落进 jixiao_claims
       且 `settled_at IS NULL 即未发放`，jixiao_settle 会把它发出去
  POST /admin/reports/claim / release / resolve —— 认领、放号、结案/驳回举报
       （resolve 还会给举报人发私信）

另有两处 fail-open（批五十九/六十同族）：`SELECT EXISTS(... jixiao_types)`
与「本期已登记」两条判断都写 `.unwrap_or(false)` —— 后者查询失败时**当没登记**
放行插入 ⇒ 同一人同期同岗位可被登记两次，即重复发钱。
查询失败我造不出来，所以这条落静态判据，红证取修前命中数。

判据纪律：
  - 探针一律 `user_permissions` 覆盖 `staff.panel`，不碰真实档位授权；
  - 动钱的口用**不存在的 type_id** 打 ⇒ 业务 404 与权限 403 可区分，
    且绝不可能落库；
  - 金额界分两层验：低档职员（只有 staff.panel）打到口上必须先被**权限层**
    挡住（CE3，403 且零落库），授予 jixiao.assign 后才轮到**业务层**判
    负数/超上界（CE6a，两条 400 且零落库）——只在有权限时验业务界，
    否则「403 挡住一切」会把没实现的金额界读成绿（CE6b 是同口正控：
    合法金额必须真落一行）；
  - 探针用历史期 `1999-01`：worker 的 jixiao 结算只处理**上一期**（跨月才结，
    jixiao.rs:10），历史期永不结算 ⇒ 正控那行不会变成真发钱，收尾按
    (user_id) 删自己造的行；
  - 举报口用真实 torrent id 建探针工单，举报人=探针本人（结案回信因此不落在
    root 收件箱），reason 带前缀，收尾按前缀删工单 + 删探针收件箱 +
    删本批新增的 spark_ledger 行（按 id 水位）；
  - 绿证：CE7 要求审计**必须有行**且 ref 里带 amount/period/type_id、action
    （旧写法 `audit(..., None)` 的 ref 只有 `{"id":N}`，钱数查不出来）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ce_jixiao_money_writes.py
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
NAME = "cem%d" % TAG
NOTE = "CE探针工单%d" % TAG
HIST = "1999-01"
UID = [0]
VID = [0]
RID = [0]


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
    """去 `//` 注释后的源码。本批的补丁注释里就写着 `.unwrap_or(false)`
    （说明旧写法），整文件 count 会把说明文字当成残留——第一次跑 CE5 就是
    这么假红的（同批 BX 的门注释教训）。本文件无含 `//` 的字符串字面量。"""
    return "\n".join(ln.split("//")[0] for ln in src(f).splitlines())


def audit_refs(action):
    return psql("SELECT coalesce(string_agg(x, ' ~ '), '-') FROM "
                "(SELECT coalesce(ref::text, '[NULL]') AS x FROM audit_log "
                "WHERE action = '%s' AND id > %d ORDER BY id) t"
                % (action, AUD0))


def cleanup():
    for who in (UID[0], VID[0]):
        if who:
            psql("DELETE FROM jixiao_claims WHERE user_id = %d" % who)
        psql("DELETE FROM user_subtitle_certs WHERE user_id = %d" % UID[0])
        psql("DELETE FROM user_permissions WHERE user_id = %d" % UID[0])
        psql("DELETE FROM user_roles WHERE user_id = %d" % UID[0])
    psql("DELETE FROM reports WHERE reason = '%s'" % NOTE)
    # 探针自己的私信（结案回信落在探针名下）与本批新产生的流水
    if UID[0]:
        psql("DELETE FROM messages WHERE receiver_id = %d" % UID[0])
    psql("DELETE FROM spark_ledger WHERE id > %d AND user_id IN (%d, %d)"
         % (SL0, UID[0] or 0, VID[0] or 0))
    for who in (UID[0], VID[0]):
        if who and psql("SELECT status::text FROM users WHERE id = %d"
                        % who) != "2":
            call("POST", "/admin/users/status", {"user_id": who, "status": 2},
                 token=ROOT)
    for who in (UID[0], VID[0]):
        if who:
            call("DELETE", "/admin/users/%d" % who, None, token=ROOT)


atexit.register(cleanup)

RP0 = int(psql("SELECT count(*) FROM role_permissions") or -1)
AUD0 = int(psql("SELECT coalesce(max(id), 0) FROM audit_log") or -1)
JC0 = int(psql("SELECT count(*) FROM jixiao_claims") or -1)
SL0 = int(psql("SELECT coalesce(max(id), 0) FROM spark_ledger") or -1)
TID = int(psql("SELECT id FROM jixiao_types WHERE base_pay > 0 "
               "ORDER BY id LIMIT 1") or 0)
BASE = int(psql("SELECT base_pay FROM jixiao_types WHERE id = %d" % TID) or 0)
ok("CE0 前提：基线已取（jixiao_claims %d 行、审计水位 %d、授权表 %d 行），"
   "且考核岗位目录里有正控可用的真实岗位（id=%s 基准=%s）"
   % (JC0, AUD0, RP0, TID or "-", BASE or "-"),
   JC0 >= 0 and RP0 > 0 and TID > 0 and BASE > 0,
   "JC=%d RP=%d TID=%s BASE=%s" % (JC0, RP0, TID, BASE))

# ===== 探针职员 =====
clear_register_rl()
codes = dig(call("POST", "/admin/invites", {"user_id": 1, "count": 2},
              token=ROOT)[1], "data", "codes") or []
cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
if codes and len(nums) >= 2:
    call("POST", "/auth/register", {
        "username": NAME, "email": "%s@ce.local" % NAME,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    UID[0] = int(psql("SELECT id FROM users WHERE username = '%s'" % NAME) or 0)
    # 第二个探针：被登记考核的对象。验证码是一次性的，第二个人注册要重新取
    # 一张题；等级也要真的存在（users.class_id 有外键，class_id=80 会撞 FK，
    # 第一次跑就是这么翻车的）——用 12 档，既 >1 又 <90。
    cap2 = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
    nums2 = re.findall(r"-?\d+", str(cap2.get("question") or ""))
    if len(codes) > 1 and len(nums2) >= 2:
        call("POST", "/auth/register", {
            "username": "cev%d" % TAG, "email": "cev%d@ce.local" % TAG,
            "password": "password123", "invite_code": codes[1]["code"],
            "captcha_id": cap2.get("captcha_id"),
            "captcha_answer": int(nums2[0]) + int(nums2[1]),
            "captcha_token": "", "fields": {}}, token=ROOT)
PT = login(NAME, "password123") if UID[0] else None


def grant_key(key, val=True):
    return call("PUT", "/admin/user-permissions",
                {"user_id": UID[0], "permission_key": key, "granted": val},
                token=ROOT)[0]


psql("UPDATE users SET class_id = 12 WHERE id = %d" % (UID[0] or 0))
VID[0] = int(psql("SELECT id FROM users WHERE username = 'cev%d'" % TAG) or 0)
grant_key("staff.panel")
ok("CE1a 探针职员侧等级已抬高（class_id=%s，仍低于 90 档）"
   % psql("SELECT class_id FROM users WHERE id = %d" % (UID[0] or 0)),
   psql("SELECT class_id FROM users WHERE id = %d" % (UID[0] or 0)) == "12",
   "class=%s" % psql("SELECT class_id FROM users WHERE id = %d"
                     % (UID[0] or 0)))
PT = login(NAME, "password123") if UID[0] else None
ok("CE1 两个探针就绪：职员侧 uid=%s（class 12 + 覆盖 staff.panel，**低于 90 档**）、"
   "对象侧 uid=%s（默认低档）" % (UID[0], VID[0]),
   bool(PT) and VID[0] > 0, "actor=%s target=%s" % (UID[0], VID[0]))

# ===== CE2 动钱的口：只有 staff.panel 就能登记任意金额 =====
JX = "/admin/users/%d/jixiao" % (VID[0] or 0)
s_jx, b_jx = call("POST", JX, {"type_id": 999999, "amount": 5,
                               "period": HIST}, token=PT)
ok("CE2 P-240：不存在的考核岗位 ⇒ 权限必须在业务之前挡住（修前是 404=先进了门，"
   "修后应是 403）：http=%s body=%s" % (s_jx, str(b_jx)[:52]),
   s_jx == 403, str(s_jx))
s_neg, b_neg = call("POST", JX, {"type_id": TID, "amount": -1,
                                 "period": HIST}, token=PT)
JC1 = int(psql("SELECT count(*) FROM jixiao_claims") or -1)
# 修前：探针只有 staff() 直接进业务层 → 负数落库（200）。修后：新增 require_perm
# 在业务之前挡住 → 403；真正的金额上界（400）在 CE6 授予 jixiao.assign 后验。
# 本条断言「挡住且一行不落」——若既没权限闸又没金额界，s_neg=200 且 JC1 增，必红。
ok("CE3 P-240：负数金额请求（低档职员只有 staff.panel）必须被挡住且一行不落"
   "（修前=200 真落库；修后新权限闸先挡 → 403）。探针用历史期 %s：http=%s "
   "行数 %d→%d" % (HIST, s_neg, JC0, JC1),
   s_neg in (400, 403) and JC1 == JC0,
   "%s / JC=%d" % (str(b_neg)[:56], JC1))

# ===== CE4 举报三口 =====
TORRENT = psql("SELECT min(id) FROM torrents")
psql("INSERT INTO reports (reporter_id, ref_type, ref_id, reason, status) "
     "VALUES (%d, 'torrent', %s, '%s', 0)" % (UID[0] or 0, TORRENT, NOTE))
RID[0] = int(psql("SELECT id FROM reports WHERE reason = '%s'" % NOTE) or 0)
s_cl, b_cl = call("POST", "/admin/reports/claim", {"report_id": RID[0]},
                 token=PT)
CB = psql("SELECT coalesce(claimed_by::text, '-') FROM reports WHERE id = %d"
          % (RID[0] or 0))
s_rs, b_rs = call("POST", "/admin/reports/resolve",
                  {"report_id": RID[0], "action": "act",
                   "note": "CE探针结案"}, token=PT)
ST = psql("SELECT status::text FROM reports WHERE id = %d" % (RID[0] or 0))
ok("CE4 P-243：认领/结案必须被档位挡住（修前探针能把工单认走并结案，"
   "还给举报人发了私信）：claim=%s claimed_by=%s resolve=%s status=%s"
   % (s_cl, CB, s_rs, ST),
   s_cl == 403 and s_rs == 403 and CB == "-" and ST == "0",
   "claim=%s resolve=%s status=%s" % (s_cl, s_rs, ST))

# ===== CE5 fail-open 静态（两处 unwrap_or(false)）=====
UG = code("admin_http/user_grant.rs")
ok("CE5 P-242：`SELECT EXISTS(jixiao_types)` 与「本期已登记」两条判断都不许"
   " `.unwrap_or(false)`（后者失败=当没登记 ⇒ 同人同期同岗可登记两次="
   "重复发钱；修前命中 2 处，本条按**去注释后**的形状扫）——残留=%d 处"
   % UG.count(".unwrap_or(false)"), UG.count(".unwrap_or(false)") == 0,
   "残留=%d" % UG.count(".unwrap_or(false)"))

# ===== CE6 绿证：授予新键后打到业务层（金额界不是被权限闸遮住的）=====
grant_key("jixiao.assign")
grant_key("reports.handle")
CAP = BASE * 3
s_jx2, _ = call("POST", JX, {"type_id": 999999, "amount": 5,
                             "period": HIST}, token=PT)
s_neg2, _ = call("POST", JX, {"type_id": TID, "amount": -1,
                              "period": HIST}, token=PT)
s_big2, _ = call("POST", JX, {"type_id": TID, "amount": CAP + 1,
                              "period": HIST}, token=PT)
JC2 = int(psql("SELECT count(*) FROM jixiao_claims") or -1)
ok("CE6a P-240：授予 jixiao.assign 后打到业务层——岗位不存在=404（说明权限确实"
   "放行，负数/超限的 400 不是被粗闸挡的假绿）、负数=400、超上界（基准 %d 的"
   " 3 倍=%d 再加 1）=400，三条都不落库：%s/%s/%s 行数 %d→%d"
   % (BASE, CAP, s_jx2, s_neg2, s_big2, JC1, JC2),
   s_jx2 == 404 and s_neg2 == 400 and s_big2 == 400 and JC2 == JC1,
   "%s/%s/%s JC=%d" % (s_jx2, s_neg2, s_big2, JC2))
s_ok2, _ = call("POST", JX, {"type_id": TID, "amount": 1,
                              "period": HIST}, token=PT)
JC3 = int(psql("SELECT count(*) FROM jixiao_claims") or -1)
ok("CE6b 正控：同岗位合法金额（1）必须真登记成功并落一行（否则 CE6a 的三个码"
   "可能只是整口被封）：http=%s 行数 %d→%d" % (s_ok2, JC2, JC3),
   s_ok2 == 200 and JC3 == JC2 + 1, "%s JC=%d" % (s_ok2, JC3))
s_cl2, _ = call("POST", "/admin/reports/claim", {"report_id": RID[0]},
                token=PT)
s_rs3, _ = call("POST", "/admin/reports/resolve",
                {"report_id": RID[0], "action": "dismiss",
                 "note": "CE探针结案"}, token=PT)
ok("CE6c 授权 reports.handle 后认领/结案走通（业务码，不是 403）：%s / %s"
   % (s_cl2, s_rs3), s_cl2 == 200 and s_rs3 == 200,
   "claim=%s resolve=%s" % (s_cl2, s_rs3))

# ===== CE7 审计要留痕（钱与裁决都要，且必须真有行可查）=====
a_jx, a_rs2 = audit_refs("user.assign_jixiao"), audit_refs("report.resolve")
ok("CE7 P-241：`user.assign_jixiao` 的审计要带 {type_id, period, amount}"
   "（否则「给他登记了多少钱」查不到），`report.resolve` 要带 action——"
   "修后读数：jx=%s resolve=%s" % (a_jx[:70], a_rs2[:56]),
   "amount" in a_jx and "period" in a_jx and "type_id" in a_jx
   and "action" in a_rs2,
   "jx=%s rs=%s" % (a_jx[:52], a_rs2[:40]))

# ===== CE8 三件齐 + 档位 =====
for key, floor, files in (("jixiao.assign", 95, ["admin_http/user_grant.rs"]),
                          ("reports.handle", 92, ["admin_http/reports.rs",
                                                  "admin_http/"
                                                  "reports_resolve.rs"])):
    const = key.upper().replace(".", "_")
    has = psql("SELECT count(*) FROM permissions WHERE key = '%s'" % key)
    refs = sum(src(f).count("perm::%s" % const) for f in files)
    fl = psql("SELECT min(role_key::integer) FROM role_permissions WHERE "
              "role_type='class' AND granted AND permission_key='%s' "
              "AND role_key ~ '^[0-9]+$'" % key)
    ok("CE8 %s：目录=%s 常量=%s 引用=%d 处 最低档=%s（要 >=%d）"
       % (key, has == "1", const in src("authz/perm.rs"), refs, fl or "-",
          floor),
       has == "1" and const in src("authz/perm.rs") and refs >= 1
       and fl not in ("", "None") and int(fl) >= floor,
       "目录=%s 引用=%d 档=%s" % (has, refs, fl))


# ===== CE9 栅栏：整块无 require_perm 的后台口（本批收 4 条）=====
def coarse(dirname, outrank=True):
    """两种口径：outrank=True 把 `ensure_outranks` 也算护栏（口径 A），
    False 只认 `require_perm`（口径 B）。**本批 P-240 恰是口径 A 的盲区**：
    jixiao 口一直有 ensure_outranks，所以它从未出现在 A 清单里，却能被
    低档职员拿去给任意成员记任意金额 ⇒ 收口必须按 B 数。"""
    out = []
    guard = r"require_(any_)?perm|ensure_outranks" if outrank else \
        r"require_(any_)?perm"
    base = os.path.join(REPO, "apps", "api", "src", dirname)
    sep = chr(10) + "#["
    for fn in sorted(os.listdir(base)):
        if not fn.endswith(".rs"):
            continue
        t = io.open(os.path.join(base, fn), encoding="utf-8").read()
        for b in t.split(sep):
            m = re.match(r'(get|post|put|delete)\("([^"]+)"\)', b)
            if not m or "staff(" not in b:
                continue
            if re.search(guard, b):
                continue
            out.append(m.group(2))
    return out


A, B = coarse("admin_http"), coarse("admin_http", False)
ok("CE9 栅栏（admin_http 整块判定）：口径 A（ensure_outranks 算护栏）修前 17→"
   "本批 %d，口径 B（只认 require_perm）修前 19→本批 %d（本批收 4 条）"
   % (len(A), len(B)), len(A) <= 14 and len(B) <= 15,
   "A=%d B=%d 剩余=%s" % (len(A), len(B), sorted(B)))
FIXED = ["/admin/users/{id}/jixiao", "/admin/reports/claim",
         "/admin/reports/release", "/admin/reports/resolve"]
gap = [r for r in FIXED if r in B or r in A]
ok("CE9b 本批四条口必须从**两个口径**的清单里同时消失（漏接一条 require_perm "
   "就还在册）：仍在册=%s；口径 B 当前剩余 %d 条" % (gap or "无", len(B)),
   not gap, "仍在册=%s" % gap)

# ===== CE10 收尾 =====
cleanup()
RP1 = int(psql("SELECT count(*) FROM role_permissions") or -1)
SL1 = int(psql("SELECT count(*) FROM spark_ledger WHERE id > %d AND "
               "user_id IN (%d, %d)" % (SL0, UID[0] or 0, VID[0] or 0)) or -1)
ok("CE10a 真实授权表一条都没改（%d→%d）；jixiao_claims 回 %d→%d；"
   "探针工单/成员/收件箱清零；本批新增流水 %d 条（应 0）"
   % (RP0, RP1, JC0, int(psql("SELECT count(*) FROM jixiao_claims") or -1),
      SL1),
   RP1 == RP0
   and int(psql("SELECT count(*) FROM jixiao_claims") or -1) == JC0
   and SL1 == 0
   and psql("SELECT count(*) FROM messages WHERE receiver_id = %d"
            % (UID[0] or 0)) == "0"
   and psql("SELECT count(*) FROM reports WHERE reason LIKE 'CE%%'") == "0"
   and psql("SELECT count(*) FROM users WHERE (username LIKE 'cem%%' OR "
                   "username LIKE 'cev%%') AND status < 2") == "0",
   "JC=%s RP=%s SL=%s" % (psql("SELECT count(*) FROM jixiao_claims"), RP1,
                          SL1))

print("\n== CE11 绩效「当月发种数」三出口一律只数过审种（P-528）==")
# API 的 compute_metrics（会员屏 + /admin/jixiao/assign-batch 同口径）、
# worker 的 jixiao_loop（结算现值）与 jixiao_snap（期初基线）三处都在数
# "这个月发了多少种"。少一处 `approval_status = 1`，被拒/谁都不可见的种
# 就照样计薪。现网 2026-10：全站 210 条 vs 过审 177 条；user 52 本月 9 条
# 全部来自没过审的种（真实过审 0），却能按"发种 9 条"拿绩效。
JX_FILES = ["apps/api/src/ops_http/jixiao_compute.rs",
            "apps/worker/src/jobs/jixiao_loop.rs",
            "apps/worker/src/jobs/jixiao_snap.rs"]


def uploads_stmts(rel):
    raw = io.open(os.path.join(REPO, *rel.split("/")), encoding="utf-8",
                  errors="replace").read()
    # 剥 // 注释：本批修复的说明注释里就写着 approval_status = 1
    raw = "\n".join(re.sub(r"//.*", "", ln) for ln in raw.split("\n"))
    return [raw[m.end():m.end() + 240]
            for m in re.finditer(r"count\(\*\)\s+FROM\s+torrents", raw, re.I)]


stmts = [(os.path.basename(r), s) for r in JX_FILES
         for s in uploads_stmts(r)]
miss = [f for f, s in stmts if "approval_status = 1" not in s]
ok("CE11 三出口的 uploads 语句都带 approval_status = 1（违例 %d/%d）"
   % (len(miss), len(stmts)),
   len(stmts) == 3 and not miss,
   "取到 %d 条语句；缺过滤=%s" % (len(stmts), "、".join(miss) or "-"))
ok("CE11b 正控：判据认得出「漏过滤」与「多算别的表」两种形状",
   len(stmts) == 3
   and "approval_status = 1" not in
   "WHERE owner_id = $1 AND to_char(created_at, 'YYYY-MM') = $2"
   and not re.findall(r"count\(\*\)\s+FROM\s+torrents",
                      "SELECT count(*) FROM users WHERE 1=1"),
   "stmts=%d" % len(stmts))
# CE11c：把**本批改过的那条**语句按 Rust 行续行规则折叠后真跑一遍。
# 只查"键在不在"抓不到补丁自己写坏的 SQL——本批还原负控时就写出过
# `AND AND` 与丢掉的 `to_char`，cargo 照绿、CE11 照 PASS。
# worker 那两条在 INSERT ... SELECT 里（不能真跑，会写库），改做形状体检：
# `AND AND`、`= 1 AND (` 这类断裂的函数调用、以及 `count(*)` 后缺 FROM。
fold = lambda s: re.sub(r"\\\s*\n\s*", " ", s)
api_raw = io.open(os.path.join(REPO, *JX_FILES[0].split("/")),
                  encoding="utf-8", errors="replace").read()
m11 = re.search(r'"(SELECT count\(\*\)\s+FROM torrents[^"]*?)"',
                api_raw, re.S)
run_res = ("没有语句", False)
if m11:
    q = fold(m11.group(1)).replace("$1", "1").replace("$2", "'1999-01'")
    try:
        v = psql(q).strip()
        run_res = ("回值=%s" % v[:12], v.isdigit())
    except Exception as e:  # noqa: BLE001：这里就是要接住 SQL 报错
        run_res = ("ERR %s" % str(e)[:40], False)
SMELLS = (r"AND\s+AND", r"=\s*1\s+AND\s+\(", r"count\(\*\)\s+WHERE")
smell_hit = []
for rel in JX_FILES:
    raw = io.open(os.path.join(REPO, *rel.split("/")), encoding="utf-8",
                  errors="replace").read()
    raw = "\n".join(re.sub(r"//.*", "", ln) for ln in raw.split("\n"))
    for m in re.finditer(r"SELECT count\(\*\)\s+FROM torrents[^\"]*",
                         raw, re.S):
        frag = fold(m.group(0))[:260]
        for pat in SMELLS:
            if re.search(pat, frag):
                smell_hit.append("%s:/%s/" % (os.path.basename(rel), pat))
ok("CE11c 改过的那条在真库跑得通、三处形状都没坏（跑测 %s，形状违例 %d）"
   % (run_res[1], len(smell_hit)),
   run_res[1] and not smell_hit, "跑测=%s 形状=%s" % (run_res[0], smell_hit))

summary()
