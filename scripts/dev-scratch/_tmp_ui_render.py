# -*- coding: utf-8 -*-
"""渲染层走查：登录态页面的 HTML 里必须出现**库里的真实数据**，且无加载失败文案。"""
import re
import subprocess
import sys

sys.path.insert(0, "scripts")
from pt_audit_lib import jwt_token, psql_stdin

TOK = jwt_token("root")
COOKIE = "flux_token=%s; flux.session=%s" % (TOK, TOK)


def q(sql):
    return psql_stdin(sql).strip()


def html_of(path):
    r = subprocess.run(
        ["curl", "-sL", "--max-time", "30",
         "-H", "Cookie: " + COOKIE, "http://127.0.0.1:3000" + path],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return r.stdout or ""


def esc(s):
    return s.replace("&#x27;", "'").replace("&amp;", "&")


torrent = q("SELECT name FROM torrents WHERE approval_status=1 ORDER BY id "
            "DESC LIMIT 1")
forum = q("SELECT title FROM topics ORDER BY id DESC LIMIT 1")
topic = q("SELECT title FROM topics ORDER BY id DESC LIMIT 1")

CASES = [
    ("/", ["魔力", "签到"], []),
    ("/torrents", [torrent], ["加载失败"]),
    ("/forums", [forum], ["加载失败"]),
    ("/forums/topic/%s" % q("SELECT id FROM topics ORDER BY id DESC LIMIT 1"),
     [topic], ["加载失败"]),
    ("/me", ["root"], ["加载失败"]),
]
print("库里取样：torrent=%r forum=%r" % (torrent[:24], forum[:24]))
for path, must, mustnot in CASES:
    h = esc(html_of(path))
    miss = [m for m in must if m and m not in h]
    bad = [m for m in mustnot if m in h]
    print("%-24s len=%7d 缺=%s 出现不该有的=%s"
          % (path, len(h), miss or "无", bad or "无"))
