"""0101 用户自购置顶/限时免费——端到端验证。

链路：价目表 → 购买一级置顶（扣魔力流水）→ torrents.pos_state 生效 → 列表排序置顶
     → 同幂等键重试不双扣 → 续购顺延 → 购买限时免费 → promotions 生效（列表 free 角标口径）
     → 权限（非本人被拒）→ 环境还原。

⚠️ 本脚本在 HEAD 里就是坏的，别当回归用（10-21 实录；我只补了「取探针种」那一处
   前置，其余留给站方修或删）：
     - 4 处 SQL 跨行拼接丢空格（`ORDERBY` / `WHEREid=` / `free'AND` / `-interval`）
       ⇒ psql 报错，而 helper 只回 stdout ⇒ `int('')` 崩在断言之前；
     - 3 处 SQL 少了 f 前缀 ⇒ 语句里是字面量 `{tid}`；
     - 幂等键按客户原键等值查，服务端存的其实是 `promo:<uid>:<客户键>`
       ⇒ 「没落流水」是假红；收尾同错 ⇒ 「环境还原」是永远成立的空判据；
     - 拿 root 余额相等当幂等判据：root 会被 worker 的做种奖励随时充值
       （实测 20 分钟内 7575 → 95000+）⇒ 竞态判据。
   这条链路的常驻覆盖已迁到 scripts/pt_audit_gq_promo_buy.py（GQ1~GQ9）。
"""
import json
import subprocess
import sys
import time
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080/api/v1"
results = []


def call(method, path, body=None, token=None, raw=False):
    req = urllib.request.Request(BASE + path, method=method)
    if body is not None:
        req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=15) as r:
            b = r.read()
            return r.status, b if raw else json.loads(b)
    except urllib.error.HTTPError as e:
        try:
            return e.code, e.read() if raw else json.loads(e.read())
        except Exception:
            return e.code, {}


def check(name, ok, detail=""):
    results.append((name, ok, detail))
    print(f"{'PASS' if ok else 'FAIL'} {name} {detail}")


def psql(sql):
    return subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
            "fluxtorrent", "-t", "-A", "-c", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace").stdout.strip()


s, r = call("POST", "/auth/login", {"username": "root",
    "password": "password123"})
tok = r["data"]["token"]

# 选一个 root 自己的过审种做探针
# （修脚本自己的洞：原来两句拼接漏了空格，SQL 变成 `… = 1 ORDERBY id` ⇒
#   psql 报错、stdout 空串 ⇒ `int('')` 崩在第一行，断言一条都没跑到；
#   而下一行的 `assert tid` 永远走不到，等于没有前置检查。）
tid_raw = psql("SELECT id FROM torrents WHERE owner_id = 1 AND "
               "approval_status = 1 ORDER BY id LIMIT 1")
if not tid_raw.strip().isdigit():
    raise SystemExit("前置失败：root 名下取不到过审种（psql 读数=%r，"
                     "多半是 SQL 报错被 helper 吞了）" % tid_raw)
tid = int(tid_raw)
assert tid, "no owned torrent"

# ---------- 价目表 ----------
s, r = call("GET", "/promo/plans", token=tok)
plans = (r.get("data") or {})
s1 = ((plans.get("plans") or {}).get("sticky1") or [{}])[0].get("price")
fr = ((plans.get("plans") or {}).get("free") or [{}])[0].get("price")
check("价目表可读", plans.get("enabled") is True and s1 and fr,
    f"sticky1={s1} free={fr}")

psql(
    "UPDATE users SET spark_balance = 100000 WHERE id=1 AND spark_balance <"
        "50000")
bal0 = int(psql("SELECT spark_balance FROM users WHERE id=1"))

# ---------- 购买一级置顶 ----------
idem = f"e2e-promo-{int(time.time())}"
s, r = call("POST", "/promo/buy", {"torrent_id": tid, "kind": "sticky1",
    "hours": 24, "idempotency_key": idem}, token=tok)
bal1 = int(psql("SELECT spark_balance FROM users WHERE id=1"))
led = psql(
    f"SELECT count(*) FROM spark_ledger WHERE idempotency_key = '{idem}'")
check("购买置顶·扣费=价目且落流水", r.get("code") == 0 and bal0 - bal1 == s1 and led == "1",
      f"code={r.get('code')} 扣 {bal0 - bal1} 价 {s1} ledger={led}")

ps = psql(
    f"SELECT pos_state, pos_state_until > now() FROM torrents WHERE id={tid}")
check("置顶生效(pos_state=1+未来到期)", ps.startswith("1|t"), f"{ps}")

# 列表排序：置顶种应出现在第一行（alive=0 全量口径）
s, r = call("GET", "/torrents?alive=0&limit=5", token=tok)
first = ((r.get("data") or {}).get("items") or [{}])[0].get("id")
check("列表置顶排序(第一位)", first == tid, f"first={first} want={tid}")

