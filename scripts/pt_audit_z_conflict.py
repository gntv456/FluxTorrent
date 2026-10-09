# -*- coding: utf-8 -*-
"""站型成熟度补齐 · G13 闸门（文件树冲突检测）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_z_conflict.py

判据（同组内重复文件路径 = Gazelle file conflict 语义）：
  1. 未入组的种 → group_id=null，无冲突（不报错）
  2. 同组且路径重合 → 命中冲突，返回对方种 id/name/path
  3. 同组但路径不重合 → 0 冲突
  4. 不同组即使路径相同 → 不算冲突（组是冲突的比较域）
  5. 未过审的对方种不计入（approval_status=1 才算）
  6. 不存在的种 → 404
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402
from pt_audit_g_logcheck import make_torrent, multipart, BOUNDARY  # noqa: E402
import urllib.error  # noqa: E402
import urllib.request  # noqa: E402

PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]

PATH_A = "AuditConflict/shared.flac"


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def upload(tok, name):
    fields = {"name": name, "small_descr": "gate", "descr": "audit",
              "category_id": "1"}
    body = multipart(fields, [("file", "audit.torrent",
                               make_torrent(name=name))])
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


def mk(name, path, group=None, live_flag=True):
    """造一颗种 + 指定其文件路径（直接落 files 表，绕开 torrent 解析细节）。"""
    s, r = upload(tok=TOK, name=name)
    tid = (r.get("data") or {}).get("id")
    psql("DELETE FROM files WHERE torrent_id=%s" % tid)
    psql("INSERT INTO files (torrent_id, file_index, path, size) "
         "VALUES (%s, 0, '%s', 1024)" % (tid, path))
    if group is not None:
        psql("UPDATE torrents SET group_id=%s WHERE id=%s" % (group, tid))
    psql("UPDATE torrents SET approval_status=%d, seeders=1 WHERE id=%s"
         % (1 if live_flag else 0, tid))
    return tid


TOK = None


def main():
    global TOK
    TOK = login()
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Conflict%'")
    psql("DELETE FROM torrent_groups WHERE name IN "
         "('AuditConflictG1','AuditConflictG2')")
    # psql -t 对 INSERT ... RETURNING 仍会带命令状态行，改用 SELECT 取值
    psql("INSERT INTO torrent_groups (name) VALUES ('AuditConflictG1')")
    psql("INSERT INTO torrent_groups (name) VALUES ('AuditConflictG2')")
    g1 = psql("SELECT id FROM torrent_groups WHERE name='AuditConflictG1'")
    g2 = psql("SELECT id FROM torrent_groups WHERE name='AuditConflictG2'")

    a = mk("Audit.Conflict.A", PATH_A, g1)
    b = mk("Audit.Conflict.B", PATH_A, g1)
    c = mk("Audit.Conflict.C", "AuditConflict/other.flac", g1)
    d = mk("Audit.Conflict.D", PATH_A, g2)
    made = [a, b, c, d]

    # 1. 同组同路径 → 冲突
    s, r = call("GET", "/torrents/%s/file-conflicts" % a, token=TOK)
    d1 = r.get("data") or {}
    ok("同组同路径命中冲突", d1.get("total") == 1, d1)
    hit = (d1.get("conflicts") or [{}])[0]
    ok("冲突指向 B 且带路径",
       hit.get("other_id") == b and hit.get("path") == PATH_A, hit)

    # 2. 反向对称
    s, r = call("GET", "/torrents/%s/file-conflicts" % b, token=TOK)
    ok("反向同样命中", (r.get("data") or {}).get("total") == 1, r)

    # 3. 不同组不算冲突
    s, r = call("GET", "/torrents/%s/file-conflicts" % d, token=TOK)
    ok("不同组不算冲突", (r.get("data") or {}).get("total") == 0, r)

    # 4. 路径不重合 → 0
    s, r = call("GET", "/torrents/%s/file-conflicts" % c, token=TOK)
    ok("路径不重合 → 0 冲突", (r.get("data") or {}).get("total") == 0, r)

    # 5. 未过审的对方不计入
    psql("UPDATE torrents SET approval_status=0 WHERE id=%s" % b)
    s, r = call("GET", "/torrents/%s/file-conflicts" % a, token=TOK)
    ok("未过审的对方不计入", (r.get("data") or {}).get("total") == 0, r)
    psql("UPDATE torrents SET approval_status=1 WHERE id=%s" % b)

    # 6. 未入组 → group_id=null
    e = mk("Audit.Conflict.E", PATH_A, None)
    made.append(e)
    s, r = call("GET", "/torrents/%s/file-conflicts" % e, token=TOK)
    ok("未入组返回 group_id=null",
       (r.get("data") or {}).get("group_id") is None and
       (r.get("data") or {}).get("total") == 0, r)

    # 7. 404
    s, r = call("GET", "/torrents/999999999/file-conflicts", token=TOK)
    ok("不存在的种 → 404", s == 404, "%s %s" % (s, r))

    for t in made:
        psql("DELETE FROM torrents WHERE id=%s" % t)
    psql("DELETE FROM torrent_groups WHERE name IN "
         "('AuditConflictG1','AuditConflictG2')")


if __name__ == "__main__":
    main()
    summary()
