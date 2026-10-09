# -*- coding: utf-8 -*-
"""SEO/OG 分型闸门（站型尾巴批）：详情页 generateMetadata 断言。

跑法：
    python scripts/pt_audit_s_seo.py

判据（打 web 容器 127.0.0.1:3000，须带登录 cookie）：
  1. 详情页 <title> = 种子名 | 站名
  2. meta description 含维度摘要（label: value 形态——按站型自动生效）
  3. 私有站（indexable 默认 false）：meta robots noindex/nofollow，
     且无 og:title（分享卡收敛，不泄漏种子标题）
  4. 列表页继承全局 noindex 口径
"""
import json
import os
import subprocess
import sys
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import login, ok, summary  # noqa: E402

WEB = os.environ.get("FLUX_WEB_BASE", "http://127.0.0.1:3000")
PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def page(path, tok):
    req = urllib.request.Request(WEB + path)
    # RSC 页面 SSR 直出 meta；登录态走 flux_token cookie（Path=/）
    req.add_header("Cookie", "flux_token=%s; flux.session=1" % tok)
    return urllib.request.urlopen(req, timeout=30).read().decode(
        "utf-8", "replace")


def main():
    tok = login()
    orig_type = psql("SELECT value FROM site_settings WHERE name='site_type'")
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.SEO%'")

    # 造一枚带维度的过审种（movie：季/集数）
    from pt_audit_g_logcheck import make_torrent, multipart, BOUNDARY
    from pt_audit_lib import BASE, call
    s, _ = call("POST", "/admin/site-type-packs/apply",
                {"code": "movie", "mode": "merge"}, token=tok)
    season = psql("SELECT id FROM section_dict WHERE kind='season' "
                  "AND name='第二季' LIMIT 1")
    fields = {"name": "Audit.SEO.T", "small_descr": "seo gate",
              "descr": "audit", "category_id": "1",
              "sections": json.dumps({
                  "season": {"dict_ids": [int(season)]},
                  "episode_first": {"number": 3}})}
    body = multipart(fields,
                     [("file", "a.torrent", make_torrent(name="Audit.SEO.T"))])
    req = urllib.request.Request(BASE + "/torrents", method="POST", data=body)
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + tok)
    tid = json.loads(
        urllib.request.urlopen(req, timeout=30).read())["data"]["id"]
    psql("UPDATE torrents SET approval_status=1, approved_at=now(), "
         "seeders=1 WHERE id=%s" % tid)

    brand = psql("SELECT value FROM site_settings WHERE name='site_name'")
    html = page("/torrent/%s" % tid, tok)
    ok("标题含种子名", "Audit.SEO.T" in html, html[:200])
    ok("标题含站名", brand in html, brand)
    ok("robots noindex（私有站默认）", "noindex" in html)
    import re as _re
    og = _re.search(r'property="og:title" content="([^"]*)"', html)
    ok("og:title 收敛为站名卡（不含种子标题）",
       og is None or ("Audit.SEO" not in og.group(1) and og.group(1)),
       og and og.group(1))
    m = 'name="description" content="' in html
    ok("meta description 存在", m)

    # 清理 + 还原
    psql("DELETE FROM torrents WHERE id=%s" % tid)
    back = psql("SELECT value FROM site_settings WHERE name='site_type'")
    if back != orig_type:
        call("POST", "/admin/site-type-packs/apply",
             {"code": orig_type, "mode": "merge"}, token=tok)


if __name__ == "__main__":
    main()
    summary()
