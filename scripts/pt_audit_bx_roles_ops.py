# -*- coding: utf-8 -*-
"""BX 闸门（批五十九）：结构字典与运维写口的审计余量 + 删除口语义。

覆盖此前零闸门的两族：
  1) 结构字典 CRUD：职务字典（roles）、分区维度（section_kinds）、
     促销价目档（promo-tiers）、等级档规则（admin/classes）；
  2) 运维/删除口：按 IP 解封、任务手动触发、种分类删除、站点规则删除。

批五十八把四族字典的 7 处 `audit(…, None)` 收口后，同形在
`admin_p3_http` 里还剩 11 处；本批一次扫平（含 ops 的 backup_run）。
另三条是本批新立的洞：
  P-221 删促销档位会把它的价目档一起 CASCADE 带走（护栏只数
        promo_purchases，不数 promo_kind_tiers）；
  P-228 删种分类只数 torrents，另四张表（promotions/promo_intents/
        auto_promo_rules/wishlist）都是 NO ACTION ⇒ 撞外键变 500；
  P-229 删站点规则丢掉 rows_affected ⇒ 删不存在的 id 回成功并照写审计。

判据纪律：审计类一律按「本轮水位线 AUD0 之后」取数（历史累计条数会
恒红）；留痕/审计的 ref 直接读 `audit_log`（最深的落库层，不走 API 过滤）；
等级档那一处会真改数据 ⇒ 先建备份表，收尾用整行 `IS DISTINCT FROM` 复原，
复原不依赖被验代码；fail-open 造不出行为红 ⇒ 落静态判据，并把修前命中数
写进读数。探针一律专属前缀（bxro/bxsk/bxkd/BX 分类/BX 规则/203.0.113.x）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bx_roles_ops.py
"""
import io
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, login, ok, summary)

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TAG = int(time.time()) % 100000
RK = "bxro%d" % TAG            # 职务字典 key（小写下划线）
SK = "bxsk%d" % TAG           # 分区维度 kind
KD = "bxkd%d" % TAG           # 促销档位 kind
CN = "BX分类%d" % TAG         # 种分类名
RT = "BX规则%d" % TAG         # 站点规则标题
IP1 = "203.0.113.7%d" % (TAG % 7)     # 探针被封 IP
IP2 = "203.0.113.1%02d" % (TAG % 100)  # 从没封过的 IP
CLASS_ID = 1
NOEXIST = 990000 + TAG % 1000


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:48], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def cnt(t):
    return int(psql("SELECT count(*) FROM %s" % t) or -1)


def aud(action):
    """本轮水位线之后该 action 的全部 ref（NULL 写成 [NULL]）。"""
    return psql(
        "SELECT coalesce(string_agg(x, ' ~ '), '-') FROM "
        "(SELECT coalesce(ref::text, '[NULL]') AS x FROM audit_log "
        "WHERE action = '%s' AND id > %d ORDER BY id) t" % (action, AUD0))


def src(f):
    return io.open(os.path.join(REPO, "apps", "api", "src", f),
                   encoding="utf-8").read()


def cleanup():
    psql("DELETE FROM user_roles WHERE role_key = '%s'" % RK)
    psql("DELETE FROM roles WHERE key = '%s'" % RK)
    psql("DELETE FROM section_kinds WHERE kind = '%s'" % SK)
    psql("DELETE FROM promo_kind_tiers WHERE kind = '%s'" % KD)
    psql("DELETE FROM promo_kinds WHERE kind = '%s'" % KD)
    psql("DELETE FROM auto_promo_rules WHERE name = 'BX探针规则%d'" % TAG)
    psql("DELETE FROM categories WHERE name = '%s'" % CN)
    psql("DELETE FROM site_rules WHERE title = '%s'" % RT)
    psql("DELETE FROM ip_bans WHERE ip = '%s'::inet" % IP1)
    # job_triggers 里那条 ensure_partitions 是合法运维产物（同 audit_log），
    # 只计数不删：worker 会在下一个 tick 消费掉。
    # 等级档：只按备份表整行回写，不依赖被验代码
    bk = psql("SELECT count(*) FROM bx_bk_class_rules")
    if bk != "1":
        print("   备份表行数异常（%s），不回写，人工核" % bk)
    else:
        psql("UPDATE class_rules SET name = b.name, "
             "min_uploaded = b.min_uploaded, "
             "min_download_count = b.min_download_count, "
             "min_seed_hours = b.min_seed_hours, "
             "min_account_age_days = b.min_account_age_days, "
             "demotable = b.demotable, promo_sparks = b.promo_sparks "
             "FROM bx_bk_class_rules b WHERE class_rules.class_id = b.class_id")
        psql("UPDATE user_classes SET name = b.name FROM bx_bk_user_classes b "
             "WHERE user_classes.id = b.id")


