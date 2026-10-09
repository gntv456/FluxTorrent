# -*- coding: utf-8 -*-
"""0330 sports 对阵实体闸门。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_r_match.py

判据：
  1. admin 建赛（英超 2025-26 第 10 轮 切尔西 vs 曼联，未赛无比分）
  2. /matches 按队筛命中；赛季/轮次筛选命中
  3. 发两枚种挂 match_id（全场+集锦）→ /matches/{id} 对阵页并列两枚
  4. 比分补录 2:1 → 列表行带比分
  5. 悬空 match_id 静默降级（发种不炸、无链）
  6. 清理还原
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402
from pt_audit_g_logcheck import make_torrent, multipart, BOUNDARY  # noqa: E402
import urllib.error  # noqa: E402
import urllib.request  # noqa: E402

PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def upload(tok, name, match_id=None):
    fields = {"name": name, "small_descr": "match gate", "descr": "audit",
              "category_id": "1"}
    if match_id:
        fields["match_id"] = str(match_id)
    body = multipart(fields, [("file", "a.torrent",
                               make_torrent(name=name))])
    req = urllib.request.Request(
        "http://127.0.0.1:8080/api/v1/torrents", method="POST", data=body)
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(req, timeout=30)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        return e.code, {}


def main():
    tok = login()
    orig_type = psql("SELECT value FROM site_settings "
                     "WHERE name='site_type'")
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Match%'")

    # 1. 建赛
    s, r = call("POST", "/admin/matches", {
        "league": "英超", "season": "2025-26", "round": "第 10 轮",
        "home": "切尔西", "away": "曼联",
        "kickoff": "2026-10-11T19:30:00Z"}, token=tok)
    ok("建赛成功", s == 200 and r.get("code") == 0, r)
    mid = (r.get("data") or {}).get("id")

    # 2. 队筛 / 轮筛
    import urllib.parse
    s, r = call("GET", "/matches?team=" + urllib.parse.quote("切尔西"),
                token=tok)
    items = r.get("data") or []
    ok("按队筛命中（主队）", any(m.get("id") == mid for m in items), items)
    s, r = call("GET", "/matches?round=" + urllib.parse.quote("第 10 轮"),
                token=tok)
    items = r.get("data") or []
    ok("按轮筛命中", any(m.get("id") == mid for m in items), items)
    s, r = call("GET", "/matches?team=" + urllib.parse.quote("拜仁"),
                token=tok)
    items = r.get("data") or []
    ok("无关队不命中", not any(m.get("id") == mid for m in items))

    # 3. 两枚种挂链
    for nm in ("Audit.Match.Full", "Audit.Match.Highlights"):
        s, r = upload(tok, nm, match_id=mid)
        ok("发种 %s" % nm, s == 200 and r.get("code") == 0, r)
        tid = (r.get("data") or {}).get("id")
        psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
             "seeders=1 WHERE id=%s" % tid)
    s, r = call("GET", "/matches/%s" % mid, token=tok)
    items = (r.get("data") or {}).get("items") or []
    ok("对阵页并列两枚", len(items) == 2, items)

    # 4. 比分补录
    s, r = call("PUT", "/admin/matches/%s/score" % mid,
                {"home_score": 2, "away_score": 1}, token=tok)
    ok("比分补录", s == 200 and r.get("code") == 0, r)
    s, r = call("GET", "/matches?team=" + urllib.parse.quote("曼联"),
                token=tok)
    row = next((m for m in (r.get("data") or [])
                if m.get("id") == mid), None)
    ok("列表带比分 2:1",
       row and row.get("home_score") == 2 and row.get("away_score") == 1,
       row)

    # 5. 悬空 match_id 降级
    s, r = upload(tok, "Audit.Match.Dangling", match_id=999999)
    ok("悬空 match_id 发种不炸", s == 200 and r.get("code") == 0, r)
    tid = (r.get("data") or {}).get("id")
    linked = psql("SELECT COALESCE(match_id::text,'NULL') FROM torrents "
                  "WHERE id=%s" % tid)
    ok("悬空链静默置空", linked == "NULL", linked)

    # 清理
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Match%'")
    psql("DELETE FROM sport_matches WHERE id=%s" % mid)
    back = psql("SELECT value FROM site_settings WHERE name='site_type'")
    if back != orig_type:
        call("POST", "/admin/site-type-packs/apply",
             {"code": orig_type, "mode": "merge"}, token=tok)
        back = psql("SELECT value FROM site_settings "
                    "WHERE name='site_type'")
    ok("站型还原", back == orig_type, (orig_type, back))


if __name__ == "__main__":
    main()
    summary()
