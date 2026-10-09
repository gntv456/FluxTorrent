# -*- coding: utf-8 -*-
"""站型成熟度补齐 · G3 后半闸门（CUE 表合规判定）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_y_cue.py

判据（G3 后半，Gazelle「HasCue 合规性」口径）：
  1. 合法 CUE（FILE + 连续 TRACK + 每轨 INDEX 01）→ valid=true
  2. 缺 FILE 声明 → valid=false，issues 含 FILE
  3. 某轨缺 INDEX 01 → valid=false 且 missing_index 命中该轨号
  4. TRACK 编号不连续 → valid=false，issues 含「不连续」
  5. 空文本 → valid=false 且 tracks=0
  6. 无 body 时回落到该种 kind='cue' 的工件正文
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

GOOD = ('PERFORMER "X"\nTITLE "Y"\nFILE "a.flac" WAVE\n'
        '  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n'
        '  TRACK 02 AUDIO\n    INDEX 01 03:20:10\n')


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


def check(tok, tid, cue=None):
    body = {} if cue is None else {"cue": cue}
    return call("POST", "/torrents/%s/cue-check" % tid, body, token=tok)


def main():
    tok = login()
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Cue%'")
    s, r = upload(tok, "Audit.Cue.A")
    tid = (r.get("data") or {}).get("id")
    ok("上传探针种", s == 200 and r.get("code") == 0, "%s %s" % (s, r))

    # 1. 合法
    s, r = check(tok, tid, GOOD)
    d = r.get("data") or {}
    ok("合法 CUE → valid", s == 200 and d.get("valid") is True, d)
    ok("TRACK 数=2", d.get("tracks") == 2, d)

    # 2. 缺 FILE
    s, r = check(tok, tid, "TRACK 01 AUDIO\n  INDEX 01 00:00:00\n")
    d = r.get("data") or {}
    ok("缺 FILE → valid=false", d.get("valid") is False, d)
    ok("issues 提到 FILE",
       any("FILE" in i for i in (d.get("issues") or [])), d.get("issues"))

    # 3. 缺 INDEX 01
    s, r = check(tok, tid,
                 'FILE "a.flac" WAVE\nTRACK 01 AUDIO\n  INDEX 01 00:00:00\n'
                 'TRACK 02 AUDIO\n')
    d = r.get("data") or {}
    ok("缺 INDEX 01 → missing_index=[2]",
       d.get("missing_index") == [2], d)

    # 4. 轨号不连续
    s, r = check(tok, tid,
                 'FILE "a.flac" WAVE\nTRACK 02 AUDIO\n  INDEX 01 00:00:00\n')
    d = r.get("data") or {}
    ok("轨号不连续被标记",
       any("不连续" in i for i in (d.get("issues") or [])),
       d.get("issues"))

    # 5. 空文本
    s, r = check(tok, tid, "   ")
    d = r.get("data") or {}
    ok("空 CUE → tracks=0 且 invalid",
       d.get("valid") is False and d.get("tracks") == 0, d)

    # 6. 回落读 artifact（kind='cue'）
    psql("INSERT INTO torrent_artifacts (torrent_id, kind, filename, body, "
         "sha256, size_bytes) VALUES (%s, 'cue', 'x.cue', %s, 'a'||repeat('0',"
         "63), %d) ON CONFLICT (torrent_id, kind, filename) DO NOTHING"
         % (tid, "'" + GOOD.replace("'", "''") + "'", len(GOOD)))
    s, r = check(tok, tid, None)
    d = r.get("data") or {}
    ok("无 body 回落 artifact → valid", d.get("valid") is True, d)

    # 不存在的种子
    s, r = check(tok, 999999999, GOOD)
    ok("不存在的种子 → 404", s == 404, "%s %s" % (s, r))

    psql("DELETE FROM torrents WHERE id=%s" % tid)


if __name__ == "__main__":
    main()
    summary()
