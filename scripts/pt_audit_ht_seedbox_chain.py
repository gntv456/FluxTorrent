# -*- coding: utf-8 -*-
"""闸门 HT：种子机（seedbox）申报-审核-生效链路的口径对账（批一百九十三 P-426）。

链路（现网实测过一次，夹具全在事务里回滚）：
  会员 POST /me/seedbox/claim（CIDR，前缀过宽 422）
    → 职员 GET /admin/seedbox/claims 队列 → POST /admin/seedbox/review
    → 写 seedbox_profiles → **视图 seedbox_effective** 供反作弊读：
       · worker/jobs/seeding.rs:72  LEFT JOIN 视图 × bonus_factor
       · worker/jobs/process_event.rs:123 读 speed_relax
       · worker/jobs/hr_snapshot.rs:75 读 hnr_exempt（P-426c 前直读基表）
    → POST /admin/seedbox/revoke 撤销

三条判据的来源（都是实测，不是读码猜）：
  1. P-426a `repo::seedbox::review` 驳回时只清 hnr_exempt，bonus_factor 与
     anticheat_speed_relax **原样落库**——与它上面「驳回时整套清零」的注释矛盾。
     今天被视图 `WHERE approved OR detected` 挡住，但 detected 一旦由 ASN 自动
     识别置上，被驳回那单的 3.0 倍率就绕过「人工背书」直接生效。
  2. P-426b 视图与成文口径 `domain::Flags::from_profile` 三处不同尺：
     hnr/bonus 没吃 approved（越权给），speed_relax 少了 `OR detected`（该给不给）。
     0405 迁移拉平；实测 A/B（现网回滚内）：
       approved 行  旧=新=(t,2.00,f)  ⇒ 合法用户零行为变化
       detected 行  旧=(t,3.00,f) → 新=(f,1.0,t)
  3. 自审自批：root 申报、root 审核，`/admin/seedbox/review` 回 **200**。
     「提交即 pending，不自动生效——放宽反作弊必须人工审核背书」（本文件口径）
     在只有一个人有权限的站里等于没有背书。**是否要拦属审核口径，待站方裁**，
     HT5 只把现状钉住（改成拦了会红，逼来者同步本注释）。

判据：
  HT1 现网三口在册（会员 status/claims + 职员队列）；
  HT2 视图按 0405 应用后必须满足 from_profile 语义（事务内探针 + ROLLBACK）；
  HT3 静态：review() 落库的三列都必须过 approve 闸（旧形状必须被抓住＝正控）；
  HT4 静态：hr_snapshot 必须读视图而不是基表（同一条尺）；
  HT5 现状钉桩：自审自批目前被允许（改了会红）；
  HT6 现网账面：approved=false 却仍带非默认标志的 profile 行数 = 0（只许 0）；
  HT7 前端入口计数（申报/审核面板仍未成屏＝钉桩，附「判据能搜到已知串」正控）。

用法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ht_seedbox_chain.py
      HT_LIVE=0 跳过 HT1/HT2/HT5/HT6 的现网与库侧只读事务。
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import call, grep_files, login, ok, summary  # noqa: E402

LIVE = os.environ.get("HT_LIVE", "1") != "0"
REPO = "apps/api/src/repo/seedbox.rs"
HRS = "apps/worker/src/jobs/hr_snapshot.rs"
MIG = "apps/api/migrations/0405_seedbox_effective_parity.sql"
PROBE_IDS = (6702, 6703)  # 墓碑号：会员面不可见，专属本轮前缀之外无副作用


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip_rs(src):
    out = []
    for ln in src.split("\n"):
        t = ln.strip()
        if t.startswith("//") or t.startswith("///"):
            continue
        out.append(ln)
    return "\n".join(out)


def review_block(src):
    i = src.find("pub async fn review(")
    j = src.find("pub async fn flags_of", i)
    return src[i:j if j > i else len(src)]


def reject_binds(block):
    """返回「驳回时没清零」的列名列表（逐列判 bind 有没有过 approve 闸）。

    ⚠️ 必须逐列：整段判断会让 hnr 那一句的 `approve &&` 把 bonus_factor
    的不过闸洗白（HT3b 首跑就是这么假绿的）。
    """
    m = re.search(r"\.bind\(uid\)(.*?)\.execute\(", block, re.S)
    if not m:
        return ["找不到 bind 序列（形状变了，先读源码）"]
    segs = m.group(1).split(".bind(")
    bad = []
    for name in ("bonus_factor", "hnr_exempt", "speed_relax"):
        hit = [s for s in segs if re.search(r"\b%s\b" % name, s)]
        if not hit:
            bad.append("%s（找不到该列的 bind）" % name)
        elif "approve" not in hit[0]:
            bad.append(name)
    return bad


def psql(tx_sql):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1",
         "-e", "PGCLIENTENCODING=UTF8", "flux-postgres", "psql", "-U",
         "flux", "-d", "fluxtorrent", "-q", "-t", "-A", "-F", "|"],
        input=tx_sql, capture_output=True, text=True, encoding="utf-8",
        errors="replace")
    if "ERROR" in (r.stderr or ""):
        raise RuntimeError("psql: " + (r.stderr or "")[:400])
    return (r.stdout or "").strip()


blk = strip_rs(review_block(strip_rs(rd(REPO))))
bad = reject_binds(blk)
ok("HT3 驳回必须三样一起清（bonus/hnr/speed 全过 approve 闸）", bad == [],
   "未过闸的列=%s" % (bad,))
old_shape = blk.replace("if approve { bonus_factor.max(1.0) } else { 1.0 }",
                        "bonus_factor")
ok("HT3b 承重：旧形状（倍率不过闸）被本判据抓住",
   len(reject_binds(old_shape)) >= 1, "抓=%s" % (reject_binds(old_shape),))
hr = strip_rs(rd(HRS))
ok("HT4 H&R 侧读视图而非基表（与 0405 同一条尺）",
   "seedbox_effective sp" in hr and "FROM seedbox_profiles sp" not in hr,
   "hr_snapshot 里 sp 的来源=" + (
       "视图" if "seedbox_effective sp" in hr else "基表"))

if LIVE:
    tok = login()
    for name, path in (("会员 status", "/me/seedbox/status"),
                       ("会员 claims", "/me/seedbox/claims"),
                       ("职员队列", "/admin/seedbox/claims?limit=5")):
        st, b = call("GET", path, None, tok)
        ok("HT1 %s 口在册（%s）" % (name, path), st == 200 and b.get("code") == 0,
           "http=%s code=%s" % (st, b.get("code")))

    # HT2：在同一事务里铺探针 → 应用 0405 → 读口径 → ROLLBACK。
    # 现网视图是否已换新版不影响本条（判据只看应用迁移后的语义）。
    mig = rd(MIG)
    tx = (
        "BEGIN;\n"
        "INSERT INTO seedbox_profiles (user_id, approved, detected, "
        "hnr_exempt, bonus_factor, anticheat_speed_relax, updated_at) VALUES "
        "(6702, true, false, true, 2.0, false, now()), "
        "(6703, false, true, true, 3.0, false, now()) "
        "ON CONFLICT (user_id) DO UPDATE SET approved = EXCLUDED.approved, "
        "detected = EXCLUDED.detected, hnr_exempt = EXCLUDED.hnr_exempt, "
        "bonus_factor = EXCLUDED.bonus_factor, "
        "anticheat_speed_relax = EXCLUDED.anticheat_speed_relax;\n"
        + mig + "\n"
        "SELECT user_id, hnr_exempt, bonus_factor, speed_relax "
        "FROM seedbox_effective WHERE user_id IN (6702,6703) "
        "ORDER BY user_id;\n"
        "ROLLBACK;\n")
    rows = [l.split("|") for l in psql(tx).split("\n") if l.startswith("6")]
    got = {r[0]: r[1:] for r in rows}
    ok("HT2a approved 行：hnr=t、bonus=2.00、speed=f（合法用户零行为变化）",
       got.get("6702") == ["t", "2.00", "f"], "读数=%s" % (got.get("6702"),))
    ok("HT2b detected-only 行：hnr=f、bonus=1.0、speed=t（强承诺要人工背书）",
       got.get("6703") == ["f", "1.0", "t"], "读数=%s" % (got.get("6703"),))
    ok("HT2c 前置断言：探针两行确实被查出来（防判据读空恒真）",
       len(got) == 2, "命中行数=%d" % len(got))
    left = psql("SELECT count(*) FROM seedbox_profiles;")
    ok("HT2d 收尾：事务已回滚，profile 行数不含探针", left == "1",
       "现网=%s（期望 1＝只有 root）" % left)

    st, b = call("POST", "/me/seedbox/claim",
                 {"cidrs": ["203.0.113.0/24"]}, tok)
    cid = (b.get("data") or {}).get("claim_id")
    st2, b2 = call("POST", "/admin/seedbox/review", {
        "claim_id": cid, "approve": True, "hnr_exempt": True,
        "bonus_factor": 1.5, "speed_relax": True}, tok)
    ok("HT5 现状钉桩：自审自批（申报人==审核人）仍被允许＝待裁",
       st2 == 200 and b2.get("code") == 0,
       "http=%s code=%s（若加了拦阻请同步本注释并撤桩）" % (st2, b2.get("code")))
    call("POST", "/admin/seedbox/revoke", {"user_id": 1}, tok)
    psql("DELETE FROM seedbox_claims WHERE cidrs::text LIKE '%203.0.113%';")
    ok("HT5b 收尾：本轮 claim 夹具删净",
       psql("SELECT count(*) FROM seedbox_claims;") == "0",
       "残留=%s" % psql("SELECT count(*) FROM seedbox_claims;"))
    dirty = psql("SELECT count(*) FROM seedbox_profiles WHERE NOT approved "
                 "AND (bonus_factor <> 1.0 OR anticheat_speed_relax "
                 "OR hnr_exempt);")
    ok("HT6 无「未通过审核却带非默认标志」的脏 profile（只许 0）",
       dirty == "0", "现网=%s" % dirty)
    st3, b3 = call("GET", "/me/seedbox/status", None, tok)
    d = (b3.get("data") or {})
    ok("HT6b 收尾：root 标志已回默认",
       d.get("hnr_exempt") is False and d.get("bonus_factor") in (1, 1.0)
       and d.get("speed_relax") is False, "读数=%s" % (d,))
else:
    print("[HT1/HT2/HT5/HT6] HT_LIVE=0 跳过现网与库侧")

hits = grep_files("seedbox", ["apps/web"], root=ROOT, ignore_case=True)
ok("HT7b 正控：同一 grep 手段在 apps/api 搜 seedbox 必须有命中（防路径写错恒 0）",
   len(grep_files("seedbox", ["apps/api/src"], root=ROOT,
                  ignore_case=True)) > 3,
   "apps/api 命中=%s" % len(hits))
ok("HT7 前端入口仍是 0 屏（申报与审核都只能在 API 操作＝待补的屏，钉桩）",
   len(hits) == 0, "apps/web 命中=%s（补界面后撤本条）" % (hits,))

summary()
