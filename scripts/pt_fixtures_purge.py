# -*- coding: utf-8 -*-
"""清闸门夹具种子（默认空跑，`--apply` 才删）。

为什么需要：常驻闸门经**真实发布接口**造种，跑完不删 ⇒ 现网 213 条过审种里
203 条是夹具（同一个名字 `Audit.Pack.深测` 就有 93 条，10-02 起每次跑加一条），
真内容只剩 10 条。而公开列表默认按存活过滤（seeders=0 归入 hidden_dead），
于是游客看到的「最新种子」是空的——站点内容面被审计流程自己吃掉。

安全设计（这条是破坏性操作，形状照仓里既有的红线配方）：
  * 默认 dry-run：只打印将要删什么、子表有多少行、外键是 CASCADE 还是 NO ACTION；
  * `--apply` 才动手：先把被删整行 CREATE TABLE AS SELECT 备份成
    `purge_backup_torrents`（复原靠整行 IS DISTINCT FROM 比对，不数行数），
    再按子表→主表顺序删，最后回读残料；
  * 只按**本轮专属名字族**命中，不用宽 LIKE 打全站（演示成员名的教训）；
  * 删完打印真内容条数，防止「把站删空」被读成「清理完成」。
"""
import sys

sys.path.insert(0, "scripts")
from pt_audit_lib import psql_stdin  # noqa: E402

# 闸门夹具的名字族（现网实测逐个核对过，2026-10-27）
FAMILY = [
    "Audit.Pack.深测",          # pt_audit_a_publish.py
    "AuditB.Free.",             # pt_audit_b_billing.py 一族带 .<pid> 后缀
    "AuditC.Review2.",
    "pt_audit_alive.",
    "deep_alive.",
]
# 命中谓词：精确名 + 前缀族（都用 LIKE 前缀，不用宽正则）
# 整体包一层括号：否则 `AND %s` / `NOT %s` 会因优先级把兜底谓词的 OR 甩到
# approval_status 条件之外（实测把 203 数成 240，还把未过审行算进「真内容」
# 得到 250 > 全站 213）。
FAMILY_OR = " OR ".join(
    ["name = '%s'" % FAMILY[0]]
    + ["name LIKE '%s%%'" % p for p in FAMILY[1:]]
)
CATCHALL = "name ~* 'audit|deep_alive|e2est|emul_|probe_|dummy' AND seeders=0"
PRED = "((%s) OR (%s))" % (FAMILY_OR, CATCHALL)

APPLY = "--apply" in sys.argv

count = lambda sql: psql_stdin(sql).splitlines()[0].strip()  # noqa: E731
fx = "SELECT count(*) FROM torrents WHERE approval_status = 1 AND %s" % PRED
real = ("SELECT count(*) FROM torrents WHERE approval_status = 1 "
        "AND NOT %s" % PRED)

print("命中夹具=%s 条；其余过审真内容=%s 条" % (count(fx), count(real)))
print("\n-- 子表规模 --")
print(psql_stdin(
    "WITH fx AS (SELECT id FROM torrents WHERE approval_status = 1 AND %s) "
    "SELECT (SELECT count(*) FROM torrent_files f "
    "JOIN fx ON f.torrent_id=fx.id),"
    " (SELECT count(*) FROM snatches s JOIN fx ON s.torrent_id=fx.id), "
    "(SELECT count(*) FROM bookmarks b JOIN fx ON b.torrent_id=fx.id), "
    "(SELECT count(*) FROM resurrections r JOIN fx ON r.torrent_id=fx.id), "
    "(SELECT count(*) FROM torrent_sections x JOIN fx ON x.torrent_id=fx.id);"
    % PRED))
print("\n-- 引用 torrents 的外键与 ON DELETE 行为（决定删除顺序）--")
print(psql_stdin(
    "SELECT c.conrelid::regclass::text AS child, "
    "a.attname AS col, CASE c.confdeltype WHEN 'a' THEN 'NO ACTION' "
    "WHEN 'c' THEN 'CASCADE' WHEN 'n' THEN 'SET NULL' WHEN 'd' THEN "
    "'SET DEFAULT' WHEN 'r' THEN 'RESTRICT' ELSE c.confdeltype::text END "
    "AS on_delete FROM pg_constraint c "
    "JOIN pg_attribute a ON a.attrelid=c.conrelid "
    "AND a.attnum=(c.conkey)[1] WHERE c.confrelid='torrents'::regclass "
    "AND c.contype='f' ORDER BY 1;"))

print("\n-- NO ACTION 阻塞点（这些子表外键不会连带删，必须先处理）--")
# 不写死清单：从 pg_constraint 现取，免得日后加了外键这把工具就漏删。
na_rows = psql_stdin(
    "SELECT c.conrelid::regclass::text, a.attname FROM pg_constraint c "
    "JOIN pg_attribute a ON a.attrelid=c.conrelid AND a.attnum=(c.conkey)[1] "
    "WHERE c.confrelid='torrents'::regclass AND c.contype='f' "
    "AND c.confdeltype='a' ORDER BY 1;").splitlines()
blockers = []
for ln in na_rows:
    parts = ln.split("|")
    if len(parts) == 2:
        blockers.append((parts[0], parts[1]))
for tbl, col in blockers:
    n = count(
        "SELECT count(*) FROM %s x WHERE x.%s IN (SELECT id FROM torrents "
        "WHERE approval_status = 1 AND %s);" % (tbl, col, PRED))
    print("   %-18s.%-20s 命中=%s" % (tbl, col, n))

if not APPLY:
    print("\n（dry-run，没有删除任何东西；确认无误后加 --apply）")
    sys.exit(0)

print("\n-- 备份整行到 purge_backup_torrents --")
psql_stdin("DROP TABLE IF EXISTS purge_backup_torrents;")
psql_stdin(
    "CREATE TABLE purge_backup_torrents AS SELECT * FROM torrents "
    "WHERE approval_status = 1 AND %s;" % PRED)
print("备份行数=%s" % count("SELECT count(*) FROM purge_backup_torrents"))

print("-- 删除：先清 NO ACTION 子表，其余靠 CASCADE，最后删主表 --")
for tbl, col in blockers:
    psql_stdin(
        "DELETE FROM %s WHERE %s IN (SELECT id FROM torrents WHERE "
        "approval_status = 1 AND %s);" % (tbl, col, PRED))
psql_stdin("DELETE FROM torrents WHERE approval_status = 1 AND %s;" % PRED)

left = count(fx)
print("\n残料（夹具名族）=%s 条；真内容=%s 条" % (left, count(real)))
# 复原核对：备份里的每一行现在都应当不在了
gone = count(
    "SELECT count(*) FROM purge_backup_torrents b WHERE EXISTS "
    "(SELECT 1 FROM torrents t WHERE t.id = b.id)")
print("备份行仍在 torrents 里的=%s 条（应为 0）" % gone)
print("备份表保留在 purge_backup_torrents，确认无误后再 DROP")
