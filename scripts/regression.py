"""阶段全功能复查脚本：对运行中的本地环境做端到端回归。

覆盖：认证（登录/登出撤销/我）、种子（列表/详情/评论）、经济（火花/流水/商店/银行/签到/站免池/装扮）、
社区（论坛/短讯/好友/勋章）、玩法（刮刮乐/猜大小/九宫格/农场全链路）、运营（排行/课本/保种/求种）、PWA 资产。
输出每项 PASS/FAIL 与总结论。
"""

import json
import subprocess
import sys
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080/api/v1"
WEB = "http://localhost:3000"

results = []


def call(method, path, body=None, token=None, base=BASE, token_header=None):
    req = urllib.request.Request(base + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    if token_header:
        req.add_header("Authorization", token_header)
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=10) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}


def http_code(url):
    try:
        with urllib.request.urlopen(url, timeout=15) as r:
            return r.status
    except urllib.error.HTTPError as e:
        return e.code
    except Exception:
        return 0


def check(name, ok, detail=""):
    results.append((name, ok, detail))
    print(f"{'PASS' if ok else 'FAIL'} {name} {detail}")


def psql(sql):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent", "-t", "-c", sql],
        capture_output=True, text=True,
    )
    return p.stdout.strip()


# ============ 1. 认证 ============
s, r = call("POST", "/auth/login", {"username": "root", "password": "password123"})
tok = (r.get("data") or {}).get("token")
check("认证·登录", s == 200 and r["code"] == 0 and bool(tok))

s, r = call("GET", "/me", token=tok)
check("认证·我的信息", r.get("code") == 0 and r["data"]["username"] == "root")

import time as _t
_t.sleep(2)  # 确保 nbf 时间戳严格大于 token 的 iat（同秒内 iat==nbf 不算撤销）
s, r = call("POST", "/auth/logout", {}, token=tok)
check("认证·登出", r.get("code") == 0)
s, r = call("GET", "/me", token=tok)
check("认证·旧token已撤销", r.get("code") == 2001)
s, r = call("POST", "/auth/login", {"username": "root", "password": "password123"})
if r.get("code") != 0:
    print(f"FATAL 登录失败（可能被限流）：{r}")
    sys.exit(2)
tok = r["data"]["token"]

# ============ 2. 种子 ============
s, r = call("GET", "/torrents?limit=5", token=tok)
rows = (r.get("data") or {}).get("rows") or (r.get("data") or {}).get("items") or []
check("种子·列表", r.get("code") == 0)
s, r = call("GET", "/torrents?limit=5")
check("种子·列表匿名拒绝(准入收口)", r.get("code") == 2001)
tid = rows[0]["id"] if rows else None
if tid:
    s, r = call("GET", f"/torrents/{tid}", token=tok)
    check("种子·详情", r.get("code") == 0)
    s, r = call("GET", f"/torrents/{tid}/comments", token=tok)
    check("种子·评论列表", r.get("code") == 0)

s, r = call("GET", "/stats", token=tok)
check("运营·站点统计", r.get("code") == 0)

# ============ 3. 经济 ============
# 历次运行会不断扣款（慈善捐赠/玩法），先顶满测试余额保证用例可重复执行
psql("UPDATE users SET spark_balance = 100000 WHERE username='root' AND spark_balance < 10000")
s, r = call("GET", "/me/spark", token=tok)
bal0 = (r.get("data") or {}).get("balance")
check("经济·火花余额", r.get("code") == 0 and bal0 is not None)

s, r = call("GET", "/me/spark/ledger?limit=5", token=tok)
check("经济·流水", r.get("code") == 0)

s, r = call("GET", "/shop/items")
check("经济·商店", r.get("code") == 0 and len(r["data"]) > 0)

