# -*- coding: utf-8 -*-
"""站型成熟度补齐 · 批次 D 闸门（死列处置 G8 + 聚合锚点通用化 G10/G11）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_w_anchor.py

判据（迁移 0338）：
  G8 死列：
    1. torrents 不再有 technical_info 列
  G11 通用锚点（artists.kind 参数化）：
    2. artists 有 kind 列，且唯一约束是 (kind, name)
    3. 发种带 author 维度 → artists 出现 kind='author' 的锚点行（增量同步）
    4. GET /artists?kind=author 命中该书作者；默认 kind=artist 不含它（隔离）
    5. GET /artists/{id} 返回 kind=author
    6. 同名的 artist 与 author 可共存（(kind,name) 唯一键生效）
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402
from pt_audit_g_logcheck import make_torrent, multipart, BOUNDARY  # noqa: E402
import urllib.error  # noqa: E402
import urllib.parse  # noqa: E402
import urllib.request  # noqa: E402

PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]

AUTHOR = "AuditAnchorAuthor"


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


def main():
    tok = login()
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Anchor%'")
    psql("DELETE FROM artists WHERE name IN ('%s')" % AUTHOR)
    orig = psql("SELECT value FROM site_settings WHERE name='site_type'")
    made = []

    # ---- G8 死列 ----
    col = psql("SELECT count(*) FROM information_schema.columns "
               "WHERE table_name='torrents' "
               "AND column_name='technical_info'")
    ok("torrents.technical_info 已移除", col == "0", col)

    # ---- G11 锚点 kind ----
    kind_col = psql("SELECT count(*) FROM information_schema.columns "
                    "WHERE table_name='artists' AND column_name='kind'")
    ok("artists.kind 列存在", kind_col == "1", kind_col)
    ok("唯一索引为 (kind,name)",
       psql("SELECT count(*) FROM pg_indexes WHERE tablename='artists' "
            "AND indexdef LIKE '%(kind, name)%'") == "1")
    ok("旧 (name) 唯一索引已移除",
       psql("SELECT count(*) FROM pg_indexes WHERE tablename='artists' "
            "AND indexname='artists_name_key'") == "0")

    # ---- 发种带 author → 锚点增量同步 ----
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "ebook", "mode": "merge"}, token=tok)
    ok("apply ebook", s == 200 and r.get("code") == 0, r)
    s, r = upload(tok, make_torrent(name="Audit.Anchor.B1"),
                  "Audit.Anchor.B1", {"author": {"text": AUTHOR}})
    t1 = (r.get("data") or {}).get("id")
    made.append(t1)
    ok("上传带 author 的种", s == 200 and r.get("code") == 0,
       "%s %s" % (s, r))
    ok("artists 出现 kind=author 锚点",
       psql("SELECT count(*) FROM artists WHERE kind='author' AND name='%s'"
            % AUTHOR) == "1")

    # ---- 列表按 kind 隔离 ----
    s, r = call("GET", "/artists?kind=author&limit=100", token=tok)
    names = [i["name"] for i in (r.get("data") or {}).get("items") or []]
    ok("kind=author 列表命中", AUTHOR in names, names[:5])
    s, r = call("GET", "/artists?kind=artist&limit=100", token=tok)
    names_a = [i["name"] for i in (r.get("data") or {}).get("items") or []]
    ok("默认 artist 列表不含 author 锚点", AUTHOR not in names_a)

    # ---- 实体页返回 kind ----
    aid = psql("SELECT id FROM artists WHERE kind='author' AND name='%s'"
               % AUTHOR)
    s, r = call("GET", "/artists/%s" % aid, token=tok)
    ok("实体页返回 kind=author",
       (r.get("data") or {}).get("kind") == "author", r)

    # ---- 同名跨 kind 共存 ----
    psql("INSERT INTO artists (kind, name, norm_name) VALUES "
         "('artist', '%s', LOWER('%s')) ON CONFLICT DO NOTHING"
         % (AUTHOR, AUTHOR))
    ok("同名 artist/author 可共存",
       psql("SELECT count(*) FROM artists WHERE name='%s'" % AUTHOR) == "2")

    # ---- 清理 + 还原 ----
    for t in made:
        if t:
            psql("DELETE FROM torrents WHERE id=%s" % t)
    psql("DELETE FROM artists WHERE name='%s'" % AUTHOR)
    back = psql("SELECT value FROM site_settings WHERE name='site_type'")
    if back != orig:
        call("POST", "/admin/site-type-packs/apply",
             {"code": orig, "mode": "merge"}, token=tok)
        back = psql("SELECT value FROM site_settings WHERE name='site_type'")
    ok("站型还原", back == orig, (orig, back))


if __name__ == "__main__":
    main()
    summary()