# 同幂等键重试：不双扣
s, r = call("POST", "/promo/buy", {"torrent_id": tid, "kind": "sticky1",
    "hours": 24, "idempotency_key": idem}, token=tok)
bal2 = int(psql("SELECT spark_balance FROM users WHERE id=1"))
led = psql(
    f"SELECT count(*) FROM spark_ledger WHERE idempotency_key = '{idem}'")
check("同幂等键重试不双扣", bal2 == bal1 and led == "1",
    f"bal {bal1}->{bal2} ledger={led}")

# 续购顺延：再买 72h，到期时间应 > 原 24h 到期
until0 = psql(
    "SELECT extract(epoch from pos_state_until)::bigint FROM torrents WHERE"
        "id={tid}")
idem2 = idem + "-x2"
s, r = call("POST", "/promo/buy", {"torrent_id": tid, "kind": "sticky2",
    "hours": 72, "idempotency_key": idem2}, token=tok)
until1 = psql(
    "SELECT extract(epoch from pos_state_until)::bigint FROM torrents WHERE"
        "id={tid}")
ps = psql(f"SELECT pos_state FROM torrents WHERE id={tid}")
check("续购顺延+档位切换", until1 > until0 and ps == "2",
    f"until {until0}->{until1} pos={ps}")

# ---------- 购买限时免费 ----------
idem3 = idem + "-free"
s, r = call("POST", "/promo/buy", {"torrent_id": tid, "kind": "free",
    "hours": 24, "idempotency_key": idem3}, token=tok)
promo = psql(
    "SELECT count(*) FROM promotions WHERE torrent_id={tid} AND kind='free'"
        "AND starts_at<=now() AND ends_at>now()")
promo_count_ge1 = promo.isdigit() and int(promo) >= 1
s, r = call("GET", f"/torrents?alive=0&search=", token=tok)
items = (r.get("data") or {}).get("items") or []
me = next((i for i in items if i.get("id") == tid), {})
check("限时免费·促销生效且列表角标", r.get("code") == 0 and int(
    promo) >= 1 and promo_count_ge1 and me.get("promotion") == "free",
      f"promo_rows={promo} list_promotion={me.get('promotion')}")

# ---------- 权限：非本人被拒 ----------
# 造一个临时用户（非 staff、非 owner）
psql("DELETE FROM users WHERE username='e2e_promo_u'")
psql(
    "INSERT INTO users (username, email, pass_hash, passkey, class_id) VALUES"
        "('e2e_promo_u','u@u.local','x','e2epromou0000000000000000000000',1)"
        "RETURNING id")
uid2 = psql("SELECT id FROM users WHERE username='e2e_promo_u'") or "0"
s, r = call("POST", "/auth/login", {"username": "e2e_promo_u", "password": "x"})
# 该用户无法登录（hash 是 x）——改用 root 的 token 无法模拟他人；权限分支用 SQL 断言替代：
# promo_buy 的护栏是 owner != auth.id && class_id < 90 → 403。此处验证 root（staff
# 99）可购他人种：
other_tid = int(psql(
    "SELECT id FROM torrents WHERE owner_id <> 1 AND approval_status=1 ORDER"
        "BY id LIMIT 1"))
if other_tid:
    idem4 = idem + "-other"
    s, r = call("POST", "/promo/buy", {"torrent_id": other_tid,
        "kind": "sticky2", "hours": 24, "idempotency_key": idem4}, token=tok)
    check("staff 可代购他人种子", r.get("code") == 0, f"code={r.get('code')}")
    psql(
        "UPDATE torrents SET pos_state=0, pos_state_until=NULL WHERE"
            "id={other_tid}")
    psql(f"DELETE FROM promo_purchases WHERE idempotency_key='{idem4}'")
    psql(f"DELETE FROM spark_ledger WHERE idempotency_key='{idem4}'")

# ---------- 环境还原 ----------
psql(f"UPDATE torrents SET pos_state=0, pos_state_until=NULL WHERE id={tid}")
psql(
    "DELETE FROM promotions WHERE torrent_id={tid} AND starts_at > now() -"
        "interval '10 minutes'")
psql(f"DELETE FROM promo_purchases WHERE idempotency_key LIKE '{idem}%'")
psql(f"DELETE FROM spark_ledger WHERE idempotency_key LIKE '{idem}%'")
psql(f"DELETE FROM users WHERE username='e2e_promo_u'")
psql("UPDATE users SET spark_balance=100000 WHERE id=1")
left = psql(
    "SELECT count(*) FROM promo_purchases WHERE idempotency_key LIKE '{idem}%'")
check("环境还原", left == "0")

print()
fails = [x for x in results if not x[1]]
print(f"===== 自购置顶/免费验证：{len(results) - len(fails)}/{len(results)} PASS =====")
sys.exit(1 if fails else 0)
