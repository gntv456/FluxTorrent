# -*- coding: utf-8 -*-
"""批次 3b 闸门：审核台分型字段（0326，admin review sections/artifacts/logs）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_n_review_typed.py

判据：
  1. 发一枚带维度 + 工件 + 日志的待审种（movie 站型：season/episode）
  2. /admin/reviews 队列行带 sections（含「季」键与值）、artifacts
     （checksums 工件）、logs（discs≥1）
  3. 无维度的待审种 sections 为 NULL（不是 {} 假象）
  4. 清理
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402
from pt_audit_g_logcheck import (  # noqa: E402
    make_torrent, multipart, BOUNDARY, EAC_100,
)
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


def upload_full(tok, name, sections, arts, logs=()):
    fields = {"name": name, "small_descr": "typed review gate",
              "descr": "audit", "category_id": "1",
              "sections": json.dumps(sections)}
    parts = [("file", "a.torrent", make_torrent(name=name))]
    parts += [("artifact", fn, data.encode()) for fn, data in arts]
    parts += [("log", fn, data) for fn, data in logs]
    body = multipart(fields, parts)
    req = urllib.request.Request(BASE + "/torrents", method="POST", data=body)
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(req, timeout=30)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}


def main():
    tok = login()
    orig_type = psql("SELECT value FROM site_settings WHERE name='site_type'")

    # movie 站型（season/episode 维度在包里）
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "movie", "mode": "merge"}, token=tok)
    ok("apply movie", s == 200 and r.get("code") == 0, r)
    # 开 tag 让日志落库（不 require）
    psql("INSERT INTO site_settings (name, value) VALUES "
         "('logcheck_policy','tag') ON CONFLICT (name) DO UPDATE "
         "SET value='tag'")
    season = psql("SELECT id FROM section_dict WHERE kind='season' "
                  "AND name='第二季' LIMIT 1")
    secs = {"season": {"dict_ids": [int(season)]},
            "episode_first": {"number": 3},
            "episode_last": {"number": 10}}
    s, r = upload_full(tok, "Audit.Review.Typed", secs,
                       [("changelog_1.1.txt", "v1.1 修复"),
                        ("setup.sfv", "x 0123")],
                       logs=[("disc1.log", EAC_100)])
    ok("待审种发出（维度+工件+日志）", s == 200 and r.get("code") == 0,
       "%s %s" % (s, r))
    tid = (r.get("data") or {}).get("id")
    if not tid:
        raise SystemExit("发种失败")

    # root（class≥92）发种即免审——拉回待审态才能进队列
    psql("UPDATE torrents SET approval_status=0, approved_at=NULL "
         "WHERE id=%s" % tid)
    # 无维度对照种
    s, r = upload_full(tok, "Audit.Review.Plain", {}, [])
    plain = (r.get("data") or {}).get("id")
    if plain:
        psql("UPDATE torrents SET approval_status=0, approved_at=NULL "
             "WHERE id=%s" % plain)

    # 队列读回
    s, q = call("GET", "/admin/reviews?limit=200&oldest_first=false",
                token=tok)
    items = (q.get("data") or {}).get("items") or []
    row = next((i for i in items if i.get("id") == tid), None)
    ok("队列含待审种", row is not None, tid)
    if row:
        secs_out = row.get("sections") or {}
        ok("sections 带季（值=第二季）",
           secs_out.get("季", {}).get("v") == "第二季", secs_out)
        ok("sections 带起始集（v=3）",
           secs_out.get("起始集", {}).get("v") == "3", secs_out)
        arts = row.get("artifacts") or []
        ok("artifacts 两件（changelog+checksums）",
               len(arts) == 2 and
               {a.get("kind") for a in arts} == {"changelog", "checksums"},
               arts)
        logs = row.get("logs") or {}
        ok("logs 概要（discs=1, min=100）",
           logs.get("discs") == 1 and logs.get("min") == 100, logs)
    row2 = next((i for i in items if i.get("id") == plain), None)
    ok("无维度种 sections 为 NULL",
       row2 is not None and row2.get("sections") is None,
       row2 and row2.get("sections"))

    # 清理
    for t in (tid, plain):
        if t:
            psql("DELETE FROM torrents WHERE id=%s" % t)
    psql("UPDATE site_settings SET value='off' WHERE name='logcheck_policy'")
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
