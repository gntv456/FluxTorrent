# -*- coding: utf-8 -*-
"""闸门 FP：举报口不得驱动作弊案件的状态机（P-475）。

现场（10-21 01:09 现网实测）：`reports` 表里 `ref_type='cheat'` 有 35 行
（33 未决 / 2 已处理），而 GET /admin/reports 的 SQL **没有任何类型过滤** ⇒
版主一打开「举报队列」，pending 34 行里 33 行是作弊案件，且队列的 ref_label
CASE 没有 cheat 分支 ⇒ 这些行的对象名全是空，界面只能回落成 `#<ref_id>`。
更要紧的是三个写状态口（claim/release/resolve）都只判 `REPORTS_HANDLE`，而
作弊案件的正门 `POST /admin/cheat-cases/{id}/resolve` 判 `CHEAT_SANCTIONS`、
要处置说明、校验 sanction 档位、比高低（ensure_outranks）、顺带解除证据停发，
高危处置还要四眼批准（0369 M3）。从举报口点「处理」会把案件写成已处理并
跳过以上全部。

判据（活体 + 静态，夹具只用本批自己造的行）：
  FP1 队列里不得出现 cheat（all 与 pending 两档都数，并报现网 cheat 总行数）
  FP2 claim 一条 cheat 工单必须被**点名**拒（400 + 文案含「作弊」），
      且该行 status 仍 0、claimed_by 仍 NULL（旧代码：200 且翻成 2＝缺陷本体）
  FP3 release 同样点名拒（用 SQL 造「已被 root 认领」的前置，让守卫真被跑到）
  FP4 resolve 点名拒（旧代码不发这半边证据：会真结案并给举报人发私信，
      所以只在修复侧断言拒绝；旧代码的越权由 FP2 的活体红账承担）
  FP5 正控（守卫不许过头）：同一条口对 ref_type='user' 的探针行必须照常
      claim 200 + release 200（举报自己的流转不受影响）
  FP6 静态：三个写口都读到 CHEAT_GUARD；列表 SQL 带 `<> 'cheat'`
  FP9 收尾：本批造的探针行按 reason 删净并回读残留 0

跑法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_fp_report_scope.py
侧进程：FLUX_API_BASE=http://127.0.0.1:8099/api/v1 python -u \\
    scripts/pt_audit_fp_report_scope.py
"""
import io
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, psql_stdin, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MARK = "FP 闸门探针"
SENT = "作弊案件请到「作弊处置队列」处理（需作弊处置权限）"


def q(sql):
    return psql_stdin(sql).strip()


tok = login()
ROOT = q("SELECT id FROM users WHERE username = 'root'")
VICTIM = q("SELECT id FROM users WHERE id <> %s ORDER BY id DESC LIMIT 1"
           % ROOT)
print("== FP0 地基：root=%s 对照用户=%s ==" % (ROOT, VICTIM))
# 开局自愈：清掉上一轮可能中断留下的探针行
q("DELETE FROM reports WHERE reason = '%s'" % MARK)


def mkrow(ref_type):
    rid = q("INSERT INTO reports (reporter_id, ref_type, ref_id, reason) "
             "VALUES (%s, '%s', %s, '%s') RETURNING id"
             % (ROOT, ref_type, VICTIM, MARK))
    # psql 会把命令标签（`INSERT 0 1`）跟在 RETURNING 值后面 ⇒ 只取第一行
    return int(rid.splitlines()[0].strip())


cheat_id = mkrow("cheat")
user_id = mkrow("user")
total_cheat = q("SELECT count(*) FROM reports WHERE ref_type = 'cheat'")
print("   探针行：cheat=%d user=%d（现网 cheat 工单总数=%s）"
      % (cheat_id, user_id, total_cheat))

print("== FP1 队列不许把作弊案件当举报列出来 ==")
s_all, b_all = call("GET", "/admin/reports?status=all", token=tok)
s_pend, b_pend = call("GET", "/admin/reports", token=tok)
rows_all = (b_all or {}).get("data") or []
rows_pend = (b_pend or {}).get("data") or []
leak_all = [r["id"] for r in rows_all if r.get("ref_type") == "cheat"]
leak_pend = [r["id"] for r in rows_pend if r.get("ref_type") == "cheat"]
blank = [r["id"] for r in rows_all if not r.get("ref_label")]
ok("FP1 all/pending 两档都零 cheat（实测 %d/%d 行，现网在册 %s 条）"
   % (len(leak_all), len(leak_pend), total_cheat),
   s_all == 200 and s_pend == 200 and not leak_all and not leak_pend,
   "泄漏=%s… 空标签行=%d（旧形状：33 条案件灌进 pending，界面只能显示 #id）"
   % (leak_all[:3], len(blank)))

print("== FP2 claim 一条 cheat 工单必须点名拒，且不动状态 ==")
s2, r2 = call("POST", "/admin/reports/claim", {"report_id": cheat_id},
              token=tok)
st_after = q("SELECT status || '/' || coalesce(claimed_by::text, 'NULL') "
             "FROM reports WHERE id = %d" % cheat_id)
msg2 = str(r2.get("message") or r2.get("detail") or "")
ok("FP2 claim cheat → 400 且文案点到「作弊」（http=%s msg=%s）" % (s2, msg2[:40]),
   s2 == 400 and "作弊" in msg2 and r2.get("code") != 0, str(r2)[:140])
