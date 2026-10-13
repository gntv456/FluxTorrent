# -*- coding: utf-8 -*-
"""闸门 BV：审核链上的「驳回理由字典 + 暂缓(postpone)/恢复(resume) 第四态」。
路由：/admin/deny-reasons 四条 + /admin/reviews/postpone /admin/reviews/resume，
常驻 pt_audit_* 零命中（审核流本身有 d_review_flow 闸门，但它打的是
decide/batch/通知，没打过字典与暂缓态）。

打的四条：
  - **P-215** 理由文本按**字节**判长（`r.len() > 200`，文案「原因长度 1-200」）
    ⇒ 中文理由约 67 字就自拒。这是「字节 vs 字符」在本仓的第七处
    （P-200/203/214 同族），所以两条正控一起钉：200 字符放行、201 字符仍拒。
  - **P-216** `deny_reason.update` / `.delete` 的审计 `ref=None` ⇒「改了/删了**哪条**
    理由」读不出（与 P-202/204/208/212 同族第六处）。delete 沿用「先读后删」：
    行没了以后 reason 文本就是唯一线索。
  - **P-217（纵深）** 删除前的引用计数写成 `.unwrap_or(0)` ⇒ 那条查询一失败，
    「仍被种子引用不许删」这道护栏就**默认放行**。本库 FK 是 NO ACTION，所以最坏
    是从「友好 400」退化成 500，不是静默解绑——按读码 + 静态判据记，不夸大成数据事故。
  - **P-218** 审核留痕三处（decide / postpone / resume）都是 `let _ = sqlx::query(...
    torrent_operation_logs ...)` ⇒ 写不进操作时间线时**无人知道**。种子操作记录正是
    站长回看「这颗种什么时候被谁怎么处置过」的唯一一处，失败必须可见（改成 warn!，
    不让留痕失败反过来阻断裁决）。

暂缓态用**库里已有的真实待审种子**打，并按批四十的规矩做整行备份复原
（`CREATE TABLE ... AS SELECT` + `IS DISTINCT FROM` 逐列比对，收尾不依赖被验代码）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bv_deny_review.py
"""
import io
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
RSN = "BV超长理由" + "字" * 70          # 74 字符 / 226 字节
BK = "bvbak%d" % TAG


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:50], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


TID = int(psql("SELECT min(id) FROM torrents WHERE approval_status IN (0, 2) "
               "AND (owner_id IS NULL OR owner_id <> 1)") or 0)
RID = [0]


def bk_alive():
    return psql("SELECT count(*) FROM information_schema.tables WHERE "
                "table_name = '%s'" % BK.lower()) == "1"


def cleanup():
    if TID and bk_alive():
        psql("UPDATE torrents t SET approval_status = b.approval_status, "
             "deny_note = b.deny_note FROM %s b WHERE b.id = t.id "
             "AND t.id = %d" % (BK, TID))
    psql("DELETE FROM torrent_deny_reasons WHERE id = %d "
         "OR reason LIKE 'BV%%'" % (RID[0] or 0))


import atexit  # noqa: E402

atexit.register(cleanup)

T0 = psql("SELECT to_char(now(), 'YYYY-MM-DD HH24:MI:SS.US')")
RC0 = int(psql("SELECT count(*) FROM torrent_deny_reasons") or -1)
ok("V0a 基线：拒绝理由 %d 条；可暂缓状态(0/2)的种子探针 id=%d" % (RC0, TID),
   RC0 >= 0 and TID > 0, "RC=%s TID=%s" % (RC0, TID))
if TID:
    psql("DROP TABLE IF EXISTS %s" % BK)
    psql("CREATE TABLE %s AS SELECT id, approval_status, deny_note FROM "
         "torrents WHERE id = %d" % (BK, TID))
    ok("V0b 前提：待审种子已整行备份（status+deny_note），收尾按备份复原",
       psql("SELECT count(*) FROM %s" % BK) == "1",
       "备份原状态=%s note=%s" % (psql("SELECT approval_status FROM %s" % BK),
                                  psql("SELECT coalesce(deny_note,'(空)') "
                                       "FROM %s" % BK)))

