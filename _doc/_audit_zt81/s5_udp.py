# -*- coding: utf-8 -*-
"""S5: UDP tracker (6969) billing path — BEP15 handshake (connect) + extended announce
(98-byte packet + trailing passkey as tracker_id). Verify: standard 98B packet gets clear
error (HTTP fallback guidance), extended packet bills identically to HTTP path."""
import sys, os, json, socket, struct, time, hashlib
sys.path.insert(0, os.path.dirname(__file__))
from core import req, psql, case, OUT

UID_B = 49
PASS_B = psql("SELECT passkey FROM users WHERE id=49").strip()
S1 = json.load(open(os.path.join(os.path.dirname(__file__), "_s1.json")))
tid = int(S1["tid"])
ih = bytes.fromhex(S1["ih_hex"])

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.settimeout(5)
addr = ("127.0.0.1", 6969)

# --- BEP15 connect ---
tid_txn = 0x53551234
conn_req = struct.pack(">QII", 0x41727101980, 0, tid_txn)
sock.sendto(conn_req, addr)
data, _ = sock.recvfrom(2048)
action, rtxn, conn_id = struct.unpack(">IIQ", data[:16])
case("S5.1 UDP connect handshake", action == 0 and rtxn == tid_txn, f"action={action} txn_ok={rtxn==tid_txn}")

# --- standard 98-byte announce (no passkey): expect explicit error ---
peer_id = b"-zt0001-udpauditabcd"[:20]
up0 = int(psql(f"SELECT COALESCE(sum(delta_up),0) FROM traffic_ledger WHERE user_id={UID_B} AND torrent_id={tid}"))
pkt = struct.pack(">QII", conn_id, 1, tid_txn) + ih + peer_id  # 16+20+20=56
pkt += struct.pack(">Q", 1073741824)   # downloaded 1GB cumulative
pkt += struct.pack(">Q", 0)            # left 0
pkt += struct.pack(">Q", up0 + 5368709120)  # uploaded cumulative +0.5GB raw
pkt += struct.pack(">IIIIH", 0, 0, 0, 50, 6881)  # event,ip,key,numwant,port = 18
assert len(pkt) == 98, len(pkt)
sock.sendto(pkt, addr)
data, _ = sock.recvfrom(2048)
action2 = struct.unpack(">I", data[:4])[0]
msg = data[8:].decode("utf-8", "replace")
case("S5.2 standard 98B packet rejected with clear guidance (no silent billing without identity)",
     action2 == 3 and ("HTTP" in msg or "passkey" in msg), f"action={action2} msg={msg[:80]}")

# --- extended announce: trailing passkey as tracker_id ---
pkt_ext = pkt + PASS_B.encode()
sock.sendto(pkt_ext, addr)
data, _ = sock.recvfrom(2048)
action3 = struct.unpack(">I", data[:4])[0]
case("S5.3 extended packet accepted", action3 == 1, f"action={action3} body={data[:40]}")

# wait for worker billing: expect +0.5GB up raw => billed 0.5GB (x2 promo still on from S1)
target = up0 + 1073741824  # x2 => 1GB billed
billed = up0
for _ in range(15):
    time.sleep(10)
    billed = int(psql(f"SELECT COALESCE(sum(delta_up),0) FROM traffic_ledger WHERE user_id={UID_B} AND torrent_id={tid}"))
    if billed >= target: break
case("S5.4 UDP announce bills via same stream (0.5GB raw, x2 promo => 1GB)",
     billed == target, f"up {up0}->{billed} expected {target}")

# --- invalid conn_id rejected ---
bad = struct.pack(">QII", 0xDEADBEEF, 1, tid_txn) + ih + peer_id + struct.pack(">QQQ", 0, 0, 0) + struct.pack(">IIiH", 0, 0, 50, 6881) + PASS_B.encode()
sock.sendto(bad, addr)
data, _ = sock.recvfrom(2048)
action4 = struct.unpack(">I", data[:4])[0]
case("S5.5 invalid connection_id rejected", action4 == 3, f"action={action4}")

# --- wrong passkey rejected ---
pkt_badpk = pkt + ("f" * 32).encode()
sock.sendto(pkt_badpk, addr)
data, _ = sock.recvfrom(2048)
action5 = struct.unpack(">I", data[:4])[0]
msg5 = data[8:].decode("utf-8", "replace")
case("S5.6 wrong passkey rejected (no billing)", action5 == 3 and "passkey" in msg5, f"action={action5} msg={msg5[:60]}")

print("S5 done", sum(1 for x in OUT["cases"] if x["ok"]), "/", len(OUT["cases"]))
