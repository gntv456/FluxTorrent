# -*- coding: utf-8 -*-
"""LV：迁移「应用之后又被改」的静默漂移（P-442）。

机制：sqlx 只按**版本号**记账（`_sqlx_migrations.version`），已在账里的号下次
启动直接跳过，**不看文件内容**。所以「往一个已经跑过的迁移文件里追加内容」
= 新库（CI、新客站点）拿到追加部分，存量站永远拿不到，而台账写的是 success=t。
这不是理论风险：0419 的 installed_on 是 2026-10-25 02:58，文件进仓库是
2026-10-26 07:10（505f853a「补 add 漏交的功能源文件——8 迁移」），现网实测
0419 补的四把键里只有 `upload_title_pattern` 两表皆无——它跑的时候文件里
还没有这一条。

判据（10-28 随「0426 已真落地」重写过一次，别照旧账读）：
  LV1  正控：合成一份「一半落地一半没落地」的迁移正文喂给检测器，必须点名
       缺的那把。**不钉真实号**——286/419 已被 0426 补种自愈，钉实号会退化
       成永不可能成立的假承重
  LV2  「跑过却只落地一半」的集合 = 台账（现为空集＝不变量：今后谁再往已应用
       的文件里追加键，当场点名）
  LV3  「已应用但 HEAD 里没这个文件」= 站方活账 ⇒ **现为红**（0423/0424/0425
       已随重建跑进现网，文件却仍未提交 ⇒ 干净环境装机会少这三步）
  LV4  缺陷本体活体：upload_title_pattern / resurrection_hours 两表都在
       （10-28 api 重启后已转绿＝0426 真生效）
  LV5  0426 的 SQL 在现网事务里再跑一遍 + **增量对账**：跑前/事务内/跑后行数
       一致 ⇒ 证明确实幂等（ON CONFLICT DO NOTHING）且闸门不留残料。
       另断两张卡的 card_order 没被别人占着。

跑法：python scripts/pt_audit_lv_migration_drift.py
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

MIG_DIR = "apps/api/migrations"
KEY = "upload_title_pattern"
FIX = os.path.join(MIG_DIR, "0426_reseed_upload_title_pattern.sql")

# 台账：跑过却只落地一半的迁移。**现为空集，这是不变量而非「站干净」**：
#   286/419 两案（`upload_title_pattern` 缺登记）已由 0426 补种并在 10-28
#   api 重启时真落地（现网 site_settings 与 settings_meta 两行都在）⇒ 自愈。
# 留空集的用处：今后任何人再往「已应用」的迁移文件里追加键，本判据当场点名。
PARTIAL_LEDGER = set()
# 台账：已应用但 HEAD 里没有对应文件（靠工作树未提交文件跑掉的迁移）。
# 这三条**是站方的活账**（10-28 实测 0423/0424/0425 已在现网跑过、文件仍未 add）
# ⇒ 干净环境首启会少这三步。只有站长提交能关，故不入台账、让它继续红。
UNTRACKED_LEDGER = {"423"}


def sh(*cmd):
    return subprocess.run(cmd, capture_output=True, text=True,
                          encoding="utf-8", errors="replace").stdout


def installed():
    """版本号 -> (installed_epoch, installed_on 文本, success)。

    epoch 比较而不是字符串比较：`installed_on` 的 to_char 走会话时区（本站
    是 UTC），git 的 %at 是绝对秒数——两边都换成秒才不会被 +08:00 骗。
    """
    out = psql_stdin(
        "SELECT version::text, floor(extract(epoch FROM installed_on))::text, "
        "to_char(installed_on,'YYYY-MM-DD HH24:MI'), success::text "
        "FROM _sqlx_migrations ORDER BY version;")
    m = {}
    for ln in out.splitlines():
        c = ln.split("|")
        if len(c) >= 4:
            m[c[0].lstrip("0") or "0"] = (c[1], c[2], c[3])
    return m


def last_commit_by_file():
    """一次 git log 拿到每个迁移文件的最后一次提交秒数（新→旧，取首个）。"""
    out = sh("git", "log", "--format=C\t%at", "--name-only", "--", MIG_DIR)
    dates = {}
    cur = None
    for ln in out.splitlines():
        if ln.startswith("C\t"):
            cur = ln[2:].strip()
        elif ln.strip() and cur:
            m = re.search(r"(\d{4})_[^/]*\.sql$", ln.strip())
            if m:
                dates.setdefault(m.group(1).lstrip("0") or "0", cur)
    return dates


inst = installed()
committed = last_commit_by_file()
head_files = {
    (re.search(r"(\d{4})_", os.path.basename(p)).group(1).lstrip("0")
     or "0")
    for p in sh("git", "ls-tree", "-r", "--name-only", "HEAD",
                MIG_DIR).splitlines()
    if re.search(r"(\d{4})_", p)
}

drift = set()
for ver, (epoch, when, _ok) in inst.items():
    c = committed.get(ver)
    if c and c.isdigit() and epoch.isdigit() and int(c) > int(epoch):
        drift.add(ver)

# 「文件在应用之后又提交过」在这仓里是常态（迁移常先在工作树跑掉、之后才
# add，505f853a 就是专门补这种漏交的 8 个迁移）⇒ 拿它当判据会一次点出上百个
# 版本，等于没有判据。真正要紧的形状是**同一个迁移文件只落地了一半**：
# sqlx 记账后不看内容，所以「跑过之后再往文件里加键」= 加的那部分对存量站
# 永远不执行。于是按文件分组比对现网存在性，并把后来被 DELETE 退役掉的键
# 从缺失里剔出去。
def strip_sql_comments(text):
    """先剥 `--` 行注释再扫。0419 的头注里写着「全仓仅两处 DELETE FROM
    site_settings」，不剥的话那句散文会被当成一条 DELETE 语句，把它后面
    引号里的键名全部记成「已退役」——正好把本次要抓的 upload_title_pattern
    自己洗白了（实测第一版 LV1 因此报 None）。"""
    out = []
    for ln in text.splitlines():
        i = ln.find("--")
        out.append(ln if i == -1 else ln[:i])
    return "\n".join(out)


def names_in(text, table):
    """INSERT INTO <table> ... VALUES ('a',…),('b',…) 里第一列的键名。"""
    out = []
    for m in re.finditer(
            r"INSERT INTO %s\b[^;]*?VALUES(.*?)(?:ON CONFLICT[^;]*)?;" % table,
            text, re.S):
        for row in re.findall(r"\(\s*'([a-z0-9_\.]+)'", m.group(1)):
            out.append(row)
    return out


def retired_names(all_text):
    out = set()
    for m in re.finditer(
            r"DELETE FROM (?:site_settings|settings_meta)\b[^;]*;", all_text,
            re.S):
        blk = m.group(0)
        out |= set(re.findall(r"'([a-z0-9_\.]+)'", blk))
        # LIKE 'prefix%…' 形态的退役（如 games_scratch_%_pct）按前缀记账；
        # % 后面不一定紧跟引号（'games_scratch_%_pct' 就是 %_pct'）
        for like in re.findall(r"LIKE '([a-z0-9_\.]+?)%", blk):
            out.add(like)
    return out


live_val = {r.strip() for r in psql_stdin(
    "SELECT name FROM site_settings;").splitlines() if r.strip()}
live_meta = {r.strip() for r in psql_stdin(
    "SELECT name FROM settings_meta;").splitlines() if r.strip()}
corpus = "\n".join(
    strip_sql_comments(open(os.path.join(MIG_DIR, f), encoding="utf-8",
                            errors="replace").read())
    for f in os.listdir(MIG_DIR) if f.endswith(".sql"))
retired = retired_names(corpus)


def retired_hit(n):
    return n in retired or any(
        n.startswith(r) and r for r in retired if r.endswith(("_", ".")))


def landing_of(txt):
    """返回 (现网已落地的条数, 文件里有、现网没有、也没被退役的键清单)。"""
    live = {"v": live_val, "m": live_meta}
    want = ([(n, "v") for n in names_in(txt, "site_settings")]
            + [(n, "m") for n in names_in(txt, "settings_meta")])
    if not want:
        return 0, []
    have = sum(1 for n, w in want if n in live[w])
    miss = [n for n, w in want if n not in live[w] and not retired_hit(n)]
    return have, list(dict.fromkeys(miss))


partial = {}
for f in sorted(os.listdir(MIG_DIR)):
    if not f.endswith(".sql"):
        continue
    ver = f[:4].lstrip("0") or "0"
    if ver not in inst:
        continue  # 未应用的迁移不算（它还没机会落地）
    txt = strip_sql_comments(open(os.path.join(MIG_DIR, f), encoding="utf-8",
                                  errors="replace").read())
    have, miss = landing_of(txt)
    if 0 < have and miss:
        partial[ver] = miss

# 正控**不钉真实迁移号**：286/419 那两案已被 0426 补种自愈（现网两行都在），
# 「某号必须报缺」在事实变化后会退化成永不可能成立的断言＝假承重。
# 改成合成一份「一半落地一半没落地」的正文喂给同一把尺，它必须点名缺的那把。
PROBE_ABSENT = "lv_probe_key_never_seeded"
_ctl_have, _ctl_miss = landing_of(
    "INSERT INTO site_settings (name, value) VALUES ('%s', '1'), ('%s', '2');"
    "\nINSERT INTO settings_meta (name, group_key) VALUES ('%s', 'ops');"
    % (KEY, PROBE_ABSENT, KEY))
ok("LV1 正控：合成「一半落地一半没落地」的正文必须被点名（尺子认得这形状）",
   _ctl_have > 0 and _ctl_miss == [PROBE_ABSENT],
   "已落地=%d 报缺=%s" % (_ctl_have, _ctl_miss))
ok("LV2 「跑过但只落地一半」的迁移=台账 %s（应用后再加键，存量站拿不到）"
   % sorted(PARTIAL_LEDGER),
   set(partial) == PARTIAL_LEDGER,
   "实测=%s" % {k: partial[k] for k in sorted(partial, key=int)})

print("[读数] 迁移文件在应用之后又提交过的版本数=%d"
      "（本仓常态：先在工作树跑掉、之后才 add，不能当判据）"
      % len(drift))

untracked = {v for v in inst if v not in head_files}
ok("LV3 「已应用但 HEAD 无文件」的集合=台账 %s（未提交文件跑掉的迁移）"
   % sorted(UNTRACKED_LEDGER),
   untracked == UNTRACKED_LEDGER,
   "实测=%s" % sorted(untracked, key=int))

# 0426 补登记的两把键：(键名, 落到的卡片组, card_order)
CARDS = [(KEY, "torrent", 84), ("resurrection_hours", "ops", 64)]
IN_LIST = ",".join("'%s'" % k for k, _, _ in CARDS)

for k, g, co in CARDS:
    both = psql_stdin(
        "SELECT (EXISTS(SELECT 1 FROM site_settings WHERE name='%s'))::text, "
        "(EXISTS(SELECT 1 FROM settings_meta WHERE name='%s'))::text;"
        % (k, k)).splitlines()[0].split("|")
    ok("LV4 缺陷本体：「%s」在 site_settings 与 settings_meta 里都在（缺任一行=设置页根本不渲染这张卡）"
       % k, both[0] == "true" and both[1] == "true",
       "值行=%s 元数据=%s（红＝这两把键从来没落地；0426 随下次 api 启动应用后转绿）"
       % (both[0], both[1]))
    occ = psql_stdin(
        "SELECT count(*) FROM settings_meta WHERE group_key='%s' "
        "AND card_order=%d AND name <> '%s';" % (g, co, k))
    ok("LV5c %s 卡的 card_order %d 空着（0426 不制造排序撞位）" % (g, co),
       occ.splitlines()[0].strip() == "0",
       "占用=%s" % occ.splitlines()[0].strip())

sql = open(FIX, encoding="utf-8").read()
body = "\n".join(ln for ln in sql.splitlines()
                 if not ln.lstrip().startswith("--"))
# 0426 现已真落地 ⇒ 「事务内插得进、回滚后归零」这个老探针的**前提没了**
# （键本来就在表里，回滚后当然还是 2|2，红在判据自己身上）。改成**增量对账**：
# 跑前/跑后各数一次，要求「跑通 SQL 且行数一克不多」——0426 用的是
# ON CONFLICT DO NOTHING，幂等本身就是它该证的性质。
pre = psql_stdin(
    "SELECT 'pre', (SELECT count(*) FROM site_settings WHERE name IN (%s)), "
    "(SELECT count(*) FROM settings_meta WHERE name IN (%s));"
    % (IN_LIST, IN_LIST))
probe = psql_stdin(
    "BEGIN;\n%s\nSELECT 'in_tx', (SELECT count(*) FROM site_settings WHERE "
    "name IN (%s)), (SELECT count(*) FROM settings_meta WHERE name IN (%s));\n"
    "ROLLBACK;\n" % (body, IN_LIST, IN_LIST))
post = psql_stdin(
    "SELECT 'post', (SELECT count(*) FROM site_settings WHERE name IN (%s)), "
    "(SELECT count(*) FROM settings_meta WHERE name IN (%s));"
    % (IN_LIST, IN_LIST))
pre_t = re.search(r"pre\|(\d+)\|(\d+)", pre)
in_tx = re.search(r"in_tx\|(\d+)\|(\d+)", probe)
post_t = re.search(r"post\|(\d+)\|(\d+)", post)
n = len(CARDS)
ok("LV5a 0426 的 SQL 在现网事务里真跑通（%d 把键的行数都在，外键顺序没错）"
   % n, in_tx is not None and int(in_tx.group(1)) >= n
   and int(in_tx.group(2)) >= n,
   "事务内=%s" % (in_tx.group(0) if in_tx else "没取到读数"))
ok("LV5b 幂等且不增殖：跑前/跑后行数一致（重放 0426 不会多加行、闸门不留残料）",
   pre_t is not None and post_t is not None
   and pre_t.group(0)[4:] == post_t.group(0)[5:]
   and in_tx is not None and in_tx.group(1) == pre_t.group(1),
   "跑前=%s 事务内=%s 跑后=%s"
   % ((pre_t.group(1), pre_t.group(2)) if pre_t else "?",
      (in_tx.group(1), in_tx.group(2)) if in_tx else "?",
      (post_t.group(1), post_t.group(2)) if post_t else "?"))

summary()
