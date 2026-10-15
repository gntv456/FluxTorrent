# -*- coding: utf-8 -*-
"""资深 PT 深测公共层：登录/调用/断言输出"""
import os
import urllib.request, urllib.error, json, sys, time

# Windows 控制台默认 GBK：凡是打印到 GBK 编不出的字符（⇒ × ≥ → 之外的
# 那类）都会抛 UnicodeEncodeError 并**当场中断**整个闸门——后面所有判据
# 一条都不执行，FAIL 计数自然少报，看上去像「跑过了」。
# 批一百四十一实测：EP 跑到 EP2a 就断在 ⇒ 上，EP3~EP9 六段判据根本没跑。
# 统一把输出重配成 UTF-8（真编不出的字符用 replace 兜底），保证「红」能打完。
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except Exception:  # 老版本 Python / 已被包装过的流：能跑就好
        pass

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

def call_h(method, path, body=None, token=None):
    """和 call() 同源，但把响应头也带回来。

    分页类判据要看 `X-Total-Count` / `X-Has-More`——那些信息刻意不进 body
    （消息中心三处前端把 data 当数组消费），所以判据只能从响应头取。
    返回 (status, payload_dict, headers_dict)。
    """
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        r = urllib.request.urlopen(req, data, timeout=30)
        status, payload, hdrs = r.status, r.read(), dict(r.headers)
    except urllib.error.HTTPError as e:
        status, payload, hdrs = e.code, e.read(), dict(e.headers)
    try:
        return status, json.loads(payload), hdrs
    except Exception:  # noqa: BLE001
        return status, {"raw": payload[:300].decode("utf-8", "replace")}, hdrs


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

def dig(resp, *keys):
    """Dig a nested key out of an API envelope without exploding on null.

    `X.get("data", {}).get(K)` only defaults when the KEY IS MISSING: a
    response of {"data": null} (rejected registrations do that) makes it return
    None and the NEXT .get raises AttributeError -- the gate dies before its main
    assertions while the finally-block cleanup still prints PASS, i.e. a run that
    looks green but measured nothing (batch 143, gates EB/EC). Callers write
    `dig(...) or []`.
    """
    cur = resp
    for k in keys:
        if not isinstance(cur, dict):
            return None
        cur = cur.get(k)
    return cur


def summary():
    print()
    if FAILURES:
        print("== FAILURES (%d): %s" % (len(FAILURES), ", ".join(FAILURES)))
        sys.exit(1)
    print("== ALL PASS")

def clear_register_rl():
    """闸门侧清掉**自己造出来的**注册额度占用。

    批一百五十三实测：这里原来只清 Redis 的 `rl:register-ip:*`，可 24 小时
    同 IP 限额根本不查 Redis——`register_gates::check_ip_limit` 数的是
    `reg_ip_signups` 表（sha3(ip) 摘要，窗口默认 24h，默认上限 2）。
    于是清 Redis 对它毫无作用：只要当天注册过 2 个探针号，第三支闸门
    建号必得 `同一网络环境 24 小时内最多注册 2 个账号`，`new_member`
    返回 None，下游全红在一片「未认证」上——红在前置而不是被测口径，
    极易误读成产品回归（#75 记的就是这件事，但当时归因成 Redis 频控）。

    所以要清两个面：Redis 的分钟级计数 + reg_ip_signups 的窗口计数。
    **只删自己造的探针号**（用户名/邮箱后缀匹配），不动 root 与真人账号，
    也不会削弱产品侧的防刷阈值——24h/2 是正当口径，产品侧一行不改。
    """
    import subprocess

    def psql(sql):
        return subprocess.run(
            ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
             "-d", "fluxtorrent", "-tAc", sql],
            capture_output=True, text=True, encoding="utf-8",
            errors="replace")

    # ① 分钟级 Redis 计数（登录/发信限流**不动**）
    cmd = ('redis-cli -a "$REDIS_PASSWORD" --no-auth-warning --scan '
           '--pattern "rl:register-ip:*" '
           '| xargs -r redis-cli -a "$REDIS_PASSWORD" --no-auth-warning del')
    subprocess.run(["docker", "exec", "flux-redis", "sh", "-c", cmd],
                   capture_output=True, text=True, encoding="utf-8")
    # ② 24h 窗口计数：只回收探针号留下的痕迹
    # 邮箱后缀各闸门不统一（aj/fc/as/bb/...），所以按「本地保留域」兜住：
    # *.local 不是可路由域，只可能是探针号；真人账号不会落在这里。
    # 探针邮箱域名实测有四类：<gate>.local（本仓闸门主流）、@localhost、
    # @audit.invalid（b_billing）、@example.com（若干闸门）。前两类靠保留域
    # 判定，后两类**不是**保留域，只能按「本仓闸门造号前缀」认——
    # 实测漏掉 audit.invalid 时 clear_register_rl() 静默空转（判据删除 0 行
    # 却不报错），b_billing 于是仍红在「两个测试号可用」。
    probe = (
        "SELECT id FROM users WHERE "
        "email LIKE '%%@%%.local' OR email LIKE '%%@localhost' "
        "OR email LIKE '%%@audit.invalid' OR email LIKE '%%@example.com' "
        "OR username LIKE 'deleted-%%'")
    out = psql("SELECT count(*) FROM reg_ip_signups WHERE user_id IN (%s)" % probe)
    if (out.stdout or "").strip() not in ("", "0"):
        psql("DELETE FROM reg_ip_signups WHERE user_id IN (%s)" % probe)


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
