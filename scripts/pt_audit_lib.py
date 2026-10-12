# -*- coding: utf-8 -*-
"""资深 PT 深测公共层：登录/调用/断言输出"""
import os
import urllib.request, urllib.error, json, sys, time

# FLUX_API_BASE：打侧容器（宿主 :8080 常被别的进程占着，直连会拿到
# 别人的响应，判成败全错）。不设则沿用主栈。
BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")

def call(method, path, body=None, token=None, raw=False):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        r = urllib.request.urlopen(req, data, timeout=30)
        payload = r.read()
        if raw:
            return r.status, payload
        return r.status, json.loads(payload)
    except urllib.error.HTTPError as e:
        payload = e.read()
        if raw:
            return e.code, payload
        try:
            return e.code, json.loads(payload)
        except Exception:
            return e.code, {"raw": payload[:300].decode("utf-8", "replace")}

def login(username="root", pw="password123", tries=8):
    """带退避的登录。

    dev 实例上 `/auth/login` 会限流：几套闸门连着跑（每套要登好几次）之后
    直接返回无 token。后果比失败本身严重——放在 finally 里的兜底复原也要登录，
    于是兜底一起失败，脏数据留在原地。所以退避收在这一层，各闸门不必各自实现。
    """
    last = None
    for i in range(tries):
        s, r = call("POST", "/auth/login", {"username": username, "password": pw})
        if s == 200 and r.get("code") == 0:
            return r["data"]["token"]
        last = (s, r)
        if s != 429:
            break
        time.sleep(min(2 + i * 3, 20))
    raise SystemExit(
        "login failed for %s: %s %s"
        % (username, last[0], last[1].get("message"))
    )

def ok(name, cond, detail=""):
    mark = "PASS" if cond else "FAIL"
    print("[%s] %s %s" % (mark, name, ("- " + str(detail)[:220]) if (detail and not cond) else ""))
    if not cond:
        FAILURES.append(name)
    return cond

FAILURES = []

def summary():
    print()
    if FAILURES:
        print("== FAILURES (%d): %s" % (len(FAILURES), ", ".join(FAILURES)))
        sys.exit(1)
    print("== ALL PASS")

def clear_register_rl():
    """闸门侧清掉注册限流键（rl:register-ip:<IP>，Redis 计数 + 60s TTL）。

    几套闸门连着跑时各自要注册 1~4 个成员，额度（5/分钟/IP）会互相吃光，
    表现是 `new_member` 返回无 token、下游一片「未认证」。与其每人睡 60 秒，
    不如清掉自己造的计数——只碰 register-ip 这一个面，登录/发信限流照旧生效。
    """
    import subprocess
    cmd = ('redis-cli -a "$REDIS_PASSWORD" --no-auth-warning --scan '
           ' --pattern "rl:register-ip:*" '
           '| xargs -r redis-cli -a "$REDIS_PASSWORD" --no-auth-warning del')
    subprocess.run(["docker", "exec", "flux-redis", "sh", "-c", cmd],
                   capture_output=True, text=True, encoding="utf-8")


def totp_now(secret_b32, skew=0):
    """按 RFC 6238 算 TOTP 码，与服务端 `twofa_http::totp_at` 同口径
    （HMAC-SHA1、30 秒窗口、动态截断、%1e6）。闸门要「真开 2FA」时才用它：
    `/me/2fa/setup` 回里就带 base32 密钥，返回 int 可直接进 JSON 的 code。
    `skew` 用来取相邻窗口的码（服务端容差 ±1 窗）。"""
    import base64
    import hashlib
    import hmac
    import time
    raw = base64.b32decode(secret_b32.upper().replace(" ", "").rstrip("="))
    step = int(time.time()) // 30 + skew
    digest = hmac.new(raw, step.to_bytes(8, "big"), hashlib.sha1).digest()
    off = digest[19] & 0x0F
    code = int.from_bytes(digest[off:off + 4], "big") & 0x7FFF_FFFF
    return code % 1_000_000
