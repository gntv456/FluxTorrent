# -*- coding: utf-8 -*-
"""GC 闸门：过审种的「原始 .torrent 存档」完整性（批一百七十）。

事实（本轮实测）：`torrent_files(torrent_id, raw)` 全仓**只有一个生产者**——
`publish_http/upload.rs` 的真人上传口。所以凡不走上传口入库的种（闸门夹具、
演示数据、将来任何「内容包导入种」的链路）都天生没有存档，会员点下载拿到的是
`400 / code=1002 参数校验失败: 种子文件缺失（原始 .torrent 未存档…）`。

现网读数：过审种 164 条，其中 **25 条没有 raw**——
  · 18 条是 AN/AO 等闸门的夹具种（名字 `AnTeam.s*` / `AoExam.t.*`，size=1024）；
  · 7 条是本程序 `e2e_round10.py` 造的素材种（`沙丘：复仇｜4K REMUX…` 之类，
    size 7–92GB、numfiles 1–24，看着就是真条目）。
也就是说：**这批红账是我自己的残料，不是产品洞**。正因如此更要钉住——
残料会以「像真种」的形态混进公开目录（计数、分类页、艺人页都读它），
而产品侧一旦上线「内容包带种」，同一个坑会变成真的会员可见故障。

判据分层：
  GC1 台账棘轮（只降不升）：缺 raw 的过审种 ≤ 25；
  GC2 正控：有 raw 的过审种抽样下载必须 200 且是合法 bencode；
  GC3 缺 raw 的种下载口不得 5xx（现状 400/1002，先按在册读数报，等站方
      定「数据缺失该用哪个码」——把服务器端的数据缺席报成「参数校验失败」
      是同 P-401 那一族的信封问题）；
  GC4 静态：产品侧写入 raw 的点仍必须存在（上传口一断，全站下载全废）。

用法：`python -u scripts/pt_audit_gc_seed_archive_parity.py`
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import call, login, ok, summary  # noqa: E402

NO_RAW_CEIL = 25
UPLOAD = "apps/api/src/publish_http/upload.rs"


def one(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        raise RuntimeError("psql 失败: " + (r.stderr or "")[:300])
    return (r.stdout or "").strip()


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def cut(s):
    return "\n".join(
        ln for ln in s.split("\n") if not ln.strip().startswith("//"))


approved = one("SELECT count(*) FROM torrents WHERE approval_status = 1")
no_raw = one("SELECT count(*) FROM torrents t WHERE t.approval_status = 1 "
             "AND NOT EXISTS (SELECT 1 FROM torrent_files f "
             "WHERE f.torrent_id = t.id)")
probe_like = one("SELECT count(*) FROM torrents t WHERE t.approval_status = 1 "
                 "AND t.name ~ 'AoExam|AnTeam|Probe|e2e|demo|REMUX|BluRay|HDR' "
                 "AND NOT EXISTS (SELECT 1 FROM torrent_files f "
                 "WHERE f.torrent_id = t.id)")
ok("GC1 缺原始存档的过审种 %s ≤ 台账上界 %s（只降不升）"
   % (no_raw, NO_RAW_CEIL),
   int(no_raw or 0) <= NO_RAW_CEIL,
   "每多一条就是又多一个「列表里看得见、下载拿不到」的种；本轮读数全部"
   "来自闸门夹具与本程序素材种（其中 %s 条名字就能认出来）" % probe_like)
print("   [读数] 过审种 %s 条；缺 raw %s 条；按名字可认出是夹具/素材的 %s 条"
      % (approved, no_raw, probe_like))

up = cut(rd(UPLOAD))
ok("GC4 产品侧仍必须有写 torrent_files.raw 的点（上传口）",
   "INSERT INTO torrent_files (torrent_id, raw)" in up,
   "这个点一断，全站下载全废——它是 raw 的唯一生产者")

TOK = login("root")
if TOK and int(approved or 0) > 0:
    good = one("SELECT string_agg(id::text, ',') FROM (SELECT t.id FROM "
               "torrents t WHERE t.approval_status = 1 AND exists "
               "(SELECT 1 FROM torrent_files f WHERE f.torrent_id = t.id) "
               "ORDER BY t.id LIMIT 3) x")
    sample = [s for s in (good or "").split(",") if s]
    got = []
    for tid in sample:
        st, b = call("GET", "/torrents/%s/download" % tid, None, TOK, raw=True)
        bb = b if isinstance(b, bytes) else str(b).encode()
        got.append((tid, st, len(bb), bb[:1] == b"d"))
    ok("GC2 正控：有存档的过审种抽样下载必须 200 且首字节是 bencode 字典",
       bool(got) and all(st == 200 and n > 80 and is_b
                         for _t, st, n, is_b in got),
       "抽样(id,http,字节,是否bencode)=%s" % got)
    bad = one("SELECT min(t.id) FROM torrents t WHERE t.approval_status = 1 "
              "AND NOT EXISTS (SELECT 1 FROM torrent_files f "
              "WHERE f.torrent_id = t.id)")
    st, b = call("GET", "/torrents/%s/download" % bad, None, TOK, raw=True)
    txt = b if isinstance(b, str) else (b or b"").decode("utf-8", "replace")
    m = re.search(r'"message":"([^"]{0,80})', txt)
    ok("GC3 缺存档的种（id=%s）下载口不得 5xx（实测 http=%s）" % (bad, st),
       400 <= st < 500, "5xx＝把数据缺席演成服务故障，运维会被叫错方向")
    print("   [在册] 缺 raw 的下载信封：http=%s message=%s"
          "  ——数据缺席目前报成「参数校验失败」（同 P-401 那一族），"
          "该用哪个码待站方定；定完把这条改成硬判据" % (st, (m and m.group(1))))

summary()
