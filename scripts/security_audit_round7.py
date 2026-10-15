# -*- coding: utf-8 -*-
"""第七轮审查：安全与越权专项探测（修正版）。

用 root 通过 /admin/adduser 创建一次性普通用户（class 0），以其身份探测：
管理面越权、JWT 篡改提权、SQL 注入探针、伪造头、IDOR。全部可恢复。
"""
import base64
import json
import urllib.parse
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080/api/v1"
results = []

def call(method, path, body=None, token=None, headers=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    for k, v in (headers or {}).items():
        req.add_header(k, v)
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=10) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}
    except Exception as e:
        return -1, {"err": str(e)}

def check(name, cond, detail=""):
    results.append((name, cond))
    print(("PASS " if cond else "FAIL ") + name + (
        f"  {detail}" if detail else ""))

# --- 前置 ---
st, r = call("POST", "/auth/login", {"username": "root",
    "password": "password123"})
check("前置·root登录", st == 200 and r.get("data", {}).get("token"))
root_tok = r["data"]["token"]

SEC_USER, SEC_PW = "qasec7", "Qasec7Pass123!"
st, r = call("POST", "/admin/adduser", {"username": SEC_USER,
    "email": "qasec7@test.local", "password": SEC_PW}, token=root_tok)
# 已存在（上次探测建的）也算就绪
check("前置·root创建普通用户", st == 200 or st == 400, f"got {st}")
st, r = call("POST", "/auth/login", {"username": SEC_USER, "password": SEC_PW})
check("前置·普通用户登录", st == 200, f"got {st}")
user_tok = r["data"]["token"] if st == 200 else None

ADMIN_GET = [
    "/admin/overview", "/admin/users", "/admin/claims?state=active",
        "/admin/settings/schema",
    "/admin/audit?limit=5", "/admin/appeals", "/admin/torrents",
        "/admin/staffpanel",
    "/admin/reports", "/admin/cheaters", "/admin/dbstats",
        "/admin/syslog?limit=5",
    "/admin/login-logs?limit=5", "/admin/spark-logs?limit=5",
]
ADMIN_POST = [
    ("/admin/users/adjust", "POST", {"user_id": 2, "spark_delta": 1}),
    ("/admin/settings/groups", "PUT", {"values": {"maxseedsize": "123456"}}),
    ("/admin/claims/release", "POST", {"torrent_id": 1, "reason": "sec-probe"}),
    ("/admin/freeleech", "POST", {"kind": "free", "hours": 1}),
    ("/admin/resetpass", "POST", {"user_id": 2}),
    ("/admin/hr/pardon", "POST", {"torrent_id": 1, "user_id": 2,
        "note": "sec-probe"}),
]

# --- 1. 无认证 ---
for p in ADMIN_GET:
    st, _ = call("GET", p)
    check(f"无认证·GET {p.split('?')[0]} 拒绝", st in (401, 403), f"got {st}")
for p, m, b in ADMIN_POST:
    st, _ = call(m, p, b)
    check(f"无认证·{m} {p} 拒绝", st in (401, 403), f"got {st}")

# --- 2. 普通用户越权 ---
if user_tok:
    for p in ADMIN_GET:
        st, _ = call("GET", p, token=user_tok)
        check(f"普通用户·GET {p.split('?')[0]} 拒绝", st in (401, 403), f"got {st}")
    for p, m, b in ADMIN_POST:
        st, _ = call(m, p, b, token=user_tok)
        check(f"普通用户·{m} {p} 拒绝", st in (401, 403), f"got {st}")

    st, _ = call("GET", "/admin/overview", token=user_tok,
                 headers={"X-Role": "sysop", "X-User-Id": "1",
                     "X-Forwarded-For": "127.0.0.1"})
    check("伪造头·X-Role/X-User-Id 不提权", st in (401, 403), f"got {st}")

    # IDOR：普通用户调管理面用户详情 / 编辑别人种子
    st, _ = call("GET", "/admin/users/2/detail", token=user_tok)
    check("IDOR·管理面用户详情拒绝", st in (401, 403, 404), f"got {st}")

    # 普通用户冒充编辑 root 发布的种子（torrent 1 owner 是 root）
    st, _ = call("PUT", "/torrents/1", {"title": "sec-probe"}, token=user_tok)
    check("越权·普通用户改他人种子被拒", st in (401, 403, 404), f"got {st}")
    st, _ = call("DELETE", "/torrents/1", token=user_tok)
    check("越权·普通用户删他人种子被拒", st in (401, 403, 404), f"got {st}")
