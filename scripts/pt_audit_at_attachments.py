# -*- coding: utf-8 -*-
"""闸门 AT：论坛附件（图床/视频）的写入配额、去重语义与读取口（批二十九）。

钉住四件事：
  S1 P-57 同一份字节跨用户可各有一行权限行。0100 的 sha256 唯一索引与 0294
     「他人已有行且本次要 private 时插独立行」互斥，实测后来者 500；索引换成
     (sha256, user_id) 后，读取端的可见性判定必须走**聚合**（有 shared 行就
     放行），不能取任意一行——两行时取到哪条全凭数据库心情；
  S2 P-58 0190 明文「视频只吃 video_quota_mib，不吃图片配额」，视频侧 SQL
     筛了 kind='video'，图片侧漏筛 kind ⇒ 传过视频的人图片配额被视频字节吃掉；
  S3 P-59 enableattach 在后台「附件设定」可见可改（fake_switch 基线 289 条
     之一），全仓零消费：站长以为关了附件，上传照过。现在两个上传口都判，
     而**读取口不判**（关一次把老帖的图全 404 是新缺陷）；
  S4 P-60 HEAD 只该回头不要体。旧实现为怕 actix 把 Content-Length 归零，
     挂了与文件等长的 0 占位 body（200MiB 视频每预检一次整块分配一次）。
     这里用「HEAD 的 Content-Length 必须等于库里 size」做回归守卫——占位
     body 摘掉后如果这条红了，就把它换回去。

共享设置（attach_quota_mib / video_quota_mib / forum_video_upload /
enableattach）串行改、finally 原值写回并回读。探针产物（附件行 + 物理文件 +
成员号 + 邀请码）自己收尾。

跑法：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_at_attachments.py
"""
import atexit
import hashlib
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
CLEAN_SHA = []
PIN = {}


def q(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip()[:200])
    return r.stdout.strip()


