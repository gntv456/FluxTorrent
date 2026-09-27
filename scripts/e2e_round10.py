#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Round10 验收：G17 wishlist grade_id 退净 / G20 分类层级与排序下发 /
G21 站型包 apply 的台账·回看·回滚。

跑在侧栈（一次性库），前置：api/web 已起、装机向导已完成。用法：
  FLUX_E2E_BASE=http://127.0.0.1:8100/api/v1 \
  FLUX_E2E_WEB=http://127.0.0.1:3100 FLUX_E2E_DB=r10_x \
  FLUX_E2E_ROOT_PW=<装机后口令> python scripts/e2e_round10.py

G21 的回滚语义（0167 先例）：把 apply 前的全量快照当一次 apply 重放，
所以断言用「回滚后分类表逐行 == apply 前」这种最强口径，而不是反向操作比对。
"""
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

API = os.environ.get("FLUX_E2E_BASE", "http://127.0.0.1:8100/api/v1")
WEB = os.environ.get("FLUX_E2E_WEB", "http://127.0.0.1:3100")
DB = os.environ.get("FLUX_E2E_DB", "r10_x")
PW = os.environ.get("FLUX_E2E_ROOT_PW", "password123")
PASS = FAIL = 0

CATS_AGG = ("SELECT string_agg(id||':'||name||':'||"
            "COALESCE(parent_id::text,'-')||':'||sort||':'||"
            "COALESCE(icon_key,'')||':'||COALESCE(bg_color,'-'), "
            "'|' ORDER BY id) FROM categories")
MODS_AGG = ("SELECT string_agg(name||'='||value, '|' ORDER BY name) "
            "FROM site_settings WHERE name LIKE 'module\\_%'")


def check(name, cond, detail=""):
    global PASS, FAIL
    if cond:
        PASS += 1
        print(f"PASS {name}")
    else:
        FAIL += 1
        print(f"FAIL {name}  {str(detail)[:240]}")


def psql(sql):
    q = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
         DB, "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (q.stdout or "").strip()


def setting(name):
    return psql(f"SELECT value FROM site_settings WHERE name='{name}'")


def call(method, path, body=None, token=None):
    """返回 (status, envelope)；envelope 是完整信封 {code,data,message}。"""
    req = urllib.request.Request(API + path, method=method)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = None
    if body is not None:
        req.add_header("Content-Type", "application/json")
        data = json.dumps(body).encode()
    try:
        with urllib.request.urlopen(req, data, timeout=30) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}


def webpage(path, token=None):
    req = urllib.request.Request(WEB + path)
    if token:
        req.add_header("Cookie", f"flux.session=1; flux_token={token}")
    with urllib.request.urlopen(req, timeout=30) as r:
        return r.status, r.read().decode("utf-8", errors="replace")


def login(name="root", pwd=PW):
    for _ in range(5):
        st, env = call("POST", "/auth/login",
                       {"username": name, "password": pwd})
        t = ((env or {}).get("data") or {}).get("token")
        if st == 200 and t:
            return t
        time.sleep(0.5)
    return None


tok = login()
check("root 登录（装机后口令）", bool(tok))
if not tok:
    print("口令不对就先跑 scripts/install_e2e.py 或传 FLUX_E2E_ROOT_PW")
    sys.exit(1)
# 探针自检：token 必须真能用，否则后面的「否定式断言」会因 401 假通过
st, env = call("GET", "/me", token=tok)
check("探针自检：GET /me 通", st == 200 and (env or {}).get("code") == 0,
      str(env)[:150])

print("\n===== ① G17：wishlist grade_id 退净（0222） =====")
n = psql("SELECT count(*) FROM information_schema.columns "
         "WHERE table_name='wishlist' AND column_name='grade_id'")
check("DB 列已 DROP", n == "0", n)
st, env = call("POST", "/wishlist", {"keyword": "round10-probe"}, token=tok)
check("POST /wishlist 可用", st in (200, 201), f"HTTP{st} {env}")
st, env = call("GET", "/wishlist", token=tok)
rows = (env or {}).get("data") or []
check("GET /wishlist 回读含探针条目",
      any(r.get("keyword") == "round10-probe" for r in rows), rows)
check("响应无 grade_id 字段", "grade_id" not in json.dumps(rows))

print("\n===== ② G20：分类层级与排序下发 =====")
st, env = call("POST", "/admin/categories",
               {"name": "R10父", "sort": -7}, token=tok)
pid = ((env or {}).get("data") or {}).get("id")
check("建父分类（sort=-7）", st == 200 and pid, f"HTTP{st} {env}")
st, env = call("POST", "/admin/categories",
               {"name": "R10子", "parent_id": pid, "sort": -6}, token=tok)
cid = ((env or {}).get("data") or {}).get("id")
check("建子分类（parent_id/ sort=-6）", st == 200 and cid, f"HTTP{st} {env}")
st, env = call("GET", "/site-profile")
cats = ((env or {}).get("data") or {}).get("categories") or []
check("site-profile 按 sort 出序（-7 排首位，非 id 序）",
      bool(cats) and cats[0].get("id") == pid, cats[:3])
sub = next((c for c in cats if c.get("id") == cid), None)
check("site-profile 下发 parent_id", (sub or {}).get("parent_id") == pid, sub)
st, env = call("GET", "/admin/categories", token=tok)
adm = (env or {}).get("data") or []
check("后台分类列表带 sort 且按 sort 出序",
      bool(adm) and adm[0].get("id") == pid and "sort" in adm[0], adm[:2])
st, html = webpage("/torrents", token=tok)
check("资源库页载荷含「R10父 › R10子」（分级面层级可见）",
      "R10父 › R10子" in html, "payload 未命中")
st, html = webpage("/upload", token=tok)
# 分类下拉在 UploadForm（client 组件）里客户端取数，SSR 只验表单容器落地；
# 「R10父 › R10子」的可见性归浏览器验收
check("发布页渲染上传表单容器", "upload-form" in html)

print("\n===== ③ G21：apply 台账（回看） =====")
before_cats = psql(CATS_AGG)
before_type = setting("site_type")
before_mods = psql(MODS_AGG)
before_tagline = setting("site_tagline")
before_cat_n = int(psql("SELECT count(*) FROM categories"))
st, env = call("POST", "/admin/site-type-packs/apply",
               {"code": "movie", "mode": "merge"}, token=tok)
aid = ((env or {}).get("data") or {}).get("apply_id")
check("apply movie(merge) 成功且响应带 apply_id",
      st == 200 and isinstance(aid, int) and aid > 0, f"HTTP{st} {env}")
st, env = call("GET", "/admin/site-type-packs/applies", token=tok)
rows = (env or {}).get("data") or []
top = rows[0] if rows else {}
check("台账首行 = 本次 apply",
      top.get("id") == aid and top.get("pack_code") == "movie"
      and top.get("mode") == "merge", top)
check("变更预览非空（回看）",
      isinstance(top.get("changes"), list) and len(top["changes"]) > 0,
      top.get("changes"))
check("预览含 site_type → movie",
      any(c.get("key") == "site_type" and c.get("new") == "movie"
          for c in (top.get("changes") or [])), top.get("changes"))
check("操作人 = root", top.get("actor") == "root", top.get("actor"))
check("新记录未回滚", not top.get("rolled_back_at"), top)
check("快照记录的是 apply 前站型",
      psql(f"SELECT snapshot->'site'->>'site_type' "
           f"FROM site_pack_applies WHERE id={aid}") == before_type)
check("快照含分类段（可作回滚基线）",
      psql(f"SELECT jsonb_array_length(snapshot->'categories') "
           f"FROM site_pack_applies WHERE id={aid}") == str(before_cat_n))
check("site_type 已切 movie", setting("site_type") == "movie")
check("同 id 分类改名（movie 包 id=1 从「电影」→ 含 BluRay）",
      "BluRay" in psql("SELECT name FROM categories WHERE id=1"))
check("分类数不变（movie 1-7 与 general 1-7 同 id：改名而非追加）",
      int(psql("SELECT count(*) FROM categories")) == before_cat_n)

print("\n===== ③ G21：apply 后站长改动，回滚应带回（快照重放语义） =====")
st, env = call("PUT", f"/admin/categories/{pid}",
               {"name": "R10父改"}, token=tok)
check("站长改名（apply 后、回滚前）", st == 200, f"HTTP{st} {env}")
st, env = call("POST", f"/admin/site-type-packs/applies/{aid}/rollback",
               None, tok)
check("回滚最近一次 apply 成功", st == 200 and (env or {}).get("code") == 0,
      f"HTTP{st} {env}")
check("分类表逐行还原（含改名回退/自建父子/排序/图标/色）",
      psql(CATS_AGG) == before_cats, psql(CATS_AGG))
check("site_type 还原", setting("site_type") == before_type)
check("module_* 全集还原", psql(MODS_AGG) == before_mods)
check("site_tagline 还原", setting("site_tagline") == before_tagline)
check("台账标记 rolled_back（人/时）",
      psql(f"SELECT (rolled_back_at IS NOT NULL)||':'||"
           f"COALESCE(rolled_back_by::text,'-') FROM site_pack_applies "
           f"WHERE id={aid}") == "true:1")
st, env = call("GET", "/admin/site-type-packs/applies", token=tok)
top = ((env or {}).get("data") or [{}])[0]
check("列表反映已回滚", bool(top.get("rolled_back_at")), top)
st, env = call("POST", f"/admin/site-type-packs/applies/{aid}/rollback",
               None, tok)
check("重复回滚被拒（已回滚过）",
      st == 400 and "已回滚" in ((env or {}).get("message") or ""),
      f"HTTP{st} {env}")

print("\n===== ④ G21：replace 整表重建后回滚（图标也必须还原） =====")
check("前置：无种子，replace 可用",
      psql("SELECT count(*) FROM torrents") == "0")
st, env = call("POST", "/admin/site-type-packs/apply",
               {"code": "movie", "mode": "replace"}, token=tok)
aid2 = ((env or {}).get("data") or {}).get("apply_id")
check("apply movie(replace) 成功", st == 200 and aid2, f"HTTP{st} {env}")
check("分类整表重建为 movie 的 7 个",
      int(psql("SELECT count(*) FROM categories")) == 7
      and "BluRay" in psql("SELECT name FROM categories WHERE id=1"))
check("旧包独有分类已清（id=8 不存在）",
      psql("SELECT count(*) FROM categories WHERE id=8") == "0")
st, env = call("POST", f"/admin/site-type-packs/applies/{aid2}/rollback",
               None, tok)
check("回滚 replace 成功", st == 200, f"HTTP{st} {env}")
check("分类表逐行还原（replace 后图标停在 movie 推断值，也须以快照还原）",
      psql(CATS_AGG) == before_cats, psql(CATS_AGG))
check("自建子分类层级回归（replace 连父子一并重建）",
      psql(f"SELECT parent_id::text FROM categories "
           f"WHERE id={cid}") == str(pid))

print("\n===== ④ G21：旧记录不可回滚 + 「未回滚栈顶」口径（本次修复点） =====")
st, env = call("POST", "/admin/site-type-packs/apply",
               {"code": "movie", "mode": "merge"}, token=tok)
aid3 = ((env or {}).get("data") or {}).get("apply_id")
st, env = call("POST", "/admin/site-type-packs/apply",
               {"code": "general", "mode": "merge"}, token=tok)
aid4 = ((env or {}).get("data") or {}).get("apply_id")
check("连续两次 apply 各留台账", bool(aid3 and aid4 and aid4 > aid3),
      f"{aid3}/{aid4}")
check("分类数不变且 id=1 回到 general 名（电影）",
      int(psql("SELECT count(*) FROM categories")) == before_cat_n
      and psql("SELECT name FROM categories WHERE id=1") == "电影")
st, env = call("POST", f"/admin/site-type-packs/applies/{aid3}/rollback",
               None, tok)
check("回滚旧记录被拒（不是最近一次）",
      st == 400 and "只能回滚" in ((env or {}).get("message") or ""),
      f"HTTP{st} {env}")
st, env = call("POST", f"/admin/site-type-packs/applies/{aid4}/rollback",
               None, tok)
check("回滚栈顶成功（快照 = movie merge 后的状态）", st == 200,
      f"HTTP{st} {env}")
check("回滚后 id=1 回到 movie 名（BluRay）",
      "BluRay" in psql("SELECT name FROM categories WHERE id=1"))
st, env = call("POST", f"/admin/site-type-packs/applies/{aid3}/rollback",
               None, tok)
check("栈顶已回滚后旧记录可继续回滚（旧 max(id) 口径在此死锁）",
      st == 200, f"HTTP{st} {env}")
check("连续回滚后分类表逐行还原", psql(CATS_AGG) == before_cats)

print("\n===== ④ G21：回滚清理本次 apply 新建、快照没有的分类 =====")
st, env = call("POST", "/admin/site-type-packs/apply",
               {"code": "movie", "mode": "merge"}, token=tok)
aid5 = ((env or {}).get("data") or {}).get("apply_id")
st, env = call("POST", "/admin/categories", {"name": "R10新"}, token=tok)
newid = ((env or {}).get("data") or {}).get("id")
check("apply 后新建分类", st == 200 and newid, f"HTTP{st} {env}")
st, env = call("POST", f"/admin/site-type-packs/applies/{aid5}/rollback",
               None, tok)
check("回滚成功", st == 200, f"HTTP{st} {env}")
check("快照外的自建分类被清理",
      psql(f"SELECT count(*) FROM categories WHERE id={newid}") == "0")
check("清理后分类表逐行还原", psql(CATS_AGG) == before_cats)

print("\n===== ⑤ G21：分类色（0183）随快照还原 =====")
st, env = call("PUT", f"/admin/categories/{pid}",
               {"name": "R10父", "bg_color": "#123456"}, token=tok)
check("给自建分类上色", st == 200, f"HTTP{st} {env}")
st, env = call("POST", "/admin/site-type-packs/apply",
               {"code": "movie", "mode": "replace"}, token=tok)
aid6 = ((env or {}).get("data") or {}).get("apply_id")
check("replace 整表重建抹掉该分类",
      st == 200 and psql(f"SELECT count(*) FROM categories "
                         f"WHERE id={pid}") == "0", f"HTTP{st} {env}")
st, env = call("POST", f"/admin/site-type-packs/applies/{aid6}/rollback",
               None, tok)
check("回滚成功", st == 200, f"HTTP{st} {env}")
check("分类色随快照还原",
      psql(f"SELECT bg_color FROM categories WHERE id={pid}") == "#123456")

print("\n===== ⑥ 渲染层（server 侧粗验；可见文本由浏览器验收另跑） =====")
st, env = call("POST", "/admin/site-type-packs/apply",
               {"code": "movie", "mode": "merge"}, token=tok)
aid7 = ((env or {}).get("data") or {}).get("apply_id")
check("留一条未回滚记录给 UI（apply_id=%s）" % aid7, bool(aid7))
st, html = webpage("/admin?tool=cats", token=tok)
check("后台分类页载荷含「站型应用记录」", "站型应用记录" in html)
check("后台分类页载荷含「回滚」按钮文案", "回滚" in html)
check("后台分类页载荷含新表头「排序」", "排序" in html)

print(f"\n===== Round10 探针结果：PASS {PASS} / FAIL {FAIL} =====")
sys.exit(1 if FAIL else 0)
