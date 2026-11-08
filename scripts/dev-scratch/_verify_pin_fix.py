# -*- coding: utf-8 -*-
"""官种置顶插件修复验证（plugin_sql 撤授窗口竞赛）：
上传 N 个种子 → 逐个 PUT official 标签（staff 权限）→ 1) api 日志不得再出现 pin update failed
              2) torrents.sticky 必须为 TRUE（置顶真生效）
"""
import sys, io, json, hashlib, urllib.request, urllib.error, time, random, subprocess
sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, "scripts")
from pt_audit_lib import login, call, ok

def benc(obj):
    if isinstance(obj, int): return b"i%de" % obj
    if isinstance(obj, bytes): return b"%d:%s" % (len(obj), obj)
    if isinstance(obj, str): return benc(obj.encode())
    if isinstance(obj, list): return b"l" + b"".join(benc(x) for x in obj) + b"e"
    if isinstance(obj, dict):
        out = b"d"
        for k in sorted(obj.keys()):
            out += benc(k) + benc(obj[k])
        return out + b"e"
    raise TypeError(type(obj))

def make_torrent():
    salt = ("%f" % random.random()).encode()
    piece_len = 32768
    total = 52428800
    # 分片数必须与文件大小自洽（服务端校验：total/piece_len 向上取整）
    pieces_count = (total + piece_len - 1) // piece_len
    pieces = hashlib.sha1(b"piece:" + salt).digest() * pieces_count
    info = {b"name": b"Official.PinTest.%d" % random.randint(1, 999999),
            b"piece length": piece_len, b"pieces": pieces, b"private": 1,
            b"files": [{b"length": total, b"path": [b"pin.bin"]}]}
    tor = {b"info": info, b"announce": b"http://127.0.0.1:7070/announce/x"}
    return benc(tor), hashlib.sha1(benc(info)).hexdigest()

BOUNDARY = "----fluxpin42"
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
    body = multipart(fields, "file", "pin.torrent", tor_bytes)
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
        except Exception: return e.code, {"raw": e.read()[:200].decode("utf-8", "replace")}

tok = login()
OFFICIAL_TAG_ID = 3  # tag_dict: 官种/official/torrent

tids = []
for i in range(3):
    tor, ih = make_torrent()
    s, r = upload_torrent(tok, tor, category_id="1",
                          name="Official.PinTest.%d" % i,
                          small_descr="官种置顶验证",
                          descr="官种置顶插件修复验证专用种子，自动置顶应当生效，本种子为官方资源性质。")
    tid = ((r.get("data") or {}).get("id")) if s == 200 else None
    tids.append(tid)
    print("upload #%d → st=%s tid=%s msg=%s" % (i, s, tid, r.get("message") if s != 200 else ""))
    if tid:
        # PUT official 标签（staff class 99）→ sync_official_tag → TorrentUploaded 事件
        s2, r2 = call("PUT", "/torrents/%d/tags" % tid,
                      body={"tag_id": OFFICIAL_TAG_ID, "on": True}, token=tok)
        print("   tag official → st=%s %s" % (s2, r2.get("message", "")))
    time.sleep(1.5)  # 让插件 hook（spawn + 事件消费）都跑完

print("== 等待 2s 后查库 ==")
time.sleep(2)
for tid in tids:
    if not tid: continue
    out = subprocess.run(["docker", "exec", "flux-postgres", "psql", "-U", "flux",
                          "-d", "fluxtorrent", "-t", "-c",
                          "SELECT id, official_tag, sticky FROM torrents WHERE id=%d" % tid],
                         capture_output=True, text=True)
    print("tid=%s → %s" % (tid, out.stdout.strip()))

print("== api 日志中的 pin 报错（近 5 分钟） ==")
out = subprocess.run(["docker", "logs", "flux-api", "--since", "5m"],
                     capture_output=True, text=True)
bad = [l for l in (out.stdout + out.stderr).splitlines() if "pin update failed" in l]
ok("无 pin update failed 报错", len(bad) == 0, "%d 条" % len(bad))
for l in bad[:5]: print("   BAD:", l[:150])

print("== 汇总 ==")
ok("3 个种子上传成功", all(tids), str(tids))
for tid in tids:
    if not tid: continue
    out = subprocess.run(["docker", "exec", "flux-postgres", "psql", "-U", "flux",
                          "-d", "fluxtorrent", "-t", "-c",
                          "SELECT official_tag, sticky FROM torrents WHERE id=%d" % tid],
                         capture_output=True, text=True)
    row = out.stdout.strip().replace("|", "").split()
    ok("tid=%s official+sticky" % tid, row == ["t", "t"], str(row))