def drop_bk():
    psql("DROP TABLE bx_bk_class_rules")
    psql("DROP TABLE bx_bk_user_classes")


# ===== X0 基线 =====
ROLES0, SK0 = cnt("roles"), cnt("section_kinds")
KINDS0, TIERS0 = cnt("promo_kinds"), cnt("promo_kind_tiers")
CATS0, RULES0 = cnt("categories"), cnt("site_rules")
BANS0, JOBS0 = cnt("ip_bans"), cnt("job_triggers")
AUD0 = int(psql("SELECT coalesce(max(id), 0) FROM audit_log") or -1)
CL0 = psql("SELECT name FROM user_classes WHERE id = %d" % CLASS_ID)
psql("DROP TABLE IF EXISTS bx_bk_class_rules")
psql("DROP TABLE IF EXISTS bx_bk_user_classes")
psql("CREATE TABLE bx_bk_class_rules AS SELECT * FROM class_rules "
     "WHERE class_id = %d" % CLASS_ID)
psql("CREATE TABLE bx_bk_user_classes AS SELECT * FROM user_classes "
     "WHERE id = %d" % CLASS_ID)
ok("X0a 前提：等级档 %d 已整行备份（class_rules+user_classes 两表），"
   "收尾按备份复原" % CLASS_ID,
   psql("SELECT count(*) FROM bx_bk_class_rules") == "1"
   and psql("SELECT count(*) FROM bx_bk_user_classes") == "1",
   "现名=%s 审计水位=%d" % (CL0, AUD0))

# ===== X1 职务字典：三处审计要带字符串主键 key =====
s, b = call("POST", "/admin/roles", {"key": RK, "name": "布偶职务%d" % TAG},
            token=ROOT)
ok("X1a 建探针职务：http=%s，roles 行数 %d→%d" % (s, ROLES0, cnt("roles")),
   s == 200 and cnt("roles") == ROLES0 + 1, str(b)[:60])
s2, b2 = call("POST", "/admin/roles", {"key": RK, "name": "重复"}, token=ROOT)
ok("X1b 正控：重复 key 要被拦且库里不许多一行（预检查询失败时旧写法会"
   "当「不重复」放行 ⇒ 撞主键变 500，正是它注释里想避免的那件事）："
   "http=%s 行数=%d" % (s2, cnt("roles")),
   s2 == 400 and cnt("roles") == ROLES0 + 1, str(b2)[:70])
call("PUT", "/admin/roles/" + RK, {"key": RK, "name": "布偶职务%d改" % TAG},
     token=ROOT)
a_add, a_upd = aud("roles.add"), aud("roles.update")
ok("X1c 职务 add/update 的审计要带 key（ref 装不下字符串主键，"
   "extra 是唯一出处）：add=%s update=%s" % (a_add[:52], a_upd[:52]),
   RK in a_add and RK in a_upd, "add=%s upd=%s" % (a_add[:40], a_upd[:40]))
s, b = call("DELETE", "/admin/roles/" + RK, None, token=ROOT)
ok("X1d 探针职务删除 http=%s 且审计 roles.del 带 key（行没了以后这是唯一"
   "出处）" % s, s == 200 and RK in aud("roles.del"), aud("roles.del")[:60])

# ===== X2 分区维度：同样三处 =====
s, b = call("POST", "/admin/section-kinds",
            {"kind": SK, "label": "布偶维度%d" % TAG, "field_type": "text"},
            token=ROOT)
ok("X2a 建探针维度：http=%s 行数 %d→%d" % (s, SK0, cnt("section_kinds")),
   s == 200 and cnt("section_kinds") == SK0 + 1, str(b)[:60])
call("PUT", "/admin/section-kinds/" + SK,
     {"kind": SK, "label": "布偶维度%d改" % TAG, "field_type": "text"},
     token=ROOT)
