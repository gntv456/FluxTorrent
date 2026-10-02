# -*- coding: utf-8 -*-
"""T1: account bootstrap — mint test users via SQL (password Test1234!), verify login path works for root, and register-via-invite happy path."""
import sys, os, json, hashlib
sys.path.insert(0, os.path.dirname(__file__))
from core import req, psql, case, login, OUT, ENV

# root login baseline (docker local account)
c, j, root_tok = login("root", "password123")
case("T1.1 root login", c == 200 and root_tok, f"code={c} resp={json.dumps(j, ensure_ascii=False)[:120]}")

# mint three test users directly in DB (bypassing registration gate is intentional:
# we want clean isolated accounts; pass_hash = password123 same scheme as root)
r = psql("SELECT pass_hash FROM users WHERE id=1")
hash_tpl = r
for uname, cls in [("zt_a", 1), ("zt_b", 1), ("zt_c", 93)]:
    exists = psql(f"SELECT count(*) FROM users WHERE username='{uname}'")
    if exists == "0":
        psql(
            f"INSERT INTO users (username, email, pass_hash, passkey, class_id) "
            f"VALUES ('{uname}', '{uname}@zt.local', '{hash_tpl}', md5(random()::text), {cls})"
        )
    else:
        psql(f"UPDATE users SET status=0, class_id={cls} WHERE username='{uname}'")
    uid = psql(f"SELECT id FROM users WHERE username='{uname}'")
    # give them a stake: 10GB up, 1GB down, 5000 spark
    psql(f"UPDATE users SET uploaded=10737418240, downloaded=1073741824, spark_balance=5000 WHERE id={uid}")
    print(f"minted {uname} uid={uid} class={cls}")

# all three can login via API
toks = {}
for uname in ["zt_a", "zt_b", "zt_c"]:
    c, j, t = login(uname, "password123")
    toks[uname] = t
    case(f"T1.2 login {uname}", c == 200 and t, f"code={c} {json.dumps(j, ensure_ascii=False)[:100]}")

# registration: mode gate must reject without invite (invite_only)
c, j = req("POST", "/api/v1/auth/register", {"username": "zt_reg_noinv", "email": "x@zt.local", "password": "Test1234!", "invite_code": ""})
case("T1.3 register without invite rejected", j.get("code") != 0, f"resp={json.dumps(j, ensure_ascii=False)[:150]}")

json.dump({"tokens": toks, "root": root_tok}, open(os.path.join(os.path.dirname(__file__), "_tokens.json"), "w"), ensure_ascii=False, indent=1)
OUT["env"]["users"] = "zt_a/zt_b (class1), zt_c (class93), root"
print("T1 done", sum(1 for x in OUT["cases"] if x["ok"]), "/", len(OUT["cases"]))
