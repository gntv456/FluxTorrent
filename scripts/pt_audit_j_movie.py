# -*- coding: utf-8 -*-
"""0321 movie/documentary 剧集话数维度闸门（阶段 2 movie 批）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_j_movie.py

判据（与 anime 闸门同构，影视语境）：

  1. movie/documentary 维度物化：season(select) + episode_first/last(number)
  2. 发种带「第 2 季 3-10 集 + 压制组」写入 torrent_sections
  3. sec_episode_first 区间筛选命中（「补 S02E05-E08」检索）
  4. 详情 aggregate 读出维度
  5. documentary 随 movie 同批生效；回滚还原
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

    # ---- 1. apply movie 并断言维度物化
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "movie", "mode": "merge"}, token=tok)
    ok("apply movie", s == 200 and r.get("code") == 0, r)
    kinds = dict(
        (l.split("=")[0], l.split("=")[1])
        for l in psql(
            "SELECT kind || '=' || field_type FROM section_kinds "
            "WHERE kind IN ('season','episode_first','episode_last',"
            "'subtitle_group')").splitlines() if "=" in l
    )
    ok("season=select", kinds.get("season") == "select", kinds)
    ok("episode_first=number", kinds.get("episode_first") == "number", kinds)
    ok("episode_last=number", kinds.get("episode_last") == "number", kinds)
    ok("subtitle_group=multiselect",
       kinds.get("subtitle_group") == "multiselect", kinds)
    ok("影视季词表含最终季",
       psql("SELECT count(*) FROM section_dict WHERE kind='season' "
            "AND name='最终季'") == "1")

    # ---- 2. 发种带维度（S02E03-E10）
    season_id = psql("SELECT id FROM section_dict WHERE kind='season' "
                     "AND name='第二季' LIMIT 1")
    sub_id = psql("SELECT id FROM section_dict WHERE kind='subtitle_group' "
                  "AND name='CHDBits' LIMIT 1")
    ok("压制组词表就绪", sub_id != "", sub_id)
    s, r = upload(
        tok, make_torrent(name="Audit.Movie.Ep"),
        sections=json.dumps({
            "season": {"dict_ids": [int(season_id)]},
            "episode_first": {"number": 3},
            "episode_last": {"number": 10},
            "subtitle_group": {"dict_ids": [int(sub_id)]},
        }))
    ok("发种带剧集维度成功", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    tid = (r.get("data") or {}).get("id")
    if not tid:
        raise SystemExit("发种失败")
    ok("集数落库",
       psql("SELECT count(*) FROM torrent_sections WHERE torrent_id=%s "
            "AND kind IN ('episode_first','episode_last')" % tid) == "2")
    ok("季落库（dict）",
       psql("SELECT count(*) FROM torrent_sections ts JOIN section_dict sd "
            "ON sd.id = ts.dict_id WHERE ts.torrent_id=%s AND "
            "sd.kind='season'" % tid) == "1")

    # ---- 3. 区间筛选（探针置过审+活种，见 anime 闸门坑位记录）
    psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
         "seeders=1 WHERE id=%s" % tid)
    s, r = call("GET", "/torrents?sec_episode_first_min=1"
                "&sec_episode_first_max=5", token=tok)
    items = (r.get("data") or {}).get("items") or []
    ok("区间筛选命中（E03 在 1-5 内）",
       any(i.get("id") == tid for i in items), len(items))
    s, r = call("GET", "/torrents?sec_episode_first_min=11", token=tok)
    items = (r.get("data") or {}).get("items") or []
    ok("区间外不命中（E11 起）",
       not any(i.get("id") == tid for i in items))

    # ---- 4. 详情读出
    s, agg = call("GET", "/torrents/%s/aggregate" % tid, token=tok)
    detail = (agg.get("data") or {}).get("detail") or {}
    secs = detail.get("sections")
    ok("详情 sections 含 episode_first",
       bool(secs and secs.get("episode_first")),
       list((secs or {}).keys()) if isinstance(secs, dict) else secs)

    # ---- 5. 清理 + 还原
    psql("DELETE FROM torrents WHERE id=%s" % tid)
    aid = psql("SELECT max(id) FROM site_pack_applies "
               "WHERE rolled_back_at IS NULL")
    s, r = call("POST", "/admin/site-type-packs/applies/%s/rollback" % aid,
                None, token=tok)
    ok("回滚 movie apply", s == 200 and r.get("code") == 0, r)
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