a_sa, a_su = aud("section_kind.add"), aud("section_kind.update")
ok("X2b 维度 add/update 审计带 kind：%s / %s" % (a_sa[:44], a_su[:44]),
   SK in a_sa and SK in a_su, "add=%s upd=%s" % (a_sa[:36], a_su[:36]))
s, b = call("DELETE", "/admin/section-kinds/" + SK, None, token=ROOT)
ok("X2c 维度删除 http=%s，审计带 kind 与删除前读到的 label"
   % s, s == 200 and SK in aud("section_kind.del"),
   aud("section_kind.del")[:70])

# P-221 同族第二处：section_dict.kind 对 section_kinds 也是 CASCADE
SK2 = "bxs2%d" % TAG
call("POST", "/admin/section-kinds",
     {"kind": SK2, "label": "布偶维度二%d" % TAG, "field_type": "text"},
     token=ROOT)
psql("INSERT INTO section_dict (kind, name, sort) VALUES "
     "('%s', 'BX字典值', 0)" % SK2)
s, b = call("DELETE", "/admin/section-kinds/" + SK2, None, token=ROOT)
ok("X2d 维度只被字典值引用（还没种子用过）时删除要被拦：http=%s、"
   "维度还在=%s、字典值还在=%s（旧护栏只数 torrent_sections ⇒ 回 200，"
   "字典值被 CASCADE 一起带走）"
   % (s, psql("SELECT count(*) FROM section_kinds WHERE kind = '%s'" % SK2),
      psql("SELECT count(*) FROM section_dict WHERE kind = '%s'" % SK2)),
   s == 400 and psql("SELECT count(*) FROM section_dict WHERE kind = '%s'"
                     % SK2) == "1", str(b)[:64])
psql("DELETE FROM section_dict WHERE kind = '%s'" % SK2)
psql("DELETE FROM section_kinds WHERE kind = '%s'" % SK2)

# ===== X3 价目档 upsert 审计 + P-221 连带删除 =====
call("POST", "/admin/promo-kinds",
     {"kind": KD, "label_zh": "布偶档位%d" % TAG, "effect": "half",
      "enabled": False}, token=ROOT)
s, b = call("POST", "/admin/promo-kinds/tiers",
            {"kind": KD, "hours": 24, "price": 9}, token=ROOT)
TIERS1 = cnt("promo_kind_tiers")
ok("X3a 建探针档位 + 1 个价目档：upsert http=%s，tiers %d→%d"
   % (s, TIERS0, TIERS1), s == 200 and TIERS1 == TIERS0 + 1, str(b)[:50])
a_t = aud("promo_tier.upsert")
ok("X3b 价目档 upsert 的审计要带 kind+hours（主键是 (kind,hours)，"
   "ref=None 等于什么都没留）：%s" % a_t[:56],
   KD in a_t and "24" in a_t, a_t[:60])
s, b = call("DELETE", "/admin/promo-kinds/" + KD, None, token=ROOT)
TIERS2 = cnt("promo_kind_tiers")
ok("X3c P-221：档位还挂着 1 个价目档、一份购买都没有 ⇒ 删除必须被拦"
   "（旧写法护栏只数 promo_purchases ⇒ 回 200，价目档被 ON DELETE "
   "CASCADE 静默带走：http=%s tiers %d→%d）" % (s, TIERS1, TIERS2),
   s == 400 and TIERS2 == TIERS1, "%s / tiers=%d" % (str(b)[:60], TIERS2))

# ===== X4 按 IP 解封：假审计 + 审计不带 IP =====
AJ0 = psql("SELECT count(*) FROM audit_log WHERE action='ban.del_by_ip' "
           "AND id > %d" % AUD0)
s, b = call("POST", "/admin/bans/by-ip/delete", {"ip": IP2}, token=ROOT)
AJ1 = psql("SELECT count(*) FROM audit_log WHERE action='ban.del_by_ip' "
           "AND id > %d" % AUD0)
ok("X4a P-225：解封一个从没封过的 IP ⇒ 真删 0 行时**不许**写审计"
   "（旧写法 rows_affected=0 也照样 audit ⇒ 账面虚增「有人封禁处置过」）："
   "http=%s deleted=%s 本轮新增审计 %s→%s 条"
   % (s, str(b)[:40], AJ0, AJ1),
   AJ1 == AJ0, "新增=%s" % (int(AJ1 or 0) - int(AJ0 or 0)))
