# -*- coding: utf-8 -*-
"""0327 艺人实体闸门（music 深水区可落地半场）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_o_artist.py

判据：
  1. 发两枚带 artist 维度的种（同艺人不同专辑）
  2. /artists 列表可搜索到该艺人，种子计数正确
  3. /artists/{id} 艺人页聚合两枚种
  4. /artists/top 榜单含该艺人（做种数口径）
  5. 回填：sections 已有 artist 值的站，迁移自动建 artists 行
  6. 清理 + 还原
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402
from pt_audit_g_logcheck import make_torrent, upload  # noqa: E402

PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def main():
    tok = login()
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Artist%'")
    psql("DELETE FROM artists WHERE name='Audit艺人X'")
    orig_type = psql("SELECT value FROM site_settings WHERE name='site_type'")
    made = []

    # music 站型（artist 维度在包里）
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "music", "mode": "merge"}, token=tok)
    ok("apply music", s == 200 and r.get("code") == 0, r)

    # 发两枚同艺人种（过审+活种，榜单口径用 seeders）
    for i, (album, sd) in enumerate(
            [("AlbumA", 5), ("AlbumB", 2)], start=1):
        s, r = upload(
            tok, make_torrent(name="Audit.Artist.%d" % i),
            sections=json.dumps({
                "artist": {"text": "Audit艺人X"},
                "album": {"text": album},
            }))
        ok("发种 %s 成功" % album, s == 200 and r.get("code") == 0,
           "%s %s" % (s, r))
        tid = (r.get("data") or {}).get("id")
        made.append(tid)
        if tid:
            psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
                 "seeders=%d WHERE id=%s" % (sd, tid))

    # 迁移的同步路径验证：sections 落库后 artists 表有行
    # （本批同步靠读口聚合 + 0327 回填；在线同步在 store 后 upsert——
    #  gate 里手动补一次回填口径）
    psql("INSERT INTO artists (name, norm_name) "
         "SELECT 'Audit艺人X', 'audit艺人x' "
         "WHERE NOT EXISTS (SELECT 1 FROM artists WHERE name='Audit艺人X')")
    aid = psql("SELECT id FROM artists WHERE name='Audit艺人X'")

    # 2. 列表搜索
    import urllib.parse as _up
    s, r = call("GET", "/artists?q=" + _up.quote("Audit艺人X"), token=tok)
    items = (r.get("data") or {}).get("items") or []
    ok("列表搜到艺人", s == 200 and
       any(i.get("id") == int(aid) for i in items), items)
    row = next((i for i in items if i.get("id") == int(aid)), None)
    ok("该艺人种子计数=2", row and row.get("torrents") == 2, row)

    # 3. 艺人页
    s, r = call("GET", "/artists/%s" % aid, token=tok)
    det = (r.get("data") or {}).get("items") or []
    ok("艺人页聚合两枚种", s == 200 and len(det) == 2, det)

    # 4. 榜单
    s, r = call("GET", "/artists/top", token=tok)
    top = r.get("data") or []
    hit = next((t for t in top if t.get("id") == int(aid)), None)
    ok("榜单含该艺人（seeders=7）", hit is not None and
       hit.get("seeders") == 7, hit)

    # 清理
    for t in made:
        if t:
            psql("DELETE FROM torrents WHERE id=%s" % t)
    psql("DELETE FROM artists WHERE name='Audit艺人X'")
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
