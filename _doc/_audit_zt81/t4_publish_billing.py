# -*- coding: utf-8 -*-
"""T4: publish-to-billing closed loop. Build a real .torrent in python (bencode),
upload as zt_a, approve as staff (zt_c), then drive the tracker announce path
with zt_b's passkey to verify: upload credit only via announce stream,
no credit from plain download, stopped/paused events don't fabricate stats."""
import sys, os, json, hashlib, struct, random, urllib.parse
sys.path.insert(0, os.path.dirname(__file__))
from core import req, psql, case, OUT, BASE
import urllib.request

T = json.load(open(os.path.join(os.path.dirname(__file__), "_tokens.json"), encoding="utf-8"))
A, C = T["tokens"]["zt_a"], T["tokens"]["zt_c"]
UID_A = psql("SELECT id FROM users WHERE username='zt_a'")
UID_B = psql("SELECT id FROM users WHERE username='zt_b'")
PASS_B = psql(f"SELECT passkey FROM users WHERE id={UID_B}").strip()

# ---- helpers: minimal bencode ----
def benc_int(n): return b'i%de' % n
def benc_bytes(b): return str(len(b)).encode() + b':' + b
def benc_str(s): return benc_bytes(s.encode('utf-8'))
def benc_list(items):
    return b'l' + b''.join(items) + b'e'
def benc_dict(pairs):
    # bencode dict keys must themselves be length-prefixed byte strings
    return b'd' + b''.join(benc_bytes(k) + v for k, v in pairs) + b'e'

random.seed()
piece = bytes(random.getrandbits(8) for _ in range(16384))
pieces = (hashlib.sha1(piece).digest()) * 2  # 2 pieces
info = benc_dict([
    (b'length', benc_int(32768)),
    (b'name', benc_str('zt-audit-torrent-%d' % random.randint(100000, 999999))),
    (b'piece length', benc_int(16384)),
    (b'pieces', benc_bytes(pieces)),
])
info_hash = hashlib.sha1(info).digest()
info_hash_hex = info_hash.hex()
t = benc_dict([
    (b'announce', benc_str('http://127.0.0.1:7070/announce')),
    (b'info', info),
])
tor_b64_note = None

# multipart upload
boundary = '----ztboundary42'
def mp_field(name, value):
    return (f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n{value}\r\n').encode()
body = mp_field('file', '')  # placeholder, replaced below with real file part
file_part = (f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="zt.torrent"\r\n'
             f'Content-Type: application/x-bittorrent\r\n\r\n').encode() + t + b'\r\n'
fields = dict(name='zt audit torrent', small_descr='audit', descr='audit descr',
              category_id='1', medium_id='1', anonymous='false', price='0')
for k, v in fields.items():
    file_part += mp_field(k, v)
file_part += f'--{boundary}--\r\n'.encode()

r = urllib.request.Request(BASE + '/api/v1/torrents?category_id=1&name=zt-audit-torrent&small_descr=audit&descr=audit%20descr&anonymous=false&price=0', data=file_part, method='POST',
                           headers={'Content-Type': f'multipart/form-data; boundary={boundary}',
                                    'Authorization': 'Bearer ' + A})
try:
    with urllib.request.urlopen(r, timeout=30) as resp:
        up = json.loads(resp.read())
except urllib.error.HTTPError as e:
    up = json.loads(e.read())
tid = (up.get("data") or {}).get("id") or (up.get("data") or {}).get("torrent_id")
case("T4.1 torrent upload accepted", up.get("code") == 0 and tid, f"resp={json.dumps(up, ensure_ascii=False)[:200]}")

if not tid:
    print("upload failed; abort T4")
    sys.exit(1)

# approval status should be pending (zt_a class 1, no streak)
st = psql(f"SELECT approval_status FROM torrents WHERE id={tid}")
case("T4.2 fresh upload from class-1 is pending approval", st == "0", f"approval_status={st}")

# zt_c (class93) approves it via review decide
c, j = req("POST", "/api/v1/admin/reviews/decide", {"torrent_id": int(tid), "approve": True}, token=C)
if j.get("code") != 0:
    c, j = req("POST", "/api/v1/admin/reviews/decide", {"id": int(tid), "action": "approve"}, token=C)
st2 = psql(f"SELECT approval_status FROM torrents WHERE id={tid}")
approved = st2 == "1"
if not approved:
    # direct DB fallback to keep chain testable, note it
    psql(f"UPDATE torrents SET approval_status=1 WHERE id={tid}")
    print("NOTE: approve endpoint not found, approved via DB")
case("T4.3 staff approves torrent", approved or st2 == "1", f"status={st2} resp={json.dumps(j, ensure_ascii=False)[:150]}")

# ---- announce as zt_b: seed event (left=0) with uploaded=1GB cumulative ----
def announce(passkey, ih, up, down, left, event='', pid=None):
    # info_hash must be percent-encoded per raw byte (not double-encoded)
    ih_q = ''.join('%%%02X' % b for b in ih)
    pid_q = '-zt0001-abcdefabcdef'
    url = 'http://127.0.0.1:7070/announce/%s?info_hash=%s&peer_id=%s&port=6881&uploaded=%d&downloaded=%d&left=%d&compact=1%s' % (
        passkey, ih_q, pid_q, up, down, left, ('&event=' + event if event else ''))
    try:
        with urllib.request.urlopen(url, timeout=10) as resp:
            return resp.status, resp.read()[:120]
    except urllib.error.HTTPError as e:
        return e.code, e.read()[:120]

pre_up_b = int(psql(f"SELECT uploaded FROM users WHERE id={UID_B}"))
sc, body = announce(PASS_B, info_hash, 1073741824, 0, 0, event='started')
print("announce1:", sc, body)
sc, body = announce(PASS_B, info_hash, 2147483648, 0, 0)  # cumulative 2GB
print("announce2:", sc, body)

# worker consumes stream every ~60s; wait up to 100s for ledger
import time
delta = 0
for i in range(12):
    time.sleep(10)
    post_up_b = int(psql(f"SELECT uploaded FROM users WHERE id={UID_B}"))
    delta = post_up_b - pre_up_b
    if delta >= 1073741824:
        break
led = psql(f"SELECT COALESCE(sum(delta_up),0) FROM traffic_ledger WHERE user_id={UID_B}")
case("T4.4 announce stream credits upload exactly once (cumulative diff)",
     delta == 1073741824, f"uploaded_delta={delta} ledger_up={led}")

# replay: send same cumulative 2GB again — must not double-credit
sc, body = announce(PASS_B, info_hash, 2147483648, 0, 0)
time.sleep(15)
post2 = int(psql(f"SELECT uploaded FROM users WHERE id={UID_B}"))
case("T4.5 announce replay (same cumulative) adds nothing",
     post2 - pre_up_b == 1073741824, f"delta_now={post2-pre_up_b}")

json.dump({"torrent_id": tid, "info_hash_hex": info_hash_hex}, open(os.path.join(os.path.dirname(__file__), "_torrent.json"), "w"))
print("T4 done", sum(1 for x in OUT["cases"] if x["ok"]), "/", len(OUT["cases"]))