else:
    print("SKIP 普通用户探测")

# --- 3. JWT 篡改 ---
def b64u(b): return base64.urlsafe_b64encode(b).rstrip(b"=").decode()

parts = root_tok.split(".")
none_tok = b64u(json.dumps({"alg": "none", "typ": "JWT"}).encode(
    )) + "." +            b64u(json.dumps({"sub": "1", "class_id": 99,
        "iat": 9999999999, "exp": 9999999999}).encode()) + "."
st, _ = call("GET", "/me", token=none_tok)
check("JWT·alg=none 被拒", st in (401, 403), f"got {st}")

bad_sig = parts[0] + "." + parts[1] + "." + b64u(b"deadbeef")
st, _ = call("GET", "/me", token=bad_sig)
check("JWT·坏签名被拒", st in (401, 403), f"got {st}")

try:
    pad = parts[1] + "=" * (-len(parts[1]) % 4)
    payload = json.loads(base64.urlsafe_b64decode(pad))
    payload["class_id"] = 99
    forged = parts[0] + "." + b64u(json.dumps(payload).encode()) + "." + parts[
        2]
    st, _ = call("GET", "/admin/overview", token=forged)
    check("JWT·改payload提权被拒", st in (401, 403), f"got {st}")
except Exception as e:
    check("JWT·改payload提权被拒", False, str(e))

# 过期令牌（iat/exp 很老 + 用当前有效签名不可伪造，直接构造 exp=1 的 none 变体走签名校验失败路径即可）
# → 已由 alg=none / 坏签名覆盖。

# --- 4. SQL 注入探针（参数化查询应返回 200/400 而非 500 + SQL 错误泄漏） ---
probes = [
    ("/torrents?search=" + urllib.parse.quote("' OR '1'='1"), root_tok),
    ("/torrents?search=" + urllib.parse.quote("'; DROP TABLE users; --"),
        root_tok),
    ("/torrents?search=" + urllib.parse.quote(
        "1' UNION SELECT pass_hash FROM users--"), root_tok),
    ("/forums?search=" + urllib.parse.quote("1 UNION SELECT 1"), root_tok),
    ("/admin/claims?search=" + urllib.parse.quote("' OR 1=1"), root_tok),
    ("/admin/users?search=" + urllib.parse.quote("'; --"), root_tok),
]
for p, tok in probes:
    st, r = call("GET", p, token=tok)
    blob = json.dumps(r).lower()
    leaked = ("sqlstate" in blob) or ("syntax error" in blob) or (
        "db error" in blob) or ("postgres" in blob and st >= 500)
    check(f"注入·{urllib.parse.unquote(p)[:60]} 安全", st in (200, 400,
        422) and not leaked, f"status={st}")

# --- 4b. 种子内容未鉴权外泄（修复验证）：无 token 直读详情扩展/文件/感谢/评论 ---
for p in ["/torrents/1/detail", "/torrents/1/files", "/torrents/1/thanks",
    "/torrents/1/comments", "/torrents/1"]:
    st, _ = call("GET", p)
    check(f"未鉴权·GET {p} 拒绝", st in (401, 403), f"got {st}")

# --- 5. 登录爆破限流：throttle 先行 INCR，超限表现为 HTTP 400 + code=1015 ---
codes = []
for i in range(8):
    st, r = call("POST", "/auth/login", {"username": "ratelimit_probe_u",
        "password": "wrong"})
    codes.append((st, r.get("code")))
    if r.get("code") == 1015:
        break
check("限流·登录爆破触发1015", any(c == 1015 for _, c in codes), f"codes={codes}")

# --- 清理：等限流窗口过掉再验证 root 仍可登录（说明限流是按 IP+用户名 的窗口，不是封号） ---
# （不阻塞，仅提示）
fails = [n for n, c in results if not c]
print(f"\n===== 安全专项：{len(results)-len(fails)}/{len(results)} PASS =====")
if fails:
    print("FAILED:")
    for n in fails:
        print("  - " + n)
