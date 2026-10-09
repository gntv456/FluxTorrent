# -*- coding: utf-8 -*-
"""站型成熟度补齐 · 批次 B 闸门（trumping G1 + 多版本分槽 G2 + features 机制）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_u_trump.py

判据（迁移 0336）：
  1. site_type_packs 有 features 列；movie 包 features.trumping=true
  2. apply movie 后：举报走通（features 消费点在起作用）
  3. apply general（features 空）后：举报被拒 → 证明「能力随站型可开关」
  4. 举报 → 版主待处理列表可见 → 裁决 accept → 被举报种 approval_status=2
  5. 多版本分槽：同组三版本（1080p/2160p/720p）→ tier 分别 HD/UHD/SD
  6. 淘汰后被举报种从组内可见列表消失
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


def upload(tok, tor, sections, name, group_id=None):
    fields = {"name": name, "small_descr": "gate", "descr": "audit",
              "category_id": "1", "sections": json.dumps(sections)}
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


def live(tid):
    psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
         "seeders=1 WHERE id=%s" % tid)


def std_id(name):
    return psql("SELECT id FROM section_dict WHERE kind='standard' "
                "AND name='%s' LIMIT 1" % name)


def main():
    tok = login()
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Trump%'")
    psql("DELETE FROM torrent_groups WHERE name='Audit.Trump.Group'")
    orig = psql("SELECT value FROM site_settings WHERE name='site_type'")
    made = []

    # ---- 1. features 列与包声明 ----
    has_col = psql("SELECT count(*) FROM information_schema.columns "
                   "WHERE table_name='site_type_packs' "
                   "AND column_name='features'")
    ok("site_type_packs.features 列存在", has_col == "1", has_col)
    ok("movie 包 features.trumping=true",
       psql("SELECT features->>'trumping' FROM site_type_packs "
            "WHERE code='movie'") == "true")
    ok("general 包 features 为空（无 trumping）",
       psql("SELECT features FROM site_type_packs WHERE code='general'")
       in ('{}', ''))

    # ---- 2. apply movie，造三版本同组 ----
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "movie", "mode": "merge"}, token=tok)
    ok("apply movie", s == 200 and r.get("code") == 0, r)
    ids = {}
    for tag, std in (("HD", "1080p"), ("UHD", "2160p/4K"), ("SD", "720p")):
        sid = std_id(std)
        if not sid:
            raise SystemExit("standard 词表缺 " + std)
        s, r = upload(tok, make_torrent(name="Audit.Trump." + tag),
                      {"standard": {"dict_ids": [int(sid)]}},
                      "Audit.Trump." + tag)
        tid = (r.get("data") or {}).get("id")
        ids[tag] = tid
        made.append(tid)
        ok("上传 %s(%s)" % (tag, std), s == 200 and r.get("code") == 0,
           "%s %s" % (s, r))

    # 挂同一组（UHD 先入组，其余按名挂）
    s, g = call("POST", "/torrents/%s/group" % ids["UHD"],
                {"name": "Audit.Trump.Group"}, token=tok)
    ok("UHD 建组", s == 200 and g.get("code") == 0, g)
    gid = psql("SELECT id FROM torrent_groups "
               "WHERE name='Audit.Trump.Group'")
    for tag in ("HD", "SD"):
        call("POST", "/torrents/%s/group" % ids[tag],
             {"name": "Audit.Trump.Group"}, token=tok)
    for t in made:
        live(t)

    # ---- 3. 多版本分槽：tier 派生 ----
    s, g = call("GET", "/torrents/%s/group" % ids["UHD"], token=tok)
    items = {i["id"]: i.get("tier") for i in
             (g.get("data") or {}).get("items") or []}
    ok("组内 tier HD", items.get(ids["HD"]) == "HD", items)
    ok("组内 tier UHD", items.get(ids["UHD"]) == "UHD", items)
    ok("组内 tier SD", items.get(ids["SD"]) == "SD", items)

    # ---- 4. 举报（movie 站，trumping 开） ----
    me = psql("SELECT id FROM users WHERE username='root'")
    other = psql("SELECT id FROM users WHERE id <> %s ORDER BY id LIMIT 1"
                 % me)
    psql("UPDATE torrents SET owner_id=%s WHERE id=%s" % (other, ids["HD"]))
    s, r = call("POST", "/torrents/%s/trump" % ids["HD"],
                {"reason": "bad_quality", "note": "画质劣于 UHD 版",
                 "target_torrent_id": int(ids["UHD"])}, token=tok)
    ok("举报走通（movie features.trumping）",
       s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    trump_id = (r.get("data") or {}).get("id")
    s, r = call("GET", "/torrents/%s/trumps" % ids["HD"], token=tok)
    ok("待处理计数=1", (r.get("data") or {}).get("pending") == 1, r)
    s, r = call("GET", "/admin/trumps", token=tok)
    listed = [i["id"] for i in (r.get("data") or {}).get("items") or []]
    ok("版主待处理列表可见", trump_id in listed, listed)

    # ---- 5. 裁决 accept → 淘汰 ----
    s, r = call("POST", "/admin/trumps/%s/resolve" % trump_id,
                {"accept": True, "note": "确为劣质"}, token=tok)
    ok("裁决 accept", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    ok("被举报种 approval_status=2",
       psql("SELECT approval_status FROM torrents WHERE id=%s" % ids["HD"])
       == "2")
    s, g = call("GET", "/torrents/%s/group" % ids["UHD"], token=tok)
    left = [i["id"] for i in (g.get("data") or {}).get("items") or []]
    ok("淘汰后离开组内可见列表", ids["HD"] not in left, left)
    ok("重复裁决被拒",
       call("POST", "/admin/trumps/%s/resolve" % trump_id,
            {"accept": True}, token=tok)[0] == 400)

    # ---- 6. 切 general → 能力关闭（features 门真的在起作用） ----
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": "general", "mode": "merge"}, token=tok)
    ok("apply general", s == 200 and r.get("code") == 0, r)
    s, r = call("POST", "/torrents/%s/trump" % ids["UHD"],
                {"reason": "dead"}, token=tok)
    ok("general 站举报被拒（features 门生效）",
       s == 400, "%s %s" % (s, r))

    # ---- 清理 + 还原 ----
    for t in made:
        if t:
            psql("DELETE FROM torrents WHERE id=%s" % t)
    psql("DELETE FROM torrent_groups WHERE name='Audit.Trump.Group'")
    back = psql("SELECT value FROM site_settings WHERE name='site_type'")
    if back != orig:
        call("POST", "/admin/site-type-packs/apply",
             {"code": orig, "mode": "merge"}, token=tok)
        back = psql("SELECT value FROM site_settings WHERE name='site_type'")
    ok("站型还原", back == orig, (orig, back))


if __name__ == "__main__":
    main()
    summary()
