# -*- coding: utf-8 -*-
"""闸门 HK：内置插件的 SQL 必须带 schema 限定；咨询锁必须与连接同生同死
（批一百八十二 P-420 / P-420b / P-421，都来自现网日志指纹）。

三条实测依据（`runtime_logs`，只读）：
  · WARN **24,432** 条 `sqlx::postgres::notice: you don't own a lock of type
    ExclusiveLock`（最近 2026-10-19 16:19）⇒ 会话级 `pg_try_advisory_lock` 与
    `pg_advisory_unlock` 分别丢给 pool 执行，两条语句之间连接已归还池子：
    解锁落在别的连接上（所以 PG 说不持有），而别的连接再 try_lock 会**照样成功**
    ⇒ 互斥根本不成立，消费循环可并发跑（置顶/通知被双份分发）。
  · ERROR **330** 条 `pin update failed [plugin=auto_pin_official …
    relation "torrents" does not exist]`（最近 2026-10-19 10:51）⇒
    `plugin_sql::begin_isolated` 把 `search_path` 收成只剩 `pg_catalog`（防同名表
    被前序项劫持，注释成文），要求业务表**显式 schema 限定**；内置插件写的却是
    裸名 `UPDATE torrents …` ⇒ 这个「官种自动置顶」从上线起**一次都没成功过**。
  · 同一条 SQL 加 `public.` 前缀后，用插件角色 + `search_path=pg_catalog` 的真实
    上下文重放（BEGIN…ROLLBACK，零改动）⇒ 正常执行；`has_table_privilege(...)`
    为 t ⇒ 权限本来就够，缺的只是限定名。

判据：
  HK1 plugins_builtin.rs 里每条 db_exec/db_fetch_* 的 SQL 都必须带 schema 限定
      （裸表名＝违例）；
  HK2 sql_guard 必须拒绝非 public 的 schema 限定（`bad_schema` 判据在册）——
      护栏此前只取 ObjectName 最后一段比对白名单，`evil.torrents` 能借声明过的
      表名溜过去，实际写到别的 schema 的同名表；
  HK3 活体正控：以插件角色 + `search_path=pg_catalog` 重放限定名 SQL，必须不报错
      （BEGIN…ROLLBACK，不改任何数据）；同时重放裸名必须报
      `relation "torrents" does not exist`（证明判据抓到的是同一个失败模式）；
  HK4 两处消费循环（domain_events_consumer / push_notify）的 try_lock 与 unlock
      都走同一条 `&mut *conn`，且函数里先 `acquire()`；
  HK5 承重自证：把「旧形状」合成码喂同一把尺，必须报违例。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hk_plugin_lock.py`
      HK_LIVE=0 跳过 HK3 的活体重放。
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

LIVE = os.environ.get("HK_LIVE", "1") != "0"
BUILTIN = "apps/api/src/plugins_builtin.rs"
GUARD = "apps/api/src/sql_guard/mod.rs"
COLLECT = "apps/api/src/sql_guard/collect.rs"
CONSUMERS = [
    "apps/api/src/domain_events_consumer.rs",
    "apps/api/src/push_notify.rs",
]
ROLE_SQL = (
    "SELECT rolname FROM pg_roles WHERE rolname LIKE "
    "'flux_plugin_auto_pin_official%' LIMIT 1")
QUALIFIED = ("UPDATE public.torrents SET sticky = TRUE WHERE id = 1 AND "
             "official_tag")
BARE = "UPDATE torrents SET sticky = TRUE WHERE id = 1 AND official_tag"

OLD_PLUGIN_SQL = r"""
        match ctx.db_exec(
            "UPDATE torrents SET sticky = TRUE WHERE id = $1 AND official_tag",
            &[Bind::I64(torrent_id)],
        ) {
"""
OLD_LOCK = r"""
    let got: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(DRAIN_LOCK)
        .fetch_one(pool)
        .await
        .unwrap_or(false);
    if !got {
        return Ok(());
    }
    let r = drain_locked(pool).await;
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(DRAIN_LOCK)
        .execute(pool)
        .await;
"""

STMT_RE = re.compile(
    r'"((?:SELECT|INSERT|UPDATE|DELETE)\b[^"]*)"', re.I)
TABLE_RE = re.compile(
    r"\b(?:FROM|JOIN|UPDATE|INTO)\s+([A-Za-z_][\w.]*)", re.I)


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip(src):
    return "\n".join(
        ln for ln in src.split("\n")
        if not ln.strip().startswith("//") and not ln.strip().startswith("///"))


def bare_table_sqls(text):
    """返回该文本里「引用了不带 schema 的业务表」的 SQL 字面量。"""
    bad = []
    for m in STMT_RE.finditer(text):
        sql = re.sub(r"\s+", " ", m.group(1))
        for t in TABLE_RE.findall(sql):
            if "." not in t and t.lower() not in (
                    "select", "insert", "update", "delete"):
                bad.append("%s -> %s" % (t, sql[:56]))
                break
    return bad


def lock_unbalanced(text):
    """try_lock 或 advisory_unlock 直接对 pool/db 执行（而非 &mut *conn）＝违例。"""
    bad = []
    for m in re.finditer(r"pg_try_advisory_lock|pg_advisory_unlock", text):
        seg = text[m.start():m.start() + 260]
        tail = seg[:seg.find(".await") if ".await" in seg else 260]
        if "pool)" in tail or "db)" in tail:
            bad.append(m.group(0))
    return bad


b = strip(rd(BUILTIN))
naked = bare_table_sqls(b)
ok("HK1 内置插件 SQL 全部带 schema 限定", naked == [],
   "裸表名=%s" % (naked[:2],))

gsrc = strip(rd(GUARD)) + strip(rd(COLLECT))
ok("HK2 护栏拒绝非 public 的 schema 限定",
   "bad_schema" in gsrc and "只允许 public" in gsrc, "缺 public 限定判据")

locks = []
for rel in CONSUMERS:
    t = strip(rd(rel))
    hits = lock_unbalanced(t)
    if hits or ".acquire()" not in t:
        locks.append("%s:%s" % (os.path.basename(rel), hits or "无 acquire"))
ok("HK4 两处消费循环的锁与解锁同连接", locks == [], "违例=%s" % (locks,))

old_naked = bare_table_sqls(OLD_PLUGIN_SQL)
old_locks = lock_unbalanced(OLD_LOCK)
ok("HK5 承重：旧插件 SQL 与旧锁写法都被抓",
   len(old_naked) == 1 and len(old_locks) >= 2,
   "旧 SQL=%s 旧锁=%s" % (old_naked, old_locks))

if LIVE:
    def psql(q):
        r = subprocess.run(
            ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
             "-d", "fluxtorrent", "-q", "-tA"], input=q,
            capture_output=True, text=True, encoding="utf-8", errors="replace")
        return (r.stdout or "").strip(), (r.stderr or "").strip()

    role, _ = psql(ROLE_SQL)
    if not role:
        ok("HK3 活体：插件角色在册", False, "没找到 flux_plugin_auto_pin_*")
    else:
        def ctx(sql):
            return psql("BEGIN; SET LOCAL ROLE %s; SET LOCAL search_path = "
                        "pg_catalog; %s; ROLLBACK;" % (role, sql))[1]
        err_bare = ctx(BARE)
        err_ok = ctx(QUALIFIED)
        ok("HK3 活体：裸名必失败、限定名必通过（同隔离上下文）",
           'relation "torrents" does not exist' in err_bare
           and "ERROR" not in err_ok,
           "裸名=%s 限定名=%s" % (err_bare.splitlines()[:1],
                                  err_ok.splitlines()[:1] or "无错"))
else:
    print("[HK3] HK_LIVE=0 跳过活体重放")

summary()
