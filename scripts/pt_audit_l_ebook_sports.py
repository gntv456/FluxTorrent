# -*- coding: utf-8 -*-
"""ebook 书目层 + sports 对阵层闸门（§7.5 最后两站型收尾）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_l_ebook_sports.py

ebook 判据（0315 维度已配，这里验「切到 ebook 后像不像电子书站」）：
  1. apply ebook 后 author/isbn/series/bookformat 维度物化且类型正确
  2. 发种带书目维度（text author + text isbn + select bookformat）落库
  3. sec_author 关键词筛选命中；同 ISBN 两版本 → torrent_groups 聚组后
     详情并列（GroupVersions 承载「同书多版本」）
  4. DRM bool 维度三态筛选

sports 判据：
  5. apply sports 后 league/season/round 维度物化
  6. 发种带联赛+赛季+轮次；组合筛选（league=英超 AND round=10）命中
  7. 回滚还原
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


def upload(tok, tor, sections=None, name="Audit.Ebook", group_id=None):
    fields = {"name": name, "small_descr": "gate", "descr": "audit",
              "category_id": "1"}
    if sections is not None:
        fields["sections"] = json.dumps(sections)
    if group_id:
        fields["group_id"] = str(group_id)
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


def clear_cache(tid):
    rp = os.environ.get("REDIS_PASSWORD", "")
    if not _env_redis(rp):
        return
    subprocess.run(
        ["docker", "exec", "flux-redis", "sh", "-c",
         "redis-cli -a '%s' DEL cache:tdetail:v1:%s:1 "
         "cache:tdetail:v1:%s:0" % (rp, tid, tid)],
        capture_output=True)


def _env_redis(rp):
    if not rp:
        rp = subprocess.run(
            ["docker", "exec", "flux-redis", "printenv", "REDIS_PASSWORD"],
            capture_output=True, text=True).stdout.strip()
    return rp


def live(tid):
    psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
         "seeders=1 WHERE id=%s" % tid)


def main():
    tok = login()
    # 清上一轮可能残留的探针（判重会挡住重跑）
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Ebook%' "
         "OR name LIKE 'Audit.Sports%'")
    psql("DELETE FROM torrent_groups WHERE name='射雕英雄传'")
    orig_type = psql("SELECT value FROM site_settings WHERE name='site_type'")
    made = []

    # ================= ebook =================
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "ebook", "mode": "merge"}, token=tok)
    ok("apply ebook", s == 200 and r.get("code") == 0, r)
    kinds = dict(
        (l.split("=")[0], l.split("=")[1])
        for l in psql(
            "SELECT kind || '=' || field_type FROM section_kinds "
            "WHERE kind IN ('author','isbn','series','bookformat','drm')"
        ).splitlines() if "=" in l
    )
    ok("author=text", kinds.get("author") == "text", kinds)
    ok("isbn=text", kinds.get("isbn") == "text", kinds)
    ok("series=text", kinds.get("series") == "text", kinds)
    ok("bookformat=select", kinds.get("bookformat") == "select", kinds)
    ok("drm=bool", kinds.get("drm") == "bool", kinds)

    fmt_id = psql("SELECT id FROM section_dict WHERE kind='bookformat' "
                  "AND name='EPUB' LIMIT 1")
    ok("bookformat 词表含 EPUB", fmt_id != "", fmt_id)
    secs = {"author": {"text": "金庸"}, "isbn": {"text": "9787020002207"},
            "series": {"text": "射雕三部曲"},
            "bookformat": {"dict_ids": [int(fmt_id)]},
            "drm": {"bool": False}}
    s, r = upload(tok, make_torrent(name="Audit.Ebook.V1"), secs,
                  "Audit.Ebook.V1")
    ok("发种带书目维度", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    t1 = (r.get("data") or {}).get("id")
    made.append(t1)
    ok("author/isbn 落库",
       psql("SELECT count(*) FROM torrent_sections WHERE torrent_id=%s "
            "AND kind IN ('author','isbn','series','bookformat','drm')"
            % t1) == "5")

    live(t1)
    # V1 先发（无组），按组名挂组（group_attach 按名找/建）——
    # 「同书多版本」的常见动线：第二版发出后，把第一版也归进同名组
    s, g = call("POST", "/torrents/%s/group" % t1,
                {"name": "射雕英雄传"}, token=tok)
    ok("V1 按名入组", s == 200 and g.get("code") == 0, g)
    s, r = call("GET", "/torrents?sec_author=" + "金庸".encode(
        "unicode_escape").decode().replace("\\u", "%u"), token=tok)
    # sec_author 中文关键词——pt_audit_lib 的 call 走 urllib 已是 UTF-8，
    # 直接拼 URL 会 400；改用百分号编码
    import urllib.parse
    s, r = call("GET", "/torrents?sec_author="
                + urllib.parse.quote("金庸"), token=tok)
    items = (r.get("data") or {}).get("items") or []
    ok("sec_author 关键词筛选命中", s == 200 and
       any(i.get("id") == t1 for i in items), len(items))
    s, r = call("GET", "/torrents?sec_drm=false", token=tok)
    items = (r.get("data") or {}).get("items") or []
    ok("sec_drm=false 筛选命中（无 DRM 档）",
       any(i.get("id") == t1 for i in items), len(items))

    # 同 ISBN 第二版本 → 同组聚合并列
    gid = psql("SELECT COALESCE(max(id),0)+1 FROM torrent_groups")
    psql("INSERT INTO torrent_groups (id, name) VALUES (%s, '射雕英雄传') "
         "ON CONFLICT (name) DO NOTHING" % gid)
    gid = psql("SELECT id FROM torrent_groups WHERE name='射雕英雄传'")
    fmt2 = psql("SELECT id FROM section_dict WHERE kind='bookformat' "
                "AND name='PDF' LIMIT 1")
    secs2 = {"author": {"text": "金庸"}, "isbn": {"text": "9787020002207"},
             "bookformat": {"dict_ids": [int(fmt2)]}}
    s, r = upload(tok, make_torrent(name="Audit.Ebook.V2"), secs2,
                  "Audit.Ebook.V2", group_id=int(gid))
    t2 = (r.get("data") or {}).get("id")
    made.append(t2)
    ok("第二版本（同 ISBN 不同格式）入同组",
       psql("SELECT group_id::text FROM torrents WHERE id=%s" % t2)
       == str(gid))
    live(t2)
    clear_cache(t2)
    # 组并列走 /torrents/{id}/group（group_info，items=过审组员）
    s, g = call("GET", "/torrents/%s/group" % t2, token=tok)
    versions = (g.get("data") or {}).get("items") or []
    ok("同组两版本并列（GroupVersions）", len(versions) == 2,
       "t1=%s t2=%s v1g=%s items=%s" % (
           t1, t2,
           psql("SELECT COALESCE(group_id::text,'NULL') FROM torrents "
                "WHERE id=%s" % t1),
           [v.get("id") for v in versions]))

    # ================= sports =================
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "sports", "mode": "merge"}, token=tok)
    ok("apply sports", s == 200 and r.get("code") == 0, r)
    kinds = dict(
        (l.split("=")[0], l.split("=")[1])
        for l in psql(
            "SELECT kind || '=' || field_type FROM section_kinds "
            "WHERE kind IN ('league','season','round')"
        ).splitlines() if "=" in l
    )
    ok("league=select", kinds.get("league") == "select", kinds)
    ok("season=text", kinds.get("season") == "text", kinds)
    ok("round=text", kinds.get("round") == "text", kinds)
    lg = psql("SELECT id FROM section_dict WHERE kind='league' "
              "AND name='足球' LIMIT 1")
    if not lg:
        psql("INSERT INTO section_dict (kind, name, sort) "
             "SELECT 'league','足球',10 WHERE NOT EXISTS "
             "(SELECT 1 FROM section_dict WHERE kind='league' "
             "AND name='足球')")
        lg = psql("SELECT id FROM section_dict WHERE kind='league' "
                  "AND name='足球' LIMIT 1")
    ok("league 词表含足球（gate 自补）", lg != "", lg)
    s, r = upload(tok, make_torrent(name="Audit.Sports.M"),
                  {"league": {"dict_ids": [int(lg)]},
                   "season": {"text": "2025-26"},
                   "round": {"text": "10"}}, "Audit.Sports.M")
    t3 = (r.get("data") or {}).get("id")
    made.append(t3)
    ok("对阵维度落库",
       psql("SELECT count(*) FROM torrent_sections WHERE torrent_id=%s "
            "AND kind IN ('league','season','round')" % t3) == "3")
    live(t3)
    s, r = call("GET", "/torrents?sec_league=%s&sec_round=10" % lg,
                token=tok)
    items = (r.get("data") or {}).get("items") or []
    ok("league+round 组合筛选命中",
       any(i.get("id") == t3 for i in items), len(items))
    s, r = call("GET", "/torrents?sec_round=11", token=tok)
    items = (r.get("data") or {}).get("items") or []
    ok("round=11 不命中", not any(i.get("id") == t3 for i in items))

    # ---- 清理 + 还原
    for t in made:
        if t:
            psql("DELETE FROM torrents WHERE id=%s" % t)
    psql("DELETE FROM torrent_groups WHERE name='射雕英雄传'")
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
