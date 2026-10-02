# -*- coding: utf-8 -*-
"""T2: economy link tests — attendance, casino, bank, medals shop, gacha. All via API as zt_a; DB snapshots before/after to catch silent drift."""
import sys, os, json
sys.path.insert(0, os.path.dirname(__file__))
from core import req, psql, case, OUT

T = json.load(open(os.path.join(os.path.dirname(__file__), "_tokens.json"), encoding="utf-8"))
A = T["tokens"]["zt_a"]
UID = psql("SELECT id FROM users WHERE username='zt_a'")


def bal(u=UID):
    return int(psql(f"SELECT spark_balance FROM users WHERE id={u}"))


def ledger(kinds):
    return psql(f"SELECT count(*) FROM spark_ledger WHERE user_id={UID} AND kind IN ({kinds})")


# ---------- T2.1 attendance ----------
b0 = bal()
c, j = req("POST", "/api/v1/attendance/checkin", token=A)
c2, j2 = req("POST", "/api/v1/attendance/checkin", token=A)
b1 = bal()
rew = int(psql(f"SELECT reward FROM attendance WHERE user_id={UID} ORDER BY date DESC LIMIT 1") or 0)
case("T2.1 checkin pays once, second rejected",
     j.get("code") == 0 and j2.get("code") != 0 and b1 - b0 == rew,
     f"first={j.get('code')} second={j2.get('code')} delta={b1-b0} reward={rew}")

# ---------- T2.2 casino scratch idempotency ----------
b0 = bal()
key = "zt-scratch-1"
c, j = req("POST", "/api/v1/games/scratch", {"bet": 10, "idempotency_key": key}, token=A)
c2, j2 = req("POST", "/api/v1/games/scratch", {"bet": 10, "idempotency_key": key}, token=A)
rows = psql(f"SELECT count(*) FROM spark_ledger WHERE user_id={UID} AND idempotency_key LIKE '%{key}%'")
case("T2.2 scratch replay rejected, single ledger row",
     j.get("code") == 0 and j2.get("code") != 0 and rows == "2",  # spend + win rows share prefix
     f"first={j.get('code')} replay={j2.get('code')} rows={rows}")

# bet=0 / negative
c3, j3 = req("POST", "/api/v1/games/scratch", {"bet": 0, "idempotency_key": "zt-s0"}, token=A)
c4, j4 = req("POST", "/api/v1/games/scratch", {"bet": -5, "idempotency_key": "zt-sn"}, token=A)
case("T2.3 bet validation (0/negative rejected)", j3.get("code") != 0 and j4.get("code") != 0,
     f"zero={j3.get('code')} neg={j4.get('code')}")

# ---------- T2.4 bank demand deposit/withdraw ----------
b0 = bal()
k1, k2 = "zt-dep-1", "zt-wd-1"
c, j = req("POST", "/api/v1/bank/demand/deposit", {"amount": 100, "idempotency_key": k1}, token=A)
c2, j2 = req("POST", "/api/v1/bank/demand/deposit", {"amount": 100, "idempotency_key": k1}, token=A)
dep = int(psql(f"SELECT COALESCE(balance,0) FROM bank_demand_accounts WHERE user_id={UID}") or 0)
case("T2.4 demand deposit idempotent", j.get("code") == 0 and j2.get("code") != 0 and dep == 100,
     f"first={j.get('code')} replay={j2.get('code')} demand_balance={dep}")
c3, j3 = req("POST", "/api/v1/bank/demand/withdraw", {"amount": 40, "idempotency_key": k2}, token=A)
c4, j4 = req("POST", "/api/v1/bank/demand/withdraw", {"amount": 40, "idempotency_key": k2}, token=A)
dep2 = int(psql(f"SELECT COALESCE(balance,0) FROM bank_demand_accounts WHERE user_id={UID}") or 0)
b2 = bal()
case("T2.5 demand withdraw idempotent", j3.get("code") == 0 and j4.get("code") != 0 and dep2 == 60 and b2 == b0 - 60,
     f"first={j3.get('code')} replay={j4.get('code')} demand={dep2} spark_delta={b2-b0}")
# overdraw
c5, j5 = req("POST", "/api/v1/bank/demand/withdraw", {"amount": 10**9, "idempotency_key": "zt-wd-big"}, token=A)
case("T2.6 overdraw rejected", j5.get("code") != 0, f"code={j5.get('code')}")
# negative amount
c6, j6 = req("POST", "/api/v1/bank/demand/deposit", {"amount": -100, "idempotency_key": "zt-dep-neg"}, token=A)
case("T2.7 negative deposit rejected", j6.get("code") != 0, f"code={j6.get('code')}")

# ---------- T2.8 medal buy/gift ----------
meds = psql("SELECT id, price FROM medals WHERE price > 0 ORDER BY price LIMIT 1")
if meds:
    mid, price = meds.split("|")
    price = int(price)
    b0 = bal()
    c, j = req("POST", "/api/v1/medals/buy", {"medal_id": int(mid), "idempotency_key": "zt-med-1"}, token=A)
    owned = psql(f"SELECT count(*) FROM user_medals WHERE user_id={UID} AND medal_id={mid}")
    case("T2.8 medal buy deducts & grants", j.get("code") == 0 and owned == "1" and b0 - bal() >= price,
         f"code={j.get('code')} owned={owned} price={price} delta={b0-bal()}")
else:
    case("T2.8 medal buy deducts & grants", False, "no priced medal in catalog")

# gift from zt_a to zt_b with replay
B = T["tokens"].get("zt_b")
UID_B = psql("SELECT id FROM users WHERE username='zt_b'")
if B and meds:
    mid2 = psql("SELECT id FROM medals WHERE price > 0 ORDER BY price DESC LIMIT 1")
    b0b = int(psql(f"SELECT spark_balance FROM users WHERE id={UID_B}"))
    c, j = req("POST", "/api/v1/medals/gift", {"medal_id": int(mid), "to_user": "zt_b", "idempotency_key": "zt-gift-1"}, token=A)
    c2, j2 = req("POST", "/api/v1/medals/gift", {"medal_id": int(mid), "to_user": "zt_b", "idempotency_key": "zt-gift-1"}, token=A)
    owned_b = psql(f"SELECT count(*) FROM user_medals WHERE user_id={UID_B} AND medal_id={mid}")
    case("T2.9 medal gift idempotent", j.get("code") == 0 and j2.get("code") != 0 and owned_b >= "1",
         f"first={j.get('code')} replay={j2.get('code')} owned_b={owned_b}")

print("T2 done", sum(1 for x in OUT["cases"] if x["ok"]), "/", len(OUT["cases"]))
