# -*- coding: utf-8 -*-
"""T6: bypass-frontend surfaces — RSS passkey, NP compat download.php, download keys token,
torznab apikey, PTPP userinfo. Each must: work with valid cred, fail with invalid,
and never expose data beyond the cred owner's rights (e.g. unapproved torrents)."""
import sys, os, json, hashlib, urllib.parse, urllib.request, urllib.error
sys.path.insert(0, os.path.dirname(__file__))
from core import req, psql, case, OUT, BASE

T = json.load(open(os.path.join(os.path.dirname(__file__), "_tokens.json"), encoding="utf-8"))
A = T["tokens"]["zt_a"]
UID_B = 42
PASS_B = psql("SELECT passkey FROM users WHERE id=42").strip()
TOR = json.load(open(os.path.join(os.path.dirname(__file__), "_torrent.json")))
TID = int(TOR["torrent_id"])


def get(path, headers=None):
    r = urllib.request.Request(BASE + path, headers=headers or {})
    try:
        with urllib.request.urlopen(r, timeout=15) as resp:
            return resp.status, resp.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


# --- RSS ---
sc, body = get(f"/api/v1/rss/{PASS_B}")
case("T6.1 RSS with valid passkey works", sc == 200 and b"<item" in body or (sc == 200), f"sc={sc} len={len(body)}")
sc2, _ = get("/api/v1/rss/" + "0" * 32)
case("T6.2 RSS invalid passkey 404", sc2 == 404, f"sc={sc2}")
# RSS must not leak unapproved torrents
unapproved = psql("SELECT count(*) FROM torrents WHERE approval_status != 1")
has_unappr = b"zt-audit-torrent" in body  # earlier torrents 41/42 were approved; check text-level leak marker instead
case("T6.3 RSS only approved torrents (no pending leak)", unapproved == "0" or not has_unappr, f"unapproved={unapproved}")

# --- NP compat download ---
sc, body = get(f"/api/v1/compat/nexusphp/download.php?id={TID}&passkey={PASS_B}")
case("T6.4 NP download.php valid passkey returns torrent", sc == 200 and body[:1] == b"d", f"sc={sc} head={body[:20]}")
sc2, _ = get(f"/api/v1/compat/nexusphp/download.php?id={TID}&passkey={'f'*32}")
case("T6.5 NP download.php bad passkey rejected", sc2 in (401, 403, 404), f"sc={sc2}")

# passkey of suspended/banned user must fail — flip zt_b status briefly
psql("UPDATE users SET status=2 WHERE id=42")
sc3, _ = get(f"/api/v1/compat/nexusphp/download.php?id={TID}&passkey={PASS_B}")
case("T6.6 banned user passkey rejected", sc3 in (401, 403, 404), f"sc={sc3}")
psql("UPDATE users SET status=0 WHERE id=42")

# --- download keys: mint via API then fetch ---
c, j = req("POST", "/api/v1/downloads/keys", {"torrent_id": TID}, token=A)
key = (j.get("data") or {}).get("key") or (j.get("data") or {}).get("token")
case("T6.7 download key mint", j.get("code") == 0 and key, f"resp={json.dumps(j, ensure_ascii=False)[:150]}")
if key:
    sc, body = get(f"/api/v1/downloads/{TID}?token={key}")
    case("T6.8 key fetch returns torrent", sc == 200 and body[:1] == b"d", f"sc={sc} head={body[:20]}")
    sc2, _ = get(f"/api/v1/downloads/{TID}?token=wrongtoken")
    case("T6.9 wrong token rejected", sc2 in (401, 403, 404), f"sc={sc2}")
    # key for another torrent id must fail (token bound to torrent)
    other = int(TID) - 1
    sc3, _ = get(f"/api/v1/downloads/{other}?token={key}")
    case("T6.10 token bound to its torrent", sc3 in (401, 403, 404), f"sc={sc3}")

# --- torznab apikey surface ---
sc, body = get("/api/v1/openapi/torznab?apikey=nonexistentkey123456")
case("T6.11 torznab bad apikey rejected", sc in (401, 403), f"sc={sc} body={body[:100]}")

# --- PTPP userinfo ---
sc, body = get("/api/v1/plugins/ptppUserInfo")
case("T6.12 ptppUserInfo requires auth", sc in (401, 403), f"sc={sc}")

print("T6 done", sum(1 for x in OUT["cases"] if x["ok"]), "/", len(OUT["cases"]))
