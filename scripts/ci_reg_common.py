# -*- coding: utf-8 -*-
"""注册治理 CI 端到端的公共层（被 ci_reg_gates_e2e / ci_reg_quiz_e2e 引用）。

环境适配：
  CI：`DATABASE_URL` + `JWT_SECRET` 环境变量 + 无密码 `redis-cli`；
  本地：回落 `docker exec flux-postgres / flux-api / flux-redis`
  （本地 redis 有密码，从容器 printenv 取）。

两个硬约定（都是踩过的坑）：
  1. SQL 一律经 stdin 喂——`psql -c "..."` 会被 shell 拆词，reg_window
     的 JSON 值因此报 unterminated quoted string；
  2. JWT 的守卫按 sub **查库取档**（不看 claim 里的 class_id），所以
     staff token 必须签在真实 class>=90 的 uid 上，不能想当然 sub=1。
"""
import base64
import hashlib
import hmac
import json
import os
import re
import subprocess
import time
import urllib.error
import urllib.request

BASE = os.environ.get("FLUX_API", "http://127.0.0.1:8080")
CI = bool(os.environ.get("DATABASE_URL"))
REDIS_PW = ""
FAILED = []


def sh(*args: str) -> str:
    try:
        r = subprocess.run(args, capture_output=True, text=True)
    except OSError:                 # 工具缺失（如 redis-cli 未装）→ 视为空
        return ""
    return r.stdout.strip()


def psql(sql: str) -> str:
    if CI:
        cmd = ["psql", os.environ["DATABASE_URL"], "-t", "-A", "-q"]
    else:
        cmd = ["docker", "exec", "-i", "flux-postgres", "psql",
               "-U", "flux", "-d", "fluxtorrent", "-t", "-A", "-q"]
    try:
        r = subprocess.run(cmd, input=sql, capture_output=True, text=True)
    except OSError as e:
        print(f"    ! psql 不可用：{e}")
        return ""
    if r.returncode != 0:
        print(f"    ! psql 失败：{r.stderr.strip()[:160]}")
    return "\n".join(r.stdout.splitlines()).strip()


def redis_cli(*args: str) -> str:
    """redis 命令：CI 无密码直连；本地经容器并带上密码。"""
    if CI:
        return sh("redis-cli", *args)
    global REDIS_PW
    if not REDIS_PW:
        REDIS_PW = sh("docker", "exec", "flux-redis",
                      "printenv", "REDIS_PASSWORD")
    return sh("docker", "exec", "flux-redis", "redis-cli", "-a", REDIS_PW,
              "--no-auth-warning", *args)


def clear_throttle(patterns=("rl:register-ip:*", "rl:quiz:*", "rl:pow:*")):
    for pat in patterns:
        keys = redis_cli("--scan", "--pattern", pat).split()
        if keys:
            redis_cli("DEL", *keys)


def secret() -> str:
    s = os.environ.get("JWT_SECRET", "")
    if not s:
        s = sh("docker", "exec", "flux-api", "printenv", "JWT_SECRET")
    return s


def b64url(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


def jwt(uid: int, cls: int = 99, ttl: int = 3600) -> str:
    now = int(time.time())
    head = b64url(json.dumps({"alg": "HS256", "typ": "JWT"}).encode())
    body = b64url(json.dumps(
        {"sub": uid, "class_id": cls, "iat": now, "exp": now + ttl}
    ).encode())
    sig = hmac.new(secret().encode(), f"{head}.{body}".encode(),
                   hashlib.sha256).digest()
    return f"{head}.{body}.{b64url(sig)}"


def staff_uid() -> int:
    uid = psql("SELECT id FROM users WHERE class_id >= 90 "
               "ORDER BY id LIMIT 1")
    return int(uid or 0)


def check(name: str, cond: bool, extra: str = "") -> None:
    print(f"  {'✓' if cond else '✗'} {name}" + (f" — {extra}" if extra else ""))
    if not cond:
        FAILED.append(name)


def call(method: str, path: str, tok=None, body=None):
    req = urllib.request.Request(BASE + path, method=method)
    if tok:
        req.add_header("Authorization", f"Bearer {tok}")
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, data) as r:
            return r.status, json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read() or b"{}")
        except Exception:
            return e.code, {}


def data_of(r) -> dict:
    return (r or {}).get("data") or {}


def msg(r) -> str:
    return str((r or {}).get("message", ""))[:44]


def set_setting(name: str, value: str) -> None:
    psql("INSERT INTO site_settings (name, value) VALUES "
         f"('{name}', '{value}') ON CONFLICT (name) DO UPDATE "
         "SET value = EXCLUDED.value")


def captcha():
    st, r = call("GET", "/api/v1/auth/captcha", None)
    d = data_of(r)
    m = re.match(r"\s*(\d+)\s*\+\s*(\d+)", str(d.get("question", "")))
    cid = str(d.get("captcha_id", ""))
    ans = (int(m.group(1)) + int(m.group(2))) if m else 0
    return cid, ans


def register(name: str, email: str, extra=None):
    cid, ans = captcha()
    payload = {"username": name, "email": email,
               "password": "Passw0rd!ci", "invite_code": "",
               "captcha_id": cid, "captcha_answer": ans, "fields": {}}
    payload.update(extra or {})
    return call("POST", "/api/v1/auth/register", None, payload)


def meets(hexs: str, bits: int) -> bool:
    """与 api pow.rs::meets_difficulty 同口径（前导零 bit 数）。"""
    if bits == 0 or bits > len(hexs) * 4:
        return False
    full = bits // 4
    if any(c != "0" for c in hexs[:full]):
        return False
    rem = bits % 4
    if rem == 0:
        return True
    return int(hexs[full], 16) <= (0xF >> rem)


def solve_pow(challenge: str, bits: int, limit=8_000_000) -> str:
    n = 0
    while n < limit:
        s = str(n)
        h = hashlib.sha256(f"{challenge}:{s}".encode()).hexdigest()
        if meets(h, bits):
            return s
        n += 1
    return ""


def backup_settings(*names: str) -> dict:
    return {n: psql(f"SELECT value FROM site_settings WHERE name='{n}'")
            for n in names}


def restore_settings(saved: dict) -> None:
    for n, v in saved.items():
        set_setting(n, v)


def tombstone(mark: str) -> None:
    """测试账号按产品口径墓碑化（audit_log 0266 哈希链禁改删 + FK 无
    ON DELETE，硬删必炸）；连带清掉注册摘要与私信/字段值。"""
    psql(
        "DELETE FROM reg_ip_signups WHERE user_id IN "
        f"(SELECT id FROM users WHERE username LIKE '{mark}%'); "
        f"DELETE FROM messages WHERE receiver_id IN "
        f"(SELECT id FROM users WHERE username LIKE '{mark}%') OR "
        f"sender_id IN (SELECT id FROM users WHERE username LIKE "
        f"'{mark}%'); "
        f"DELETE FROM user_field_values WHERE user_id IN "
        f"(SELECT id FROM users WHERE username LIKE '{mark}%'); "
        "UPDATE users SET username = 'zzdel_' || id, "
        "email = 'zzdel_' || id || '@invalid.test', status = 2 "
        f"WHERE username LIKE '{mark}%';"
    )


def leftover(mark: str) -> str:
    return psql(f"SELECT count(*) FROM users WHERE username LIKE '{mark}%'")


def report() -> int:
    print()
    if FAILED:
        print(f"✗ {len(FAILED)} 项失败：")
        for f in FAILED:
            print(f"   - {f}")
        return 1
    print("✓ 全部通过")
    return 0


def nowmark() -> str:
    return str(int(time.time()))
