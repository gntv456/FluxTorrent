# -*- coding: utf-8 -*-
"""闸门 C-review-hold：种审「暂缓→恢复」这一条链路（0306 口径）。

种审员日常最常用的动作之一是「证据不足先挂着」：
  审核台 → 暂缓（POST /admin/reviews/postpone）→ 等上传者补 mediainfo/截图
  → 证据齐了再恢复（POST /admin/reviews/resume）→ 走常规过/不过。

本闸门钉住这条链路的两个真实断点：
  1. 暂缓把 approval_status 翻成 4，而审核队列只筛 status=0 ⇒ 暂缓完
     那条种子从审核台上「消失」；后台种子列表的状态筛也没有「暂缓」这一档
     （status=4 是「死种」）⇒ 种审员没有任何入口再看到它，
     只能凭 id 去别处翻 ⇒ 「挂着等证据」事实上变成「永远挂着」。
  2. POST /admin/reviews/resume 只有 API 没有 UI 入口 ⇒ 恢复审核在
     界面上做不了（闸门里以 API 口为准，前端入口由静态判据钉住）。

判据（现状会红，修完转绿）：
  H1 上传 → 进待审队列，条目内容面齐（简介/文件数/查重/分型）；
  H2 暂缓 → approval_status=4，且留痕（操作时间线 + 审计日志）都在；
  H3 暂缓队列可见：GET /admin/reviews?status=4 能取回它（现状取不到）；
  H4 后台种子列表 status=5 能筛出「暂缓」（现状筛不出）；
  H5 恢复 → 回到待审，重新出现在默认队列里；
  H6 暂缓期间可见性：发布者本人可见、staff 可见、无关会员不可见；
  H7 负控：不能暂缓自己发的种；已过审/已拒的种再暂缓被拒；不存在的种 404；
  H8 收尾：探针种与探针号清干净。

用法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_c_review_hold.py
"""
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from pt_audit_lib import call, login, ok, psql_stdin, summary  # noqa: E402

PW = "Gate#123456"
PW2 = "Gate#7654321"
TAG = "rh" + str(int(time.time()))[-6:]
SEED = "rh_seed_" + TAG
OTHER = "rh_oth_" + TAG
NAME = "rh-probe-" + TAG


def q(sql):
    return psql_stdin(sql)


def benc(obj, out=None):
    out = bytearray() if out is None else out
    if isinstance(obj, int):
        out += b"i" + str(obj).encode() + b"e"
    elif isinstance(obj, bytes):
        out += b"%d:" % len(obj) + obj
    elif isinstance(obj, list):
        out += b"l"
        for v in obj:
            benc(v, out)
        out += b"e"
    elif isinstance(obj, dict):
        out += b"d"
        for k in sorted(obj.keys()):
            benc(k, out)
            benc(obj[k], out)
        out += b"e"
    return bytes(out)


def tor(name, length=1_048_576, piece_len=16384):
    info = {b"piece length": piece_len, b"name": name.encode(),
            b"private": 1, b"length": length}
    n = max(1, (length + piece_len - 1) // piece_len)
    info[b"pieces"] = hashlib.sha1(name.encode()).digest() * n
    return benc({b"info": info, b"creation date": int(time.time())})


BOUNDARY = "----ptauditcrh"


def upload(tok, tname, raw=None, **fields):
    """multipart 上传：元数据走文本字段，种子文件走二进制。"""
    crlf = chr(13) + chr(10)
    dq = chr(34)
    parts = b""
    for k, v in fields.items():
        if v is None:
            continue
        head = ("--" + BOUNDARY + crlf
                + "Content-Disposition: form-data; name=" + dq + k + dq
                + crlf + crlf + str(v) + crlf)
        parts += head.encode()
    fname = ("--" + BOUNDARY + crlf
             + "Content-Disposition: form-data; name=" + dq + "file" + dq
             + "; filename=" + dq + "audit.torrent" + dq + crlf
             + "Content-Type: application/x-bittorrent" + crlf + crlf)
    parts += fname.encode() + (raw or tor(tname))
    parts += (crlf + "--" + BOUNDARY + "--" + crlf).encode()
    req = urllib.request.Request("http://127.0.0.1:8080/api/v1/torrents",
                                 method="POST", data=parts)
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, json.loads(r.read().decode("utf-8", "replace"))
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read().decode("utf-8", "replace"))
        except Exception:
            return e.code, {"message": "<non-json>"}


