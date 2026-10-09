# -*- coding: utf-8 -*-
"""0330 出品方聚合页闸门（documentary 专项 P1 深水区）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_q_network.py

判据：
  1. apply documentary 站型 → network 维度在 section_dict 有 14 条词表
  2. content_networks 表被迁移回填（network 词条建锚点行）
  3. 发两枚带 network 维度的种（同出品方不同题材）并过审
  4. GET /networks 列表可搜到该出品方，收录数 = 2
  5. GET /networks/{id} 厂牌页聚合两枚种
  6. GET /networks/top 榜单含该出品方（seeders 口径）
  7. 未过审种不计入（approval 口径）
  8. /api/v1/section-dict 透出 network 且带词表（前端导航 dims 门的数据源）
  9. 清理 + 还原站型
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402
from pt_audit_g_logcheck import make_torrent, upload  # noqa: E402

PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]

NAME = "Audit出品方Z"


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def main():
    tok = login()
    # 前置清场：上一次失败运行的残留种会让 seeders 口径偏大
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Net%'")
    psql("DELETE FROM torrent_sections WHERE kind='network' AND dict_id IN "
         "(SELECT id FROM section_dict WHERE kind='network' AND name='%s')"
         % NAME)
    psql("DELETE FROM content_networks WHERE name = '%s'" % NAME)
    orig_type = psql("SELECT value FROM site_settings WHERE name='site_type'")
    made = []

    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "documentary", "mode": "merge"}, token=tok)
    ok("apply documentary", s == 200 and r.get("code") == 0, r)

    # 1. network 词表存在
    n_dict = psql("SELECT count(*) FROM section_dict WHERE kind='network'")
    ok("network 维度词表非空", int(n_dict or 0) > 0, n_dict)

    # 2. 迁移回填锚点行
    n_row = psql("SELECT count(*) FROM content_networks WHERE kind='network'")
    ok("content_networks 已回填 network 锚点",
       int(n_row or 0) > 0, n_row)

    # 注入审计出品方锚点（前置：字典里没有这个词，靠 psql 建）
    psql("INSERT INTO content_networks (kind, name, norm_name) "
         "VALUES ('network', '%s', 'audit出品方z') "
         "ON CONFLICT (kind, name) DO NOTHING" % NAME)
    nid = psql("SELECT id FROM content_networks WHERE name='%s'" % NAME)

    # 先建 section_dict 词条，才能发种时挂 select 值
    psql("DELETE FROM section_dict WHERE kind='network' AND name='%s'"
         % NAME)
    psql("INSERT INTO section_dict (kind, name, sort) "
         "VALUES ('network', '%s', 999)" % NAME)
    did = psql("SELECT id FROM section_dict WHERE kind='network' "
               "AND name='%s'" % NAME)
    ok("审计词条已建", bool(did), did)

    # doc_type 是 select 枚举，须用 dict_ids（不是 text）
    dt_id = psql("SELECT id FROM section_dict WHERE kind='doc_type' "
                 "ORDER BY sort LIMIT 1")
    ok("doc_type 词条存在", bool(dt_id), dt_id)

    # 3. 发两枚同出品方种
    for i in (1, 2):
        s, r = upload(
            tok, make_torrent(name="Audit.Net.%d" % i),
            sections=json.dumps({
                "network": {"dict_ids": [int(did)]},
                "doc_type": {"dict_ids": [int(dt_id)]},
            }))
        ok("发种 %d 成功" % i, s == 200 and r.get("code") == 0,
           "%s %s" % (s, r))
        tid = (r.get("data") or {}).get("id")
        made.append(tid)
        if tid:
            psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
                 "seeders=%d WHERE id=%s" % (i * 3, tid))

    # 4. 列表搜索 + 收录数
    import urllib.parse as _up
    s, r = call("GET", "/networks?q=" + _up.quote(NAME), token=tok)
    items = (r.get("data") or {}).get("items") or []
    row = next((i for i in items if i.get("id") == int(nid)), None)
    ok("列表搜到出品方", s == 200 and row is not None, items)
    ok("收录数=2", row is not None and row.get("torrents") == 2, row)

    # 5. 厂牌页
    s, r = call("GET", "/networks/%s" % nid, token=tok)
    det = (r.get("data") or {}).get("items") or []
    ok("厂牌页聚合两枚种", s == 200 and len(det) == 2, det)

    # 6. 榜单（两枚种 seeders 分别 3 与 6 ⇒ 合计 9）
    s, r = call("GET", "/networks/top", token=tok)
    top = r.get("data") or []
    hit = next((t for t in top if t.get("id") == int(nid)), None)
    ok("榜单含该出品方", hit is not None, top[:3])
    ok("榜单 seeders=9", hit is not None and hit.get("seeders") == 9, hit)

    # 7. 未过审不计入
    if made and made[0]:
        psql("UPDATE torrents SET approval_status=0 WHERE id=%s" % made[0])
        s, r = call("GET", "/networks/%s" % nid, token=tok)
        det2 = (r.get("data") or {}).get("items") or []
        ok("未过审种不计入厂牌页", len(det2) == 1, len(det2))
        psql("UPDATE torrents SET approval_status=1 WHERE id=%s" % made[0])

    # 8. section-dict 透出 network（导航 dims 门数据源）
    s, r = call("GET", "/section-dict", token=tok)
    d = r.get("data") or {}
    ok("section-dict 透出 network 且有词表",
       s == 200 and isinstance(d.get("network"), list)
       and len(d["network"]) > 0, len(d.get("network") or []))

    # 清理
    for t in made:
        if t:
            psql("DELETE FROM torrents WHERE id=%s" % t)
    psql("DELETE FROM section_dict WHERE kind='network' AND name='%s'"
         % NAME)
    psql("DELETE FROM content_networks WHERE name='%s'" % NAME)
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
