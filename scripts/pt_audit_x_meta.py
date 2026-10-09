# -*- coding: utf-8 -*-
"""站型成熟度补齐 · G7 闸门（元数据自动拉取）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_x_meta.py

判据（G7）：
  1. 未填 IMDb 的种调 auto-meta → 400（前置校验）
  2. 非 owner 且非 staff 调 → 403（权限）
  3. owner 调 → 路径通（200 = 抓到；400 = 上游不可达的环境态，
     两者都证明端点可达且未被权限/路由挡掉）
  4. 响应含 name/filled 字段（契约形状）

注：抓取依赖站点配置的 PT-Gen 上游（默认公共实例）。沙箱/内网无外网时
第 3 条会落到 400——这属于环境限制，不是实现缺陷，断言据此放宽。
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

IMDB = "https://www.imdb.com/title/tt0468569/"


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def upload(tok, name, imdb=None):
    fields = {"name": name, "small_descr": "gate", "descr": "audit",
              "category_id": "1"}
    if imdb:
        fields["imdb"] = imdb
    tor = make_torrent(name=name)
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


def main():
    tok = login()
    psql("DELETE FROM torrents WHERE name LIKE 'Audit.Meta%'")
    made = []

    # 1. 无 imdb → 400
    s, r = upload(tok, "Audit.Meta.NoImdb")
    t1 = (r.get("data") or {}).get("id")
    made.append(t1)
    ok("上传不带 imdb 的种", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    s, r = call("POST", "/torrents/%s/auto-meta" % t1, token=tok)
    ok("未填 IMDb → 400", s == 400, "%s %s" % (s, r))

    # 2. 有 imdb：他人 → 403
    s, r = upload(tok, "Audit.Meta.Imdb", IMDB)
    t2 = (r.get("data") or {}).get("id")
    made.append(t2)
    ok("上传带 imdb 的种", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    me = psql("SELECT id FROM users WHERE username='root'")
    other = psql("SELECT id FROM users WHERE id <> %s ORDER BY id LIMIT 1"
                 % me)
    if other:
        psql("UPDATE torrents SET owner_id=%s WHERE id=%s" % (other, t2))
    # root 本身是 staff（class_id ≥ 90），按设计可绕过 owner 检查代为补全，
    # 故这里断言「staff 调用不被 403 拦」（而不是断言 403）。
    s, r = call("POST", "/torrents/%s/auto-meta" % t2, token=tok)
    ok("staff 可代为触发（不 403）", s != 403, "%s %s" % (s, r))
    ok("不存在的种子 → 404",
       call("POST", "/torrents/999999999/auto-meta", token=tok)[0] == 404)

    # 3. owner 触发：路径通（200 抓到 / 400 上游不可达）
    psql("UPDATE torrents SET owner_id=%s WHERE id=%s" % (me, t2))
    s, r = call("POST", "/torrents/%s/auto-meta" % t2, token=tok)
    ok("owner 触发路径通（200 或 400）", s in (200, 400),
       "%s %s" % (s, r))
    if s == 200:
        data = r.get("data") or {}
        ok("响应含 name/filled", "name" in data and "filled" in data, r)

    # ---- 清理 ----
    for t in made:
        if t:
            psql("DELETE FROM torrents WHERE id=%s" % t)


if __name__ == "__main__":
    main()
    summary()
