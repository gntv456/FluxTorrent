# -*- coding: utf-8 -*-
"""CI 用：反作弊运营层 + 注册治理 M1~M5 端点冒烟（无外部依赖）。

前置：api(8080) 已在本机起好、迁移已跑完；`JWT_SECRET` 已设；能连库
（CI 走 `DATABASE_URL`，本地走 `docker exec flux-postgres psql`）。

四组断言：
  A 匿名拒绝矩阵：staff/user 端点匿名必须 401/403/404/410——任何 200 都
    意味着守卫缺失（M1~M5 新增 7 个端点里漏挂 `staff()` 就是这么藏的）；
  B staff 面板可达：持 staff JWT → 200 且响应结构就位（防「路由活着但
    查询炸了」——sqlx 是运行时查询，编译绿不代表能跑）；
  C 公开端点：`/register-mode`、`/auth/quiz-challenge`、
    `/auth/pow-challenge?scope=register`；
  D 防泄题：抽题响应**不得**出现 `answer`——注册侧只能拿到选项，
    答案下标仅管理端可见（M5 设计红线）。

退出码非 0 = 失败，CI 直接红。
"""
import base64
import hashlib
import hmac
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

API = os.environ.get("FLUX_API", "http://127.0.0.1:8080") + "/api/v1"
failures = []


def check(name, cond, detail=""):
    print(("PASS " if cond else "FAIL ") + name + (
        f"  {detail}" if detail else ""))
    if not cond:
        failures.append(name)


# ---------- 库访问（取一个 staff uid：守卫按 sub 查库取档，不看 claim）----

def _sql(query):
    url = os.environ.get("DATABASE_URL")
    if url:
        cmd = ["psql", url, "-t", "-A", "-c", query]
    else:
        cmd = ["docker", "exec", "flux-postgres", "psql",
               "-U", "flux", "-d", "fluxtorrent", "-t", "-A", "-c", query]
    try:
        out = subprocess.run(cmd, capture_output=True, text=True,
                             timeout=20, encoding="utf-8", errors="replace")
    except Exception as e:                      # noqa: BLE001
        return None, str(e)[:120]
    if out.returncode != 0:
        return None, (out.stderr or "").strip()[:120]
    return out.stdout.strip(), ""


def staff_uid():
    uid, err = _sql(
        "SELECT id FROM users WHERE class_id >= 90 ORDER BY id LIMIT 1")
    if err or not uid:
        return None, err or "库里没有 class>=90 的用户"
    return int(uid), ""


# ---------- JWT（与 apps/api 的 require_auth 同口径：HS256 + JWT_SECRET）--

def b64url(raw):
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


def jwt(uid, cls=99, ttl=1800):
    secret = os.environ.get("JWT_SECRET", "")
    if not secret:
        r = subprocess.run(
            ["docker", "exec", "flux-api", "printenv", "JWT_SECRET"],
            capture_output=True, text=True, encoding="utf-8", errors="replace")
        secret = r.stdout.strip()
    now = int(time.time())
    head = b64url(json.dumps({"alg": "HS256", "typ": "JWT"}).encode())
    body = b64url(json.dumps(
        {"sub": uid, "class_id": cls, "iat": now, "exp": now + ttl}
    ).encode())
    sig = hmac.new(secret.encode(), f"{head}.{body}".encode(),
                   hashlib.sha256).digest()
    return f"{head}.{body}.{b64url(sig)}"


# ---------- HTTP（4xx/5xx 不当异常抛，返回码与体）------------------------

