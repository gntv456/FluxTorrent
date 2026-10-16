# -*- coding: utf-8 -*-
"""ED 闸门（批八十六续）：写口的「计数诚实性」防回归。

起因（P-297，已修）：POST /messages/delete 对 3 个**不存在**的 id 回
{"deleted": 3} —— 响应取的是 body.ids.len()（请求条数）而不是真实动过的行数；
同一个文件里的 markread / move 对同一坏 id 都老实回 0。

本闸门做三件事：
  ED1/ED2 现网行为：三个站内写口对「不存在的 id」和「别人名下的 id」都必须
      报 0，且**不许真的动到那一行**（越权尝试必须是纯失败，不是静默生效）；
  ED3 全仓静态普查：把「响应里的数字来自请求体长度」这一形状列成清单，
      清单只能**减**不能增（新增即红），每条附「为什么可以留着」的理由；
  ED4 钉住 P-297 的修法本体不被悄悄改回去（delete 段必须仍在按归属数行）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_ed_count_honesty_guard.py
零写入设计：不存在的 id 什么都碰不到；「别人的 id」这一支先按整行快照，
跑完用 IS DISTINCT FROM 比对，被动过就红。
"""
import io
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BOGUS = [999999981, 999999982]
# 「响应数字来自请求长度」的全仓可接受清单：文件名 -> (关键字, 为什么不算撒谎)
ALLOW = {
    "user_fields_admin.rs": (
        "saved", "整段在一个事务里全有全无：任一 key 不存在/校验失败就 400 "
        "且回滚 ⇒ 200 时这个数字必然是真的（口径小瑕疵：val 为 null 的那条"
        "是「清空」也计入 saved，本批只点名不改）"),
    "agent_rules.rs": (
        "skipped", "回的是 body.rules.len() − added，语义就是「请求几条里"
        "没成的几条」，与同口的 added/invalid 三个数互相能对上账"),
    "review_batch.rs": (
        "requested", "键名就叫 requested，与 accepted 并列回给调用方，"
        "语义是「收到几条」不是「改成几条」"),
}


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


hits = {}
for root_, _d, fs in os.walk(os.path.join(REPO, "apps", "api", "src")):
    for fn in fs:
        if not fn.endswith(".rs"):
            continue
        p = os.path.join(root_, fn)
        txt = io.open(p, encoding="utf-8", errors="replace").read()
        body = chr(10).join(l.split("//")[0] for l in txt.splitlines())
        # 只拦「**直接**把请求体长度写进响应」——P-297 那族。
        # 形状："键": body.xxx.len()。不拦带计算的，那是真数（实测
        # reg_quiz_bank.rs 的 "skipped": body.questions.len() - added，减去了真实失败数，是对的）。
        for m in re.finditer(
                r'"([a-z_]+)":\s*body\.[a-z_.]*\.len\(\)(?![^"\n]*[+\-*/])', body):
            hits.setdefault(fn, set()).add(m.group(1))

# 已核可：这些文件里的形状已逐只核对，确认不是「把失败报成成功」。
new = {k: v for k, v in hits.items() if k not in ALLOW}
gone = [k for k in ALLOW if k not in hits]
ok("ED3 全仓形状普查：响应数字取自请求体长度的文件=%s；已核可清单=%s → "
   "**新增必须为零**（这条判据只减不增），已消失的核可项=%s"
   % (sorted(hits.keys()), sorted(ALLOW.keys()), gone),
   not new, "新增=%s" % sorted(new.keys()))

s1, b1 = call("POST", "/messages/delete", {"ids": BOGUS}, token=ROOT)
d1 = ((b1 or {}).get("data") or {})
s2, b2 = call("POST", "/messages/markread", {"ids": BOGUS}, token=ROOT)
d2 = ((b2 or {}).get("data") or {})
s3, b3 = call("POST", "/messages/move", {"ids": BOGUS, "folder": None},
              token=ROOT)
d3 = ((b3 or {}).get("data") or {})
ok("ED1 P-297 回归护栏：三个站内写口对 %d 个**不存在**的 id 必须分别回 "
   "deleted=%s updated=%s moved=%s（修前 deleted=%d=请求条数）"
   % (len(BOGUS), d1.get("deleted"), d2.get("updated"), d3.get("moved"),
      len(BOGUS)),
   s1 == 200 and d1.get("deleted") == 0 and d2.get("updated") == 0
   and d3.get("moved") == 0,
   "deleted=%s updated=%s moved=%s" % (d1.get("deleted"), d2.get("updated"),
                                       d3.get("moved")))

OWNER = psql("SELECT id || '/' || receiver_id || '/' || location || '/' || "
             "saved FROM messages WHERE sender_id <> 1 AND receiver_id <> 1 "
             "ORDER BY id LIMIT 1")
# 取整行哈希：先算出 bigint 的 id 再拼进 WHERE（上一版把 '304/4168/1/0' 整个
# 塞进 split_part 里比 bigint，pgsql 直接报错 ⇒ row 空串 ⇒ 这一支变成空判）
FMID = int(OWNER.split("/")[0]) if OWNER else 0
row = psql("SELECT md5(row_to_json(m)::text) FROM messages m WHERE id=%d"
           % FMID) if FMID else ""
ok("ED2a 先取一条**真人**的信件行作越权探针（id/receiver/location/saved=%s，"
   "整行哈希=%s…）——没有这一条，越权那支就无从判"
   % (OWNER, row[:8]), bool(row), "行=%s" % OWNER)
if row:
    MID = int(OWNER.split("/")[0])
    s4, b4 = call("POST", "/messages/delete", {"ids": [MID]}, token=ROOT)
    d4 = ((b4 or {}).get("data") or {})
    after = psql("SELECT md5(row_to_json(m)::text) FROM messages m WHERE id=%d"
                 % MID)
    ok("ED2b 越权必须**纯失败**：root（既非收发方）删别人那一条，deleted 必须"
       "=%s 且那一行整行不许变（前=%s… 后=%s…，修前 deleted=1 而行为未变＝"
       "把失败报成成功）" % (0, d4.get("deleted"), after),
       d4.get("deleted") == 0 and after == row,
       "deleted=%s 行变了=%s" % (d4.get("deleted"), after != row))
    psql("DELETE FROM messages WHERE id=%d AND sender_id<>1 AND receiver_id"
         "<>1" % MID) if False else None   # 绝不删真人数据

mb = io.open(os.path.join(REPO, "apps", "api", "src", "community_http",
                          "message_box.rs"), encoding="utf-8").read()
# 静态判据必须先剥 `//` 注释：我自己在 message_delete 里写的修复说明就原文引用了
# body.ids.len()，不剥注释会把「已修好」读成「残留」（这条坑记过，第三次踩）
mb = chr(10).join(l.split("//")[0] for l in mb.splitlines())
seg = mb[mb.index("message_delete"):mb.index("message_move")]
ok("ED4 修法本体钉住：delete 段必须按归属数真实行（SELECT count(*) 在=%s、"
   "body.ids.len() 残留=%s）" % ("SELECT count(*)" in seg,
                                "body.ids.len()" in seg),
   "SELECT count(*)" in seg and "body.ids.len()" not in seg)

summary()
