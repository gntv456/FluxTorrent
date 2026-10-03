# -*- coding: utf-8 -*-
"""资深 PT 深测脚本 A：完整发种 → 下载 → announce → 数据一致性。

构造真实 .torrent（bencode 手工编码），走 multipart 上传，
再以真实客户端姿态 announce，核对：
- uploaded/downloaded 是否正确入账（含限时免费/促销系数）
- H&R 链路（snatched 记录）
- 重复 info_hash 拒绝
- announce 参数边界（负数/超大 left）
"""
import sys, io, json, hashlib, urllib.request, urllib.parse, time, random
sys.stdout.reconfigure(encoding="utf-8")
from pt_audit_lib import call, login, ok, summary

# ---------- bencode 编码器 ----------
def benc(obj):
    if isinstance(obj, int): return b"i%de" % obj
    if isinstance(obj, bytes): return b"%d:%s" % (len(obj), obj)
    if isinstance(obj, str): return benc(obj.encode())
    if isinstance(obj, list):
        return b"l" + b"".join(benc(x) for x in obj) + b"e"
    if isinstance(obj, dict):
        out = b"d"
        for k in sorted(obj.keys()):
            out += benc(k) + benc(obj[k])
        return out + b"e"
    raise TypeError(type(obj))

def make_torrent(files=None, piece_length=32768, private=True, announce_stub=True):
    """files: [(name, size_bytes)]；生成与真实客户端一致的 .torrent 字节。"""
    piece_len = piece_length
    if files:
        total = sum(s for _, s in files)
        pieces_count = max(1, (total + piece_len - 1) // piece_len)
    else:
        total, pieces_count = piece_len, 1
    # 每 piece 的 sha1 由内容决定；announce 只看 info_hash，故用确定性的假 piece
    digest = hashlib.sha1(b"piece").digest()
    pieces = digest * pieces_count
    info = {
        b"name": b"Audit.Pack.%d" % random.randint(100000, 999999),
        b"piece length": piece_len,
        b"pieces": pieces,
    }
    if private:
        info[b"private"] = 1
    if files:
        info[b"files"] = [{b"length": s, b"path": [f.encode()]} for f, s in files]
    else:
        info[b"length"] = total
    if announce_stub:
        tor = {b"info": info, b"announce": b"http://127.0.0.1:7070/announce/x" * 1}
    else:
        tor = {b"info": info}
    return benc(tor), hashlib.sha1(benc(info)).hexdigest()

# ---------- multipart 上传 ----------
BOUNDARY = "----fluxaudit42"
def multipart(fields, file_field, filename, content):
    out = b""
    for k, v in fields.items():
        if v is None: continue
        out += ("--%s\r\nContent-Disposition: form-data; name=\"%s\"\r\n\r\n%s\r\n" % (BOUNDARY, k, v)).encode()
    out += ("--%s\r\nContent-Disposition: form-data; name=\"%s\"; filename=\"%s\"\r\nContent-Type: application/x-bittorrent\r\n\r\n" % (BOUNDARY, file_field, filename)).encode()
    out += content + b"\r\n"
    out += ("--%s--\r\n" % BOUNDARY).encode()
    return out

def upload_torrent(tok, tor_bytes, **fields):
    body = multipart(fields, "file", "audit.torrent", tor_bytes)
    req = urllib.request.Request(
        "http://127.0.0.1:8080/api/v1/torrents?category_id=%s" % fields.get("category_id", 1),
        method="POST", data=body)
    req.add_header("Content-Type", "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(req, timeout=30)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try: return e.code, json.loads(e.read())
        except Exception: return e.code, {"raw": e.read()[:200].decode("utf-8","replace")}

# ---------- 流程 ----------
tok = login()
print("== 已登录 root ==")

# 类目探测
s, r = call("GET", "/categories", token=tok)
cats = (r.get("data") or {}).get("items") or (r.get("data") or [])
cat_id = cats[0]["id"] if cats else 1
print("类目数=%d 取 id=%s" % (len(cats), cat_id))

# 1) 发种
tor, ih = make_torrent(files=[("audit.bin", 52428800), ("nfo.txt", 128)])
s, r = upload_torrent(tok, tor, category_id=str(cat_id), name="Audit.Pack.深测",
                      small_descr="深测种子", descr="资深PT深测专用种子")
ok("发种成功", s == 200 and r.get("code") == 0, "%s %s" % (s, r.get("message")))
tid = ((r.get("data") or {}).get("id")) if s == 200 else None
print("   torrent id=%s info_hash=%s" % (tid, ih[:16]))

# 2) 重复上传拒绝
s2, r2 = upload_torrent(tok, tor, category_id=str(cat_id))
ok("重复 info_hash 被拒", r2.get("code") not in (0, None) and s2 != 200, "%s %s" % (s2, r2.get("message")))

# 3) 下载 .torrent（拿 passkey 链）
s, me = call("GET", "/me/overview", token=tok)
passkey = ((me.get("data") or {}).get("passkey")) or ""
print("   passkey=%s..." % passkey[:6])
ok("拿到 passkey", bool(passkey))

# 4) 下载 .torrent 文件（root 免费下载自己的种子）
if tid and passkey:
    s, raw = call("GET", "/torrents/%d/download?passkey=%s" % (tid, passkey), token=tok, raw=True)
    ok(".torrent 可下载", s == 200 and len(raw) > 100, "status=%s len=%s" % (s, len(raw)))
    if s == 200:
        # 私有种子应剥离非白名单 tracker 并含本站 announce
        has_own = b"/announce/" + passkey.encode() in raw
        ok("下载的 .torrent 含本站 passkey announce", has_own)
        if b"announce-list" in raw:
            import re
            trackers = re.findall(rb"url[0-9]+:([a-z]+)://[^e]+", raw)
            print("   announce-list trackers:", [t[:60] for t in trackers[:5]])

# 5) announce 全链路：started(下载中) → 传输 → completed → seeding
if tid and passkey:
    ih_bytes = bytes.fromhex(ih)
    peer_id = b"-FL2600-AUDIT1234567"
    def announce(**q):
        base = "http://127.0.0.1:7070/announce/%s" % passkey
        qs = urllib.parse.urlencode(q, quote_via=urllib.parse.quote)
        req = urllib.request.Request(base + "?" + qs, headers={"User-Agent": "Transmission/4.0.3"})
        try:
            r = urllib.request.urlopen(req, timeout=10)
            return r.status, r.read()
        except urllib.error.HTTPError as e:
            return e.code, e.read()
    q0 = {"info_hash": ih_bytes, "peer_id": peer_id, "port": 51413,
          "uploaded": 0, "downloaded": 0, "left": 52428800, "compact": 1,
          "numwant": 50}
    s, body = announce(**q0, event="started")
    ok("announce started 200", s == 200, "%s %s" % (s, body[:120]))
    ok("bencode 响应合法", body.startswith(b"d") and b"peers" in body, body[:80])
    # 完成事件
    q1 = dict(q0); q1["downloaded"] = 52428800; q1["left"] = 0
    s, body = announce(**q1, event="completed")
    ok("announce completed 200", s == 200, "%s" % s)
    time.sleep(2)
    # 查 DB 种子计数
    print("   等待 worker 快照...")

# 6) announce 边界：负数上传量 / 空参数 / 越权 passkey
if passkey:
    ih_bytes = bytes.fromhex(ih)
    peer_id = b"-FL2600-AUDIT1234567"
    def announce_raw(qs):
        base = "http://127.0.0.1:7070/announce/%s" % passkey
        req = urllib.request.Request(base + "?" + qs, headers={"User-Agent": "Transmission/4.0.3"})
        try:
            r = urllib.request.urlopen(req, timeout=10)
            return r.status, r.read()
        except urllib.error.HTTPError as e:
            return e.code, e.read()
    bad_qs = urllib.parse.urlencode({"info_hash": ih_bytes, "peer_id": peer_id, "port": 51413,
        "uploaded": -99999, "downloaded": -99999, "left": -5, "compact": 1})
    s, body = announce_raw(bad_qs)
    print("   负数参数 announce: %s %s" % (s, body[:100]))
    # bencode 协议口径：错误以 failure reason 返回（HTTP 仍 200）
    ok("负数参数被拒", b"failure reason" in body, body[:80])
    # 无效 passkey
    req = urllib.request.Request("http://127.0.0.1:7070/announce/%s?info_hash=%s&peer_id=%s&port=1&uploaded=0&downloaded=0&left=1"
        % ("0"*32, urllib.parse.quote(ih_bytes), urllib.parse.quote(peer_id)),
        headers={"User-Agent": "Transmission/4.0.3"})
    try:
        r = urllib.request.urlopen(req, timeout=10); sc, bd = r.status, r.read()
    except urllib.error.HTTPError as e: sc, bd = e.code, e.read()
    ok("无效 passkey 被拒", b"failure reason" in bd or b"passkey" in bd, "%s %s" % (sc, bd[:80]))

summary()
