# -*- coding: utf-8 -*-
"""GB 闸门：反作弊「回收虚报上传量」的流水与快照对账（批一百六十九 P-403）。

洞的形状（仓里早有定例，见 `admin_http/user_ops.rs` 的 P0 错账注释：
「流水必须记实际发生的差额而非请求 delta」）：
  扣减侧 `uploaded = GREATEST(uploaded - 请求值, 0)` 会被夹到 0，
  而 traffic_ledger 写的是**请求值** ⇒
    ① sum(ledger) 与 users.uploaded 永久撕裂；
    ② 申诉推翻按同一枚请求值回补 ⇒ 虚报数大于现有上传量的案件被推翻后，
       会员白得差额（分享率、等级、成就全读这一列）。
实测（旧镜像 :8080，探针号 uploaded=100、reclaim_up=500）：
  快照 100→0（Δ=-100），流水合计 -500 ⇒ **撕裂 400 字节**。

判据同时钉反向解除只读 `reclaim_applied`（缺键的历史案件仍按请求值走，
因为那些案的扣减侧本来就是请求值，账目自洽，不许凭空改口径）。

用法：`python -u scripts/pt_audit_gb_cheat_reclaim.py`
"""
import io
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import call, login, ok, summary, clear_register_rl
SANCT = "apps/api/src/http/cheat_case/sanction.rs"
REV = "apps/api/src/repo/cheat_reverse.rs"
# P-403 收尾：回收的正向执行下沉到 repo（sanction.rs 曾长到 312 行破 300 门禁），
# 所以 GB1 的读向要跟着换文件——**判据跟着实现走，不许反过来让实现迁就判据**。


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def cut(s):
    return "\n".join(
        ln for ln in s.split("\n") if not ln.strip().startswith("//"))


def one(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (r.stdout or "").strip()


san = cut(rd(SANCT))
rev = cut(rd(REV))
# 下沉后：正向执行的 SQL 在 rev 里，san 里只剩一次调用
CALL = "crate::repo::cheat_reverse::apply_reclaim_tx"
# 锚点只能取**码体**：这段的说明文字在 `//` 注释里，而 `cut()` 已把注释整行
# 剥掉——按注释关键词去 split 会得到空串，判据恒红（仪器自伤，本轮实测）。
READ = "SELECT uploaded FROM users WHERE id = $1 FOR UPDATE"
# 下沉后这两处都在 repo 侧：索引必须建在同一个文件里，否则恒红（本轮就红过一次）
i_read = rev.find(READ)
i_ledger = rev.find("INSERT INTO traffic_ledger")
ok("GB1 回收前先锁行读实际上传量，流水绑的是实际值（下沉后的 repo 侧）",
   0 <= i_read < i_ledger and ".bind(-applied)" in rev
   and ".bind(-reclaim_up)" not in rev and "applied = requested.min(" in rev,
   "写请求值＝sum(ledger) 与快照永久撕裂（对照 user_ops 的 P0 错账定例）")
ok("GB1b handler 侧只调一次下沉原语，且把返回值写进案件 JSON",
   CALL in san and "let reclaim_applied = " in san,
   "编排层自己再拼一份 SQL＝第二把尺，正反对账迟早漂移")
ok("GB2 案件 JSON 同时留请求值与实际值（前者读数、后者回补）",
   '"reclaim_up": reclaim_up' in san and '"reclaim_applied": reclaim_applied'
   in san, "只留一个数就没法两边对账，也没法判断历史案件该按哪个口径")
ap_pos = rev.find('get("reclaim_applied")')
up_pos = rev.find('get("reclaim_up")')
ok("GB3 反向解除优先读 applied（缺键历史案件才回落请求值）",
   0 <= ap_pos < up_pos and "or_else" in rev[:up_pos + 200],
   "按请求值回补＝推翻一次白得一批上传量")
OLD = ('        .bind(uid)\n        .bind(-reclaim_up)\n'
       '        .bind(format!("cheat reclaim case #{id}"))')
ok("GB4 承重自证：旧的「流水绑请求值」写法必须判红",
   ".bind(-reclaim_up)" in OLD, "打不中说明 GB1 的认法已经瞎了")

# ---------- 现网台账：新案件不许再落「只有请求值」的旧形状 ----------
cases = one("SELECT count(*) FROM reports WHERE ref_type='cheat' "
            "AND coalesce((sanction->>'reclaim_up')::bigint,0) > 0")
old_shape = one("SELECT count(*) FROM reports WHERE ref_type='cheat' "
                "AND coalesce((sanction->>'reclaim_up')::bigint,0) > 0 "
                "AND sanction->>'reclaim_applied' IS NULL")
print("   [读数] 现网带 reclaim 的作弊案 %s 件；回收流水行 %s 条；"
      "旧形状（只有请求值）%s 件" % (
          cases, one("SELECT count(*) FROM traffic_ledger WHERE "
                     "reason LIKE 'cheat reclaim %'"), old_shape))
ok("GB5 带回收的作弊案必须写明实际扣减数（旧形状只许是历史已结案件）",
   one("SELECT count(*) FROM reports WHERE ref_type='cheat' AND "
       "coalesce((sanction->>'reclaim_up')::bigint,0) > 0 AND "
       "sanction->>'reclaim_applied' IS NULL AND status <> 1") == "0",
   "未结案件缺 reclaim_applied＝有人绕过 sanction 口写了 JSON，"
   "反向解除会退回按请求值回补的老洞")

# ---------- 可选活体：端到端跑一遍「回收 → 申诉推翻 → 回补额」 ----------
# 默认关：站方镜像未重建时这条必然红（旧码就是会白给流量），常驻红账没人看；
# 要复跑：GB_LIVE=1 FLUX_API_BASE=http://127.0.0.1:8099/api/v1 python -u 本脚本
LIVE = os.environ.get("GB_LIVE", "0") != "0"
UP0, RECLAIM = 100, 500
CLN = []
UIDS = []


def dat(p):
    return (p or {}).get("data") or {}


def mk_member(seq, staff=False):
    """造探针号；staff=True 时临时提到 class 99（复核要四眼，决定人不能自复核）。"""
    clear_register_rl()
    time.sleep(6)
    cs = dat(call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                  token=ROOT_TOK)[1]).get("codes") or []
    if not cs:
        return None
    code, cid = cs[0]["code"], cs[0]["id"]
    cap = dat(call("GET", "/auth/captcha", token=ROOT_TOK)[1])
    ns = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(ns) < 2:
        return None
    nm = "gb%d%d" % (seq, int(time.time()) % 100000)
    s2, r2 = call("POST", "/auth/register", {
        "username": nm, "email": "%s@gb.local" % nm,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(ns[0]) + int(ns[1]),
        "captcha_token": "", "fields": {}})
    u2 = dat(r2).get("user_id")
    if not u2:
        print("   [GB 造号失败] %s" % str(r2)[:150])
        return None
    UIDS.append(int(u2))
    CLN.append("UPDATE users SET status = 2, class_id = 1, uploaded = 0 "
               "WHERE id = %s" % u2)
    CLN.append("DELETE FROM invites WHERE id = %d" % cid)
    if staff:
        one("UPDATE users SET class_id = 99 WHERE id = %s" % u2)
    tok = None
    for _ in range(10):
        s3, r3 = call("POST", "/auth/login",
                      {"username": nm, "password": "password123"})
        tok = dat(r3).get("token")
        if tok:
            break
        time.sleep(3)
    return tok if tok else None


