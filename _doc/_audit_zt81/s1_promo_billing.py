# -*- coding: utf-8 -*-
"""S1: promotion x billing live closed loop.
Upload real torrent as zt_a -> approve as zt_c -> set free promo via /admin/torrents/batch
-> drive UDP+HTTP announces as zt_b with download counters -> verify ledger down=0 while free,
then remove promo -> announce more -> verify down billed. Also x2 upload promo."""
import sys, os, json, hashlib, random, time, urllib.request, urllib.error
sys.path.insert(0, os.path.dirname(__file__))
from core import req, psql, case, OUT, BASE

T = json.load(open(os.path.join(os.path.dirname(__file__), "_tokens.json"), encoding="utf-8"))
A, C = T["tokens"]["zt_a"], T["tokens"]["zt_c"]
UID_B = 49
PASS_B = psql("SELECT passkey FROM users WHERE id=49").strip()

# ---- build torrent ----
def benc_bytes(b): return str(len(b)).encode() + b':' + b
def benc_str(s): return benc_bytes(s.encode('utf-8'))
def benc_int(n): return b'i%de' % n
def benc_dict(pairs): return b'd' + b''.join(benc_bytes(k) + v for k, v in pairs) + b'e'

random.seed()
piece = bytes(random.getrandbits(8) for _ in range(16384))
pieces = (hashlib.sha1(piece).digest()) * 2
info = benc_dict([
    (b'length', benc_int(32768)),
    (b'name', benc_str('zt-s1-billing-%d' % random.randint(100000, 999999))),
    (b'piece length', benc_int(16384)),
    (b'pieces', benc_bytes(pieces)),
])
ih = hashlib.sha1(info).digest()
ih_hex = ih.hex()
ih_q = ''.join('%%%02X' % b for b in ih)
t = benc_dict([(b'announce', benc_str('http://127.0.0.1:7070/announce')), (b'info', info)])

boundary = '----ztboundary42'
file_part = (f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="zt.torrent"\r\nContent-Type: application/x-bittorrent\r\n\r\n').encode() + t + b'\r\n'
file_part += f'--{boundary}--\r\n'.encode()
r = urllib.request.Request(BASE + '/api/v1/torrents?category_id=1&name=zt-s1-billing&small_descr=s1&descr=s1&anonymous=false&price=0',
    data=file_part, method='POST',
    headers={'Content-Type': f'multipart/form-data; boundary={boundary}', 'Authorization': 'Bearer ' + A})
try:
    with urllib.request.urlopen(r, timeout=30) as resp: up = json.loads(resp.read())
except urllib.error.HTTPError as e: up = json.loads(e.read())
tid = (up.get("data") or {}).get("id")
case("S1.1 upload s1 torrent", up.get("code") == 0 and tid, f"resp={json.dumps(up, ensure_ascii=False)[:120]}")
c, j = req("POST", "/api/v1/admin/reviews/decide", {"torrent_id": int(tid), "approve": True}, token=C)
case("S1.2 approve", j.get("code") == 0, f"code={j.get('code')}")

# ---- set free promo via batch ----
until = "2026-10-05T00:00:00Z"
c, j = req("POST", "/api/v1/admin/torrents/batch", {"action": "promo", "ids": [int(tid)], "promo_kind": "free", "promo_until": until}, token=C)
promo = psql(f"SELECT kind FROM promotions WHERE torrent_id={tid} AND now() < ends_at")
case("S1.3 free promo set via batch", j.get("code") == 0 and promo == "free", f"code={j.get('code')} kind={promo} resp={json.dumps(j,ensure_ascii=False)[:120]}")

# ---- announce leech (left>0) with downloaded=1GB cumulative, event completed ----
def announce(up, down, left, event=''):
    url = 'http://127.0.0.1:7070/announce/%s?info_hash=%s&peer_id=-zts10001-abcdefabcdef&port=6881&uploaded=%d&downloaded=%d&left=%d&compact=1%s' % (PASS_B, ih_q, up, down, left, ('&event=' + event if event else ''))
    try:
        with urllib.request.urlopen(url, timeout=10) as resp: return resp.read()
    except urllib.error.HTTPError as e: return e.read()

def wait_billing(target_min_bytes, field='down', timeout=150):
    for _ in range(timeout // 10):
        time.sleep(10)
        v = int(psql(f"SELECT COALESCE(sum(delta_{field}),0) FROM traffic_ledger WHERE user_id={UID_B} AND torrent_id={tid}"))
        if v >= target_min_bytes: return v
    return v

pre_led_down = int(psql(f"SELECT COALESCE(sum(delta_down),0) FROM traffic_ledger WHERE user_id={UID_B} AND torrent_id={tid}"))
announce(0, 1073741824, 32768, event='started')
announce(0, 2147483648, 0, event='completed')
led_down = wait_billing(1)
case("S1.4 FREE promo: 2GB downloaded billed 0 (promo honored at event time)",
     led_down == 0, f"ledger_down={led_down}")

# ---- remove promo (expire) then announce more down ----
psql(f"UPDATE promotions SET ends_at = now() - interval '1 hour' WHERE torrent_id={tid}")
announce(0, 3221225472, 0)  # cumulative 3GB => new delta 1GB at full price
led_down2 = wait_billing(1)
case("S1.5 promo expired: subsequent download billed (delta 1GB)",
     led_down2 - led_down == 1073741824, f"down {led_down}->{led_down2}")

# ---- x2 upload promo ----
psql(f"DELETE FROM promotions WHERE torrent_id={tid}")
c, j = req("POST", "/api/v1/admin/torrents/batch", {"action": "promo", "ids": [int(tid)], "promo_kind": "x2", "promo_until": until}, token=C)
pre_up = int(psql(f"SELECT COALESCE(sum(delta_up),0) FROM traffic_ledger WHERE user_id={UID_B} AND torrent_id={tid}"))
announce(4294967296, 3221225472, 0)  # cumulative up 4GB => raw delta 1GB, x2 => 2GB
led_up = 0
for _ in range(15):
    time.sleep(10)
    led_up = int(psql(f"SELECT COALESCE(sum(delta_up),0) FROM traffic_ledger WHERE user_id={UID_B} AND torrent_id={tid}"))
    if led_up - pre_up >= 2147483648: break
case("S1.6 x2 promo doubles upload credit (1GB raw -> 2GB billed)",
     led_up - pre_up == 2147483648, f"up {pre_up}->{led_up}")

json.dump({"tid": tid, "ih_hex": ih_hex}, open(os.path.join(os.path.dirname(__file__), "_s1.json"), "w"))
print("S1 done", sum(1 for x in OUT["cases"] if x["ok"]), "/", len(OUT["cases"]))