import time as _bt
key = f"regr-buy-{bal0}-{int(_bt.time())}"  # 每次运行唯一键，保证重复运行也可复验
s, r = call("POST", "/shop/buy", {"item_id": 18, "idempotency_key": key}, token=tok)
s2, r2 = call("POST", "/shop/buy", {"item_id": 18, "idempotency_key": key}, token=tok)
bal_after = int(psql("SELECT spark_balance FROM users WHERE username='root'"))
n_ledger = int(psql(f"SELECT count(*) FROM spark_ledger WHERE idempotency_key='{key}'"))
check("经济·购买幂等", r.get("code") == 0 and r2.get("code") == 0
      and n_ledger == 1 and bal_after == bal0 - 1000,
      f"(同键两次仅扣一次 {bal0}->{bal_after}, ledger={n_ledger})")

s, r = call("POST", "/shop/buy", {"item_id": 18}, token=tok)
check("经济·无幂等键被拒", r.get("code") == 1002)

s, r = call("GET", "/attendance", token=tok)
check("经济·签到状态", r.get("code") == 0)

s, r = call("GET", "/bank/deposits", token=tok)
check("经济·银行", r.get("code") == 0)

s, r = call("GET", "/magic-pool", token=tok)
if r.get("code") != 0:
    s, r = call("GET", "/pool", token=tok)
check("经济·站免池", r.get("code") == 0)

# ============ 4. 装扮 ============
s, r = call("GET", "/dressup/list", token=tok)
items = r.get("data") or []
worn = [d for d in items if d.get("wearing")]
check("装扮·列表与佩戴态", r.get("code") == 0 and len(items) >= 4)
if items:
    owned = [d for d in items if d.get("owned")]
    if owned:
        target = owned[0]
        s, r = call("POST", "/dressup/wear", {"item_id": target["item_id"], "wear": True}, token=tok)
        check("装扮·佩戴", r.get("code") == 0)
    unowned = [d for d in items if not d.get("owned")]
    if unowned:
        s, r = call("POST", "/dressup/wear", {"item_id": unowned[0]["item_id"], "wear": True}, token=tok)
        check("装扮·未拥有被拒", r.get("code") == 1002)

# ============ 5. 社区 ============
s, r = call("GET", "/forums")
check("社区·论坛列表", r.get("code") == 0)
s, r = call("GET", "/medals")
check("社区·勋章", r.get("code") == 0)
s, r = call("GET", "/messages/inbox", token=tok)
check("社区·收件箱", r.get("code") == 0)
s, r = call("GET", "/friends", token=tok)
if r.get("code") != 0:
    s, r = call("GET", "/friend/list", token=tok)
check("社区·好友", r.get("code") == 0)

# ============ 6. 玩法 ============
s, r = call("GET", "/games")
check("玩法·总览", r.get("code") == 0)

s, r = call("POST", "/games/scratch", {"bet": 10}, token=tok)
check("玩法·刮刮乐", r.get("code") == 0)
s, r = call("POST", "/games/bigsmall", {"bet": 10, "guess": "big"}, token=tok)
check("玩法·猜大小", r.get("code") == 0)
s, r = call("POST", "/games/jgg", None, tok)
check("玩法·九宫格", r.get("code") == 0)

s, r = call("GET", "/farm", token=tok)
check("农场·总览", r.get("code") == 0 and len(r["data"].get("crops", [])) == 5)
empty_slot = next((i + 1 for i in range(6) if not any(p["slot"] == i + 1 for p in r["data"]["plots"])), None)
if empty_slot:
    s, r = call("POST", "/farm/plant", {"slot": empty_slot, "crop_id": 1}, token=tok)
    check("农场·种植", r.get("code") == 0)
    s, r = call("POST", "/farm/harvest", {"slot": empty_slot}, token=tok)
    check("农场·未熟拒收", r.get("code") == 1002)

# ============ 7. 内容/运营 ============
s, r = call("GET", "/textbooks")
check("内容·课本中心", r.get("code") == 0)
s, r = call("GET", "/preserve")
check("内容·保种区", r.get("code") == 0)
s, r = call("GET", "/requests", token=tok)
check("内容·求种", r.get("code") == 0)
s, r = call("GET", "/top/users")
check("运营·排行榜", r.get("code") == 0)