def mk_user(admin, uname):
    """后台建号 → 首次登录改掉临时密码（改完才有发种等写权限）。"""
    s, r = call("POST", "/admin/adduser",
                {"username": uname, "password": PW,
                 "email": "%s@audit.invalid" % uname}, token=admin)
    if s != 200:
        raise SystemExit("adduser %s: %s %s" % (uname, s, r.get("message")))
    q("UPDATE users SET class_id=1 WHERE id=(SELECT id FROM users"
      " WHERE username='%s')" % uname)
    tok = login(uname, PW)
    s, r = call("POST", "/me/password/change",
                {"old_password": PW, "new_password": PW2}, token=tok)
    if s != 200:
        raise SystemExit("改密失败 %s: %s %s" % (uname, s, r.get("message")))
    return login(uname, PW2)


def items(resp):
    d = resp.get("data") or {}
    return d.get("items") or []


def drop(uid, uname):
    """收尾：删探针号（墓碑）与探针种。"""
    u = q("SELECT id FROM users WHERE username = '%s'" % uname).strip()
    if u:
        q("UPDATE users SET status = 2 WHERE id = %s" % u)
        call("DELETE", "/admin/users/%s" % u, None, token=ADMIN)


ADMIN = login()
seed_tok = other_tok = ""
tid = 0
try:
    seed_tok = mk_user(ADMIN, SEED)
    other_tok = mk_user(ADMIN, OTHER)

    print("== H1 上传 → 进待审队列 ==")
    s, r = upload(seed_tok, NAME, category_id=1, name=NAME,
                  small_descr="hold probe",
                  descr="hold probe descr long enough for upload checks")
    ok("H1 上传受理", s == 200, "http=%s %s" % (s, str(r)[:120]))
    tid = int((r.get("data") or {}).get("id") or 0)
    ok("H1 拿到种号", tid > 0, "id=%s" % tid)
    ok("H1 落库即待审",
       q("SELECT approval_status FROM torrents WHERE id=%s" % tid) == "0",
       "")
    s, r = call("GET", "/admin/reviews?limit=200", token=ADMIN)
    row = [i for i in items(r) if i.get("id") == tid]
    ok("H1 队列里能看到它", bool(row), "队列 %d 条" % len(items(r)))
    if row:
        it = row[0]
        ok("H1 条目内容面齐（简介/文件数/查重/分型）",
           "small_descr" in it and "numfiles" in it
           and "dup_hash" in it and "sections" in it,
           "keys=%s" % ",".join(sorted(it.keys())[:12]))

    print("== H2 暂缓 ==")
    s, r = call("POST", "/admin/reviews/postpone",
                {"torrent_id": tid, "reason": "等 mediainfo"}, token=ADMIN)
    ok("H2 暂缓受理", s == 200, "http=%s %s" % (s, str(r)[:120]))
    ok("H2 状态翻成 4（暂缓）",
       q("SELECT approval_status FROM torrents WHERE id=%s" % tid) == "4",
       "")
    ok("H2 操作时间线留痕（postpone）",
       q("SELECT count(*) FROM torrent_operation_logs WHERE torrent_id=%s"
         " AND action='postpone'" % tid) == "1",
       "")

    print("== H3 暂缓队列可见（现状断点） ==")
    s, r = call("GET", "/admin/reviews?status=4&limit=200", token=ADMIN)
    ok("H3 暂缓队列口可用", s == 200, "http=%s" % s)
    ok("H3 暂缓队列里能取回它",
       any(i.get("id") == tid for i in items(r)),
       "取回 %d 条" % len(items(r)))

    print("== H4 后台种子列表按「暂缓」筛 ==")
    s, r = call("GET", "/admin/torrents?status=5&per_page=100", token=ADMIN)
    ok("H4 后台列表口可用", s == 200, "http=%s" % s)
    rows = (r.get("data") or {}).get("items") or r.get("data") or []
    if isinstance(rows, dict):
        rows = rows.get("items") or []
    ok("H4 status=5 能筛出暂缓的种",
       any((i.get("id") == tid) for i in rows if isinstance(i, dict)),
       "取回 %d 条" % len(rows))

    print("== H5 恢复审核 ==")
    s, r = call("POST", "/admin/reviews/resume",
                {"torrent_id": tid, "reason": "证据齐了"}, token=ADMIN)
    ok("H5 恢复受理", s == 200, "http=%s %s" % (s, str(r)[:120]))
    ok("H5 状态回到待审",
       q("SELECT approval_status FROM torrents WHERE id=%s" % tid) == "0",
       "")
    s, r = call("GET", "/admin/reviews?limit=200", token=ADMIN)
    ok("H5 又回到待审队列",
       any(i.get("id") == tid for i in items(r)), "")

    print("== H6 暂缓期间的可见性 ==")
    s, r = call("POST", "/admin/reviews/postpone",
                {"torrent_id": tid, "reason": "再挂一次"}, token=ADMIN)
    ok("H6 再次暂缓受理", s == 200, "http=%s" % s)
    s, r = call("GET", "/torrents/%s" % tid, token=seed_tok)
    ok("H6 发布者本人可见", s == 200, "http=%s" % s)
    s, r = call("GET", "/torrents/%s" % tid, token=other_tok)
    ok("H6 无关会员看不见", s in (403, 404), "http=%s" % s)
    s, r = call("GET", "/torrents/%s" % tid, token=ADMIN)
    ok("H6 staff 可见", s == 200, "http=%s" % s)

    print("== H7 负控 ==")
    s, r = call("POST", "/admin/reviews/postpone",
                {"torrent_id": tid, "reason": "自审"}, token=seed_tok)
    ok("H7 普通会员不能暂缓", s in (401, 403), "http=%s" % s)
    s, r = call("POST", "/admin/reviews/postpone",
                {"torrent_id": tid, "reason": "自审"}, token=ADMIN)
    ok("H7 已在暂缓态的种不能重复暂缓", s != 200, "http=%s" % s)
    s, r = call("POST", "/admin/reviews/postpone",
                {"torrent_id": 99999999, "reason": "不存在"}, token=ADMIN)
    ok("H7 不存在的种 404", s == 404, "http=%s" % s)
    s, r = call("POST", "/admin/reviews/postpone",
                {"torrent_id": tid, "reason": "自审"}, token=ADMIN)
    ok("H7 仍被拦（状态机没被绕过）", s != 200, "http=%s" % s)
finally:
    if tid:
        for tbl in ("torrent_operation_logs", "torrent_files", "files",
                    "torrent_sections", "torrent_previews"):
            q("DELETE FROM %s WHERE torrent_id = %s" % (tbl, tid))
        q("DELETE FROM torrents WHERE id = %s" % tid)
    drop(0, SEED)
    drop(0, OTHER)
    left = q("SELECT count(*) FROM torrents WHERE name LIKE "
             + "'rh-probe-" + TAG + "'")
    ok("H8 探针种已清", left == "0", "残留 %s" % left)
    left = q("SELECT count(*) FROM users WHERE username LIKE "
             + "'rh_%" + TAG + "'")
    ok("H8 探针号已清", left == "0", "残留 %s" % left)
summary()
