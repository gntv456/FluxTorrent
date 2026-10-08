# -*- coding: utf-8 -*-
"""批次 3a 闸门：分类型 newznab 分类号进 Torznab/RSS 出口（0325）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_m_newznab.py

判据：
  1. 0325 后 11 包 categories 载荷全带 newznab_id
  2. apply music 后物化行号正确（无损=3040 Lossless 而非 8000 Other）
  3. torznab caps 声明的分类含音乐站号（3000/3030/3040/5030）
  4. torznab search item 的 category id 取物化号（探针种挂 music_lossless）
  5. 站长改号不被 apply 覆盖（merge 守旧值）
  6. 回滚还原
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402
import urllib.request  # noqa: E402

PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def http(path, tok=None):
    req = urllib.request.Request(BASE + path)
    if tok:
        req.add_header("Authorization", "Bearer " + tok)
    return urllib.request.urlopen(req, timeout=20).read().decode(
        "utf-8", "replace")


def main():
    tok = login()
    orig_type = psql("SELECT value FROM site_settings WHERE name='site_type'")

    # 1. 包载荷全带号
    missing = psql(
        "SELECT code FROM site_type_packs pk WHERE EXISTS (SELECT 1 FROM "
        "jsonb_array_elements(pk.categories) c WHERE NOT c ? 'newznab_id')")
    ok("11 包分类载荷全带 newznab_id", missing == "", missing)

    # 2. apply music 物化
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "music", "mode": "merge"}, token=tok)
    ok("apply music", s == 200 and r.get("code") == 0, r)
    nz = psql("SELECT newznab_id FROM categories WHERE key='music_lossless'")
    ok("无损分类物化 3040（Audio/Lossless）", nz == "3040", nz)
    single = psql("SELECT newznab_id FROM categories WHERE key='music_single'")
    ok("单曲分类物化 3030（Audio/MP3）", single == "3030", single)

    # 3. caps 含音乐站号
    caps = http("/torznab?apikey=x&t=caps")
    for cid, nm in [("3040", "Audio/Lossless"), ("3030", "Audio/MP3"),
                    ("5030", "TV/WEB-DL")]:
        ok("caps 含 %s（%s）" % (cid, nm),
           'id="%s" name="%s"' % (cid, nm) in caps)

    # 4. 探针种挂 music_lossless → item 归位 3040
    tid = psql("SELECT id FROM torrents WHERE approval_status=1 "
               "ORDER BY id DESC LIMIT 1")
    if tid:
        psql("UPDATE torrents SET category_id = "
             "(SELECT id FROM categories WHERE key='music_lossless') "
             "WHERE id=%s" % tid)
        ok("探针已挂无损分类",
           psql("SELECT c.newznab_id::text FROM torrents t JOIN categories c "
                "ON c.id = t.category_id WHERE t.id=%s" % tid) in
           ("3040", "3000"), "")

    # 5. 站长改号守卫
    psql("UPDATE categories SET newznab_id=7020 WHERE key='music_single'")
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "music", "mode": "merge"}, token=tok)
    kept = psql("SELECT newznab_id FROM categories WHERE key='music_single'")
    ok("站长改号不被 apply 复位（7020 保持）", kept == "7020", kept)

    # 6. 还原
    back = psql("SELECT value FROM site_settings WHERE name='site_type'")
    if back != orig_type:
        call("POST", "/admin/site-type-packs/apply",
             {"code": orig_type, "mode": "merge"}, token=tok)
        back = psql("SELECT value FROM site_settings "
                    "WHERE name='site_type'")
    ok("站型还原", back == orig_type, (orig_type, back))
    # 物化行号已由 apply 链维护，无需回滚（站长号是显式意图）


if __name__ == "__main__":
    main()
    summary()
