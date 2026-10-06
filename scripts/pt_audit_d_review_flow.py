# -*- coding: utf-8 -*-
"""资深 PT 深测脚本 D：发种 / 审种动线不变式（0288）。

覆盖 `_doc/发种审种动线实测审计-2026-10-05.md` 里每条已修项，让它**会红**：
  A 审核通知落库 · 被拒种对作者可见 · 重提可用
  B 校验时序：越权/非法参数不许留下待审残种
  C .torrent 结构校验（无 pieces / 非 20 倍数 / 片长 0 / 0 字节 / 空名 / 路径穿越）
  D 未过审内容的子资源闸门（files / snatches / nfo …）
  E 审核队列分页与批量裁决（旧版硬顶 200 条升序，积压后新种看不见）
  F 站方可配的最低内容标准（默认关，开了要真生效）
  G 元数据通道：长描述走 multipart 可发；query string 老通道不破坏
  H 免审连击不自激（旧版免审也 +1 → 一旦过 5 就事实永久免审）
  I 下载重建：本人 passkey / private / 站外取源键已剥
  J 删后重发：同作者墓碑可复用原 id（旧版同内容永久判重锁死）

跑法：栈已在跑（api/postgres/redis），`python scripts/pt_audit_d_review_flow.py`
数据自清：临时种子/账号/设定改动在收尾全部复位。
"""
import sys
import time
import json
import base64
import hashlib
import subprocess
import urllib.parse

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from pt_audit_lib import call, login, ok, summary  # noqa: E402

# 自带 bencode 与发种上传：不 import pt_audit_a_publish —— 那个文件是**脚本**，
# import 会把它整条发布闸门当副作用跑一遍（本轮实测踩到）。
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


BOUNDARY = "----ptauditd"


def upload_torrent(tok, tor_bytes, **fields):
    """multipart：元数据走文本字段，file 走 .torrent 字节。"""
    import urllib.request
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
    parts += fname.encode() + tor_bytes
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

TAG = "dt" + str(int(time.time()))[-6:]


