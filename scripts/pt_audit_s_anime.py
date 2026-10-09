# -*- coding: utf-8 -*-
"""0334 动漫站型专项闸门（air_season 播出季 + 字幕组聚合 + 订阅）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_s_anime.py

判据：
  1. apply anime 站型 → air_season / bangumi_id 维度已建
  2. air_season 词表非空（YYYY季 口径）
  3. 发一枚挂「播出季 + 字幕组 + bangumi_id」的种
  4. **锚点增量同步**：新字幕组词条发种后自动进 content_networks
     （0334 修复——此前该表只有迁移回填、无增量写入口）
  5. GET /seasonal 默认当季且含该种；?season= 切到指定季
  6. GET /networks?kind=subtitle_group 列表含该字幕组；详情聚合该种
  7. 字幕组订阅：订 → 过审 → subgroup_new_release 站内信 → 退订
  8. /section-dict 透出 air_season 且带词表（导航 dims 门数据源）
  9. 通知偏好键 subgroup_new_release 可设
 10. 清理 + 还原站型
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

SUB = "Audit字幕组S"


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def main():
    tok = login()
    # 前置清场
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Ani%'")
    psql("DELETE FROM torrent_sections WHERE kind='subtitle_group' AND "
         "dict_id IN (SELECT id FROM section_dict "
         "WHERE kind='subtitle_group' AND name='%s')" % SUB)
    psql("DELETE FROM content_networks WHERE kind='subtitle_group' "
         "AND name='%s'" % SUB)
    psql("DELETE FROM section_dict WHERE kind='subtitle_group' "
         "AND name='%s'" % SUB)
    psql("DELETE FROM messages WHERE kind='subgroup_new_release'")
    psql("DELETE FROM messages WHERE receiver_id IN "
         "(SELECT id FROM users WHERE username='audit-poster')")
    psql("DELETE FROM torrents WHERE owner_id IN "
         "(SELECT id FROM users WHERE username='audit-poster')")
    psql("DELETE FROM users WHERE username='audit-poster'")
    orig_type = psql("SELECT value FROM site_settings WHERE name='site_type'")
    made = []

    # 1. apply anime
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "anime", "mode": "merge"}, token=tok)
    ok("apply anime", s == 200 and r.get("code") == 0, r)

    # 2. 维度已建
    n_kind = psql("SELECT count(*) FROM section_kinds "
                  "WHERE kind IN ('air_season','bangumi_id')")
    ok("air_season/bangumi_id 维度已建", int(n_kind or 0) == 2, n_kind)

    # 3. 词表非空
    seas = psql("SELECT id||'|'||name FROM section_dict "
                "WHERE kind='air_season' ORDER BY sort DESC, id DESC")
    rows = [x for x in seas.splitlines() if x.strip()]
    ok("air_season 词表非空", len(rows) >= 4, len(rows))
    cur_id, cur_name = rows[0].split("|")
    # 取一个"非当季"做切换测试（sort 最小项）
    old_id, old_name = rows[-1].split("|")

    # 4. 建字幕组词条（不建 content_networks 锚点——验证发种自动同步）
    psql("INSERT INTO section_dict (kind, name, sort) "
         "VALUES ('subtitle_group', '%s', 999)" % SUB)
    sub_id = psql("SELECT id FROM section_dict WHERE kind='subtitle_group' "
                  "AND name='%s'" % SUB)
    ok("字幕组词条已建", bool(sub_id), sub_id)
    pre = psql("SELECT count(*) FROM content_networks "
               "WHERE kind='subtitle_group' AND name='%s'" % SUB)
    ok("发种前无锚点（前提）", int(pre or 0) == 0, pre)

    # 5. 发一枚当季种（挂 播出季 + 字幕组 + bangumi_id）
    s, r = upload(
        tok, make_torrent(name="Audit.Ani.S1"),
        sections=json.dumps({
            "air_season": {"dict_ids": [int(cur_id)]},
            "subtitle_group": {"dict_ids": [int(sub_id)]},
            "bangumi_id": {"text": "90001"},
        }))
    tid = (r.get("data") or {}).get("id")
    ok("发种（当季+字幕组+bangumi_id）", s == 200 and tid, r)
    if tid:
        made.append(tid)

    # 6. 锚点增量同步：字幕组自动进 content_networks
    post = psql("SELECT count(*) FROM content_networks "
                "WHERE kind='subtitle_group' AND name='%s'" % SUB)
    ok("发种后字幕组锚点自动生成", int(post or 0) == 1, post)

    # 7. /seasonal 默认当季含该种
    s, r = call("GET", "/seasonal", token=tok)
    d = r.get("data") or {}
    ok("/seasonal 默认当季", d.get("season") == cur_name,
       (d.get("season"), cur_name))
    ok("/seasonal 当季含该种",
       any(i.get("id") == int(tid) for i in (d.get("items") or [])),
       len(d.get("items") or []))
    # seasons 是「有内容的季」（HAVING count>0）：只发过当季 ⇒ 恰 1 项
    ok("/seasonal 季列表 = 有内容的季数",
       len(d.get("seasons") or []) == 1,
       len(d.get("seasons") or []))

    # 8. /seasonal?season=<非当季> 切换（用 dict id 传参，避开中文 URL 编码）
    s, r = call("GET", "/seasonal?season=%s" % old_id, token=tok)
    d2 = r.get("data") or {}
    ok("/seasonal 指定季生效", d2.get("season") == old_name, d2.get("season"))
    ok("/seasonal 非当季不含该种（隔离）",
       not any(i.get("id") == int(tid) for i in (d2.get("items") or [])),
       len(d2.get("items") or []))

    # 9. 字幕组列表 + 详情
    s, r = call("GET", "/networks?kind=subtitle_group&limit=100", token=tok)
    items = (r.get("data") or {}).get("items") or []
    gid = next((g.get("id") for g in items if g.get("name") == SUB), None)
    ok("字幕组列表含该组", gid is not None, len(items))
    if gid:
        s, r = call("GET", "/networks/%s" % gid, token=tok)
        gd = r.get("data") or {}
        ok("字幕组页聚合该种",
           any(x.get("id") == int(tid) for x in (gd.get("items") or [])),
           len(gd.get("items") or []))

    # 10. 字幕组订阅 → 发种 → 过审 → 站内信 → 退订
    if gid:
        s, r = call("POST", "/networks/%s/subscribe" % gid, None, token=tok)
        ok("订阅字幕组", s == 200 and r.get("code") == 0, r)
        psql("INSERT INTO users (username, email, pass_hash, passkey, "
             "class_id, status) SELECT 'audit-poster', 'ap@audit.local', "
             "pass_hash, 'auditposterpasskey00000000000000', 1, 3 "
             "FROM users WHERE username='root' "
             "ON CONFLICT (username) DO NOTHING")
        poster = psql("SELECT id FROM users WHERE username='audit-poster'")
        s, r = upload(
            tok, make_torrent(name="Audit.Ani.Sub"),
            sections=json.dumps({
                "air_season": {"dict_ids": [int(cur_id)]},
                "subtitle_group": {"dict_ids": [int(sub_id)]},
            }))
        tid2 = (r.get("data") or {}).get("id")
        ok("订阅后发种成功", s == 200 and tid2, r)
        if tid2:
            made.append(tid2)
            psql("UPDATE torrents SET owner_id=%s, approval_status=0, "
                 "approved_at=NULL WHERE id=%s" % (poster, tid2))
            s2, r2 = call("POST", "/admin/reviews/decide",
                          {"torrent_id": int(tid2), "approve": True},
                          token=tok)
            ok("过审端点可用", s2 == 200 and r2.get("code") == 0, r2)
            n_msg = psql(
                "SELECT count(*) FROM messages "
                "WHERE kind='subgroup_new_release' "
                "AND receiver_id=(SELECT id FROM users "
                "WHERE username='root')")
            ok("过审触发字幕组新作站内信", int(n_msg or 0) >= 1, n_msg)
        s, r = call("POST", "/networks/%s/unsubscribe" % gid, None, token=tok)
        ok("退订字幕组", s == 200 and r.get("code") == 0, r)

    # 11. section-dict 透出 air_season（导航 dims 门数据源）
    s, r = call("GET", "/section-dict", token=tok)
    d = r.get("data") or {}
    ok("section-dict 透出 air_season 且有词表",
       isinstance(d.get("air_season"), list) and len(d["air_season"]) > 0,
       len(d.get("air_season") or []))

    # 12. 偏好键放行
    s, r = call("POST", "/me/notice-prefs",
                {"key": "subgroup_new_release", "enabled": False}, token=tok)
    ok("通知偏好键 subgroup_new_release 可设",
       s == 200 and r.get("code") == 0, r)
    call("POST", "/me/notice-prefs",
         {"key": "subgroup_new_release", "enabled": True}, token=tok)

    # 清理
    psql("DELETE FROM network_subscriptions WHERE network_id IN "
         "(SELECT id FROM content_networks WHERE kind='subtitle_group' "
         "AND name='%s')" % SUB)
    psql("DELETE FROM messages WHERE kind='subgroup_new_release'")
    psql("DELETE FROM messages WHERE receiver_id IN "
         "(SELECT id FROM users WHERE username='audit-poster')")
    psql("DELETE FROM torrents WHERE owner_id IN "
         "(SELECT id FROM users WHERE username='audit-poster')")
    psql("DELETE FROM users WHERE username='audit-poster'")
    for t in made:
        if t:
            psql("DELETE FROM torrents WHERE id=%s" % t)
    psql("DELETE FROM content_networks WHERE kind='subtitle_group' "
         "AND name='%s'" % SUB)
    psql("DELETE FROM section_dict WHERE kind='subtitle_group' "
         "AND name='%s'" % SUB)
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
