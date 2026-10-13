# -*- coding: utf-8 -*-
"""闸门 BU：版块结构（分区 CRUD + 重排）与版主任命链（/admin/forums 五口 +
/admin/forums/{id}/mods 两口 + /admin/forum-categories 四口）。常驻闸门零命中
（AC/批二十八 打的是「读侧板块权限」与「帖子面」，没打过这条结构/任命链）。

未修版红证取自 `api:aa1013y`，四条：
  - **P-210** `POST /admin/forums/{不存在的 fid}/mods` 不查版块存在性，直接撞
    `forum_mods_forum_id_fkey` ⇒ 对外是 **500 内部错误**，站长看不出是「版块不存在」。
    同文件的版块 create 早就在调 `ensure_category_exists`（forum_check.rs），
    这两张版主口没照做——又是「一处先例、另一处漏」。
  - **P-211** 重复任命走 `ON CONFLICT DO NOTHING` 后既不报告也没区分：回一个
    像新建成功的体 **并照写一条 `forum.mod_add` 审计**；反方向 `DELETE`
    不判 `rows_affected`，摘一个不存在的任命也回 200 并写 `forum.mod_remove`
    ⇒ 授权面出现「假成功 + 假审计」。
  - **P-212** `forum.mod_add/mod_remove` 的审计 ref 只有 uid，不带 forum_id ⇒
    「谁被任命/摘掉**哪个版块**的版主」读不出来（与 P-202/204/208 同族第五处）。
  - **P-214** 分区名 `name` 既无长度上限也无形状约束（库里唯一约束只管大小写与
    trim 后的重复）⇒ 一条 5000 字的分区名会写进去并撑破左栏徽标与版块表头
    （P-196 的 note 无上限同族）。

纵深一条（红证演示不了，按读码 + 修复记录）：`forum_admin_delete` 统计主题数用
`.unwrap_or(0)` ⇒ 那条查询一旦失败，「非空版块必须 force=true」这道**防误删整版
内容**的闸门就静默放行。修法是把错误传出去；BU 用静态判据 U4b 钉住「这个危险
支路里不许再有 unwrap_or」，并用 U4a 正控演示 force 闸门本身成立。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bu_forum_admin.py
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
CAT = "bu-cat%d" % TAG
FID = [0]
CID = [0]
FID0 = [0]


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:52], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


def audit_rows(prefix):
    d = (call("GET", "/admin/audit?q=%s&limit=80" % prefix, None,
              token=ROOT)[1] or {}).get("data") or []
    return [r for r in d
            if str(r.get("created_at") or "").replace("T", " ")[:19]
            >= T0[:19]]


def cleanup():
    if FID[0]:
        psql("DELETE FROM topics WHERE forum_id = %d" % FID[0])
        psql("DELETE FROM forum_mods WHERE forum_id = %d" % FID[0])
        psql("DELETE FROM forums WHERE id = %d" % FID[0])
    if FID0[0]:
        psql("DELETE FROM forum_mods WHERE forum_id = %d" % FID0[0])
    # 只删本轮新建的分区（U1a 那条超长名也是一行 INSERT 拿到的 id，
    # 不能只按 CID[0] 收——否则闸门自己攒残料）
    now_ids = set(int(x) for x in psql("SELECT string_agg(id::text, ',') "
                      "FROM forum_categories").split(",") if x)
    for extra in sorted(now_ids - IDS0):
        psql("DELETE FROM forums WHERE category_id = %d" % extra)
        psql("DELETE FROM forum_categories WHERE id = %d" % extra)


import atexit  # noqa: E402

atexit.register(cleanup)

T0 = psql("SELECT to_char(now(), 'YYYY-MM-DD HH24:MI:SS.US')")
CAT0 = int(psql("SELECT count(*) FROM forum_categories") or -1)
FRM0 = int(psql("SELECT count(*) FROM forums") or -1)
MOD0 = int(psql("SELECT count(*) FROM forum_mods") or -1)
# 探针版主：一个在册普通成员（不建号，挑一个 class<90 的活账号，收尾不动它）
UID = int(psql("SELECT min(id) FROM users WHERE status < 2 "
               "AND class_id < 90") or 0)
IDS0 = set(int(x) for x in psql("SELECT string_agg(id::text, ',') "
                  "FROM forum_categories").split(",") if x)
ok("U0a 基线：分区 %d、版块 %d、版主 %d 行；版主探针 uid=%d"
   % (CAT0, FRM0, MOD0, UID), CAT0 >= 0 and FRM0 >= 0 and UID > 0,
   "CAT=%s FRM=%s UID=%s" % (CAT0, FRM0, UID))

# ===== U1 分区名长度（P-214） =====
s, b = call("POST", "/admin/forum-categories",
            {"name": "长" * 400, "sort": 900}, token=ROOT)
long_in = psql("SELECT count(*) FROM forum_categories WHERE length(name) > 60")
ok("U1a 400 字的分区名必须被拒（现在无长度上限：http=%s 库里超长行=%s）"
   % (s, long_in), s == 400 and long_in == "0", "http=%s 超长=%s" % (s, long_in))
s2, b2 = call("POST", "/admin/forum-categories",
              {"name": CAT, "sort": 900}, token=ROOT)
CID[0] = int(((b2 or {}).get("data") or {}).get("id") or 0)
s3, _ = call("POST", "/admin/forum-categories",
             {"name": "测" * 60, "sort": 901}, token=ROOT)
ok("U1b 正控：分区名 1-60 **字符**放行（60 个汉字=180 字节，按字节判会误伤），"
   "探针分区已建（id=%s）；60 字那条 http=%s" % (CID[0], s3),
   s2 == 200 and CID[0] > 0 and s3 == 200,
   "60 汉字 http=%s" % s3)
psql("DELETE FROM forum_categories WHERE name = '%s'" % ("测" * 60))

# ===== U2 版主任命链（P-210 / P-211 / P-212） =====
s4, b4 = call("POST", "/admin/forums", {"name": "bu-forum%d" % TAG,
                                       "descr": "BU 探针版块",
                                       "category_id": CID[0], "sort": 5},
              token=ROOT)
FID[0] = int(((b4 or {}).get("data") or {}).get("id") or 0)
ok("U2a 前提：探针版块建得起来（分区 id 挂着，http=%s fid=%s）"
   % (s4, FID[0]), s4 == 200 and FID[0] > 0, "fid=%s" % FID[0])

UNAME = psql("SELECT username FROM users WHERE id = %d" % UID)
s5, b5 = call("POST", "/admin/forums/99999999/mods", {"username": UNAME},
              token=ROOT)
ok("U2b 不存在的版块号任命版主必须给可读的 4xx（修前撞外键成 500）："
   "http=%s msg=%s" % (s5, str((b5 or {}).get("message", ""))[:24]),
   400 <= s5 < 500, "http=%s" % s5)

s6, b6 = call("POST", "/admin/forums/%d/mods" % FID[0],
              {"username": UNAME}, token=ROOT)
m1 = int(psql("SELECT count(*) FROM forum_mods WHERE forum_id = %d "
              "AND user_id = %d" % (FID[0], UID)))
ok("U2c 正控：真实版块 + 真实成员任命成功，库里确有 1 行（http=%s 行=%d）"
   % (s6, m1), s6 == 200 and m1 == 1, "http=%s 行=%d" % (s6, m1))

s7, b7 = call("POST", "/admin/forums/%d/mods" % FID[0],
              {"username": UNAME}, token=ROOT)
d7 = (b7 or {}).get("data") or {}
added_flag = d7.get("added") if isinstance(d7, dict) else None
ok("U2d 重复任命必须**报告没新建**（added=false），不能再写一条 mod_add 审计"
   "（修前回一个像新建成功的体、审计照写）：http=%s body=%s"
   % (s7, str(d7)[:48]),
   added_flag is False,
   "added=%s 审计条数=%d" % (added_flag, len(audit_rows("forum.mod_add"))))
n_add = len(audit_rows("forum.mod_add"))
ok("U2e 审计条数与真实新建一致：本轮 mod_add 只该有 1 条（第一次任命）"
   "——重复任命不留账=%d 条" % n_add, n_add == 1, "mod_add=%d 条" % n_add)

s8, _ = call("DELETE", "/admin/forums/%d/mods/%d" % (FID[0], 99999999),
             None, token=ROOT)
ok("U2f 摘一个根本不存在的任命要报错，不能 200 还写一条 mod_remove 审计："
   "http=%s" % s8, 400 <= s8 < 500, "http=%s" % s8)
rm_rows = audit_rows("forum.mod_remove")
ok("U2g 撤销方向同判据：本轮 mod_remove 只该有 1 条（下面 U2h 才会真摘）=%d 条"
   % len(rm_rows), len(rm_rows) == 0, "mod_remove=%d 条" % len(rm_rows))

# 审计要带 forum_id（P-212）
add_rows = audit_rows("forum.mod_add")
has_fid = all("forum_id" in str(r.get("detail")) for r in add_rows)
ok("U2h 版主任命审计要带 forum_id（ref 只有 uid ⇒「哪个版块」读不出）："
   "detail=%s" % str([r.get("detail") for r in add_rows][:1])[:60],
   bool(add_rows) and has_fid, "带 forum_id=%s 条数=%d"
   % (has_fid, len(add_rows)))

s9, _ = call("DELETE", "/admin/forums/%d/mods/%d" % (FID[0], UID),
             None, token=ROOT)
rm2 = audit_rows("forum.mod_remove")
ok("U2i 真摘成功（http=%s）且留一条带 forum_id 的撤销审计（本轮 %d 条）"
   % (s9, len(rm2)), s9 == 200 and len(rm2) == 1
   and "forum_id" in str(rm2[0].get("detail")),
   "http=%s 条数=%d" % (s9, len(rm2)))

# ===== U3 分区重排（批量原子性的对照组） =====
ids = [int(x) for x in psql("SELECT string_agg(id::text, ',') FROM ("
           "SELECT id FROM forum_categories ORDER BY id) x").split(",") if x]
rev = list(reversed(ids))
s10, _ = call("POST", "/admin/forum-categories/reorder", {"ids": rev},
              token=ROOT)
seq = psql("SELECT string_agg(id::text, ',') FROM (SELECT id FROM "
           "forum_categories ORDER BY sort, id) x")
ok("U3a 重排要真按传入顺序落库（单条 UPDATE 的原子性对照组，正控）："
   "http=%s 现序=%s 期望=%s" % (s10, seq, ",".join(map(str, rev))),
   s10 == 200 and seq == ",".join(map(str, rev)),
   "seq=%s" % seq)
s11, _ = call("POST", "/admin/forum-categories/reorder", {"ids": [1, 1, 2]},
              token=ROOT)
ok("U3b 正控：重复 id 要整批拒（http=%s）" % s11, s11 == 400, "http=%s" % s11)

# ===== U4 删除版块：force 闸门 + 吞错静态判据 =====
psql("INSERT INTO topics (forum_id, user_id, title) VALUES (%d, %d, "
     "'BU 探针主题 %d')" % (FID[0], UID, TAG))
tcnt = psql("SELECT count(*) FROM topics WHERE forum_id = %d" % FID[0])
s12, b12 = call("DELETE", "/admin/forums/%d" % FID[0], None, token=ROOT)
still = psql("SELECT count(*) FROM forums WHERE id = %d" % FID[0])
ok("U4a 非空版块（%s 个主题）删除必须显式 force=true：无 force 应 400 且版块仍在"
   "（http=%s msg=%s 版块=%s）"
   % (tcnt, s12, str((b12 or {}).get("message", ""))[:22], still),
   s12 == 400 and still == "1", "http=%s 版块=%s" % (s12, still))
s13, _ = call("DELETE", "/admin/forums/%d?force=true" % FID[0], None,
              token=ROOT)
gone = psql("SELECT count(*) FROM forums WHERE id = %d" % FID[0])
tp = psql("SELECT count(*) FROM topics WHERE forum_id = %d" % FID[0])
ok("U4b 正控：force=true 真删（http=%s 版块=%s 主题=%s，主题随 FK 级联清空）"
   % (s13, gone, tp), s13 == 200 and gone == "0" and tp == "0",
   "http=%s 版块=%s 主题=%s" % (s13, gone, tp))
FID[0] = 0
src = io.open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                           "apps", "api", "src", "admin_http",
                           "forum_mods.rs"), encoding="utf-8").read()
i0 = src.index("async fn forum_admin_delete")
i1 = src.index("#[post(\"/admin/forums/{id}/mods\")]")
ok("U4c 静态：删版块那段不许再用 unwrap_or 把「主题计数」失败当成 0"
   "（那等于静默放行级联删整版内容）",
   "unwrap_or(0)" not in src[i0:i1], "该段 unwrap_or(0) 命中=%d"
   % src[i0:i1].count("unwrap_or(0)"))

# ===== U9 收尾 =====
cleanup()
CAT1 = int(psql("SELECT count(*) FROM forum_categories") or -1)
FRM1 = int(psql("SELECT count(*) FROM forums") or -1)
MOD1 = int(psql("SELECT count(*) FROM forum_mods") or -1)
ok("U9a 探针分区/版块/任命已清：分区 %d→%d、版块 %d→%d、版主 %d→%d"
   % (CAT0, CAT1, FRM0, FRM1, MOD0, MOD1),
   CAT1 == CAT0 and FRM1 == FRM0 and MOD1 == MOD0,
   "CAT=%s FRM=%s MOD=%s" % (CAT1, FRM1, MOD1))
ok("U9b 库里没有残留的超长分区名与 bu 前缀分区",
   psql("SELECT count(*) FROM forum_categories WHERE length(name) > 60 "
        "OR name LIKE 'bu-cat%%'") == "0",
   "残留=%s" % psql("SELECT count(*) FROM forum_categories WHERE "
                  "length(name) > 60 OR name LIKE 'bu-cat%%'"))
summary()