def ex(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode:
        print("   ! SQL 失败: %s" % (r.stderr or "").strip()[:150])
    return r.returncode


CLEAN = []


def cleanup():
    """可重复执行：中途崩了 atexit 仍会把共享设置与探针产物收掉。"""
    for name in ("attach_quota_mib", "video_quota_mib", "forum_video_upload",
                 "enableattach"):
        if name in PIN:
            ex("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
               % (PIN[name], name))
    for sha in CLEAN_SHA:
        subprocess.run(
            ["docker", "exec", "flux-api", "rm", "-f",
             "./attachments/%s/%s/%s" % (sha[:2], sha[2:4], sha)],
            capture_output=True, text=True, encoding="utf-8")
    del CLEAN_SHA[:]
    stmts = list(reversed(CLEAN))
    del CLEAN[:]
    for stmt in stmts:
        ex(stmt)


atexit.register(cleanup)


def brief(x):
    return str(x)[:170]


def multipart(path, token, body, filename, ctype, extra=None):
    bd = "----atbd%d" % TAG
    parts = []
    for k, v in (extra or {}).items():
        parts.append(('--%s\r\nContent-Disposition: form-data; name="%s"'
                      '\r\n\r\n%s\r\n' % (bd, k, v)).encode())
    parts.append((
        '--%s\r\nContent-Disposition: form-data; name="file"; filename="%s"'
        '\r\nContent-Type: %s\r\n\r\n' % (bd, filename, ctype)).encode())
    parts.append(body + b"\r\n")
    parts.append(("--%s--\r\n" % bd).encode())
    req = urllib.request.Request(BASE + path, method="POST",
                                 data=b"".join(parts))
    req.add_header("Content-Type", "multipart/form-data; boundary=%s" % bd)
    req.add_header("Authorization", "Bearer " + token)
    try:
        r = urllib.request.urlopen(req, timeout=60)
        return r.status, r.read()[:240]
    except urllib.error.HTTPError as e:
        return e.code, e.read()[:240]
    except Exception as e:  # noqa: BLE001
        return -1, str(e).encode()[:160]


def raw(method, path, token, headers=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Authorization", "Bearer " + token)
    for k, v in (headers or {}).items():
        req.add_header(k, v)
    try:
        r = urllib.request.urlopen(req, timeout=30)
        return r.status, dict(r.headers), r.read()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()
    except Exception as e:  # noqa: BLE001
        return -1, {}, str(e).encode()[:120]


def png(n=24):
    return b"\x89PNG\r\n\x1a\n" + uuid.uuid4().bytes + b"\x00" * n


def mp4(n=24):
    return b"\x00\x00\x00\x18ftypmp42" + uuid.uuid4().bytes + b"\x00" * n


def sha3(b):
    return hashlib.sha3_256(b).hexdigest()


def rows_of(sha):
    return int(q("SELECT count(*) FROM attachments WHERE sha256 = '%s'" % sha)
               or 0)


def upload(token, blob, name, ctype="image/png", vis=None):
    extra = {"visibility": vis} if vis else None
    s, body = multipart("/attachments", token, blob, name, ctype, extra)
    return s, body, sha3(blob)


def new_member(tag):
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return None
    code, cid = codes[0]["code"], codes[0]["id"]
    CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None
    user = "at%s%d" % (tag, TAG)
    s, rr = call("POST", "/auth/register", {
        "username": user, "email": "%s@at.local" % user,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    uid = (rr.get("data") or {}).get("user_id")
    if not uid:
        return None
    CLEAN.append("UPDATE users SET status = 2 WHERE id = %s" % uid)
    CLEAN.append("DELETE FROM attachments WHERE user_id = %s" % uid)
    CLEAN.append("DELETE FROM spark_ledger WHERE user_id = %s" % uid)
    tok = None
    for _ in range(8):
        tok = (call("POST", "/auth/login",
                    {"username": user, "password": "password123"})[1].get(
                        "data") or {}).get("token")
        if tok:
            break
        time.sleep(6)
    if not tok:
        return None
    return (tok, uid)


def pin(name):
    PIN[name] = q("SELECT value FROM site_settings WHERE name = '%s'" % name)


def setv(name, val):
    ex("UPDATE site_settings SET value = '%s' WHERE name = '%s'" % (val, name))


print("== S0 前置：三个成员号 + 共享设置快照 ==")
for k in ("attach_quota_mib", "video_quota_mib", "forum_video_upload",
          "enableattach"):
    pin(k)
A = new_member("a")
B = new_member("b")
C = new_member("c")
ok("三个探针成员可准备（注册/登录限流下）", bool(A and B and C),
   "A=%s B=%s C=%s" % (bool(A), bool(B), bool(C)))
if not (A and B and C):
    cleanup()
    summary()
    sys.exit(1)
AT, AID = A
BT, BID = B
CT, CID = C
# 上传口要开着（enableattach 缺省已是 yes，这里显式钉住本批口径）
setv("enableattach", "yes")
setv("attach_quota_mib", "512")

print("== S1 同一份字节跨用户（P-57）==")
X = png()
s, b, sx = upload(AT, X, "x1.png")
ok("S1 地基：A 传一份新图 200，sha 唯一行 1", s == 200 and rows_of(sx) == 1,
   "http=%s %s" % (s, brief(b)))
CLEAN_SHA.append(sx)
s, b, _ = upload(AT, X, "x1dup.png")
ok("S1 地基：同人重复上传仍去重（deduplicated=true，行数不变）",
   s == 200 and b'"deduplicated":true' in b and rows_of(sx) == 1,
   "http=%s %s" % (s, brief(b)))
s, b, _ = upload(BT, X, "x1b.png")
ok("S1 地基：B 传同一份（缺省 shared）200，仍 1 行（shared 语义等价复用）",
   s == 200 and rows_of(sx) == 1, "http=%s rows=%s" % (s, rows_of(sx)))
s, b, _ = upload(BT, X, "x1bpriv.png", vis="private")
ok("S1 B 传同一份但要 private：插独立行（旧版撞 sha 唯一索引 500）",
   s == 200 and rows_of(sx) == 2, "http=%s rows=%s %s"
   % (s, rows_of(sx), brief(b)))
s, _, _ = raw("GET", "/attachments/" + sx, AT)
ok("S1 读：A（有自己行）读得到", s == 200, "http=%s" % s)
s, _, _ = raw("GET", "/attachments/" + sx, BT)
ok("S1 读：B 读得到（private 是自己的行 + 存在 shared 行）", s == 200,
   "http=%s" % s)
s, _, _ = raw("GET", "/attachments/" + sx, CT)
ok("S1 读：C 读得到（shared 行放行，与旧版一致）", s == 200, "http=%s" % s)

# 关键聚合面：只有 A private + C shared 两行时，B 必须能读
Y = png()
s, _, sy = upload(AT, Y, "ypriv.png", vis="private")
ok("S1 前置：A 先传一份 private（库里唯一行是 private）",
   s == 200 and rows_of(sy) == 1, "http=%s" % s)
CLEAN_SHA.append(sy)
s, _, _ = upload(CT, Y, "yshared.png")
ok("S1 C 传同一份（缺省 shared）不再 500，库里两行",
   s == 200 and rows_of(sy) == 2, "http=%s rows=%s" % (s, rows_of(sy)))
s, _, _ = raw("GET", "/attachments/" + sy, BT)
ok("S1 读：B 读这份「A private + C shared」= 聚合判据放行（旧写法取到 "
   "private 行就 404，取到哪条全凭运气）", s == 200, "http=%s" % s)

# 纯私有行：他人 404、本人 200、staff 200
Z = png()
s, _, sz = upload(AT, Z, "zpriv.png", vis="private")
CLEAN_SHA.append(sz)
ok("S1 前置：A 传一份纯 private", s == 200 and rows_of(sz) == 1,
   "http=%s" % s)
s, _, _ = raw("GET", "/attachments/" + sz, BT)
ok("S1 纯 private 对他人 404（不泄存在性）", s == 404, "http=%s" % s)
s, _, _ = raw("GET", "/attachments/" + sz, CT)
ok("S1 纯 private 对其他成员同样 404", s == 404, "http=%s" % s)
s, _, _ = raw("GET", "/attachments/" + sz, AT)
ok("S1 正控：上传者本人读得到", s == 200, "http=%s" % s)
s, _, _ = raw("GET", "/attachments/" + sz, ROOT)
ok("S1 正控：staff 读得到", s == 200, "http=%s" % s)
s, _, _ = raw("GET", "/attachments/" + "0" * 64, AT)
ok("S1 反控：不存在的 sha 仍是 404（判据没被改成万能拒绝）", s == 404,
   "http=%s" % s)
old_idx = q("SELECT count(*) FROM pg_indexes WHERE indexname = "
            "'attachments_sha_key'")
new_idx = q("SELECT count(*) FROM pg_indexes WHERE indexname = "
            "'attachments_sha_user_key'")
ok("S1 索引形态：sha 唯一已摘、(sha,user) 唯一已建",
   old_idx == "0" and new_idx == "1", "old=%s new=%s" % (old_idx, new_idx))

print("== S2 配额分账（P-58）==")
setv("attach_quota_mib", "1")
setv("video_quota_mib", "64")
setv("forum_video_upload", "yes")
Q = png(900000)
s, b, sq = upload(AT, Q, "q1.png")
CLEAN_SHA.append(sq)
ok("S2 地基：1MiB 额度下 900KB 图可传", s == 200, "http=%s %s" % (s, brief(b)))
s, b, _ = upload(AT, png(900000), "q2.png")
ok("S2 地基：再来一张 900KB 图被图片配额拒（上限确实在管）",
   s == 400 and "配额" in b.decode("utf-8", "replace"), "http=%s %s"
   % (s, brief(b)))
V = mp4(500000)
s, b = multipart("/attachments/video", AT, V, "v1.mp4", "video/mp4")
sv = sha3(V)
CLEAN_SHA.append(sv)
ok("S2 地基：500KB 视频可传（走 video_quota_mib）", s == 200,
   "http=%s %s" % (s, brief(b)))
s, b, sq3 = upload(AT, png(100000), "q3.png")
CLEAN_SHA.append(sq3)
ok("S2 图片配额不该计入视频字节（旧版：900KB 图 + 500KB 视频 = 已用 1MiB，"
   "再传 100KB 图被拒）", s == 200, "http=%s %s" % (s, brief(b)))
V2 = mp4(60000)
s, b = multipart("/attachments/video", AT, V2, "v2.mp4", "video/mp4")
CLEAN_SHA.append(sha3(V2))
ok("S2 反控：图片额度没满不该挡住视频（两本账互不相犯）", s == 200,
   "http=%s %s" % (s, brief(b)))
setv("video_quota_mib", "0")
setv("attach_quota_mib", "512")

print("== S3 enableattach 是不是假开关（P-59）==")
kpriv = sz
setv("enableattach", "no")
s, b, sn = upload(AT, png(), "off.png")
CLEAN_SHA.append(sn)
ok("S3 开关置 no 后上传被拒（旧版：零消费，照样 200）", s == 403,
   "http=%s %s" % (s, brief(b)))
VX = mp4()
s, b = multipart("/attachments/video", AT, VX, "off.mp4", "video/mp4")
ok("S3 视频上传口同判（两个上传入口一套开关）", s == 403, "http=%s %s"
   % (s, brief(b)))
s, _, _ = raw("GET", "/attachments/" + kpriv, AT)
ok("S3 正控：关的是上传不是读——已上传附件仍读得到（否则老帖的图全 404）",
   s == 200, "http=%s" % s)
setv("enableattach", "yes")
s, b, so = upload(AT, png(), "on.png")
CLEAN_SHA.append(so)
ok("S3 反控：还原 yes 后立刻能传（开关不是永关）", s == 200,
   "http=%s %s" % (s, brief(b)))

print("== S4 HEAD / Range（P-60）==")
blob = png(4096)
s, b, sid = upload(AT, blob, "head.png")
CLEAN_SHA.append(sid)
size = len(blob)
s, hdr, body = raw("HEAD", "/attachments/" + sid, AT)
cl = hdr.get("Content-Length") or hdr.get("content-length")
ok("S4 HEAD 回真实 Content-Length（摘掉等长占位 body 后这条就是守卫）",
   s == 200 and str(size) == str(cl), "http=%s CL=%s size=%s"
   % (s, cl, size))
ok("S4 HEAD 不返回 body", not body, "len=%s" % len(body or b""))
s, hdr, body = raw("GET", "/attachments/" + sid, AT,
                   {"Range": "bytes=5-9"})
cr = hdr.get("Content-Range") or hdr.get("content-range")
ok("S4 Range 命中 → 206 + 5 字节 + 正确 Content-Range",
   s == 206 and len(body) == 5 and cr == "bytes 5-9/%s" % size,
   "http=%s len=%s CR=%s" % (s, len(body), cr))
s, hdr, body = raw("GET", "/attachments/" + sid, AT,
                   {"Range": "bytes=999999999-"})
ok("S4 越界区间按 RFC 允许回落 200 全量（记录口径：这不是缺陷，别重复立案）",
   s == 200 and len(body) == size, "http=%s len=%s" % (s, len(body)))
s, hdr, body = raw("GET", "/attachments/" + sid, AT, {"Range": "bytes=-20"})
ok("S4 后缀区间同样回落全量（parse_range 的既有口径）",
   s == 200 and len(body) == size, "http=%s len=%s" % (s, len(body)))

print("== S5 读取限流的额度是不是注释里那个数（P-61）==")


def clear_att_bucket(uid):
    subprocess.run(
        ["docker", "exec", "flux-redis", "sh", "-c",
         'redis-cli -a "$REDIS_PASSWORD" --no-auth-warning del '
         '"rl:att:%s"' % uid],
        capture_output=True, text=True, encoding="utf-8")


# 一頁几十张缩略图是常态：连读 8 张必须全 200（旧版共用登录档 5/分，
# 第 6 张起 429 = 页面裂图）
clear_att_bucket(AID)
codes = []
for i in range(8):
    s, _, _ = raw("GET", "/attachments/" + sid, AT)
    codes.append(s)
ok("S5 同一用户连读 8 次附件全 200（旧版第 6 次起 429）",
   all(c == 200 for c in codes), "http=%s" % codes)
# 反控：限流仍然存在，只是额度是注释里的 60 而不是 5
codes2 = []
for i in range(56):
    s, _, _ = raw("GET", "/attachments/" + sid, AT)
    codes2.append(s)
tail = codes2[50:]
ok("S5 反控：60 次/分之上仍然 429（修的是额度不是关掉限流）",
   codes2.count(200) == 60 - 8 and 429 in codes2,
   "200 计数=%s 尾=%s" % (sum(1 for c in codes2 if c == 200), tail))
clear_att_bucket(AID)

cleanup()
summary()