ok("FP2 那条 cheat 工单没被动过（status/claimed_by=%s，应 0/NULL）" % st_after,
   st_after == "0/NULL", "旧代码这里会是 2/%s" % ROOT)

print("== FP3 release 同样点名拒（前置用 SQL 造「已被 root 认领」）==")
q("UPDATE reports SET status = 2, claimed_by = %s, claimed_at = now() "
  "WHERE id = %d" % (ROOT, cheat_id))
s3, r3 = call("POST", "/admin/reports/release", {"report_id": cheat_id},
              token=tok)
st3 = q("SELECT status FROM reports WHERE id = %d" % cheat_id)
msg3 = str(r3.get("message") or r3.get("detail") or "")
ok("FP3 release cheat → 400 且点名「作弊」（http=%s msg=%s）" % (s3, msg3[:40]),
   s3 == 400 and "作弊" in msg3, str(r3)[:140])
ok("FP3 工单仍是已认领（status=%s，说明没走举报的释放流转）" % st3, st3 == "2",
   "旧代码这里会被写回 0")

print("== FP4 resolve cheat 必须点名拒（正门在作弊队列）==")
s4, r4 = call("POST", "/admin/reports/resolve",
              {"report_id": cheat_id, "action": "dismiss", "note": "FP 探针"},
              token=tok)
st4 = q("SELECT status FROM reports WHERE id = %d" % cheat_id)
msg4 = str(r4.get("message") or r4.get("detail") or "")
ok("FP4 resolve cheat → 400 且点名「作弊」（http=%s msg=%s）" % (s4, msg4[:40]),
   s4 == 400 and "作弊" in msg4, str(r4)[:140])
ok("FP4 工单状态没被写成已处理（status=%s，应仍是 2）" % st4, st4 == "2",
   "旧代码会从这边结案并给举报人发私信")

print("== FP5 正控：守卫不许过头——普通举报的认领/释放照常 ==")
s5, r5 = call("POST", "/admin/reports/claim", {"report_id": user_id},
              token=tok)
ok("FP5 探针（ref_type=user）claim 200（http=%s code=%s）"
   % (s5, r5.get("code")), s5 == 200 and r5.get("code") == 0, str(r5)[:120])
st5 = q("SELECT status FROM reports WHERE id = %d" % user_id)
ok("FP5 claim 真的把工单占下了（status=%s，应 2）" % st5, st5 == "2", "")
s6, r6 = call("POST", "/admin/reports/release", {"report_id": user_id},
              token=tok)
st6 = q("SELECT status FROM reports WHERE id = %d" % user_id)
ok("FP5 release 也照常（http=%s status=%s，应 200/0）" % (s6, st6),
   s6 == 200 and st6 == "0", str(r6)[:120])


def strip_rs(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    out = []
    for line in text.split("\n"):
        if "http://" in line or "https://" in line:
            out.append(line)
            continue
        cut = re.search(r"(?<!:)//", line)
        out.append(line[:cut.start()] if cut else line)
    return "\n".join(out)


RQ = strip_rs(io.open(os.path.join(ROOTDIR, "apps", "api", "src", "admin_http",
                                   "reports.rs"), encoding="utf-8").read())
RR = strip_rs(io.open(os.path.join(ROOTDIR, "apps", "api", "src", "admin_http",
                                   "reports_resolve.rs"),
                      encoding="utf-8").read())
N_CLAIM = RQ.count('== Some("cheat")')
N_RESOLVE = RR.count('== "cheat"')
print("== FP6 静态：claim/release 类型闸门=%d 处、resolve=%d 处 =="
      % (N_CLAIM, N_RESOLVE))
ok("FP6 claim/release 各有类型闸门（共 2 处）、resolve 也有（1 处）",
   N_CLAIM >= 2 and N_RESOLVE >= 1,
   "reports=%d resolve=%d" % (N_CLAIM, N_RESOLVE))
ok("FP6 列表 SQL 排除 cheat（`ref_type <> 'cheat'` 在 WHERE 上）",
   "ref_type <> 'cheat'" in RQ, "")
ok("FP6 同一句文案有译文在册（validation_details.tsv 命中）",
   SENT in io.open(os.path.join(ROOTDIR, "apps", "api", "i18n",
                                "validation_details.tsv"),
                   encoding="utf-8").read(), "缺行=英文界面会看到中文")

print("== FP9 收尾 ==")
q("DELETE FROM reports WHERE reason = '%s'" % MARK)
left = q("SELECT count(*) FROM reports WHERE reason = '%s'" % MARK)
# 旧镜像上 FP4 会真的从举报口结案并给「举报人」发私信（root 自己）——这条残留
# 必须收掉；新代码拒单后本来就没有这行，计数为 0 不影响。
q("DELETE FROM messages WHERE receiver_id = %s AND body LIKE '%%FP 探针%%'"
  % ROOT)
pm_left = q("SELECT count(*) FROM messages WHERE receiver_id = %s "
            "AND body LIKE '%%FP 探针%%'" % ROOT)
ok("FP9 探针行删净（残留=%s）" % left, left == "0",
   "现网原有 cheat 工单未参与本批写口（claim/resolve 都被拒）")
ok("FP9 探针私信无残留（残留=%s）" % pm_left, pm_left == "0",
   "旧镜像上 FP4 会真结案并发私信，这条只清本批 note 文本命中的行")

summary()
