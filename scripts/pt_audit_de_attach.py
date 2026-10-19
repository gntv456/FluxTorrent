# -*- coding: utf-8 -*-
"""闸门 DE：附件的可见性判据、内容嗅探与尺寸闸（含一根「公开页嵌站内图」的桩）。

批一百五十七实测（全部活体，两个身份 + 匿名）：
  私有附件  本人 200 / 别的成员 404 / 匿名 401        ⇒ 越权面干净
  共享附件  本人 200 / staff 200 / 匿名 401           ⇒ 见下面那根桩
  声明 png 实为 HTML  ⇒ 400「文件内容与声明类型不符」  ⇒ 改扩展存不进 HTML
  声明 text/html      ⇒ 400「仅支持 png/…」（白名单 14 项，无 html/svg）
  响应头 nosniff + CSP default-src 'none'             ⇒ 图床不会变 XSS 床

为什么共享附件那条只钉桩不修：全站内容口（/torrents、/forums、/home）对匿名一律
401，只有 /p/{slug}、/faq、/public-pages、/site-profile 可匿名——而公告/页面
消毒白名单**明确放行** /api/v1/attachments/ 相对前缀，也就是说站长可以在公开页
里放一张站内图，但游客与爬虫取图必 401（裂图、无提示）。两条出路都是产品决定：
  ① 放开 visibility='shared' 的附件给匿名（sha256 不可枚举 + nosniff/CSP 仍在），
     代价是「附件查看最低等级」这类门槛对共享件失效；
  ② 保持登录才能取图，同时在页面编辑器里提示「站内图对游客不可见」。
本闸门把现状钉死（DE2），改动时必须同步这里与 P-342 的记录。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_de_attach.py
"""
import hashlib
import io
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
INV = []


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:160])
    return (r.stdout or "").strip()


def upload(tok, content, name, mime, visibility=None):
    """multipart 上传（字段 file + 可选 visibility）。"""
    boundary = "----deattach%d" % (int(time.time() * 1000) % 1000000)
    head = ""
    if visibility:
        head += ("--%s\r\nContent-Disposition: form-data; name=\"visibility"
                 "\"\r\n\r\n%s\r\n" % (boundary, visibility))
    head += ('--%s\r\nContent-Disposition: form-data; name="file"; '
             'filename="%s"\r\nContent-Type: %s\r\n\r\n'
             % (boundary, name, mime))
    body = head.encode() + content + ("\r\n--%s--\r\n" % boundary).encode()
    req = urllib.request.Request(BASE + "/attachments", method="POST",
                                data=body)
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % boundary)
    if tok:
        req.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(req, timeout=60)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:  # noqa: BLE001
            return e.code, {"raw": raw[:120].decode("utf-8", "replace")}


def get_raw(sha, tok=None):
    req = urllib.request.Request(BASE + "/attachments/" + sha)
    if tok:
        req.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(req, timeout=30)
        return r.status, dict(r.headers), r.read()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()


# ---------- 两个成员探针（走产品自己的注册口） ----------
clear_register_rl()