psql("INSERT INTO ip_bans (id, ip, reason, created_at) VALUES "
     "((SELECT coalesce(max(id), 0) + 1 FROM ip_bans), '%s'::inet, "
     "'BX探针', now())" % IP1)
ok("X4b 前提：探针 IP %s 已封（只这一行是本轮造的）" % IP1,
   psql("SELECT count(*) FROM ip_bans WHERE ip = '%s'::inet" % IP1) == "1",
   "ip_bans %d→%d" % (BANS0, cnt("ip_bans")))
s, b = call("POST", "/admin/bans/by-ip/delete", {"ip": IP1}, token=ROOT)
a_b = aud("ban.del_by_ip")
ok("X4c 真解封 http=%s，审计必须带是哪个 IP（解封是安全动作，"
   "「有人解过封」而不知道解的谁＝没有审计）：%s" % (s, a_b[:56]),
   s == 200 and IP1 in a_b, a_b[:60])

# ===== X5 等级档 PUT：审计带 id + 双表半包 =====
row = psql("SELECT min_uploaded||'|'||min_download_count||'|'||"
           "min_seed_hours||'|'||min_account_age_days||'|'||"
           "demotable||'|'||promo_sparks FROM class_rules "
           "WHERE class_id = %d" % CLASS_ID)
v = row.split("|")
s, b = call("PUT", "/admin/classes/%d" % CLASS_ID, {
    "name": "LV1改%d" % TAG, "min_uploaded": int(v[0]),
    "min_download_count": int(v[1]), "min_seed_hours": int(v[2]),
    "min_account_age_days": int(v[3]),
    # psql 的布尔渲染本机是 "true"/"false"（PG16），旧写法只认 "t" ⇒
    # 读到的 true 会被当 False 写回去，探针自己就把这一档的自动降级关掉了。
    # （十一轮在 AI 闸门上实测到同一族读法缺陷；现网 class 3 读到 false 而
    #  0277 出厂为 true、又无迁移改过，来源未定，最可能就是这类
    #  「客户端省略该列 ⇒ 后端按缺省 false 静默反做」。）
    "demotable": v[4] in ("t", "true"),
    "promo_sparks": int(v[5])}, token=ROOT)
ok("X5a0 正控：整对象 PUT 不许顺手改掉 demotable（省略/误读都会静默反做）",
   psql("SELECT demotable::text FROM class_rules WHERE class_id = %d"
        % CLASS_ID) == v[4],
   "%s -> %s" % (v[4], psql("SELECT demotable::text FROM class_rules "
                            "WHERE class_id = %d" % CLASS_ID)))
a_c = aud("admin_class_put")
ok("X5a 等级档改名的审计要带档位 id（这是数字主键，ref 本该装得下）："
   "http=%s ref=%s" % (s, a_c[:50]), s == 200 and str(CLASS_ID) in a_c,
   a_c[:60])
ok("X5b 正控：两表确实一起改了（class_rules 与 user_classes 同名）",
   psql("SELECT count(*) FROM class_rules cr JOIN user_classes uc "
        "ON uc.id = cr.class_id WHERE cr.class_id = %d AND "
        "cr.name IS DISTINCT FROM uc.name" % CLASS_ID) == "0",
   psql("SELECT cr.name||'/'||uc.name FROM class_rules cr JOIN "
        "user_classes uc ON uc.id = cr.class_id WHERE cr.class_id = %d"
        % CLASS_ID))

# ===== X6 任务手动触发：审计带任务名 =====
s, b = call("POST", "/admin/jobs/run", {"job": "ensure_partitions"},
            token=ROOT)
a_j = aud("job_trigger")
ok("X6a 手动触发任务的审计要带任务名（「有人手动跑过任务」不知道跑了哪个"
   "＝运维现场没了）：http=%s ref=%s" % (s, a_j[:48]),
   s == 200 and "ensure_partitions" in a_j, a_j[:60])
s2, b2 = call("POST", "/admin/jobs/run", {"job": "bx_不存在任务"}, token=ROOT)
ok("X6b 正控：目录校验还在（未知任务名要被拦，否则永远 pending）：http=%s"
   % s2, s2 == 400, str(b2)[:60])

