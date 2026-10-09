# -*- coding: utf-8 -*-
"""0328 追更中心闸门（movie/anime 剧集批）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_p_calendar.py

判据：
  1. anime 站型发一枚带 ep_last=12 的种入组并过审
  2. root 订阅该组 → /me/subscriptions/groups 行带 latest_episode=12
     与 last_update 非空
  3. 再发 ep_last=13 过审 → latest_episode 涨到 13（进度随更新推进）
  4. /me/subscriptions/calendar 当天日期分组含两枚种
  5. 清理 + 还原
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


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def main():
    tok = login()
    orig_type = psql("SELECT value FROM site_settings WHERE name='site_type'")
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Cal%'")
    psql("DELETE FROM torrent_groups WHERE name='Audit.Cal.番'")

    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "anime", "mode": "merge"}, token=tok)
    ok("apply anime", s == 200 and r.get("code") == 0, r)

    # 组 + 第一话包（ep 1-12）
    psql("INSERT INTO torrent_groups (name) VALUES ('Audit.Cal.番') "
         "ON CONFLICT DO NOTHING")
    gid = psql("SELECT id FROM torrent_groups WHERE name='Audit.Cal.番'")
    s, r = upload(tok, make_torrent(name="Audit.Cal.E12"),
                  sections=json.dumps({"ep_first": {"number": 1},
                                       "ep_last": {"number": 12}}))
    t1 = (r.get("data") or {}).get("id")
    ok("第一包发出（1-12 话）", s == 200 and t1, r)
    psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
         "seeders=1, group_id=%s WHERE id=%s" % (gid, t1))

    # root 订阅
    s, r = call("POST", "/torrents/groups/%s/subscribe" % gid, None,
                token=tok)
    ok("订阅成功", s == 200 and r.get("code") == 0, r)

    # 2. 订阅进度
    s, r = call("GET", "/me/subscriptions/groups", token=tok)
    items = r.get("data") or []
    row = next((i for i in items if i.get("group_id") == int(gid)), None)
    ok("订阅列表带 latest_episode=12", row and
       row.get("latest_episode") == 12, row)
    ok("订阅列表带 last_update", row and row.get("last_update"), row)

    # 3. 第二包 13 话 → 进度推进
    s, r = upload(tok, make_torrent(name="Audit.Cal.E13"),
                  sections=json.dumps({"ep_first": {"number": 13},
                                       "ep_last": {"number": 13}}),
                  group_id=int(gid))
    t2 = (r.get("data") or {}).get("id")
    psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
         "seeders=1, group_id=%s WHERE id=%s" % (gid, t2))
    s, r = call("GET", "/me/subscriptions/groups", token=tok)
    row = next((i for i in (r.get("data") or [])
                if i.get("group_id") == int(gid)), None)
    ok("更新后 latest_episode=13", row and
       row.get("latest_episode") == 13, row)

    # 4. 日历
    s, r = call("GET", "/me/subscriptions/calendar", token=tok)
    days = r.get("data") or []
    today = psql("SELECT to_char(now(), 'YYYY-MM-DD')")
    day0 = next((d for d in days if d.get("date") == today), None)
    ids = [i.get("torrent_id") for i in (day0 or {}).get("items") or []]
    ok("日历今天分组含两枚种", t1 in ids and t2 in ids, (t1, t2, ids))

    # 清理
    call("POST", "/torrents/groups/%s/unsubscribe" % gid, None, token=tok)
    psql("DELETE FROM torrents WHERE id IN (%s, %s)" % (t1, t2))
    psql("DELETE FROM torrent_groups WHERE id=%s" % gid)
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