def http(method, path, token="", body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(API + path, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req, timeout=15) as r:
            return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")
    except Exception as e:                      # noqa: BLE001
        return 0, str(e)[:160]


def envelope(raw):
    """取信封 data；解析失败返回 None。"""
    try:
        return json.loads(raw).get("data")
    except Exception:                           # noqa: BLE001
        return None


# ---------- A 匿名拒绝矩阵 -----------------------------------------------

ANON = [("GET", "/admin/cheat-cases"), ("GET", "/admin/risk-scores"),
        ("GET", "/admin/cheat-appeals"), ("GET", "/admin/cheat-accuracy"),
        ("GET", "/admin/email-domains"), ("GET", "/admin/reg-quiz"),
        ("GET", "/my/cheat-appeals"), ("POST", "/crossseed/lookup")]

for method, path in ANON:
    code, _ = http(method, path, body={} if method == "POST" else None)
    ok = code in (401, 403, 404, 410)
    check(f"匿名拒绝 {method} {path}", ok,
          f"{code}（应 401/403/404/410，200 = 守卫缺失）")


# ---------- B staff 面板可达 --------------------------------------------

uid, err = staff_uid()
if uid is None:
    check("解析 staff uid", False, err)
    TOKEN = ""
else:
    check("解析 staff uid", True, f"uid={uid}")
    TOKEN = jwt(uid)

# (path, 期望 data 顶层键；None = data 是裸数组)
PANELS = [("/admin/cheat-cases", ("items",)),
          ("/admin/risk-scores", ("items",)),
          ("/admin/cheat-appeals", None),
          ("/admin/cheat-accuracy?days=30", ("families",)),
          ("/admin/email-domains?limit=1", ("items", "total")),
          ("/admin/reg-quiz?limit=1", ("items",))]

if TOKEN:
    for path, keys in PANELS:
        code, raw = http("GET", path, token=TOKEN)
        data = envelope(raw)
        if keys is None:
            ok = code == 200 and isinstance(data, list)
        else:
            ok = (code == 200 and isinstance(data, dict)
                  and all(k in data for k in keys))
        check(f"面板 GET {path}", ok, f"{code} keys={keys}")

    # 内置数据落库（迁移回填）：域名库 445 条、题库 14 题
    _, raw = http("GET", "/admin/email-domains?limit=1", token=TOKEN)
    total = (envelope(raw) or {}).get("total")
    check("域名库内置回填", isinstance(total, int) and total >= 400,
          f"total={total}")
    _, raw = http("GET", "/admin/reg-quiz?limit=100", token=TOKEN)
    rows = ((envelope(raw) or {}).get("items") or [])
    check("题库内置回填", len(rows) >= 14, f"{len(rows)} 题")
    check("管理端可见答案下标",
          all("answer_index" in r for r in rows) if rows else False)


# ---------- C 公开端点 ---------------------------------------------------

code, raw = http("GET", "/register-mode")
data = envelope(raw) or {}
need = ("mode", "reg_quiz_enabled", "reg_quiz_pass_required")
check("GET /register-mode 四开关",
      code == 200 and all(k in data for k in need), f"{code} keys={need}")

code, raw = http("GET", "/auth/quiz-challenge")
q = envelope(raw) or {}
check("GET /auth/quiz-challenge",
      code == 200 and all(k in q for k in ("ticket", "question", "options")),
      f"{code}")

code, _ = http("GET", "/auth/pow-challenge?scope=register")
check("GET /auth/pow-challenge?scope=register", code == 200, f"{code}")

code, _ = http("GET", "/auth/pow-challenge")
check("PoW 缺 scope 拒绝", 400 <= code < 500, f"{code}")


# ---------- D 防泄题（M5 红线）------------------------------------------

_, raw = http("GET", "/auth/quiz-challenge")
low = raw.lower()
check("抽题响应不泄答案",
      "answer" not in low and "explanation" not in low,
      "命中 answer/explanation 即泄题")

# 公开申诉入口：缺 token = 400 校验失败（不是 401——token 本身即凭证）
code, raw = http("POST", "/appeals/token", body={})
check("POST /appeals/token 缺字段=4xx",
      400 <= code < 500 and code != 401, f"{code}")
code, _ = http("POST", "/cheat-appeals", body={})
check("POST /cheat-appeals 缺字段=4xx", 400 <= code < 500 and code != 401,
      f"{code}")


print()
if failures:
    print(f"SMOKE FAILED：{len(failures)} 项")
    for f in failures:
        print("  - " + f)
    sys.exit(1)
print("M-OPS SMOKE OK")