# ===== X7 种分类删除口：护栏只数一张表 =====
s, b = call("POST", "/admin/categories", {"name": CN}, token=ROOT)
CID = psql("SELECT id FROM categories WHERE name = '%s'" % CN)
ok("X7a 建探针分类：http=%s id=%s 行数 %d→%d"
   % (s, CID, CATS0, cnt("categories")),
   s == 200 and cnt("categories") == CATS0 + 1, str(b)[:50])
psql("INSERT INTO auto_promo_rules (position, name, kind, category_id) "
     "VALUES (99, 'BX探针规则%d', 'free', %s)" % (TAG, CID))
s, b = call("DELETE", "/admin/categories/%s" % CID, None, token=ROOT)
ok("X7b P-228：分类被自动促销规则引用 ⇒ 删除要被友好拒掉，不能撞外键成 "
   "500（旧护栏只数 torrents）：http=%s msg=%s"
   % (s, str(b)[:70]),
   s == 400, "http=%s" % s)

# ===== X8 站点规则删除口：假成功 + 假审计 =====
s, b = call("POST", "/admin/rules", {"title": RT, "body": "BX正文"},
            token=ROOT)
RID = psql("SELECT id FROM site_rules WHERE title = '%s'" % RT)
ok("X8a 建探针规则：http=%s id=%s" % (s, RID),
   s == 200 and RID != "", str(b)[:50])
AJ2 = psql("SELECT count(*) FROM audit_log WHERE action='rules.delete' "
           "AND id > %d" % AUD0)
s, b = call("DELETE", "/admin/rules/%d" % NOEXIST, None, token=ROOT)
AJ3 = psql("SELECT count(*) FROM audit_log WHERE action='rules.delete' "
           "AND id > %d" % AUD0)
ok("X8b P-229：删不存在的规则 id=%d 要 404，且不写审计（旧写法丢 "
   "rows_affected ⇒ 回 {\"ok\":true} 并照写一条「删过 99xxxx」的假账）："
   "http=%s 本轮删除审计 %s→%s 条" % (NOEXIST, s, AJ2, AJ3),
   s == 404 and AJ3 == AJ2, "%s / 新增=%s" % (str(b)[:44],
                                              int(AJ3 or 0) - int(AJ2 or 0)))
s, b = call("DELETE", "/admin/rules/%s" % RID, None, token=ROOT)
ok("X8c 正控：真删探针规则 http=%s、行数 %d→%d，审计带 id"
   % (s, RULES0, cnt("site_rules")),
   s == 200 and cnt("site_rules") <= RULES0, str(b)[:44])

# ===== X9 静态：审计与护栏写法收口（红证取修前命中数）=====
# 本批把 section_kinds 的删除口、ops 的任务域、rules_cats 的分类域各拆出
# 一个文件，判据要把新文件一起圈进来——否则「代码搬走了」会让静态判据空绿。
FILES = ("admin_p3_http/roles.rs", "admin_p3_http/section_kinds.rs",
         "admin_p3_http/section_kinds_del.rs",
         "admin_p3_http/promo_tiers.rs", "admin_p3_http/ban_ip.rs",
         "admin_p3_http/classes_admin.rs", "admin_p3_http/ops.rs",
         "admin_p3_http/ops_jobs.rs", "admin_p3_http/promo_kinds.rs",
         "staff_http/rules_cats.rs", "staff_http/categories_admin.rs")
bad_audit, bad_guard = [], []
# 判据要打得准：`unwrap_or(false)/(0)` 用在请求体默认值上是合法写法
# （`body.enabled.unwrap_or(false)`），只有「紧跟在 .await 之后」才是把
# 查询失败当成放行；读口同理，只认 to_value(...).unwrap_or_default()。
RE_FAIL = re.compile(r"\.await\s*\n\s*\.unwrap_or\((?:false|0)\)")
RE_READ = re.compile(r"to_value\([^)]*\)\s*\n?\s*\.unwrap_or_default\(\)")
for f in FILES:
    t = src(f)
    if RE_FAIL.search(t):
        bad_guard.append("%s:await→unwrap_or" % f)
    if RE_READ.search(t):
        bad_guard.append("%s:to_value→unwrap_or_default" % f)
    if ", None)" in t:
        bad_audit.append("%s:, None)" % f)