if LIVE:
    ROOT_TOK = login("root")
    ok("GB-L0 root 登录可用", bool(ROOT_TOK), "卡在 PoW/限流则活体段作废")
    if ROOT_TOK:
        victim = mk_member(1)
        reviewer = mk_member(2, staff=True)
        ok("GB-L1 受害者与第二个职员探针", bool(victim) and bool(reviewer),
           "复核申诉要四眼：root 是决定人，自复核会被拒（那条拒绝是对的）")
        if victim and reviewer:
            # 受害者是第一个造出来的号（第二个是复核职员）
            uid = UIDS[0]
            one("UPDATE users SET uploaded = %d WHERE id = %s" % (UP0, uid))
            s, r = call("POST", "/admin/cheat-cases/open",
                        {"user_id": int(uid), "note": "gb live"},
                        token=ROOT_TOK)
            tid = dat(r).get("case_id") or dat(r).get("id")
            call("POST", "/admin/cheat-cases/%s/claim" % tid,
                 {"note": "gb"}, token=ROOT_TOK)
            s, r = call("POST", "/admin/cheat-cases/%s/resolve" % tid, {
                "sanction": "none", "reclaim_up": RECLAIM,
                "note": "gb 回收虚报"}, token=ROOT_TOK)
            snap = int(one("SELECT uploaded FROM users WHERE id=%s" % uid))
            led = int(one("SELECT coalesce(sum(delta_up),0) FROM "
                          "traffic_ledger WHERE user_id=%s AND reason LIKE "
                          "'cheat reclaim %%'" % uid) or 0)
            ok("GB-L2 不变量一：流水合计 == 快照实际变化（快照=%s 流水=%s）"
               % (snap, led), snap == UP0 - 100 and led == -100,
               "旧码在这里就是撕裂 %d 字节" % (UP0 - snap + led))
            s, r = call("POST", "/cheat-appeals", {
                "case_id": int(tid),
                "body": "这不是我做的，请复核证据来源与统计口径。"},
                token=victim)
            aid = dat(r).get("appeal_id") or dat(r).get("id")
            CLN.append("DELETE FROM appeals WHERE id = %s" % aid)
            s, r = call("POST", "/admin/cheat-appeals/%s/resolve" % aid,
                        {"outcome": "overturned", "note": "gb 推翻"},
                        token=reviewer)
            end = int(one("SELECT uploaded FROM users WHERE id=%s" % uid) or 0)
            net = int(one("SELECT coalesce(sum(delta_up),0) FROM "
                          "traffic_ledger WHERE user_id=%s AND reason LIKE "
                          "'cheat %%'" % uid) or 0)
            ok("GB-L3 不变量二：推翻后回补到最初值（=%s，期望 %s；流水净额=%s）"
               % (end, UP0, net), end == UP0 and net == 0,
               "旧码这里 uploaded 变 %s＝白得 %d 字节，而流水净额 0 看不出来"
               % (end, end - UP0))
            CLN.append("DELETE FROM traffic_ledger WHERE user_id = %s" % uid)
            CLN.append("DELETE FROM case_notes WHERE case_id = %s" % tid)
            CLN.append("DELETE FROM reports WHERE id = %s" % tid)
        print("== 收尾 ==")
        for st in reversed(CLN):
            if st:
                one(st)
        ok("GB-L4 残料清零（探针流水 0、在册数不变）",
           one("SELECT count(*) FROM traffic_ledger WHERE user_id IN (%s)"
               % (",".join(str(x) for x in UIDS) or "-1")) == "0",
           "在册=%s" % one("SELECT count(*) FROM users WHERE status < 2"))

summary()
