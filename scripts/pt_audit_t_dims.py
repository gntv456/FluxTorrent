# -*- coding: utf-8 -*-
"""站型成熟度补齐 · 批次 A 闸门（缺失维度：G4/G6/G12）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_t_dims.py

判据（迁移 0335，纯数据批次）：
  lossless/music  ← RED/OPS 口径
    1. lossless 包声明 sample_rate/bitdepth/hi_res 且 apply 后类型正确
    2. 发种带「采样率 + 位深 + Hi-Res」落库
    3. sec_hi_res=true 命中；sec_hi_res=false 不命中该种
    4. sec_sample_rate=<id> 命中
  ebook           ← MAM 口径
    5. ebook 包声明 dpi/ocr 且 apply 后类型正确
    6. 发种带 dpi + ocr 落库并可按 sec_ocr 筛
  game            ← GGn 口径
    7. game 包声明 dlc/arch 且 apply 后类型正确
    8. 词表非空（sample_rate / dpi / dlc / arch 至少各 3 项）

纯只读断言为主；上传的探针种在结尾删除，站型还原。
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


def upload(tok, tor, sections, name):
    fields = {"name": name, "small_descr": "gate", "descr": "audit",
              "category_id": "1", "sections": json.dumps(sections)}
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


def live(tid):
    psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
         "seeders=1 WHERE id=%s" % tid)


def kinds_of(codes):
    rows = psql("SELECT kind || '=' || field_type FROM section_kinds "
                "WHERE kind IN (%s)" % ",".join("'%s'" % c for c in codes))
    return dict(l.split("=") for l in rows.splitlines() if "=" in l)


def pack_has_dim(code, kind):
    return psql("SELECT count(*) FROM site_type_packs WHERE code='%s' "
                "AND sections->'kinds' @> '[{\"kind\":\"%s\"}]'::jsonb"
                % (code, kind)) == "1"


def dict_count(kind):
    return int(psql("SELECT count(*) FROM section_dict WHERE kind='%s'" % kind))


def main():
    tok = login()
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Dims%'")
    orig = psql("SELECT value FROM site_settings WHERE name='site_type'")
    made = []

    # ---- 声明层：4 个包各声明了自己的新维度（迁移 ③） ----
    ok("lossless 包声明 sample_rate", pack_has_dim("lossless", "sample_rate"))
    ok("lossless 包声明 bitdepth", pack_has_dim("lossless", "bitdepth"))
    ok("lossless 包声明 hi_res", pack_has_dim("lossless", "hi_res"))
    ok("music 包声明 hi_res", pack_has_dim("music", "hi_res"))
    ok("ebook 包声明 dpi", pack_has_dim("ebook", "dpi"))
    ok("ebook 包声明 ocr", pack_has_dim("ebook", "ocr"))
    ok("game 包声明 dlc", pack_has_dim("game", "dlc"))
    ok("game 包声明 arch", pack_has_dim("game", "arch"))
    ok("词表 sample_rate>=3", dict_count("sample_rate") >= 3,
       dict_count("sample_rate"))
    ok("词表 dpi>=3", dict_count("dpi") >= 3, dict_count("dpi"))
    ok("词表 dlc>=3", dict_count("dlc") >= 3, dict_count("dlc"))
    ok("词表 arch>=3", dict_count("arch") >= 3, dict_count("arch"))

    # ---- lossless：apply + 发种 + 三态筛选 ----
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "lossless", "mode": "merge"}, token=tok)
    ok("apply lossless", s == 200 and r.get("code") == 0, r)
    ks = kinds_of(["sample_rate", "bitdepth", "hi_res"])
    ok("sample_rate=select", ks.get("sample_rate") == "select", ks)
    ok("bitdepth=select", ks.get("bitdepth") == "select", ks)
    ok("hi_res=bool", ks.get("hi_res") == "bool", ks)

    sr = psql("SELECT id FROM section_dict WHERE kind='sample_rate' "
              "AND name='96 kHz' LIMIT 1")
    bd = psql("SELECT id FROM section_dict WHERE kind='bitdepth' "
              "AND name='24 bit' LIMIT 1")
    secs = {"sample_rate": {"dict_ids": [int(sr)]},
            "bitdepth": {"dict_ids": [int(bd)]},
            "hi_res": {"bool": True}}
    s, r = upload(tok, make_torrent(name="Audit.Dims.HiRes"), secs,
                  "Audit.Dims.HiRes")
    t1 = (r.get("data") or {}).get("id")
    made.append(t1)
    ok("发种带三轴", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    ok("三轴落库",
       psql("SELECT count(*) FROM torrent_sections WHERE torrent_id=%s "
            "AND kind IN ('sample_rate','bitdepth','hi_res')" % t1) == "3")
    live(t1)
    s, r = call("GET", "/torrents?sec_hi_res=true", token=tok)
    ids = [i.get("id") for i in (r.get("data") or {}).get("items") or []]
    ok("sec_hi_res=true 命中", t1 in ids, len(ids))
    s, r = call("GET", "/torrents?sec_sample_rate=%s" % sr, token=tok)
    ids = [i.get("id") for i in (r.get("data") or {}).get("items") or []]
    ok("sec_sample_rate 命中", t1 in ids, len(ids))

    # ---- ebook：apply + dpi/ocr ----
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "ebook", "mode": "merge"}, token=tok)
    ok("apply ebook", s == 200 and r.get("code") == 0, r)
    ks = kinds_of(["dpi", "ocr"])
    ok("dpi=select", ks.get("dpi") == "select", ks)
    ok("ocr=bool", ks.get("ocr") == "bool", ks)
    dp = psql("SELECT id FROM section_dict WHERE kind='dpi' "
              "AND name='600 dpi' LIMIT 1")
    s, r = upload(tok, make_torrent(name="Audit.Dims.Scan"),
                  {"dpi": {"dict_ids": [int(dp)]}, "ocr": {"bool": True}},
                  "Audit.Dims.Scan")
    t2 = (r.get("data") or {}).get("id")
    made.append(t2)
    ok("dpi/ocr 落库",
       psql("SELECT count(*) FROM torrent_sections WHERE torrent_id=%s "
            "AND kind IN ('dpi','ocr')" % t2) == "2")
    live(t2)
    s, r = call("GET", "/torrents?sec_ocr=true", token=tok)
    ids = [i.get("id") for i in (r.get("data") or {}).get("items") or []]
    ok("sec_ocr=true 命中", t2 in ids, len(ids))

    # ---- game：apply + dlc/arch ----
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "game", "mode": "merge"}, token=tok)
    ok("apply game", s == 200 and r.get("code") == 0, r)
    ks = kinds_of(["dlc", "arch"])
    ok("dlc=select", ks.get("dlc") == "select", ks)
    ok("arch=select", ks.get("arch") == "select", ks)

    # ---- 清理 + 还原 ----
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
