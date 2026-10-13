# -*- coding: utf-8 -*-
"""CD 闸门（批六十四）：字幕授金与字幕组认证的档位、审计对象、公告吞错。

三条写口过去只判 `staff()`：
  POST /admin/subtitles/awards/grant   发 5000/2000/1000 火花 + 勋章
  POST /admin/subtitles/certs/grant    盖「certified / gold 字幕组」身份
  POST /admin/subtitles/certs/revoke   撤销该身份
`staff.panel` 挂在 class/90，class 键是 `id >= role_key` 的**阈值**语义
⇒ 91 发布员 / 92 版主 就能发钱盖章（批六十一~六十三收的是读口，本批是写口，
钱 + 荣誉比名单重一档）。

判据纪律：
  - **不发真钱**：授金一律用 `id=0` 走 CAS 卫语句（`WHERE id=$1 AND rank=0 …`
    命中 0 行 ⇒ 返回 skipped，火花与勋章都不发）。这条要在读数里看得见
    （skipped 计数），否则等于没证明「我没发钱」。
  - 荣誉授予用**探针账号自己**领 `certified`，收尾按 (user_id, tier) 删行；
  - 收紧必须配绿证：CD6 给用户级授予 `subtitles.grant`，两口重新可用；
  - 修前基线先取（认证表行数、审计水位线 AUD0），断言只看本轮增量。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_cd_subtitle_grants.py
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
NAME = "cdm%d" % TAG
PERIOD = "CD期间%d" % TAG
UID = [0]
GRANT = "/admin/subtitles/awards/grant"
CERTS = "/admin/subtitles/certs/grant"
REVOKES = "/admin/subtitles/certs/revoke"


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


def audit_refs(action):
    return psql("SELECT coalesce(string_agg(x, ' ~ '), '-') FROM "
                "(SELECT coalesce(ref::text, '[NULL]') AS x FROM audit_log "
                "WHERE action = '%s' AND id > %d ORDER BY id) t"
                % (action, AUD0))


def cleanup():
    if UID[0]:
        psql("DELETE FROM user_subtitle_certs WHERE user_id = %d" % UID[0])
        psql("DELETE FROM user_permissions WHERE user_id = %d" % UID[0])
        psql("DELETE FROM user_roles WHERE user_id = %d" % UID[0])
        if psql("SELECT status::text FROM users WHERE id = %d"
                % UID[0]) != "2":
            call("POST", "/admin/users/status", {"user_id": UID[0],
                                                 "status": 2}, token=ROOT)
        call("DELETE", "/admin/users/%d" % UID[0], None, token=ROOT)


atexit.register(cleanup)

RP0 = int(psql("SELECT count(*) FROM role_permissions") or -1)
AUD0 = int(psql("SELECT coalesce(max(id), 0) FROM audit_log") or -1)
CERT0 = int(psql("SELECT count(*) FROM user_subtitle_certs") or -1)
SPARK0 = int(psql("SELECT count(*) FROM spark_ledger WHERE kind = "
                  "'subtitle_award'") or -1)
ok("CD0 前提：基线已取（认证 %d 行、subtitle_award 流水 %d 条、审计水位 %d）"
   % (CERT0, SPARK0, AUD0), CERT0 >= 0 and SPARK0 >= 0 and AUD0 >= 0,
   "CERT=%d SPARK=%d AUD=%d" % (CERT0, SPARK0, AUD0))

# ===== 探针职员 =====
clear_register_rl()
codes = (call("POST", "/admin/invites", {"user_id": 1, "count": 1},
              token=ROOT)[1] or {}).get("data", {}).get("codes") or []
cap = (call("GET", "/auth/captcha", token=ROOT)[1] or {}).get("data") or {}
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
if codes and len(nums) >= 2:
    call("POST", "/auth/register", {
        "username": NAME, "email": "%s@cd.local" % NAME,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    UID[0] = int(psql("SELECT id FROM users WHERE username = '%s'" % NAME) or 0)
PT = login(NAME, "password123") if UID[0] else None


def grant_key(key, val=True):
    return call("PUT", "/admin/user-permissions",
                {"user_id": UID[0], "permission_key": key, "granted": val},
                token=ROOT)[0]


grant_key("staff.panel")
ok("CD1 探针职员就绪（uid=%s，只有 staff.panel）" % UID[0], bool(PT),
   "uid=%s" % UID[0])

AWARDS_BODY = {"period": PERIOD, "grants": [{"id": 0, "rank": 1}]}
CERT_BODY = {"user_id": UID[0], "tier": "certified", "reason": "CD探针"}

s_aw, b_aw = call("POST", GRANT, AWARDS_BODY, token=PT)
s_cg, b_cg = call("POST", CERTS, CERT_BODY, token=PT)
CERT1 = int(psql("SELECT count(*) FROM user_subtitle_certs") or -1)
ok("CD2 P-236：只有 staff.panel 的职员（=91 发布员起）能发评选奖励与盖章——"
   "两个口都必须 403；今天 grant=%s certs=%s（认证行 %d→%d）"
   % (s_aw, s_cg, CERT0, CERT1),
   s_aw == 403 and s_cg == 403,
   "awards=%s certs=%s 新行=%d" % (s_aw, s_cg, CERT1 - CERT0))
s_rv, _ = call("POST", REVOKES, CERT_BODY, token=PT)
ok("CD2b 撤销认证同口同档：今天 http=%s" % s_rv, s_rv == 403, str(s_rv))

# 修后才有意义：授予 subtitles.grant 前，先确认探针真的能进职员门
ok("CD3 前提：探针确实拿到了 staff.panel（/me/perms 命中）",
   bool(PT) and "staff.panel" in (((call("GET", "/me/perms", None,
                                         token=PT)[1] or {}).get("data")
                                   or {}).get("perms") or []),
   "perms 里有 staff.panel")

# ===== CD4/CD5 审计与吞错 =====
# 审计读数挪到 CD6 之后取：修后被挡住的那几次（403）根本不产生审计，
# 只有授权后的调用会落账——在 403 之前读只会读到空，判据就成了空转。
a_aw, a_cg = "-", "-"
ok("CD4 P-238：授金/盖章的审计要带对象——`subawards.grant` 要留 period 与"
   "发了哪些（ref 装不下：一次多发），`subcert.grant` 要留 tier："
   "awards=%s certs=%s" % (a_aw[:60], a_cg[:60]),
   (PERIOD in a_aw or a_aw == "-") and ("certified" in a_cg or a_cg == "-"),
   "aw=%s cg=%s" % (a_aw[:44], a_cg[:44]))
AW = src("admin_http/subtitle_awards.rs")
# 判据按形状收紧：只认「`let _ =` 之后紧跟 shoutbox 插入」这一种吞错写法。
# 第一版扫「shoutbox 之后 600 字里有没有 let _ =」，结果被自己写的注释
# （里面就有 `let _ =` 四个字面量）命中——静态判据误伤的又是作者本人。
SWALLOW = "let _ =" + chr(10) + '        sqlx::query("INSERT INTO shoutbox'
ok("CD5 P-239：揭晓公告（shoutbox）不许再被 `let _ =` 吞掉（吞错形状="
   "let _ = 紧跟 INSERT INTO shoutbox）",
   SWALLOW not in AW
   and ("tracing::warn!" in AW.split("INSERT INTO shoutbox")[1][:700]
        if "INSERT INTO shoutbox" in AW else False),
   "吞错形状=%s 公告段有 warn=%s" % (SWALLOW in AW,
   "tracing::warn!" in (AW.split("INSERT INTO shoutbox")[1][:700]
                        if "INSERT INTO shoutbox" in AW else "")))

# ===== CD6 绿证：授予新键后两口重新可用（不发真钱）=====
grant_key("subtitles.grant")
s_aw2, b_aw2 = call("POST", GRANT, AWARDS_BODY, token=PT)
CERT_BODY2 = {"user_id": UID[0], "tier": "gold", "reason": "CD探针绿证"}
s_cg2, _ = call("POST", CERTS, CERT_BODY2, token=PT)
CERT2 = int(psql("SELECT count(*) FROM user_subtitle_certs") or -1)
SPARK2 = int(psql("SELECT count(*) FROM spark_ledger WHERE kind = "
                  "'subtitle_award'") or -1)
ok("CD6 用户级授予 subtitles.grant 后两口重新可用：awards=%s（id=0 走 CAS "
   "卫语句 ⇒ skipped，不发钱）、certs=%s（认证行 %d→%d）"
   % (s_aw2, s_cg2, CERT1, CERT2),
   s_aw2 == 200 and s_cg2 == 200 and CERT2 == CERT1 + 1,
   "awards=%s body=%s" % (s_aw2, str(b_aw2)[:70]))
ok("CD6b 零发钱核对：subtitle_award 流水 %d→%d（必须不变，探针走的是不存在的 "
   "award id）" % (SPARK0, SPARK2), SPARK2 == SPARK0,
   "流水差=%d" % (SPARK2 - SPARK0))

a_aw, a_cg = audit_refs("subawards.grant"), audit_refs("subcert.grant")
ok("CD4b 修后（授权调用）审计必须有对象：subawards.grant 要带本期 period，"
   "subcert.grant 要带 tier=%s / %s" % (a_aw[:66], a_cg[:44]),
   PERIOD in a_aw and "skipped" in a_aw and "tier" in a_cg 
   and "gold" in a_cg and "CD探针" in a_cg,
   "aw=%s cg=%s" % (a_aw[:44], a_cg[:44]))

# ===== CD7 三件齐 + 档位 =====
has = psql("SELECT count(*) FROM permissions WHERE key = 'subtitles.grant'")
const = "SUBTITLES_GRANT" in src("authz/perm.rs")
refs = (src("admin_http/subtitle_awards.rs").count("SUBTITLES_GRANT")
        + src("admin_http/subtitle_certs.rs").count("SUBTITLES_GRANT"))
floor = psql("SELECT min(role_key::integer) FROM role_permissions WHERE "
             "role_type='class' AND granted AND permission_key="
             "'subtitles.grant'")
ok("CD7 「目录+常量+消费点」三件齐且授予档位 >=95：目录=%s 常量=%s "
   "引用=%d 处 最低档=%s" % (has == "1", const, refs, floor or "-"),
   has == "1" and const and refs >= 3 and floor not in ("", "None")
   and int(floor) >= 95, "三件=%s/%s/%d 档=%s" % (has, const, refs, floor))

# ===== CD8 收尾 =====
s_rv2, _ = call("POST", REVOKES, CERT_BODY, token=PT)
cleanup()
RP1 = int(psql("SELECT count(*) FROM role_permissions") or -1)
# 撤销口的断言只盯「不是 403」：档位放行是本案要证的，业务结果（该身份是否
# 还在、是否已撤销）由 CD6 的行数与 CD8b 的基线复原负责，两件事不混在一条里。
ok("CD8a 撤销口经授权探针不再被档位挡住：revoke http=%s（非 403 即放行）"
   % s_rv2, s_rv2 != 403, str(s_rv2))
ok("CD8b 基线复原：认证表回 %d→%d、role_permissions %d→%d、cdm 前缀无活号"
   % (CERT0, int(psql("SELECT count(*) FROM user_subtitle_certs") or -1),
      RP0, RP1),
   psql("SELECT count(*) FROM user_subtitle_certs") == str(CERT0)
   and RP1 == RP0 and psql("SELECT count(*) FROM users WHERE username LIKE "
                           "'cdm%%' AND status < 2") == "0",
   "CERT=%s RP=%s" % (psql("SELECT count(*) FROM user_subtitle_certs"), RP1))

summary()
