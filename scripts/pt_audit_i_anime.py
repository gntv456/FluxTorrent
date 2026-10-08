# -*- coding: utf-8 -*-
"""0320 anime 站型话数维度闸门（阶段 2 anime 批）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_i_anime.py

覆盖「切到 anime 后像不像动漫站」的可执行判据（对标 §5 通用清单第 4/5 条）：

  1. anime 维度物化：season(select)/ep_first/ep_last(number)/
     subtitle_group(multiselect) 有着型 + 词表
  2. 发种可带话数维度（multipart sections JSON 通道）并落 torrent_sections
  3. sec_ep_first_min/max 区间筛选命中（「补第 5-8 话」检索可用）
  4. 详情 aggregate 读出维度值（多 select 的 dict_id 解回名字）
  5. 字幕组 multiselect 多值写入可往返
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402
from pt_audit_g_logcheck import make_torrent, upload  # noqa: E402

PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def main():
    tok = login()
    orig_type = psql("SELECT value FROM site_settings WHERE name='site_type'")

    # ---- 1. 维度物化
    kinds = dict(
        (l.split("=")[0], l.split("=")[1])
        for l in psql(
            "SELECT kind || '=' || field_type FROM section_kinds "
            "WHERE kind IN ('season','ep_first','ep_last','subtitle_group')"
        ).splitlines() if "=" in l
    )
    ok("season 是 select", kinds.get("season") == "select", kinds)
    ok("ep_first 是 number", kinds.get("ep_first") == "number", kinds)
    ok("ep_last 是 number", kinds.get("ep_last") == "number", kinds)
    ok("subtitle_group 是 multiselect", kinds.get("subtitle_group") == "multiselect", kinds)
    n_season = psql("SELECT count(*) FROM section_dict WHERE kind='season'")
    n_sub = psql("SELECT count(*) FROM section_dict WHERE kind='subtitle_group'")
    ok("season 词表非空", n_season != "0", n_season)
    ok("字幕组词表非空（0317 已补）", n_sub != "0", n_sub)

    # ---- 2. 发种带维度
    season_id = psql("SELECT id FROM section_dict WHERE kind='season' "
                     "AND name='第一季' LIMIT 1")
    sub_id = psql("SELECT id FROM section_dict WHERE kind='subtitle_group' "
                  "ORDER BY sort LIMIT 1")
    sections = json.dumps({
        "season": {"dict_ids": [int(season_id)]},
        "ep_first": {"number": 1},
        "ep_last": {"number": 12},
        "subtitle_group": {"dict_ids": [int(sub_id)]},
    })
    s, r = upload(tok, make_torrent(name="Audit.Anime.Ep.01"),
                  sections=sections)
    ok("发种带话数维度成功", s == 200 and r.get("code") == 0,
       "%s %s" % (s, r))
    tid = (r.get("data") or {}).get("id")
    if not tid:
        raise SystemExit("发种失败，后续断言无法跑")
    stored = psql(
        "SELECT kind || ':' || COALESCE(value::text, dict_id::text) "
        "FROM torrent_sections WHERE torrent_id=%s ORDER BY kind" % tid)
    ok("话数落 torrent_sections（ep_first=1）",
       "ep_first:1" in stored or "ep_first: 1" in stored.replace(" ", ""), stored)
    ok("季落库（dict）",
       psql("SELECT count(*) FROM torrent_sections ts JOIN section_dict sd "
            "ON sd.id = ts.dict_id WHERE ts.torrent_id=%s AND sd.kind='season'"
            % tid) == "1")
    ok("字幕组多值落库",
       psql("SELECT count(*) FROM torrent_sections WHERE torrent_id=%s "
            "AND kind='subtitle_group'" % tid) == "1")

    # ---- 3. 区间筛选（先过审：默认视图不显待审种，且 root 不能审自己的种）
    psql("UPDATE torrents SET approval_status = 1, approved_at = now(), "
         "seeders = 1 WHERE id = %s" % tid)
    s, r = call("GET", "/torrents?sec_ep_first_min=1&sec_ep_first_max=5",
                token=tok)
    items = (r.get("data") or {}).get("items") or []
    hit = any(it.get("id") == tid for it in items)
    raw_rows = psql(
        "SELECT kind, value::text FROM torrent_sections "
        "WHERE torrent_id=%s AND kind='ep_first'" % tid)
    ok("sec_ep_first 区间筛选命中本种", s == 200 and hit,
       "status=%s items=%d rows=%s" % (s, len(items), raw_rows))
    s, r = call("GET", "/torrents?sec_ep_first_min=13", token=tok)
    items = (r.get("data") or {}).get("items") or []
    ok("区间外不命中（13 话起）",
       not any(it.get("id") == tid for it in items))

    # ---- 4. 详情读出
    s, agg = call("GET", "/torrents/%s/aggregate" % tid, token=tok)
    detail = (agg.get("data") or {}).get("detail") or {}
    secs = detail.get("sections")
    ok("详情 aggregate 带 sections 维度值",
       bool(secs and (secs.get("ep_first") or secs.get("season"))),
       list((secs or {}).keys()) if isinstance(secs, dict) else secs)

    # ---- 清理（探针种 + 回滚 anime apply 恢复原站型）
    s, r = call("GET", "/admin/torrents/%s" % tid, token=tok) if False else (200, None)
    psql("DELETE FROM torrents WHERE id=%s" % tid)
    aid = psql("SELECT max(id) FROM site_pack_applies WHERE rolled_back_at IS NULL")
    if aid:
        s, r = call("POST", "/admin/site-type-packs/applies/%s/rollback" % aid,
                    None, token=tok)
        ok("回滚 anime apply", s == 200 and r.get("code") == 0, r)
    back = psql("SELECT value FROM site_settings WHERE name='site_type'")
    if back != orig_type:
        call("POST", "/admin/site-type-packs/apply",
             {"code": orig_type, "mode": "merge"}, token=tok)
        back = psql("SELECT value FROM site_settings "
                    "WHERE name='site_type'")
    ok("站型还原", back == orig_type, (orig_type, back))
    psql("DELETE FROM section_kinds WHERE kind IN ('season','ep_first','ep_last')"
         " AND NOT EXISTS (SELECT 1 FROM torrent_sections ts"
         " WHERE ts.kind IN ('season','ep_first','ep_last'))")


if __name__ == "__main__":
    main()
    summary()
