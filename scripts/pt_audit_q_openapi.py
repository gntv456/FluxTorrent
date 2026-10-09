# -*- coding: utf-8 -*-
"""0329 开放 API 分型字段闸门（批次 3 尾巴）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_q_openapi.py

判据（走 API Token：/open/* 要求 token）：
  1. 发一枚带维度+工件+日志的过审种
  2. /open/recent 行带 dimensions（含季/起始集摘要）
  3. /open/announces 行同样带 dimensions
  4. /open/torrents/{id} 带 sections（全值）+ artifacts + logs
  5. 清理还原
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402
from pt_audit_g_logcheck import (  # noqa: E402
    make_torrent, multipart, BOUNDARY, EAC_100,
)
import urllib.error  # noqa: E402
import urllib.request  # noqa: E408

PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


AUDIT_TOKEN = "auditopenapi0000000000000000000009"


def mint_token():
    """铸一枚 root 的 read token（sha3-256(token) 存 token_hash，
    与 openapi_http::hash_token 同口径；反复调用幂等）。"""
    import hashlib
    th = hashlib.sha3_256(AUDIT_TOKEN.encode()).hexdigest()
    psql("INSERT INTO api_tokens (user_id, name, token_hash, scopes) "
         "SELECT id, 'audit-openapi', '%s', '{read}' "
         "FROM users WHERE username='root' "
         "ON CONFLICT (token_hash) DO NOTHING" % th)
    return AUDIT_TOKEN


def opencall(path, token):
    req = urllib.request.Request(
        "http://127.0.0.1:8080/api/v1" + path)
    req.add_header("Authorization", "Token " + token)
    try:
        r = urllib.request.urlopen(req, timeout=20)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        body = e.read()
        try:
            return e.code, json.loads(body)
        except Exception:
            return e.code, {"raw": body[:200].decode("utf-8", "replace")}


def upload_full(tok, name, sections, arts, logs=()):
    fields = {"name": name, "small_descr": "openapi gate",
              "descr": "audit", "category_id": "1",
              "sections": json.dumps(sections)}
    parts = [("file", "a.torrent", make_torrent(name=name))]
    parts += [("artifact", fn, data.encode()) for fn, data in arts]
    parts += [("log", fn, data) for fn, data in logs]
    body = multipart(fields, parts)
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
    otok = mint_token()
    orig_type = psql("SELECT value FROM site_settings WHERE name='site_type'")
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Open%'")

    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "movie", "mode": "merge"}, token=tok)
    ok("apply movie", s == 200 and r.get("code") == 0, r)
    psql("INSERT INTO site_settings (name, value) VALUES "
         "('logcheck_policy','tag') ON CONFLICT (name) DO UPDATE "
         "SET value='tag'")
    season = psql("SELECT id FROM section_dict WHERE kind='season' "
                  "AND name='第二季' LIMIT 1")
    secs = {"season": {"dict_ids": [int(season)]},
            "episode_first": {"number": 3},
            "episode_last": {"number": 10}}
    s, r = upload_full(tok, "Audit.Open.T", secs,
                       [("setup.sfv", "x 0123")],
                       logs=[("d.log", EAC_100)])
    tid = (r.get("data") or {}).get("id")
    ok("发种成功", s == 200 and tid, r)
    psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
         "seeders=1 WHERE id=%s" % tid)

    # 2. /open/recent dimensions
    s, r = opencall("/open/recent?limit=50", otok)
    items = (r.get("data") or []) if isinstance(r.get("data"), list) else \
        ((r.get("data") or {}).get("items") or [])
    row = next((i for i in items if i.get("id") == tid), None)
    ok("/open/recent 200 且含探针", row is not None, (s, tid))
    dims = (row or {}).get("dimensions") or {}
    ok("recent dimensions 带季", dims.get("季") == "第二季", dims)
    ok("recent dimensions 带起始集", dims.get("起始集") == "3", dims)

    # 3. /open/announces dimensions
    s, r = opencall("/open/announces?since_id=%d" % (tid - 1), otok)
    items = ((r.get("data") or {}).get("items")) or []
    row = next((i for i in items if i.get("id") == tid), None)
    dims = (row or {}).get("dimensions") or {}
    ok("announces dimensions 带季", dims.get("季") == "第二季", dims)

    # 4. 详情三段（429 限流兜底：recent/announces 已耗配额，重试一次）
    s, r = opencall("/open/torrents/%s" % tid, otok)
    if s == 429:
        import time as _t
        _t.sleep(2)
        s, r = opencall("/open/torrents/%s" % tid, otok)
    d = r.get("data") or {}
    if not d:
        d = ({"sections": {}, "artifacts": [], "logs": []})
    # sections 键是 kind（label 在值对象里）——与 /torrents 详情同形
    sec = (d.get("sections") or {}).get("episode_first") or {}
    ok("详情 sections 全值（起始集=3）",
       str(sec.get("name")) == "3"
       or "3" in (sec.get("values") or []),
       sec)
    arts = d.get("artifacts") or []
    ok("详情 artifacts 含 checksums",
       any(a.get("kind") == "checksums" for a in arts), arts)
    logs = d.get("logs") or []
    ok("详情 logs 含 min=100",
       any((l.get("log_score") or 0) == 100 for l in logs), logs)

    # 清理
    psql("DELETE FROM torrents WHERE id=%s" % tid)
    psql("UPDATE site_settings SET value='off' WHERE "
         "name='logcheck_policy'")
    psql("DELETE FROM api_tokens WHERE name='audit-openapi'")
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
