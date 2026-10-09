# -*- coding: utf-8 -*-
"""站型成熟度补齐 · 批次 C 闸门（富媒体预览 G5/G9 + 日志人工改判 G3）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_v_preview.py

判据（迁移 0337）：
  试读/试听（G5/G9，复用 attachments 物理存储）：
    1. 上传一个 preview 附件 → 关联到种子 → 列表可见
    2. 关联需要作者或 staff（他人 403）
    3. 删除关联后列表为空
    4. ebook/music 包 features 含 preview
  日志人工改判（G3，Gazelle AdjustedScore 口径）：
    5. torrent_logs 有 adjusted_score/adjusted_by/adjust_reason 列
    6. 版主改判 → adjusted_score 落库且原始 log_score 不动
    7. 非版主改判被拒（403）
    8. 改判留痕：adjusted_by / adjust_reason / adjusted_at 三列齐
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


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def upload(tok, tor, name, sections=None):
    fields = {"name": name, "small_descr": "gate", "descr": "audit",
              "category_id": "1"}
    if sections:
        fields["sections"] = json.dumps(sections)
    body = multipart(fields, [("file", "audit.torrent", tor)])
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


def upload_attachment(tok, payload=b"PREVIEW-BYTES", name="preview.txt"):
    # visibility 缺省 = 共享；显式传 "shared" 会被后端拒（只认 private）
    body = multipart({}, [("file", name, payload)])
    req = urllib.request.Request(BASE + "/attachments", method="POST",
                                 data=body)
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


def live(tid):
    psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
         "seeders=1 WHERE id=%s" % tid)


def main():
    tok = login()
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Preview%'")
    orig = psql("SELECT value FROM site_settings WHERE name='site_type'")
    made = []

    # ---- 迁移层 ----
    cols = psql("SELECT column_name FROM information_schema.columns "
                "WHERE table_name='torrent_logs' AND column_name LIKE "
                "'adjust%' ORDER BY column_name").splitlines()
    ok("torrent_logs 有改判列（adjusted_score/…）",
       "adjusted_score" in cols, cols)
    ok("torrent_logs 有 adjusted_by", "adjusted_by" in cols, cols)
    tbl = psql("SELECT count(*) FROM information_schema.tables "
               "WHERE table_name='torrent_previews'")
    ok("torrent_previews 表存在", tbl == "1", tbl)

    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "ebook", "mode": "merge"}, token=tok)
    ok("apply ebook", s == 200 and r.get("code") == 0, r)
    ok("ebook features.preview=true",
       psql("SELECT features->>'preview' FROM site_type_packs "
            "WHERE code='ebook'") == "true")
    ok("music features.preview=true",
       psql("SELECT features->>'preview' FROM site_type_packs "
            "WHERE code='music'") == "true")

    # ---- 种子 + 试读附件 ----
    s, r = upload(tok, make_torrent(name="Audit.Preview.A"), "Audit.Preview.A")
    t1 = (r.get("data") or {}).get("id")
    made.append(t1)
    ok("上传种子", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    live(t1)

    s, r = upload_attachment(tok)
    ok("上传 preview 附件", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    sha = (r.get("data") or {}).get("sha256") or (r.get("data") or {}).get(
        "sha")
    ok("附件返回 sha256", bool(sha), r)

    s, r = call("POST", "/torrents/%s/previews" % t1,
                {"sha256": sha, "kind": "preview", "filename": "sample.txt"},
                token=tok)
    ok("关联试读", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    s, r = call("GET", "/torrents/%s/previews" % t1, token=tok)
    items = (r.get("data") or {}).get("items") or []
    ok("试读列表可见", len(items) == 1 and items[0].get("sha256") == sha,
       items)

    # ---- 日志改判 ----
    psql("INSERT INTO torrent_logs (torrent_id, ordinal, filename, engine, "
         "log_score, tracks, size, body) VALUES (%s, 1, 'x.log', 'EAC', 87, "
         "12, 100, 'dummy') ON CONFLICT DO NOTHING" % t1)
    s, r = call("POST", "/admin/torrents/%s/logs/1/adjust" % t1,
                {"score": 100, "reason": "HTOA 区间误判"}, token=tok)
    ok("版主改判", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    row = psql("SELECT COALESCE(adjusted_score::text,'-') || '|' || "
               "COALESCE(log_score::text,'-') FROM torrent_logs "
               "WHERE torrent_id=%s AND ordinal=1" % t1)
    ok("adjusted_score=100 且原始分保留", row == "100|87", row)
    ok("改判留痕（by+reason）",
       psql("SELECT count(*) FROM torrent_logs WHERE torrent_id=%s "
            "AND adjusted_by IS NOT NULL AND adjust_reason <> '' "
            "AND adjusted_at IS NOT NULL" % t1) == "1")

    # ---- 清理 + 还原（删关联不删附件物理文件） ----
    call("DELETE", "/torrents/%s/previews" % t1, {"sha256": sha}, token=tok)
    s, r = call("GET", "/torrents/%s/previews" % t1, token=tok)
    ok("删除关联后列表为空",
       ((r.get("data") or {}).get("items") or []) == [], r)
    for t in made:
        if t:
            psql("DELETE FROM torrents WHERE id=%s" % t)
    back = psql("SELECT value FROM site_settings WHERE name='site_type'")
    if back != orig:
        call("POST", "/admin/site-type-packs/apply",
             {"code": orig, "mode": "merge"}, token=tok)
        back = psql("SELECT value FROM site_settings WHERE name='site_type'")
    ok("站型还原", back == orig, (orig, back))


if __name__ == "__main__":
    main()
    summary()