# ===== V1 理由判长的字节/字符 =====
s, b = call("POST", "/admin/deny-reasons", {"reason": RSN}, token=ROOT)
rid = ((b or {}).get("data") or {}).get("id")
RID[0] = int(rid or 0)
ok("V1a %d 个字符（%d 字节）的理由必须能建（文案写「1-200」= 字符）："
   "http=%s msg=%s" % (len(RSN), len(RSN.encode()), s,
                       str((b or {}).get("message", ""))[:22]),
   s == 200 and RID[0] > 0, "http=%s id=%s" % (s, rid))
if not RID[0]:
    # 前置红不许带崩 V2/V3：用短理由把探针行建出来，让审计判据仍有对象可打
    s, b = call("POST", "/admin/deny-reasons", {"reason": "BV probe %d" % TAG},
                token=ROOT)
    RID[0] = int(((b or {}).get("data") or {}).get("id") or 0)
s2, _ = call("POST", "/admin/deny-reasons", {"reason": "字" * 201}, token=ROOT)
ok("V1b 正控：201 字符仍须拒（别把修复写成取消限长）", s2 == 400,
   "http=%s" % s2)

# ===== V2 审计要带对象 =====
s3, _ = call("PUT", "/admin/deny-reasons/%d" % RID[0],
             {"sort": 5}, token=ROOT)
s4, _ = call("DELETE", "/admin/deny-reasons/%d" % RID[0], None, token=ROOT)
RID[0] = 0


def mine(prefix):
    d = (call("GET", "/admin/audit?q=%s&limit=60" % prefix, None,
              token=ROOT)[1] or {}).get("data") or []
    return [r for r in d
            if str(r.get("created_at") or "").replace("T", " ")[:19]
            >= T0[:19]]


up = mine("deny_reason.update")
de = mine("deny_reason.delete")
ok("V2a 改/删拒绝理由的审计要带对象（ref 或 detail 里有 id）："
   "update %d 条、delete %d 条，detail=%s / %s"
   % (len(up), len(de), str([r.get("detail") for r in up][:1])[:40],
      str([r.get("detail") for r in de][:1])[:40]),
   s3 == 200 and s4 == 200 and bool(up) and bool(de)
   and all((r.get("detail") or "").strip() not in ("", "null", "{}")
           and "id" in str(r.get("detail")) for r in up + de)
   and all("reason" in str(r.get("detail")) for r in de),
   "update带id=%s delete带reason=%s"
   % (bool(up) and all("id" in str(r.get("detail")) for r in up),
      bool(de) and all("reason" in str(r.get("detail")) for r in de)))

# ===== V3 引用护栏（正控 + 静态） =====
s5, b5 = call("POST", "/admin/deny-reasons", {"reason": "BV在用%d" % TAG},
              token=ROOT)
rid2 = int(((b5 or {}).get("data") or {}).get("id") or 0)
if rid2 and TID:
    psql("UPDATE torrents SET deny_reason_id = %d WHERE id = %d"
         % (rid2, TID))
    s6, _ = call("DELETE", "/admin/deny-reasons/%d" % rid2, None, token=ROOT)
    still = psql("SELECT count(*) FROM torrent_deny_reasons WHERE id = %d"
                 % rid2)
    ok("V3a 正控：被种子引用的理由不许删（http=%s 行仍在=%s），且引用数要出现在"
       "报错文案里" % (s6, still),
       s6 == 400 and still == "1", "http=%s 还在=%s" % (s6, still))
    psql("UPDATE torrents SET deny_reason_id = NULL WHERE id = %d" % TID)
    call("DELETE", "/admin/deny-reasons/%d" % rid2, None, token=ROOT)
src = io.open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                           "apps", "api", "src", "admin_http", "deny.rs"),
              encoding="utf-8").read()
i0 = src.index("async fn deny_reasons_delete")
ok("V3b 静态：删除口那段引用计数不许再 unwrap_or（查询失败=护栏默认放行）",
   "unwrap_or(0)" not in src[i0:], "命中=%d" % src[i0:].count("unwrap_or(0)"))

# ===== V4 暂缓态状态机与留痕 =====
st_before = psql("SELECT approval_status FROM torrents WHERE id = %d" % TID)
s7, _ = call("POST", "/admin/reviews/postpone",
             {"torrent_id": TID, "reason": "BV 暂缓：等证据"}, token=ROOT)