# ============ 7.5 开放 API / 插件 / 管理 ============
s, r = call("POST", "/me/tokens", {"name": "回归-开放API", "rate_per_min": 120}, token=tok)
tok_plain = (r.get("data") or {}).get("token", "")
tok_fid = (r.get("data") or {}).get("id")
check("开放API·签发token", r.get("code") == 0 and tok_plain.startswith("fxo_"))
if tok_plain:
    s, r = call("GET", "/open/recent", token_header=f"Token {tok_plain}")
    check("开放API·访问", r.get("code") == 0)
    s, r = call("GET", "/open/recent", token_header="Token fxo_invalid")
    check("开放API·假token被拒", r.get("code") == 2001)
    # 用签发时返回的 id 撤销（按名字回查会命中历史同名已撤销枚）
    tid_del = tok_fid  # 直接用签发返回的 id
    if tid_del:
        s, r = call("POST", "/me/tokens/revoke", {"id": tid_del}, token=tok)
        check("开放API·撤销", r.get("code") == 0)
    else:
        check("开放API·撤销", False, "(无未撤销的回归 token)")

s, r = call("GET", "/openapi.json")
check("开放API·文档", r.get("code") == 0 and "openapi" in json.dumps(r.get("data") or {}))

s, r = call("GET", "/admin/plugins", token=tok)
check("插件·清单(root可见)", r.get("code") == 0)

# ============ 8. 邀请 ============
s, r = call("POST", "/auth/login", {"username": "root", "password": "password123"})
rtok = (r.get("data") or {}).get("token")
check("认证·root登录", r.get("code") == 0)
s, r = call("GET", "/jixiao/types", token=rtok)
check("管理·考核类型（staff）", r.get("code") == 0)
s, r = call("GET", "/admin/overview", token=rtok)
check("管理·后台概览", r.get("code") == 0)
s, r = call("GET", "/admin/audit", token=rtok)
check("管理·审计日志", r.get("code") == 0)

# ============ 8.5 新增功能（RSS/2FA/规则/验证码） ============
import re as _re
try:
    rss = urllib.request.urlopen(f"{BASE}/rss/rootbootstrap0000000passkey00000", timeout=10).read().decode()
    check("RSS·订阅输出", rss.startswith("<?xml") and "<item>" in rss)
except Exception:
    check("RSS·订阅输出", False)

_, r = call("GET", "/rules/box")
check("规则·盒子声明", r.get("code") == 0 and len((r.get("data") or {}).get("rules") or []) == 5)

_, r = call("GET", "/auth/captcha")
q = (r.get("data") or {}).get("question", "")
m = _re.match(r"(\d+) \+ (\d+)", q)
check("验证码·签发", r.get("code") == 0 and bool(m))

_, r = call("GET", "/classes")
check("等级·规则表", r.get("code") == 0 and len(r.get("data") or []) == 6)

_, r = call("GET", "/me/class-progress", token=tok)
check("等级·我的进度", r.get("code") == 0)

_, r = call("GET", "/me/hr", token=tok)
check("H&R·我的追责状态", r.get("code") == 0)

_, r = call("GET", "/me/appeals", token=tok)
check("申诉·我的列表", r.get("code") == 0)

_, r = call("POST", "/me/2fa/setup", None, tok)
_b32 = (r.get("data") or {}).get("secret", "")
check("2FA·setup", r.get("code") == 0 and bool(_b32))

# ============ 9. Web / PWA ============
for path in ["/", "/torrents", "/games", "/farm", "/dressup", "/my", "/offline",
             "/manifest.webmanifest", "/sw.js", "/icons/icon-192.png"]:
    code = http_code(WEB + path)
    check(f"Web·{path}", code == 200, f"({code})")

# ============ 总结 ============
passed = sum(1 for _, ok, _ in results if ok)
total = len(results)
print(f"\n===== 复查总结：{passed}/{total} PASS =====")
sys.exit(0 if passed == total else 1)
