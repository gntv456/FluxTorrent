# -*- coding: utf-8 -*-
"""LW：闸门夹具把站点内容面吃掉了（P-444）。

实测（2026-10-27，现网只读取数）：过审种子 213 条里 **203 条是闸门夹具**
（`Audit.Pack.深测` 一个名字就 93 条，10-02 起每次跑加一条），真内容 10 条。
公开列表默认按存活过滤（seeders=0 落进 hidden_dead），于是游客看到「最新种子」
是空的——站点的内容面被审计流程自己占满。

根因是闸门的收尾纪律，不是产品代码：造种的闸门走**真实发布接口**
（`upload_torrent()` 或 INSERT INTO torrents）却不清场。仓里已有共识
「闸门自己探针数据不收尾会污染同族判据（收尾删 + 残留断言 + 开局自愈）」，
这条在种子上没落实——因为删一条种要顾及子表，写闸门的人图省事留着了。

判据：
  LW1 正控：「同名被反复上传」这个指纹必须抓得到（现网 93 次）
  LW2 活体：过审目录里的夹具数必须为 0 ⇒ **现在红 203，就是缺陷本体**
  LW3 反空转：真内容条数必须 > 0（否则「夹具清零」可以靠把站删空达成）
  LW4 静态：造种且无收尾的脚本集合必须等于台账（多一个红、少一个也红）
  LW5 账面：「最新」第一页（按会员看到的排序）里夹具条数必须为 0
清理工具：`scripts/pt_fixtures_purge.py`（默认 dry-run，--apply 才删；
先备份整行、再按 NO ACTION 阻塞点逐个报数）。删现网种子是破坏性动作，
由站长拍板执行，本闸门只钉账面。
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

FIXED = ("(name = 'Audit.Pack.深测' OR name LIKE 'AuditB.Free.%%' "
         "OR name LIKE 'AuditC.Review2.%%' OR name LIKE 'pt_audit_alive.%%' "
         "OR name LIKE 'deep_alive.%%' OR (name ~* "
         "'audit|deep_alive|e2est|emul_|probe_|dummy' AND seeders = 0))")
# 造种签名与收尾签名。三个签名都是实测补出来的，缺一就漏报：
#   · `upload_torrent(` —— 走真实发布接口（a_publish / c_review / b_billing…）
#   · `probe_torrent(`  —— 各闸门自带的造种助手（an/ao/aq/ar/deep_walk/vip_walk）
#   · `insert into torrents` —— 直接落库，**必须不分大小写**（*_base 用的是小写）
MAKE_RX = re.compile(
    r"upload_torrent\(|probe_torrent\(|insert\s+into\s+torrents\b",
    re.I)
# 回收签名。`cleanup(` 光看名字太宽（本仓有几支闸门自己的 cleanup() 根本不碰
# 种子，第一版因此把 aq/ar 两支洗白了）。an_base 的 cleanup() 是真回收——
# 它把 `DELETE FROM torrents` 攒在共享 CLEAN 列表里倒序执行，所以只有
# **「从 an_base 导入 cleanup」且真的调用了**才算收尾。
TEARDOWN_RX = re.compile(r"delete\s+from\s+torrents\b|purge_torrents\(|"
                        r"note_torrent\(|/admin/torrents", re.I)


def an_base_teardown(body):
    """an_base 的 cleanup() 是不是被这支闸门导入并调用。

    导入是多行括号写法（`from pt_audit_an_base import (` 换行列出名字），
    所以不能拿一行正则判——第一版因此把 an_team_contract 误留在台账外/内摇摆。
    """
    i = body.find("from pt_audit_an_base import")
    if i == -1:
        return False
    return "cleanup" in body[i:i + 400] and "cleanup()" in body


LOCAL_INSERT = re.compile(r"insert\s+into\s+torrents\b", re.I)


def has_teardown(body):
    """自身有回收签名，或**造法完全来自已登记的底座模块**。

    分两种造法差别对待（第一版一律算一跳，把「自己 INSERT 却指望邻居有
    DELETE」的闸门也洗白了）：
      · 文件里出现本地 `INSERT INTO torrents` ⇒ 必须自带回收，邻居不算；
      · 只通过导入的助手造种（`from pt_audit_x_base import *`）⇒ 助手所在
        模块登记了 note_torrent/DELETE，入口就是被真覆盖的。
    """
    if TEARDOWN_RX.search(body) or an_base_teardown(body):
        return True
    if LOCAL_INSERT.search(body):
        return False
    for m in re.finditer(r"from\s+(pt_audit_[a-z0-9_]+)\s+import", body):
        p = os.path.join("scripts", m.group(1) + ".py")
        if not os.path.exists(p):
            continue
        sub = "\n".join(
            ln for ln in open(p, encoding="utf-8", errors="replace")
            .read().splitlines() if not ln.lstrip().startswith("#"))
        # 只有「那边确实登记了回收」才算，不看它自己造的种怎么清
        if re.search(r"note_torrent\(|delete\s+from\s+torrents\b", sub):
            return True
    return False


# 台账=实测「造种但没登记回收」的闸门。**空集是目标不变量**：
# 任何新闸门造种而不 note_torrent/自删，LW4 立刻红并点名它。
# （10-27 前这个集合有 9 支：a_publish/c_review/aq_subtitle_flow + deep_walk
#  与 vip_walk 两族底座；逐支接上回收后清零。历史残料 203 条另算，见 LW2。）
LEDGER_NO_CLEANUP = set()


def cnt(sql):
    return psql_stdin(sql).splitlines()[0].strip()


# —— LW1 正控：反复上传的指纹 ————————————————
dup = psql_stdin(
    "SELECT name, count(*) FROM torrents WHERE approval_status = 1 "
    "GROUP BY name HAVING count(*) > 1 ORDER BY 2 DESC LIMIT 1;")
top_n = int(dup.split("|")[1]) if dup and "|" in dup else 0
ok("LW1 正控：同一标题被反复入库的指纹抓得到（现网最多一个名字 %d 条）"
   % top_n, top_n >= 2, "实测=%s" % (dup.splitlines()[0] if dup else "无读数"))

# —— LW2/LW3 活体账面：夹具占比与真内容 ——————————
fx = cnt("SELECT count(*) FROM torrents WHERE approval_status = 1 AND %s"
         % FIXED)
real = cnt("SELECT count(*) FROM torrents WHERE approval_status = 1 "
           "AND NOT %s" % FIXED)
total = cnt("SELECT count(*) FROM torrents WHERE approval_status = 1")
ok("LW2 过审目录里闸门夹具必须为 0（现在=%s 条，占 %s/%s）" % (fx, fx, total),
   fx == "0", "夹具=%s 真内容=%s 过审合计=%s" % (fx, real, total))
ok("LW3 反空转：真内容必须 > 0 条（否则 LW2 可以把站删空来变绿）",
   int(real) > 0, "真内容=%s" % real)

# —— LW4 静态：造种闸门必须自己清场 ————————
no_cleanup = set()
creators = set()
for f in sorted(os.listdir("scripts")):
    if not f.endswith(".py"):
        continue
    txt = open(os.path.join("scripts", f), encoding="utf-8",
               errors="replace").read()
    body = "\n".join(ln for ln in txt.splitlines()
                     if not ln.lstrip().startswith("#"))
    if not MAKE_RX.search(body):
        continue
    creators.add(f)
    if not has_teardown(body):
        no_cleanup.add(f)
ok("LW4 造种且无收尾的脚本集合 = 台账（多一个=新闸门又开始丢残料，"
   "少一个=补了收尾要同步删号）",
   no_cleanup == LEDGER_NO_CLEANUP,
   "实测=%s 造种脚本共=%d 个" % (sorted(no_cleanup), len(creators)))

# —— LW5 会员实际看到的「最新」排序位 ——————————
head = psql_stdin(
    "WITH h AS (SELECT name, seeders FROM torrents WHERE approval_status = 1 "
    "ORDER BY created_at DESC, id DESC LIMIT 20) "
    "SELECT count(*) FILTER (WHERE %s) FROM h;" % FIXED.replace("name",
                                                                "h.name"))
ok("LW5「最新种子」前 20 条（会员实际看到的那一屏）里夹具必须为 0",
   head.splitlines()[0].strip() == "0",
   "命中=%s/20（注：公开接口默认再按存活过滤，夹具 seeders=0 ⇒ "
   "游客那一屏其实是空的，症状是「站内没有资源」）"
   % head.splitlines()[0].strip())

print("[读数] 「最新」前 20 条里夹具=%s/20" % head.splitlines()[0].strip())

# 同族账面（**只报数不断言**）：账号侧的探针残料比种子侧更夸张。
# 2026-10-27 实测 /admin/overview 三个数自洽（P-32 恒等式成立：
# users 107 + banned_users 3841 == users_total 3948），但 status 分布是
# 0=107 / 2=2368 / 3=1464 / 4=9 —— 「被处置」是「在册」的 36 倍，
# 绝大多数是各闸门用完就停用/墓碑掉的探针号。这不是判据（停用/墓碑本身
# 是正当收尾），但站长看到的「会员数」被它 dominate，值得每轮看一眼。
uv = psql_stdin(
    "SELECT count(*) FILTER (WHERE status < 2), count(*) FILTER (WHERE "
    "status >= 2), count(*) FROM users;").splitlines()[0].strip()
print("[读数] 账号账面（在册<2 / 已处置>=2 / 合计）= %s" % uv)

# —— LW6 账面：现网 schema 里不许有「不在册」的表 ——————————
# 现网表必须三选一：迁移建过 / 分区管理自己建的子分区 / 闸门备份且仍在台账里。
# 实测（2026-10-27）抓到 4 张孤儿：`r11_bak_ahprobe{,_hr,_sn}`（2026-10-22 某支
# 一次性探针脚本的整行备份，脚本已删、备份没删，里面 3 行快照对应的种早没了）
# 和 `_mig_applied`（0 行，仓里无任何引用）。危害不是占地方：任何按表名普查的
# 判据（含本文件的夹具口径、purge 工具的子表发现）都会把它们当成业务表读。
BOOK_RX = re.compile(r"create\s+table\s+(?:if\s+not\s+exists\s+)?"
                     r"([a-z_][a-z0-9_]*)", re.I)
# 分区子表与工具表是运行期合法产物（worker jobs/maintain.rs 的预建 job）
PART_RX = re.compile(r"^(posts|spark_ledger|traffic_ledger)"
                     r"_(\d{4}_\d{2}|default)$")
TOOLING = {"_sqlx_migrations"}
# 台账=实测残料，清一张就从这里销一个号；DROP 只由站长执行（见下方提示行）。
LEDGER_OFFBOOK = {"_mig_applied", "r11_bak_ahprobe",
                  "r11_bak_ahprobe_hr", "r11_bak_ahprobe_sn"}


def book_tables():
    """迁移里真的建过的表名（**先剥注释**：迁移正文常举例写 CREATE TABLE，
    不剥就把举例当成在册——同族坑在 0419 那句 prose DELETE 上踩过一次）。"""
    out = set()
    for f in sorted(os.listdir("apps/api/migrations")):
        if not f.endswith(".sql"):
            continue
        txt = open(os.path.join("apps/api/migrations", f),
                   encoding="utf-8", errors="replace").read()
        txt = re.sub(r"/\*.*?\*/", " ", txt, flags=re.S)
        for ln in txt.splitlines():
            out.update(BOOK_RX.findall(ln.split("--")[0]))
    return {t.lower() for t in out}


BOOK = book_tables()
LIVE = {t.strip().lower() for t in
        psql_stdin("SELECT tablename FROM pg_tables "
                   "WHERE schemaname='public';").splitlines() if t.strip()}
OFFBOOK = {t for t in LIVE - BOOK
           if not PART_RX.match(t) and t not in TOOLING}
ok("LW6 现网表必须在册（迁移建的/分区/台账里的备份）——多一张=残料，"
   "少一张=清完要销号",
   OFFBOOK == LEDGER_OFFBOOK,
   "不在册=%s 现网表=%d 迁移在册=%d" % (sorted(OFFBOOK), len(LIVE),
                                        len(BOOK)))
if OFFBOOK:
    print("[待站长] 回收这几张孤儿表即销号（本闸门不 DROP 现网对象）："
          + " ".join("DROP TABLE IF EXISTS %s;" % t for t in sorted(OFFBOOK)))

# —— LW7 账面：流水不许掉进 DEFAULT 分区 ——————————
# 月分区由 worker `ensure_partitions()` 预建 [当月, +2]；掉进 default 的行
# 既不被按月裁剪（查询变慢），也不进 `drop_expired_partitions()` 的归档视野——
# 「预建 job 挂了」在界面上没有任何症状，只有这一处看得出来。
DFT = [(t, cnt("SELECT count(*) FROM %s_default" % t))
       for t in ("traffic_ledger", "spark_ledger", "posts")]
ok("LW7 三张 RANGE 流水表的 DEFAULT 分区必须 0 行（有行=月分区预建 job 没跑成）",
   all(int(v) == 0 for _, v in DFT),
   " ".join("%s_default=%s" % (n, v) for n, v in DFT))
print("[读数] default 分区行数=%s 保留期 ledger_retain_months=%s"
      % (DFT, cnt("SELECT value FROM site_settings "
                  "WHERE name='ledger_retain_months'")))

summary()
