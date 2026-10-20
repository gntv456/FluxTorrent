# -*- coding: utf-8 -*-
"""闸门 IE：缺存档种的文件列表不许演成「这种没文件」（批二百一十六）。

现网实测（root，:8080）——同一颗种在两个出口讲相反的故事：
  · `GET /torrents/49421/files` → **200 `data: []`**
  · `GET /torrents/49421/download` → 400「种子文件缺失（原始 .torrent
    未存档），请联系管理组补档」（HW6 已钉这句）
49421 是真人可见的演示种（沙丘：复仇 4K，`imdb_id` 非空、已过审）。
空数组在界面上长得像「正常结果」，用户只会理解为「这颗种里没有文件」，
而真正的事实是**这条记录是坏的**：它连原始 .torrent 都没存档，所以
文件清单从来没被解析出来过。

真库台账（本轮实测）：过审种里「无文件清单」7 枚，这 7 枚**同时**无原始
存档（`files` 0 行且 `torrent_files` 0 行）；「有清单但无存档」0 枚
⇒ `files` 为空**总是**坏记录的证据，从来不是「单文件种没东西可列」
（单文件种合法地有 1 行）。属「失败演成空数据」——比演成服务故障更坏。

两张表别搞混（我这次先踩了一次）：文件清单在 `files(torrent_id,
file_index, path, size)`，原始存档在 `torrent_files(torrent_id, raw)`。

判据：
  IE1 活体在册：对台账里的缺存档种，若 `/files` 仍回 200+空数组**且**没有
      任何可行动信号（响应头/字段），同时下载口回「补档」中文 ⇒ 判在册
      （PASS）。修法落地后要把这条翻成硬判据（同 HX3/HX4 的演进路径）。
  IE2 对照：有存档种 `/files` 必须非空 ⇒ 证明探测器不是「恒返空」。
  IE3 台账棘轮：「过审 + 无清单 + 无存档」种数只降不升（基线 7）。
  IE4 承重：把 IE2 那枚正常种喂给 IE1 的「该被判在册」谓词必须为假
      ⇒ 在册判据不是对所有种恒真。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ie_filelist_blind.py`
"""
import os
import sys
import urllib.error
import urllib.parse
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import BASE, login, ok, summary  # noqa: E402

LEDGER_BASE = 7


def get(path, tok):
    req = urllib.request.Request(
        BASE + path,
        headers={"Authorization": "Bearer " + tok,
                 "Accept-Language": "zh-CN"})
    try:
        with urllib.request.urlopen(req, timeout=20) as x:
            return x.status, x.read(), dict(x.headers)
    except urllib.error.HTTPError as e:
        return e.code, e.read(), dict(e.headers or {})


def psql_one(q):
    import subprocess
    x = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-q", "-tA"], input=q, capture_output=True,
        text=True, encoding="utf-8", errors="replace")
    if x.returncode != 0 or "ERROR" in (x.stderr or ""):
        raise RuntimeError("psql: " + (x.stderr or "")[:200])
    return (x.stdout or "").strip()


SQL_LEDGER = (
    "SELECT count(*) FROM torrents t WHERE t.approval_status = 1 "
    "AND NOT EXISTS (SELECT 1 FROM files f WHERE f.torrent_id = t.id) "
    "AND NOT EXISTS (SELECT 1 FROM torrent_files tf "
    "WHERE tf.torrent_id = t.id)")
SQL_ONE = (
    "SELECT t.id FROM torrents t WHERE t.approval_status = 1 "
    "AND NOT EXISTS (SELECT 1 FROM files f WHERE f.torrent_id = t.id) "
    "AND NOT EXISTS (SELECT 1 FROM torrent_files tf "
    "WHERE tf.torrent_id = t.id) ORDER BY t.id DESC LIMIT 1")
SQL_GOOD = (
    "SELECT t.id FROM torrents t WHERE t.approval_status = 1 "
    "AND EXISTS (SELECT 1 FROM files f WHERE f.torrent_id = t.id) "
    "AND EXISTS (SELECT 1 FROM torrent_files tf "
    "WHERE tf.torrent_id = t.id) ORDER BY t.id LIMIT 1")

bad_id = int(psql_one(SQL_ONE) or 0)
good_id = int(psql_one(SQL_GOOD) or 0)
tok = login("root", "password123")


def files_empty(tid):
    st, body, hdrs = get("/torrents/%d/files" % tid, tok)
    txt = body.decode("utf-8", "replace")
    blind = '"data":[]' in txt.replace(" ", "")
    return st, blind, hdrs, txt


def download_msg(tid):
    st, body, _h = get("/torrents/%d/download" % tid, tok)
    txt = body.decode("utf-8", "replace")
    return st, ("补档" in txt or "未存档" in txt)


if not bad_id or not good_id:
    ok("IE1 活体在册：台账里的缺存档种与正常种都取得到（否则无从对照）",
       False, "缺存档种=%s 正常种=%s——夹具缺失时判据不出结论"
       % (bad_id, good_id))
else:
    st_b, blind_b, hdrs_b, body_b = files_empty(bad_id)
    dl_st, dl_tells = download_msg(bad_id)
    signals = [k for k in hdrs_b if "archive" in k.lower()
               or "missing" in k.lower()] + [
        k for k in ("archive_missing", "hint") if k in body_b]
    ok("IE1 活体在册：缺存档种(id=%d)的 /files 仍是 200+空数组且无可行动"
       "信号，而下载口(id 同)已回「补档」中文 ⇒ 两出口口径相反" % bad_id,
       st_b == 200 and blind_b and not signals and dl_st == 400
       and dl_tells,
       "files http=%s 空数组=%s 信号=%s；download http=%s 说补档=%s"
       % (st_b, blind_b, signals, dl_st, dl_tells))

    st_g, blind_g, _h2, _b2 = files_empty(good_id)
    ok("IE2 对照：有存档种(id=%d)的 /files 必须非空（探测器不恒返空）"
       % good_id, st_g == 200 and not blind_g,
       "http=%s 空数组=%s" % (st_g, blind_g))
    ok("IE4 承重：IE1 那条「在册」谓词对正常种必须为假",
       not (st_g == 200 and blind_g), "正常种空数组=%s" % blind_g)

n = int(psql_one(SQL_LEDGER) or 0)
ok("IE3 台账棘轮：「过审 + 无文件清单 + 无原始存档」种数只降不升",
   n <= LEDGER_BASE, "现=%d 基线=%d；每少一枚就是少一颗坏记录"
   % (n, LEDGER_BASE))

summary()