ok("X9a 静态：十一个文件里护栏/读口不许再有 fail-open 写法"
   "（修前命中 6 处：roles.rs 预检查 unwrap_or(false)、roles/section_kinds "
   "读口 unwrap_or_default、ops.rs 队列计数 unwrap_or(0)、"
   "rules_cats 分类计数 unwrap_or(0) 与版本页读口）——命中=%s"
   % bad_guard, not bad_guard, "命中=%d 处" % len(bad_guard))
ok("X9b 静态：审计调用不许再传 None 而无对象标识（修前 admin_p3_http "
   "剩 11 处：roles 3、section_kinds 3、promo_tiers 1、ban_ip 1、"
   "classes_admin 1、ops 2）——命中=%s" % bad_audit, not bad_audit,
   "命中=%d 处" % len(bad_audit))

# ===== X10 收尾 =====
cleanup()
ok("X10a 探针行全清，八张表回基线：职务 %d→%d、维度 %d→%d、档位 %d→%d、"
   "价目档 %d→%d、分类 %d→%d、规则 %d→%d、封禁 %d→%d"
   % (ROLES0, cnt("roles"), SK0, cnt("section_kinds"), KINDS0,
      cnt("promo_kinds"), TIERS0, cnt("promo_kind_tiers"), CATS0,
      cnt("categories"), RULES0, cnt("site_rules"), BANS0, cnt("ip_bans")),
   cnt("roles") == ROLES0 and cnt("section_kinds") == SK0
   and cnt("promo_kinds") == KINDS0 and cnt("promo_kind_tiers") == TIERS0
   and cnt("categories") == CATS0 and cnt("site_rules") <= RULES0
   and cnt("ip_bans") == BANS0,
   "R=%s S=%s K=%s T=%s C=%s RU=%s B=%s"
   % (cnt("roles"), cnt("section_kinds"), cnt("promo_kinds"),
      cnt("promo_kind_tiers"), cnt("categories"), cnt("site_rules"),
      cnt("ip_bans")))
DIFF = psql("SELECT count(*) FROM class_rules cr JOIN "
            "bx_bk_class_rules b ON b.class_id = cr.class_id "
            "WHERE cr.class_id = %d AND cr IS DISTINCT FROM b" % CLASS_ID)
ok("X10b 等级档 %d 已按备份整行复原（IS DISTINCT FROM，不依赖被验代码）："
   "差异行=%s，名字=%s" % (CLASS_ID, DIFF, CL0),
   DIFF == "0" and psql("SELECT name FROM user_classes WHERE id = %d"
                        % CLASS_ID) == CL0,
   "diff=%s 现名=%s" % (DIFF, psql("SELECT name FROM user_classes "
                                  "WHERE id = %d" % CLASS_ID)))
ok("X10c 库里没有 bx 前缀的探针残留（含第二个探针维度 bxs2 与它的字典值）",
   psql("SELECT count(*) FROM roles WHERE key LIKE 'bxro%%'") == "0"
   and psql("SELECT count(*) FROM section_kinds WHERE kind LIKE 'bxs%%'")
   == "0" and psql("SELECT count(*) FROM section_dict WHERE kind "
                   "LIKE 'bxs%%'") == "0"
   and psql("SELECT count(*) FROM promo_kinds WHERE kind "
            "LIKE 'bxkd%%'") == "0"
   and psql("SELECT count(*) FROM categories WHERE name LIKE 'BX%%'") == "0"
   and psql("SELECT count(*) FROM site_rules WHERE title LIKE 'BX%%'") == "0"
   and psql("SELECT count(*) FROM auto_promo_rules WHERE name LIKE "
            "'BX%%'") == "0",
   "残=%s/%s/%s/%s/%s/%s" % (
       psql("SELECT count(*) FROM roles WHERE key LIKE 'bxro%%'"),
       psql("SELECT count(*) FROM section_kinds WHERE kind LIKE 'bxs%%'"),
       psql("SELECT count(*) FROM section_dict WHERE kind LIKE 'bxs%%'"),
       psql("SELECT count(*) FROM promo_kinds WHERE kind LIKE 'bxkd%%'"),
       psql("SELECT count(*) FROM categories WHERE name LIKE 'BX%%'"),
       psql("SELECT count(*) FROM site_rules WHERE title LIKE 'BX%%'")))
# 备份表要留到整行比对之后才删（上一批跑绿才发现：先 DROP 再比对＝判据
# 读的是一个已经不存在的东西，红得没有道理）
drop_bk()

summary()