def mkprobe(prefix):
    inv = call("POST", "/admin/invites", {"user_id": 1, "count": 1},
               token=ROOT)
    codes = ((inv[1] or {}).get("data") or {}).get("codes") or []
    cv = call("GET", "/auth/captcha", token=ROOT)
    cap = (cv[1] or {}).get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(nums) < 2:
        return 0, "", "发码/验证码没准备好"
    nm = "%s%d" % (prefix, TAG)
    rg = call("POST", "/auth/register", {
        "username": nm, "email": "%s@de.local" % nm,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    uid = (((rg[1] or {}).get("data") or {}).get("user_id")) or 0
    if not uid:
        uid = int(psql("SELECT id FROM users WHERE username = '%s'" % nm)
                  or 0)
    lg = call("POST", "/auth/login", {"username": nm,
                                      "password": "password123"})
    tk = ((lg[1] or {}).get("data") or {}).get("token") or ""
    INV.append(codes[0]["id"])
    return uid, tk, "" if tk else "登录没拿到 token(http=%s)" % lg[0]


M1, T1, E1 = mkprobe("de1p")
M2, T2, E2 = mkprobe("de2p")
ok("DE0 三个身份就绪（root + 成员 M1=%s M2=%s）%s"
   % (M1, M2, E1 + E2), bool(T1) and bool(T2) and M1 and M2, E1 or E2)

PNG = bytes.fromhex("89504e470d0a1a0a") + b"\x00" * 24
PRIV = PNG + b"de-private-%d" % TAG
SHA_A = PNG + b"de-shared-%d" % TAG
DUP = PNG + b"de-dup-%d" % TAG

st1, r1 = upload(ROOT, PRIV, "p.png", "image/png", "private")
SHA_PRIV = ((((r1 or {}).get("data")) or {}).get("sha256")) or ""
st2, r2 = upload(T1, SHA_A, "s.png", "image/png")
SHA_SHARED = ((((r2 or {}).get("data")) or {}).get("sha256")) or ""
ok("DE0b 上传口可用（私有 http=%s、成员共享 http=%s）" % (st1, st2),
   st1 == 200 and st2 == 200 and SHA_PRIV and SHA_SHARED,
   json.dumps([r1, r2], ensure_ascii=False)[:150])

# ---------- DE1 私有附件的越权面 ----------
own = get_raw(SHA_PRIV, ROOT)
other = get_raw(SHA_PRIV, T2)
anon = get_raw(SHA_PRIV, None)
ok("DE1 私有附件：本人 200、别的成员 404、匿名 401（实测 %s/%s/%s）"
   % (own[0], other[0], anon[0]),
   own[0] == 200 and other[0] == 404 and anon[0] == 401,
   "priv own=%s other=%s anon=%s" % (own[0], other[0], anon[0]))
vis = psql("SELECT coalesce(visibility,'NULL') FROM attachments WHERE sha256 = "
           "'%s'" % SHA_PRIV)
ok("DE1b 落库的可见性确实是 private（读到 %s）" % vis, vis == "private")

# ---------- DE2 共享附件现状桩 ----------
sother = get_raw(SHA_SHARED, T2)
sroot = get_raw(SHA_SHARED, ROOT)
sanon = get_raw(SHA_SHARED, None)
ok("DE2 共享附件：登录成员与 staff 可读、匿名 401（实测 %s/%s/%s）"
   % (sother[0], sroot[0], sanon[0]),
   sother[0] == 200 and sroot[0] == 200 and sanon[0] == 401,
   "shared other=%s root=%s anon=%s" % (sother[0], sroot[0], sanon[0]))
print("   桩：/p/{slug} 与 /faq 可匿名、消毒白名单放行 /api/v1/attachments/ 前缀，"
      "但图本身要登录 ⇒ 公开页里的站内图对游客必裂（P-342 待裁，两条出路见文件头）")

# ---------- DE3 内容与声明不符（防「改扩展名存 HTML」） ----------
HTML = (b"<html><script>alert('xss')</script><body>hi</body></html>")
st3, r3 = upload(T1, HTML, "x.png", "image/png")
ok("DE3 内容是 HTML 却声明 png 必须拒（http=%s）" % st3, st3 == 400,
   json.dumps(r3, ensure_ascii=False)[:120])
st4, r4 = upload(T1, HTML, "x.html", "text/html")
ok("DE3b 声明 text/html 直接不在白名单（http=%s）" % st4, st4 == 400,
   json.dumps(r4, ensure_ascii=False)[:120])
SVG = b'<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"></svg>'
st5, r5 = upload(T1, SVG, "s.svg", "image/svg+xml")
ok("DE3c SVG 不在白名单（带 onload 的矢量图存不进来，http=%s）" % st5,
   st5 == 400, json.dumps(r5, ensure_ascii=False)[:120])

# ---------- DE4 尺寸闸（attach_max_mib，现值见库） ----------
cap_mib = int(psql("SELECT COALESCE((SELECT value FROM site_settings WHERE "
                   "name='attach_max_mib')::text,'8')") or 8)
big = PNG + bytes(1024 * 1024 * (cap_mib + 1))
st6, r6 = upload(T1, big, "big.png", "image/png")
ok("DE4 超上限（%d MiB + 1）的上传必须被拒（http=%s）" % (cap_mib, st6),
   st6 in (400, 413), json.dumps(r6, ensure_ascii=False)[:120])

# ---------- DE5 响应头不变量（图床≠XSS 床） ----------
hdr = own[1]
ok("DE5 附件响应带 nosniff 与 CSP default-src 'none'（content-type=%s）"
   % str(hdr.get("content-type")),
   str(hdr.get("x-content-type-options", "")).lower() == "nosniff"
   and "default-src 'none'" in str(hdr.get("content-security-policy", "")),
   json.dumps({k: hdr.get(k) for k in ("content-type",
              "x-content-type-options")}, ensure_ascii=False))

# ---------- DE6 同一 sha 多行的**聚合**判定（P-57 回归钉） ----------
DUP = PNG + b"de-dup-%d" % TAG
st7, r7 = upload(ROOT, DUP, "d.png", "image/png", "private")
sha_dup = ((r7 or {}).get("data") or {}).get("sha256") or ""
before = get_raw(sha_dup, T2)
ok("DE6a 只有 root 的 private 行时，别的成员读必须 404（http=%s）" % before[0],
   before[0] == 404, "sha=%s" % sha_dup[:12])
st8, r8 = upload(T1, DUP, "d.png", "image/png")
rows_dup = psql("SELECT count(*) FROM attachments WHERE sha256 = '%s'"
                % sha_dup)
after = get_raw(sha_dup, T2)
ok("DE6b 出现一条 shared 行后，无行的第三方成员必须能读（http=%s，"
   "行数=%s）⇒ 判定必须是聚合，任取一行就会时通时不通"
   % (after[0], rows_dup), after[0] == 200 and rows_dup == "2",
   "after=%s rows=%s" % (after[0], rows_dup))
vis_row = psql("SELECT string_agg(coalesce(visibility,'NULL'), ',' ORDER BY "
               "user_id) FROM attachments WHERE sha256 = '%s'" % sha_dup)
ok("DE6c 两行的可见性各归各（root=private、M1=shared，实测 %s）" % vis_row,
   vis_row == "private,shared")

# ---------- DE7 收尾 ----------
for uid in (M1, M2):
    call("POST", "/admin/users/status", {"user_id": uid, "status": 2},
         token=ROOT)
    call("DELETE", "/admin/users/%d" % uid, None, token=ROOT)
if INV:
    psql("UPDATE invites SET status = 2 WHERE used_by IS NULL AND id IN (%s)"
         % ",".join(str(int(i)) for i in INV))
left = psql("SELECT count(*) FROM users WHERE id IN (%d, %d) AND status < 2"
            % (M1, M2))
tomb = psql("SELECT count(*) FROM users WHERE id IN (%d, %d) AND status >= 2 "
            "AND passkey LIKE 'deleted-%%'" % (M1, M2))
ok("DE7 两个探针成员走产品自己的墓碑（活号残留=%s，墓碑=%s）" % (left, tomb),
   left == "0" and tomb == "2", "left=%s tomb=%s" % (left, tomb))
for sha in (SHA_PRIV, SHA_SHARED, sha_dup):
    if sha:
        psql("DELETE FROM attachments WHERE sha256 = '%s' AND user_id IN "
             "(1, %d, %d)" % (sha, M1, M2))
res = psql("SELECT count(*) FROM attachments WHERE sha256 IN ('%s','%s','%s')"
           % (SHA_PRIV, SHA_SHARED, sha_dup))
ok("DE7b 探针附件权限行已清（残留=%s；物理文件按 sha 去重语义可留）" % res,
   res == "0", "res=%s" % res)

summary()
