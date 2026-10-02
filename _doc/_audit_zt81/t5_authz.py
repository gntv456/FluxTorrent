# -*- coding: utf-8 -*-
"""T5: authz matrix — anonymous / class-1 horizontal / class-1 -> admin vertical / forged headers.
Each probe: expected rejection unless whitelisted. Also horizontal access to other users' data."""
import sys, os, json
sys.path.insert(0, os.path.dirname(__file__))
from core import req, psql, case, OUT

T = json.load(open(os.path.join(os.path.dirname(__file__), "_tokens.json"), encoding="utf-8"))
A, C, ROOT = T["tokens"]["zt_a"], T["tokens"]["zt_c"], T.get("root")
UID_A, UID_C = psql("SELECT id FROM users WHERE username='zt_a'"), psql("SELECT id FROM users WHERE username='zt_c'")

ADMIN_PROBES = [
    ("GET", "/api/v1/admin/users?page=1"),
    ("POST", "/api/v1/admin/users/adjust", {"user_id": int(UID_A), "uploaded": 10**12, "downloaded": 0}),
    ("POST", "/api/v1/admin/amountbonus", {"user_id": int(UID_A), "amount": 99999}),
    ("GET", "/api/v1/admin/reviews"),
    ("POST", "/api/v1/admin/gacha/grant", {"user_id": int(UID_A), "item_id": 1, "qty": 99}),
    ("GET", "/api/v1/admin/settings"),
    ("GET", "/api/v1/admin/cheat-events"),
    ("GET", "/api/v1/admin/audit-log"),
]
ok_all = True
detail = []
for method, path, body in [(m, p, None) if len(x) == 2 else (x[0], x[1], x[2]) for x in ADMIN_PROBES for m, p in [(x[0], x[1])] for x in [x]]:
    pass
for probe in ADMIN_PROBES:
    method, path = probe[0], probe[1]
    body = probe[2] if len(probe) > 2 else None
    c, j = req(method, path, body, token=A)
    rejected = j.get("code") not in (0,)
    ok_all = ok_all and rejected
    detail.append(f"{method} {path} -> {j.get('code')}")
case("T5.1 class-1 CANNOT hit admin endpoints", ok_all, "; ".join(detail))

# forged header: X-User-Id / X-Forwarded-User style spoofing
c, j = req("GET", "/api/v1/me", headers={"X-User-Id": "1", "X-Forwarded-User": "root"}, token=A)
me = (j.get("data") or {}).get("username") or (j.get("data") or {}).get("id")
case("T5.2 header spoof cannot impersonate (me still zt_a)", me in ("zt_a", int(UID_A)), f"me={me}")

# token of zt_a calling zt_c's staff endpoints
c, j = req("POST", "/api/v1/staffmessages/answer", {"id": 1, "body": "x"}, token=A)
case("T5.3 staffbox answer denied for class-1", j.get("code") != 0, f"code={j.get('code')}")

# anonymous access to private endpoints
c, j = req("GET", "/api/v1/messages/inbox")
case("T5.4 anonymous inbox rejected", j.get("code") != 0, f"code={j.get('code')}")
c, j = req("POST", "/api/v1/attendance/checkin")
case("T5.5 anonymous checkin rejected", j.get("code") != 0, f"code={j.get('code')}")

# horizontal: zt_a editing zt_c profile? try user settings of another user
c, j = req("PUT", "/api/v1/users/%s/avatar" % UID_C, {"avatar_url": "http://evil/z.png"}, token=A)
case("T5.6 cannot set another user's avatar", j.get("code") != 0, f"code={j.get('code')}")

# tampered JWT (flip signature)
bad = A[:-4] + ("AAAA" if not A.endswith("AAAA") else "BBBB")
c, j = req("GET", "/api/v1/me", token=bad)
case("T5.7 tampered JWT rejected", j.get("code") != 0, f"code={j.get('code')}")

# expired/absent token
c, j = req("GET", "/api/v1/me", token="")
case("T5.8 empty bearer rejected", j.get("code") != 0, f"code={j.get('code')}")

print("T5 done", sum(1 for x in OUT["cases"] if x["ok"]), "/", len(OUT["cases"]))
