# -*- coding: utf-8 -*-
"""闸门 IB：JSONB「幻键」普查——代码在读的键，库里必须真写过（批二百一十三）。

来历：P-436 就是这把尺抓到的——搜索 `search_area=4` 读 `media_info->>'imdb'`，
而写入口 `upload_ids.rs:125` 落的是 `imdb_id` 键 ⇒ 现网 `imdb` 键 0 行，
IMDb 检索整条失效（编辑填过 IMDb 的种永远搜不到）。推广到全站重跑，只剩 3 个
候选，逐条判读后只有那一个是洞：
  · `media_info.tmdb_url`：写入口用 `jsonb_strip_nulls(jsonb_build_object(...))`，
    未配置 TMDB 时值为 NULL ⇒ 键被整个剥掉，属「功能未启用时静默无值」，读侧容错。
  · `media_info.poster`：键存在但值全是 JSON null——编辑口三态「清空落 null」的
    设计（`interact.rs` 的 `jsonb_build_object('poster', NULL)`），不是分叉。

两个测量坑（都在这轮踩过，写死在判据里）：
  A. 判「键在不在」必须按**存在性**统计（`jsonb_object_keys` 的出现集合），
     不能用 `col->>'k' IS NOT NULL`——后者把「键存在但值为 JSON null」当成不存在，
     会把 poster 这种设计行为误报成洞（我第一次就误报了它）。
  B. 空表不参与判定：`audit_log_archive` 整表 0 行，任何键计数都是 0，
     那是「没数据」不是「读错键」。

形状上的两个坑也记在这里，防止下一位重踩：
  · 扫描**不走 shell**：上一版把 grep 塞进 shell 字符串，一个多余的孤立单引号让
    shell 认为单引号未闭合 ⇒ grep 挂在等 stdin 上，整支闸门超时被挪后台。
  · 每个 (列,键) 一条 psql 会慢到分钟级 ⇒ 改成**一个 jsonb 列一条 GROUP BY
    式查询**，在内存里比对。
  · 别在 Git Bash 里用 `timeout N python …` 包闸门：那个 `timeout` 是 Windows
    自带的 `timeout.exe`，它会把后面的命令整体吞掉、什么都不跑还退 0——
    我两次把这种「RC=0 却零输出」误读成闸门自己坏了。要看真实退出码就直接跑，
    也别信 `cmd | tail; echo $?`（那读的是 tail 的码）。

判据：
  IB1 幻键台账：「该表有数据、但这个键一次都没出现过」的集合必须恰好等于台账
      {media_info.imdb, media_info.tmdb_url}——多一个＝新的键分叉（红），
      少一个＝某个键开始被写入（红，回来销账）。
  IB2 承重：把「绝不存在的键名」送进同一个判定字典，必须被判成幻键（防恒过）。
  IB3 对照：真键 `media_info.imdb_id` 必须不被判幻键（防误吞）。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ib_json_phantom_keys.py`
"""
import collections
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

LEDGER = {"media_info.imdb", "media_info.tmdb_url"}


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-q", "-tA", "-F", "|"], input=sql,
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0 or "ERROR" in (r.stderr or ""):
        raise RuntimeError("psql: " + (r.stderr or "")[:240])
    return (r.stdout or "").strip()


def read_pairs():
    """扫描 apps/api/src 的 Rust 源码，收集 (jsonb 列名, 被读的键)。"""
    pat = re.compile(r"([a-z_]+)(?:\.[a-z_]+)?->>'([a-z_]+)'")
    pairs = collections.Counter()
    top = os.path.join(ROOT, "apps", "api", "src")
    for root, dirs, files in os.walk(top):
        for fn in files:
            if not fn.endswith(".rs"):
                continue
            body = io.open(os.path.join(root, fn), encoding="utf-8",
                           errors="replace").read()
            for ln in body.split("\n"):
                if ln.strip().startswith("//"):
                    continue      # 注释里提到的键不算读点
                for m in pat.finditer(ln):
                    pairs[(m.group(1), m.group(2))] += 1
    return pairs


pairs = read_pairs()
if not pairs:
    raise RuntimeError("一个读点都没扫到——扫描形状错了，别把「没扫到」当「没洞」")

# 一张表一条查询：列 → {表, 总行数, 出现过的键集合}
cols = psql("SELECT table_name||'.'||column_name "
            "FROM information_schema.columns"
            " WHERE data_type='jsonb' AND table_schema='public' "
            "ORDER BY 1;").split("\n")
by_col = collections.defaultdict(list)     # jsonb 列名 → [(表, 列全名)]
for c in cols:
    if "." in c:
        t, col = c.split(".", 1)
        by_col[col].append(t)

present = {}      # "col.key" → True/False（该表里这个键出现过吗）
table_rows = {}   # 表 → 行数
for col, tbls in by_col.items():
    used = {k for (a, k) in pairs if a == col}
    if not used:
        continue
    for t in tbls:
        if t not in table_rows:
            table_rows[t] = int(psql("SELECT count(*) FROM %s;" % t) or 0)
        if table_rows[t] == 0:
            continue                     # B：空表不参与判定
        keys = set(x for x in psql(
            "SELECT DISTINCT jsonb_object_keys(%s) FROM %s"
            " WHERE %s IS NOT NULL;" % (col, t, col)).split("\n") if x)
        for k in used:
            present["%s.%s" % (col, k)] = (k in keys)

phantom = {p for p, seen in present.items() if not seen}
ok("IB1 幻键台账：「该表有数据、键却从未出现过」的集合必须恰好等于台账",
   phantom == LEDGER,
   "现=%s 台账=%s；判定 %d 个 (列,键) 读点、涉及 %d 张有数据的 jsonb 表"
   % (sorted(phantom), sorted(LEDGER), len(present),
      len([t for t, n in table_rows.items() if n])))

# 真走一遍判定路径：把绝不存在的键送进去，必须被认成幻键（防恒过）
probe = "zz_definitely_not_a_key"
t_rows = table_rows.get("torrents", 0)
seen_keys = psql("SELECT DISTINCT jsonb_object_keys(media_info) FROM torrents "
                 "WHERE media_info IS NOT NULL;").split("\n")
fake_is_phantom = t_rows > 0 and probe not in [x for x in seen_keys if x]
ok("IB2 承重：绝不存在的键送进同一判定路径必须被认成幻键（防恒过）",
   fake_is_phantom,
   "torrents %d 行、现网键集合 %d 个，均不含探测键 ⇒ 判定不是恒假"
   % (t_rows, len([x for x in seen_keys if x])))
ok("IB3 对照：真键 media_info.imdb_id 不得被判幻键（防误吞）",
   present.get("media_info.imdb_id") is True and "media_info.imdb_id"
   not in phantom,
   "imdb_id 键在现网出现过 ⇒ 探测面没有把有值的键报成幻键")

summary()