now4 = psql("SELECT approval_status FROM torrents WHERE id = %d" % TID)
ok("V4a 待审(0) → 暂缓(4) 真生效：http=%s 状态 %s→%s" % (s7, st_before, now4),
   s7 == 200 and now4 == "4", "http=%s 现在=%s" % (s7, now4))
s8, _ = call("POST", "/admin/reviews/postpone", {"torrent_id": TID,
                                                "reason": "x"}, token=ROOT)
ok("V4b 正控：已在暂缓态不能再暂缓（状态机守卫，http=%s）" % s8, s8 == 400,
   "http=%s" % s8)
s9, _ = call("POST", "/admin/reviews/resume", {"torrent_id": TID}, token=ROOT)
back0 = psql("SELECT approval_status FROM torrents WHERE id = %d" % TID)
ok("V4c 暂缓(4) → 待审(0) 恢复回常规审核流：http=%s 现在=%s" % (s9, back0),
   s9 == 200 and back0 == "0", "http=%s 现在=%s" % (s9, back0))
lg = psql("SELECT action FROM torrent_operation_logs WHERE torrent_id = %d "
          "AND action IN ('postpone','resume') ORDER BY id" % TID)
ok("V4d 暂缓/恢复都要真进种子操作时间线（这是站长回看处置历史的唯一一处）："
   "logs=%s" % lg, lg == "postpone\nresume" or sorted(lg.split()) ==
   ["postpone", "resume"], "logs=%s" % lg.replace("\n", ","))
rv = io.open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                          "apps", "api", "src", "v4_http.rs"),
             encoding="utf-8").read()
def unguarded(text):
    """留痕 INSERT 的条数 减去 配对了 warn 的条数，再加裸 let _ = 的处数"""
    logs = text.count("INSERT INTO torrent_operation_logs")
    warned = text.count("审核留痕写入失败")
    return max(0, logs - warned) + text.count("let _ = sqlx::query(")


swallow = unguarded(rv)
dec = io.open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                           "apps", "api", "src", "admin_http", "review.rs"),
              encoding="utf-8").read()
dec_sw = unguarded(dec)
ok("V4e 静态：三处审核留痕（decide / postpone / resume）都不许再静默吞错"
   "——判据是「有留痕 INSERT 却没配对 warn」的条数 + 裸 let _ = 的处数"
   "（v4_http %d、review %d）" % (swallow, dec_sw),
   swallow == 0 and dec_sw == 0,
   "v4_http=%d review=%d" % (swallow, dec_sw))

# ===== V9 收尾 =====
cleanup()
RC1 = int(psql("SELECT count(*) FROM torrent_deny_reasons") or -1)
st_now = psql("SELECT approval_status || '|' || coalesce(deny_note, '(空)') "
              "FROM torrents WHERE id = %d" % TID)
ok("V9a 拒绝理由回基线（%d→%d）且无 BV 前缀残留" % (RC0, RC1),
   RC1 == RC0 and psql("SELECT count(*) FROM torrent_deny_reasons "
                       "WHERE reason LIKE 'BV%%'") == "0",
   "RC=%s 探针=%s" % (RC1, psql("SELECT count(*) FROM torrent_deny_reasons "
                               "WHERE reason LIKE 'BV%%'")))
ok("V9b 探针种子已复原成备份里那一行（整行 IS DISTINCT FROM 比对，不依赖被验代码）："
   "现值=%s" % st_now,
   psql("SELECT count(*) FROM torrents t JOIN %s b ON b.id = t.id WHERE NOT "
        "((t.approval_status, coalesce(t.deny_note,'')) IS DISTINCT FROM "
        "(b.approval_status, coalesce(b.deny_note,'')))" % BK) == "1"
   and psql("DROP TABLE IF EXISTS %s; SELECT 1" % BK) == "1",
   "现值=%s 备份见 V0b" % st_now)
ok("V9c 暂缓/恢复各留下一条真操作时间线（这两条是本轮真发生过的动作，不删）",
   psql("SELECT count(*) FROM torrent_operation_logs WHERE torrent_id = %d "
        "AND action IN ('postpone','resume')" % TID) == "2",
   "留痕=%s 条" % psql("SELECT count(*) FROM torrent_operation_logs WHERE "
                     "torrent_id = %d AND action IN ('postpone','resume')"
                     % TID))
summary()