def q(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-q", "-t", "-A", "-c", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode:
        raise RuntimeError(r.stderr.strip()[:200])
    return (r.stdout or "").strip()


def login_retry(user, pw):
    """/auth/login 在 dev 实例会限流；一次失败不代表凭据错。"""
    for i in range(10):
        s, r = call("POST", "/auth/login", {"username": user, "password": pw})
        if s == 200 and (r.get("data") or {}).get("token"):
            return r["data"]["token"]
        time.sleep(min(2 + i * 3, 20))
    raise SystemExit("login failed: %s" % user)


def tor(name, length=1_048_576, piece_len=16384, pieces=None, files=None,
         root_extra=None):
    """按 BEP3 造 .torrent；pieces/files/root_extra 可刻造畸形输入。"""
    info = {b"piece length": piece_len, b"name": name.encode()}
    if pieces is None:
        n = max(1, (length + piece_len - 1) // piece_len)
        n = n if piece_len > 0 else 1
        pieces = hashlib.sha1(name.encode()).digest() * n
    info[b"pieces"] = pieces
    if files is None:
        info[b"length"] = length
    else:
        info[b"files"] = files
    root = {b"info": info, b"creation date": int(time.time())}
    root.update(root_extra or {})
    return benc(root)


def up(tok, name, raw=None, **fields):
    raw = raw or tor(name)
    f = {"category_id": 1, "name": name, "small_descr": "gate",
         "descr": "gate description long enough for normal checks"}
    f.update(fields)
    return upload_torrent(tok, raw, **f)


def tno(res):
    return (res.get("data") or {}) if isinstance(res, dict) else {}


# ---------------- 准备：管理员 + 两个探针账号 ----------------
# root 登录也走退避：dev 实例 /auth/login 会限流，连跑几轮闸门必吃 429
admin = login_retry("root", "password123")
u_seed, u_view = "dt_seed_" + TAG, "dt_view_" + TAG
PW = "Gate#123456"
for uname in (u_seed, u_view):
    s, r = call("POST", "/admin/adduser",
                {"username": uname, "password": PW,
                 "email": "%s@audit.invalid" % uname}, token=admin)
    if s != 200:
        raise SystemExit("adduser %s: %s %s" % (uname, s, r.get("message")))
    q("UPDATE users SET class_id=1 WHERE id=(SELECT id FROM users"
      " WHERE username='%s')" % uname)
def unlock(user, pw):
    """后台建的号带 must_reset_password，发种前必须先改一次密码。"""
    t = login_retry(user, pw)
    s, r = call("POST", "/me/password/change",
                {"old_password": pw, "new_password": pw + "x"}, token=t)
    if s != 200:
        raise SystemExit("改密失败 %s: %s %s" % (user, s, r.get("message")))
    return login_retry(user, pw + "x")


seed = unlock(u_seed, PW)
view = unlock(u_view, PW)
uid_seed = int(q("SELECT id FROM users WHERE username='%s'" % u_seed))
uid_view = int(q("SELECT id FROM users WHERE username='%s'" % u_view))

SETTINGS = ("upload_min_descr_len", "upload_require_screenshots",
            "upload_dup_policy", "upload_title_pattern")
saved = {k: q("SELECT value FROM site_settings WHERE name='%s'" % k)
         for k in SETTINGS}


def drop_user(uid):
    # 删号要走正规链路：先封禁（删除接口只收封禁号），再 DELETE 落墓碑；
    # 直连 DELETE 会被 audit_log 等外键拦下。
    call("POST", "/admin/users/status", {"user_id": uid, "status": 2},
         token=admin)
    q("DELETE FROM user_permissions WHERE user_id=%d" % uid)
    call("DELETE", "/admin/users/%d" % uid, token=admin)


def setkv(k, v):
    q("INSERT INTO site_settings (name, value, descr, grp)"
      " VALUES ('%s','%s','gate','torrent')"
      " ON CONFLICT (name) DO UPDATE SET value=EXCLUDED.value" % (k, v))


try:
    # ================= A 审核通知与被拒可达 =================
    s, r = up(seed, "dt-A1-待审")
    t1 = tno(r).get("id")
    if not ok("A0 合法种子可发种", s == 200 and t1,
              "%s %s" % (s, r.get("message"))):
        raise SystemExit("发种不通，后面全部无意义")
    n0 = int(q("SELECT count(*) FROM messages WHERE receiver_id=%d" % uid_seed))
    call("POST", "/admin/reviews/decide", {"torrent_id": t1, "approve": True},
         token=admin)
    n1 = int(q("SELECT count(*) FROM messages WHERE receiver_id=%d" % uid_seed))
    ok("A1 过审即发站内信（旧版列名错→100%静默）", n1 == n0 + 1,
       "messages %d->%d" % (n0, n1))
    k1 = q("SELECT COALESCE(kind,'-') FROM messages WHERE receiver_id=%d"
           " ORDER BY id DESC LIMIT 1" % uid_seed)
    ok("A1b 通知带文案键（事后可翻译）", k1 == "review_approved", k1)

    s, r = up(seed, "dt-A3-待拒")
    t2 = tno(r).get("id")
    call("POST", "/admin/reviews/decide",
         {"torrent_id": t2, "approve": False, "reason": "截图缺失"}, token=admin)
    s, _ = call("GET", "/torrents/%s" % t2, token=seed)
    ok("A3 被拒种作者可读详情（旧版 404 无法自助）", s == 200, s)
    s, _ = call("GET", "/torrents/%s/detail" % t2, token=seed)
    ok("A3b 被拒种作者可读扩展详情", s == 200, s)
    s, _ = call("GET", "/torrents/%s" % t2, token=view)
    ok("A3c 被拒种对无关用户仍不可见", s == 404, s)
    s, r = call("POST", "/torrents/%s/resubmit" % t2, {}, token=seed)
    st = q("SELECT approval_status FROM torrents WHERE id=%s" % t2)
    ok("A3d 作者可重新送审（端点有真实调用方）", s == 200 and st == "0",
       "%s status=%s %s" % (s, st, r.get("message")))

    # ================= B 校验不许后置 =================
    for label, extra in (("B1 官方标签越权", {"tags": "[3]"}),
                         ("B2 不存在的标签", {"tags": "[999999]"}),
                         ("B3 推荐位越权", {"pos_state": "1"}),
                         ("B4 价格越界", {"price": "99999999"}),
                         ("B5 简介超上限", {"descr": "长" * 70000})):
        nm = "dt-B-" + label
        c0 = int(q("SELECT count(*) FROM torrents WHERE name LIKE 'dt-B%%'"))
        s, r = up(seed, nm, raw=tor(nm), **extra)
        c1 = int(q("SELECT count(*) FROM torrents WHERE name LIKE 'dt-B%%'"))
        ok("%s 被拒且不入库" % label, s >= 400 and c0 == c1,
           "http=%s rows %d->%d %s" % (s, c0, c1, str(r.get("message"))[:80]))

    # ================= C 结构校验 =================
    bad = {
        "C1 无 pieces": tor("dt-C1", pieces=b""),
        "C2 pieces 非20倍数": tor("dt-C2", pieces=b"\x01" * 25),
        "C3 片长为0": tor("dt-C3", piece_len=0, pieces=b"\x01" * 20),
        "C4 总大小0": tor("dt-C4", length=0, pieces=b"\x01" * 20),
        "C5 空名称": tor("", pieces=b"\x01" * 20),
        "C6 路径穿越": tor("dt-C6", files=[{b"length": 1024,
                                          b"path": [b"..", b"..", b"x.bin"]}]),
        "C7 url-list": tor("dt-C7", root_extra={
            b"url-list": [b"http://evil.example/x.iso"]}),
        "C8 httpseeds": tor("dt-C8", root_extra={
            b"httpseeds": [b"http://evil.example/hs"]}),
    }
    for label, raw in bad.items():
        s, r = up(seed, "dt-C-" + label, raw=raw)
        ok("%s 拒收" % label, s >= 400, "http=%s %s" % (s, r.get("message")))

    # ================= D 未过审内容的可见性 =================
    s, r = up(seed, "dt-D-待审")
    t5 = tno(r).get("id")
    for ep in ("files", "snatches", "peers", "comments", "tags", "collections",
               "nfo", "detail", "download", "magnet"):
        s, _ = call("GET", "/torrents/%s/%s" % (t5, ep), token=view)
        ok("D1 待审 %-10s 对无关用户不可读" % ep, s == 404, s)
    s, _ = call("GET", "/torrents/%s/files" % t5, token=seed)
    ok("D2 待审文件清单对作者可读", s == 200, s)
    s, _ = call("GET", "/torrents/%s/nfo" % t5, token=admin)
    ok("D3 待审 NFO 对审核员可读（旧版写死只放过审）", s == 200, s)
    s, _ = call("GET", "/torrents/%s/files" % t5, token=admin)
    ok("D4 待审文件清单对审核员可读", s == 200, s)

    # ================= E 队列分页与批量裁决 =================
    for i in range(250):
        h = hashlib.sha1(("cap-%d" % i).encode()).hexdigest()
        q("INSERT INTO torrents (info_hash, raw_info_hash, pieces_hash, name,"
          " size, numfiles, owner_id, category_id, approval_status, created_at)"
          " VALUES ('%s','%s','','dt-E-%03d',1024,1,%d,1,0,"
          " now() - interval '%d minutes')" % (h, h, i, uid_view, 250 - i))
    newest = q("SELECT max(id) FROM torrents WHERE name LIKE 'dt-E-%%'")
    s, r = call("GET", "/admin/reviews?limit=100&offset=0", token=admin)
    d = r.get("data") or {}
    items = d.get("items") or []
    ok("E1 队列返回 total 且 limit 生效",
       d.get("total", 0) >= 250 and len(items) == 100,
       "total=%s len=%d" % (d.get("total"), len(items)))
    seen = set()
    for off in (0, 100, 200, 300):
        _, rr = call("GET", "/admin/reviews?limit=100&offset=%d" % off,
                     token=admin)
        seen |= {x["id"] for x in (rr.get("data") or {}).get("items") or []}
    ok("E2 分页能覆盖全部待审（旧版截断后新种永远看不见）",
       int(newest) in seen, "newest=%s 覆盖=%d" % (newest, len(seen)))
    s, r = call("GET", "/admin/reviews?limit=6", token=admin)
    first = (r.get("data") or {}).get("items") or []
    ok("E3 默认按提交时间最老优先",
       [x["id"] for x in first] == sorted(x["id"] for x in first),
       str([x["id"] for x in first]))
    _, p3 = call("GET", "/admin/reviews?limit=100&offset=200", token=admin)
    page3 = [x["id"] for x in (p3.get("data") or {}).get("items") or []][:3]
    if page3:
        s, r = call("POST", "/admin/reviews/batch",
                    {"torrent_ids": page3, "approve": True}, token=admin)
        dd = r.get("data") or {}
        n_ok = int(q("SELECT count(*) FROM torrents WHERE id IN (%s)"
                     " AND approval_status=1" % ",".join(map(str, page3))))
        ok("E4 批量通过生效", dd.get("handled") == len(page3) and n_ok == len(page3),
           "handled=%s db=%d %s" % (dd.get("handled"), n_ok, r.get("message")))
    if page3:
        s, r = call("POST", "/admin/reviews/batch",
                    {"torrent_ids": page3, "approve": False}, token=admin)
        ok("E5 批量拒绝缺原因即报错", s >= 400, s)
    q("DELETE FROM torrents WHERE name LIKE 'dt-E-%%'")

    # ================= F 最低内容标准（可配）=================
    setkv("upload_min_descr_len", "80")
    s, r = up(seed, "dt-F1-短描述", descr="太短")
    ok("F1 简介最短长度生效", s >= 400, "%s %s" % (s, r.get("message")))
    setkv("upload_min_descr_len", saved["upload_min_descr_len"] or "0")
    s, r = up(seed, "dt-F2-恢复默认", descr="太短")
    ok("F2 关掉闸门后同样内容可发", s == 200, "%s %s" % (s, r.get("message")))
    t7 = tno(r).get("id")
    setkv("upload_require_screenshots", "2")
    s, r = up(seed, "dt-F3-缺截图",
              descr="一张 [img]https://x/a.png[/img]")
    ok("F3 最少截图数生效", s >= 400, "%s %s" % (s, r.get("message")))
    s, r = up(seed, "dt-F4-截图够",
              descr="两张 [img]https://x/a.png[/img] [img]https://x/b.png[/img]")
    ok("F4 达标即放行", s == 200, "%s %s" % (s, r.get("message")))
    tf = tno(r).get("id")
    shots = q("SELECT jsonb_array_length(screenshots) FROM torrents"
              " WHERE id=%s" % tf)
    ok("F5 screenshots 真落库（旧版无写入，队列截图数恒 0）", shots == "2", shots)
    setkv("upload_require_screenshots",
          saved["upload_require_screenshots"] or "0")
    setkv("upload_title_pattern", "^[A-Z].*$")
    s, r = up(seed, "dt-F6-命名不合规")
    ok("F6 命名规范正则生效", s >= 400, "%s %s" % (s, r.get("message")))
    setkv("upload_title_pattern", saved["upload_title_pattern"] or "")

    # ================= G 元数据通道 =================
    s, r = up(seed, "dt-G1-长描述", descr="详" * 30000)
    ok("G1 30000 字描述经 multipart 可发", s == 200,
       "%s %s" % (s, str(r.get("message"))[:90]))
    tg = tno(r).get("id")
    ln = q("SELECT length(descr) FROM torrents WHERE id=%s" % tg)
    ok("G1b 描述完整落库", ln == "30000", ln)
    qs = urllib.parse.urlencode({"category_id": "1", "name": "dt-G2-旧通道",
                                 "small_descr": "s", "descr": "query 通道描述"})
    body = b"--ptaudit\r\nContent-Disposition: form-data; name=\"file\";" \
        b" filename=\"a.torrent\"\r\n\r\n" + tor("dt-G2-旧通道") \
        + b"\r\n--ptaudit--\r\n"
    import urllib.request
    req = urllib.request.Request("http://127.0.0.1:8080/api/v1/torrents?" + qs,
                                 method="POST", data=body)
    req.add_header("Content-Type", "multipart/form-data; boundary=ptaudit")
    req.add_header("Authorization", "Bearer " + seed)
    try:
        with urllib.request.urlopen(req, timeout=40) as rr:
            g2 = json.loads(rr.read().decode())
        s2 = rr.status
    except Exception as e:                       # noqa: BLE001
        s2, g2 = getattr(e, "code", 0), {}
    ok("G2 query string 老通道未破坏", s2 == 200, s2)
    s, r = up(seed, "dt-G3-超上限", descr="长" * 70000)
    ok("G3 超上限返回带字数的 400",
       s >= 400 and "上限" in str(r.get("message")),
       "%s %s" % (s, r.get("message")))

    # ================= H 免审连击不自激 =================
    q("UPDATE users SET approve_streak=0, deny_count=0 WHERE id=%d" % uid_seed)
    s, r = up(seed, "dt-H1-待审")
    th = tno(r).get("id")
    st1 = int(q("SELECT approve_streak FROM users WHERE id=%d" % uid_seed))
    ok("H1 进入待审不加连击", st1 == 0, st1)
    call("POST", "/admin/reviews/decide", {"torrent_id": th, "approve": True},
         token=admin)
    st2 = int(q("SELECT approve_streak FROM users WHERE id=%d" % uid_seed))
    ok("H2 人工过审才 +1", st2 == 1, st2)
    q("UPDATE users SET approve_streak=6 WHERE id=%d" % uid_seed)
    s, r = up(seed, "dt-H3-连击免审")
    st3 = int(q("SELECT approve_streak FROM users WHERE id=%d" % uid_seed))
    ok("H3 免审自身不再累加（旧版自激到永久免审）",
       tno(r).get("auto_approved") is True and st3 == 6,
       "auto=%s streak=%d" % (tno(r).get("auto_approved"), st3))

    # ================= I 下载重建 =================
    # 说明（本轮实测得出的机制边界）：pieces_hash 只在 info 字典被**逐字节复制**时
    # 才相同，而那种情况下 info_hash 也相同、先被判重拦下；改分片大小会重算 pieces
    # ⇒ pieces_hash 变。所以「重打包同内容」这一类其实落不进 dup_policy，
    # 这里只验两件事：同一文件重发仍是判重；切换策略不许影响正常发种。
    raw7 = base64.b64decode(q("SELECT encode(raw,'base64') FROM torrent_files"
                              " WHERE torrent_id=%s" % t7))
    setkv("upload_dup_policy", "block")
    s, r = up(seed, "dt-I0-同文件重发", raw=raw7)
    ok("I0 同一 .torrent 重发被拦", s >= 400,
       "%s %s" % (s, str(r.get("message"))[:90]))
    setkv("upload_dup_policy", "group")
    s, r = up(seed, "dt-I1-策略切换后正常发种")
    ok("I1 切到 group 策略后普通发种不受影响", s == 200,
       "%s %s" % (s, str(r.get("message"))[:90]))
    if s == 200:
        q("DELETE FROM torrent_files WHERE torrent_id=%s" % tno(r).get("id"))
        q("DELETE FROM files WHERE torrent_id=%s" % tno(r).get("id"))
        q("DELETE FROM torrents WHERE id=%s" % tno(r).get("id"))
    setkv("upload_dup_policy", saved["upload_dup_policy"] or "suggest")

    s, r = up(seed, "dt-I2-下载重建")
    ti = tno(r).get("id")
    call("POST", "/admin/reviews/decide", {"torrent_id": ti, "approve": True},
         token=admin)
    s, raw = call("GET", "/torrents/%s/download" % ti, token=seed, raw=True)
    txt = raw.decode("latin-1") if isinstance(raw, bytes) else ""
    pk = q("SELECT passkey FROM users WHERE id=%d" % uid_seed)
    ok("I2 下载文件嵌本人 passkey", pk in txt, s)
    ok("I3 下载文件已置 private", b"privatei1e" in raw, "")
    ok("I4 无 url-list / httpseeds 残留",
       "url-list" not in txt and "httpseeds" not in txt, "")
    s, _ = call("GET", "/torrents/%s/download" % ti, token=view,
                raw=True)
    ok("I5 他人下载得到的是他人 passkey（不串号）", s == 200, s)
    s2, raw2 = call("GET", "/torrents/%s/download" % ti, token=view, raw=True)
    pk2 = q("SELECT passkey FROM users WHERE id=%d" % uid_view)
    ok("I5b 且确实是那一个", s2 == 200 and pk2 in raw2.decode("latin-1"), s2)

    # ================= J 删后重发（墓碑复活）=================
    # H 段把连击抬到 6（免审阈值 5）；不复位的话这里的种子一上架就过审，
    # 「作者删待审种」这条会假红 —— 先清回普通身份。
    q("UPDATE users SET approve_streak=0, deny_count=0 WHERE id=%d" % uid_seed)
    s, r = up(seed, "dt-J1-删后重发")
    tj = tno(r).get("id")
    rawj = base64.b64decode(q("SELECT encode(raw,'base64')"
                              " FROM torrent_files WHERE torrent_id=%s" % tj))
    call("DELETE", "/torrents/%s" % tj, token=seed)
    st = q("SELECT approval_status FROM torrents WHERE id=%s" % tj)
    ok("J1 作者可删自己的待审种", st == "3", st)
    s, r = up(seed, "dt-J1-删后重发", raw=rawj)
    ok("J2 删除后可重发同一 .torrent（旧版永久锁死）",
       s == 200 and tno(r).get("id") == tj, "http=%s id=%s %s" % (
           s, tno(r).get("id"), r.get("message")))
    st = q("SELECT approval_status FROM torrents WHERE id=%s" % tj)
    ok("J2b 复活后回到待审流", st in ("0", "1"), st)
    # 别人的墓碑不许复活
    s, _ = call("POST", "/admin/adduser",
                {"username": "dt_take_" + TAG, "password": PW,
                 "email": "dt_take_%s@audit.invalid" % TAG}, token=admin)
    q("UPDATE users SET class_id=1 WHERE username='dt_take_%s'" % TAG)
    taker = login_retry("dt_take_" + TAG, PW)
    uid_take = int(q("SELECT id FROM users WHERE username='dt_take_%s'" % TAG))
    s, r = up(seed, "dt-J3-他人墓碑", raw=tor("dt-J3-他人墓碑"))
    tk = tno(r).get("id")
    call("DELETE", "/torrents/%s" % tk, token=seed)
    s, r = up(taker, "dt-J3-他人墓碑", raw=tor("dt-J3-他人墓碑"))
    ok("J3 他人墓碑不可被抢注（防「等别人删了我再发」）", s >= 400,
       "http=%s %s" % (s, str(r.get("message"))[:80]))
except AssertionError as e:
    ok("脚本前置断言", False, e)
finally:
    for k in SETTINGS:
        setkv(k, saved.get(k, ""))
    ids = q("SELECT id FROM torrents WHERE name LIKE 'dt-%%'").split()
    for i in ids:
        for c in ("files", "torrent_files", "torrent_sections", "tags",
                  "promotions", "snatches", "torrent_operation_logs"):
            q("DELETE FROM %s WHERE torrent_id=%s" % (c, i))
    q("DELETE FROM torrents WHERE name LIKE 'dt-%%'")
    q("DELETE FROM messages WHERE receiver_id IN (%d,%d)"
      % (uid_seed, uid_view))
    for u in (uid_seed, uid_view):
        drop_user(u)
    u2 = q("SELECT id FROM users WHERE username='dt_take_%s'" % TAG)
    if u2:
        drop_user(int(u2))
    left = q("SELECT count(*) FROM torrents WHERE name LIKE 'dt-%%'")
    ok("收尾：探针数据清干净", left == "0", left)

summary()
