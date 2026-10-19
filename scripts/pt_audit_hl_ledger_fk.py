# -*- coding: utf-8 -*-
"""闸门 HL：账本表没有 user_id 外键 ⇒ 悬空流水只能靠常驻对账盯住
（批一百八十四，只读取证，不改任何账面）。

现网实测（2026-10-19）：
  · `spark_ledger` 有 **33 行** 的 `user_id` 在 `users` 里根本不存在，分两组：
      - kind='achievement' 21 行，挂在 id 2/3/4/5/6/7/8/9/12（低号段，成就发放
        写在了从来没建过、或早已不存在的账号上）；
      - kind='user_purge' 12 行，挂在 id 4330..4341（`user-purge-<uid>` 键说明
        **墓碑清算当时是走到过的**，之后这些 users 行被物理删掉了）。
  · `traffic_ledger` 侧 0 行悬空、0 行 `user_id=0/NULL`（干净）。
  · 根因是结构性的：`spark_ledger` / `traffic_ledger` 两张表**一条外键都没有**
    （`pg_constraint` 查 contype='f' 返回空），所以写谁的 id 都不会被拒。
    而产品码里没有任何 `DELETE FROM users`（api 与 worker 全扫）——0266 之后
    删号一律墓碑化 ⇒ 这批悬空行是**外部操作**（历史闸门/运维手删）留下的，
    不是当前业务路径产生的。

所以本闸门做三件事：
  HL1 精确在册清单（22 组 user_id+kind）：孤儿组必须**恰好等于**这份清单——
      多一组红（新脏行）、少一组也红（有人清了账，那就顺手把台账划掉）；
  HL2 结构性纪律：apps 里不得出现 `DELETE FROM users`（墓碑化是唯一许可路径；
      谁绕过它，就会再造一批 33 行这种「流水还在、人没了」的悬案）；
  HL3 承重自证：合成一条 `DELETE FROM users WHERE id = $1` 喂 HL2 的判据必须被抓；
  HL4 traffic_ledger 侧零容忍（现网已 0）：悬空 user_id 与 =0/NULL 哨兵都必须为 0。

给站方的收尾建议（本闸门**不执行**，那是改账目的迁移）：清完这 33 行
（或把 id 2..12 的成就流水并到真实账号）后，给两张账本表补
`FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE RESTRICT`，
让「写不存在的账号」在库层面就失败；补约束前必须先清，否则迁移直接红。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hl_ledger_fk.py`
      HL_LIVE=0 跳过 HL1/HL4 的只读查询（HL2/HL3 是静态的，仍跑）。
"""
import io
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

LIVE = os.environ.get("HL_LIVE", "1") != "0"
SRC = [os.path.join(ROOT, "apps", "api", "src"),
       os.path.join(ROOT, "apps", "worker", "src")]
# 2026-10-19 实测的在册孤儿组（user_id, kind, 行数）
LEDGER = [
    (2, "achievement", 4), (3, "achievement", 4), (4, "achievement", 4),
    (5, "achievement", 1), (6, "achievement", 4), (7, "achievement", 1),
    (8, "achievement", 1), (9, "achievement", 1), (12, "achievement", 1),
    (4330, "user_purge", 1), (4331, "user_purge", 1), (4332, "user_purge", 1),
    (4333, "user_purge", 1), (4334, "user_purge", 1), (4335, "user_purge", 1),
    (4336, "user_purge", 1), (4337, "user_purge", 1), (4338, "user_purge", 1),
    (4339, "user_purge", 1), (4340, "user_purge", 1), (4341, "user_purge", 1),
]
OLD_DELETE = 'sqlx::query("DELETE FROM users WHERE id = $1").bind(uid)'
NEEDLE = "DELETE FROM users"


def code_lines(src):
    """剥掉 // 与 /// 行（注释里出现的字面词不算命中，也不能拿来造假绿）。"""
    return "\n".join(ln for ln in src.split("\n")
                     if not ln.strip().startswith("//")
                     and not ln.strip().startswith("///"))


def delete_hits(text, label):
    out = []
    for i, ln in enumerate(code_lines(text).split("\n"), 1):
        if NEEDLE in ln:
            out.append("%s:%d" % (label, i))
    return out


def find_delete_users(paths):
    hits = []
    for base in paths:
        for dp, _d, fns in os.walk(base):
            for fn in fns:
                if not fn.endswith(".rs"):
                    continue
                p = os.path.join(dp, fn)
                hits += delete_hits(
                    io.open(p, encoding="utf-8", errors="replace").read(),
                    os.path.relpath(p, ROOT).replace("\\", "/"))
    return hits


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-q", "-tA", "-F", "|"], input=q,
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    err = (r.stderr or "")
    if "ERROR" in err:
        raise RuntimeError("psql: " + err[:300])
    return (r.stdout or "").strip()


prod_hits = find_delete_users(SRC)
ok("HL2 产品码里不得物理删除 users 行（墓碑化是唯一路径）",
   prod_hits == [], "命中=%s" % (prod_hits[:4],))
ctl = delete_hits(OLD_DELETE, "合成样本")
ok("HL3 承重：合成 `DELETE FROM users` 被同一条判据抓住",
   len(ctl) == 1, "命中=%s" % (ctl,))

if LIVE:
    rows = psql(
        "SELECT l.user_id, l.kind, count(*) FROM spark_ledger l WHERE NOT "
        "EXISTS (SELECT 1 FROM users u WHERE u.id = l.user_id) "
        "GROUP BY 1,2 ORDER BY 1,2")
    got = []
    for ln in rows.split("\n"):
        if not ln.strip():
            continue
        a = ln.split("|")
        got.append((int(a[0]), a[1], int(a[2])))
    ok("HL1 火花账本悬空 user_id 恰好等于在册清单",
       sorted(got) == sorted(LEDGER),
       "现网组=%d 行=%d 在册组=%d（多一组=新脏行，少一组=有人清了账）"
       % (len(got), sum(n for _u, _k, n in got), len(LEDGER)))
    traffic = psql(
        "SELECT count(*) FILTER (WHERE t.user_id IS NULL OR t.user_id = 0), "
        "count(*) FILTER (WHERE t.user_id > 0 AND NOT EXISTS (SELECT 1 "
        "FROM users u WHERE u.id = t.user_id)) FROM traffic_ledger t")
    zero, dang = (traffic.split("|") + ["0", "0"])[:2]
    ok("HL4 流量账本零容忍：无 =0/NULL 哨兵、无悬空 user_id",
       int(zero) == 0 and int(dang) == 0,
       "哨兵=%s 悬空=%s" % (zero, dang))
else:
    print("[HL1/HL4] HL_LIVE=0 跳过只读查询")

summary()
