#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Round8（P2 滚动）验收：G8 控件一致性 / G9 字幕改写原子性 / G10 mediainfo 默认口径 /
G11 勋章图片上传。真库真容器，含渲染层断言（stripped-DOM 与 RSC payload 分开判）。

用法：python scripts/e2e_round8.py [--skip-g8-bool]
  · G8 需要临时建一个 bool 维度（验完删除）；主验在浏览器（本脚本只验接口侧存在性）。
"""
import base64
import json
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

API = "http://127.0.0.1:8080/api/v1"
WEB = "http://127.0.0.1:3000"
PASS = FAIL = 0
SKIP_G8 = "--skip-g8-bool" in sys.argv


def check(name, cond, detail=""):
    global PASS, FAIL
    if cond:
        PASS += 1
        print(f"PASS {name}")
    else:
        FAIL += 1
        print(f"FAIL {name}  {str(detail)[:200]}")


def psql(sql):
    q = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
         "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8")
    return (q.stdout or "").strip()


def call(method, path, body=None, token=None, ctype="application/json",
         raw_body=None):
    req = urllib.request.Request(API + path, method=method)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = None
    if raw_body is not None:
        req.add_header("Content-Type", ctype)
        data = raw_body
    elif body is not None:
        req.add_header("Content-Type", "application/json")
        data = json.dumps(body).encode()
    try:
        with urllib.request.urlopen(req, data, timeout=20) as r:
            payload = json.loads(r.read())
            return r.status, payload.get("data", payload)
    except urllib.error.HTTPError as e:
        try:
            payload = json.loads(e.read())
        except Exception:
            payload = {}
        return e.code, payload.get("data", payload)


def webpage(path, token=None, cookie=None):
    req = urllib.request.Request(WEB + path)
    if cookie:
        req.add_header("Cookie", cookie)
    elif token:
        req.add_header("Cookie", f"flux.session=1; flux_token={token}")
    with urllib.request.urlopen(req, timeout=30) as r:
        return r.status, r.read().decode("utf-8", errors="replace")


def strip_scripts(html):
    return re.sub(r"<script.*?</script>", "", html, flags=re.S)


def login(name="root", pwd="password123"):
    for _ in range(5):
        st, r = call("POST", "/auth/login", {"username": name, "password": pwd})
        if st == 200 and isinstance(r, dict) and r.get("token"):
            return r["token"]
        time.sleep(0.4)
    return None


def multipart_file(field, filename, content, ctype):
    b = "----round8" + str(int(time.time() * 1000))
    body = (f'--{b}\r\nContent-Disposition: form-data; name="{field}"; '
            f'filename="{filename}"\r\nContent-Type: {ctype}\r\n\r\n'
            ).encode() + content + f"\r\n--{b}--\r\n".encode()
    return body, f"multipart/form-data; boundary={b}"


tok = login()
check("root 登录", bool(tok))
if not tok:
    sys.exit(1)

print("\n===== ① G10：metadata_sources 默认含 mediainfo =====")
v = psql("SELECT value FROM site_settings WHERE name='metadata_sources'")
check("库内设置值含 mediainfo", "mediainfo" in v, v)
rows = psql("SELECT code || '=' || COALESCE(metadata->>'sources','-') "
            "FROM site_type_packs ORDER BY code").splitlines()
missing = [r for r in rows if "[" in r and "mediainfo" not in r]
check(f"站型包 sources 全含 mediainfo（{len(rows)} 包）", not missing, missing)
hint = psql("SELECT hint FROM settings_meta WHERE name='metadata_sources'")
check("设置卡提示写明 mediainfo", "mediainfo" in hint, hint)
n221 = psql("SELECT count(*) FROM _sqlx_migrations WHERE version=221")
check("迁移 221 已应用", n221 == "1", n221)
st, prof = call("GET", "/site-profile", token=tok)
srcs = (prof or {}).get("metadata_sources") or []
check("/site-profile 下发含 mediainfo", "mediainfo" in srcs, srcs)
st, html = webpage("/upload", token=tok)
dom = strip_scripts(html)
check("发布页 DOM 含 MediaInfo 文本框",
      "MediaInfo" in dom and "可选" in dom, "未找到 textarea")
check("发布页 DOM 含 IMDb 输入（未被误关）", "imdb.com" in dom or "IMDb" in dom)

print("\n===== ② G9：字幕改名不误伤复合词 =====")
old = psql("SELECT value FROM site_settings WHERE name='subtitle_label'")
try:
    psql("UPDATE site_settings SET value='歌词' WHERE name='subtitle_label'")
    st, html = webpage("/subtitles", token=tok)
    dom = strip_scripts(html)
    check("重命名生效：payload 有「歌词标题」", "歌词标题" in html)
    check("重命名生效：DOM 标题「歌词区」", "歌词区" in dom)
    check("复合词原子：「字幕组」保留", "字幕组" in html)
    check("复合词原子：无「歌词组」副产物", "歌词组" not in html)
    check("复合词原子：「认证字幕人」保留", "认证字幕人" in html)
    check("复合词原子：无「认证歌词人」", "认证歌词人" not in html)
    check("原子占位符：{magic} 未被撕开", "{magic}" in html)
finally:
    psql(f"UPDATE site_settings SET value='{old}' WHERE name='subtitle_label'")
st, html = webpage("/subtitles", token=tok)
check("回退后无改写残留（歌词标题 消失）", "歌词标题" not in html)
check("回退后原文回归（字幕标题 存在）", "字幕标题" in html)

print("\n===== ③ G11：勋章图片上传闭环 =====")
# 勋章面板是客户端区块（读到 searchParams 才出内容）→ SSR HTML 只有 shell，
# 组件树在 RSC 载荷（<script>）里。此处断载荷存在性；**可见渲染**判据
# 在浏览器侧（本轮已用 browser-use 实测：按钮/预览/回读全过）。
st, html = webpage("/admin?tool=medals", token=tok)
check("勋章面板载荷含「上传图片」按钮", "上传图片" in html)
check("勋章面板载荷含「勋章图片 URL」输入", "勋章图片 URL" in html)
png = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8"
    "DwHwAFAAH/q842iQAAAABJRU5ErkJggg==")
body, ctype = multipart_file("file", "e2e-round8.png", png, "image/png")
st, up = call("POST", "/attachments", token=tok, ctype=ctype, raw_body=body)
url = (up or {}).get("url") if isinstance(up, dict) else None
check("附件上传返回站内地址", st == 200 and bool(url) and
      str(url).startswith("/api/v1/attachments/"), f"st={st} {up}")
medal_id = None
if url:
    req = urllib.request.Request("http://127.0.0.1:8080" + url)
    req.add_header("Cookie", f"flux.session=1; flux_token={tok}")
    with urllib.request.urlopen(req, timeout=20) as r:
        ct = r.headers.get("content-type") or ""
        check("附件回读 200 且为图片",
              r.status == 200 and "image" in ct, ct)
    # 该载荷刻意不带 bonus_addition_factor——网页表单「加成」留空即发 null，
    # 回归 2026-09-26 修的 500（NOT NULL 列直绑）。若又被填成必填，这里会红。
    st, m = call("POST", "/admin/medals", {
        "name": "e2e-round8-tmp", "get_type": 2, "category_id": 0,
        "limited": False, "asset_ref": url}, token=tok)
    check("建勋章带 asset_ref（空加成不 500）", st in (200, 201), f"st={st} {m}")
    st, rows2 = call("GET", "/admin/medals", token=tok)
    hit = [x for x in (rows2 or []) if x.get("name") == "e2e-round8-tmp"]
    medal_id = hit[0]["id"] if hit else None
    check("列表回读 asset_ref", bool(hit) and hit[0].get("asset_ref") == url,
          hit[:1])
    st, html = webpage("/medals", token=tok)
    check("勋章页渲染 <img> 而非 🏅", url in html, "未找到 img src")

print("\n===== ④ G8：bool 维度控件可达性 =====")
if SKIP_G8:
    print("SKIP（--skip-g8-bool）")
else:
    st, kinds = call("GET", "/admin/section-kinds", token=tok)
    has_bool = any((k.get("field_type") == "bool") for k in (kinds or []))
    if has_bool:
        check("已存在 bool 维度", True)
    else:
        st, k = call("POST", "/admin/section-kinds", {
            "kind": "e2e_bool", "label": "E2E 布尔", "field_type": "bool"},
            token=tok)
        check("可建 bool 维度（上传页是/否 控件的前置）", st in (200, 201),
              f"st={st} {k}")
        st, rows3 = call("GET", "/admin/section-kinds", token=tok)
        made = [x for x in (rows3 or []) if x.get("kind") == "e2e_bool"]
        check("bool 维度回读 field_type=bool",
              bool(made) and made[0].get("field_type") == "bool", made[:1])
        call("DELETE", "/admin/section-kinds/e2e_bool", token=tok)

print("\n===== 清理 =====")
if medal_id:
    st, _ = call("DELETE", f"/admin/medals/{medal_id}", token=tok)
    check("删除临时勋章", st in (200, 204), st)
    st, rows4 = call("GET", "/admin/medals", token=tok)
    check("临时勋章已消失",
          not [x for x in (rows4 or []) if x["id"] == medal_id])

print(f"\n===== Round8 E2E: {PASS} PASS / {FAIL} FAIL =====")
sys.exit(1 if FAIL else 0)
