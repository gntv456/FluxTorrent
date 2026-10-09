# -*- coding: utf-8 -*-
"""0323 torrent_artifacts 闸门（阶段 2 game/software 批）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_k_artifact.py

判据：
  1. 发种带 checksums（.sfv）+ changelog 两个 artifact part → 落库两行，
     kind 按文件名推断（checksums/changelog），sha256 非空
  2. parent_torrent_id 更新链：第二发（更新包）挂本体 → 本体详情
     aggregate 的 artifact_children 含更新包
  3. 详情 aggregate 带 artifacts 段（不含正文 body）
  4. 工件超量（>8）拒收 400
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402
from pt_audit_g_logcheck import (  # noqa: E402
    make_torrent, multipart, BOUNDARY,
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


def upload_with_artifacts(tok, tor, arts, name, parent=None):
    """arts: [(part_name, filename, text)]；parent: parent_torrent_id 字段"""
    fields = {
        "name": name, "small_descr": "artifact gate",
        "descr": "audit artifacts", "category_id": "1",
    }
    if parent:
        fields["parent_torrent_id"] = str(parent)
    parts = [("file", "audit.torrent", tor)]
    parts += [(pname, fname, text.encode()) for pname, fname, text in arts]
    body = multipart(fields, parts)
    import urllib.request
    import urllib.error
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

    # ---- 1. 本体 + 两工件
    s, r = upload_with_artifacts(
        tok, make_torrent(name="Audit.Art.Base"),
        [("artifact", "setup.sfv", "game.exe 0123456789abcdef0123456789abcdef"),
         ("artifact", "changelog_1.0.txt", "v1.0 首发")],
        "Audit.Art.Base")
    ok("本体发种成功（带 2 工件）", s == 200 and r.get("code") == 0,
       "%s %s" % (s, r))
    base_id = (r.get("data") or {}).get("id")
    if not base_id:
        raise SystemExit("本体发种失败")
    rows = psql("SELECT kind || '|' || filename FROM torrent_artifacts "
                "WHERE torrent_id=%s ORDER BY kind" % base_id)
    ok("两工件落库且 kind 推断正确",
       "checksums|setup.sfv" in rows and "changelog|changelog_1.0.txt" in rows,
       rows)
    sha = psql("SELECT COALESCE(sha256, '') FROM torrent_artifacts "
               "WHERE torrent_id=%s AND kind='checksums'" % base_id)
    ok("checksums 工件带 sha256", len(sha) == 64, sha[:20])

    # ---- 2. 详情聚合
    psql("UPDATE torrents SET approval_status=1, approved_at=now() "
         "WHERE id=%s" % base_id)
    s, agg = call("GET", "/torrents/%s/aggregate" % base_id, token=tok)
    arts = (agg.get("data") or {}).get("artifacts") or []
    ok("aggregate 带 artifacts 段（2 行，无正文）",
       len(arts) == 2 and all("body" not in a for a in arts), arts)

    # ---- 3. 更新链：更新包挂本体
    s, r = upload_with_artifacts(
        tok, make_torrent(name="Audit.Art.Patch"),
        [("artifact", "patch.md5",
          "patch.dat fedcba9876543210fedcba9876543210"),
         ("artifact", "changelog_1.1.txt", "v1.1 修复崩溃")],
        "Audit.Art.Patch", parent=base_id)
    ok("更新包发种成功", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    patch_id = (r.get("data") or {}).get("id")
    linked = psql("SELECT COALESCE(parent_torrent_id::text,'NULL') "
                  "FROM torrent_artifacts WHERE torrent_id=%s LIMIT 1"
                  % patch_id)
    ok("更新链挂到本体", linked == str(base_id), linked)
    # 本体详情有 30s 共享缓存（步骤 2 已聚合过一次）——先清再查
    import os as _os
    _rp = _os.environ.get("REDIS_PASSWORD", "")
    if not _rp:
        _env = _os.environ.copy()
        _rp = subprocess.run(
            ["docker", "exec", "flux-redis", "printenv", "REDIS_PASSWORD"],
            capture_output=True, text=True).stdout.strip()
    _del = ("redis-cli -a '%s' DEL cache:tdetail:v1:%s:1 "
            "cache:tdetail:v1:%s:0" % (_rp, base_id, base_id))
    subprocess.run(["docker", "exec", "flux-redis", "sh", "-c", _del],
                   capture_output=True)
    s, agg = call("GET", "/torrents/%s/aggregate" % base_id, token=tok)
    children = (agg.get("data") or {}).get("artifact_children") or []
    ok("本体详情的更新链含更新包",
       any(c.get("torrent_id") == patch_id for c in children), children)

    # ---- 4. 超量拒收
    many = [("artifact", "f%d.sfv" % i, "x %d" % i) for i in range(9)]
    s, r = upload_with_artifacts(tok, make_torrent(name="Audit.Art.Many"),
                                 many, "Audit.Art.Many")
    ok("9 件工件拒收（400）", s == 400, "%s %s" % (s, r.get("message", "")))
    ok("拒收不留残种",
       psql("SELECT count(*) FROM torrents WHERE name='Audit.Art.Many'") == "0")

    # ---- 清理
    psql("DELETE FROM torrents WHERE id IN (%s, %s)" % (base_id, patch_id))


if __name__ == "__main__":
    main()
    summary()
