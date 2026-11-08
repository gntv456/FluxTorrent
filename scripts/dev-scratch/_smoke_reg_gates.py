#!/usr/bin/env python3
"""M4 注册治理四道闸冒烟（0370/0371）：用户总量 / 注册窗口 / 同 IP 限额 /
一次性邮箱 + 域名库导入导出 + 注册摘要清理 cron。

用法：python _smoke_reg_gates.py
前提：api+worker 已部署（含 0370/0371）；本脚本自签 staff JWT。
副作用：临时改 4 个 settings 键、新建 1 个测试账号，结束时全部还原/删除。
"""
import base64
import hashlib
import hmac
import json
import re
import subprocess
import sys
import time
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080"
ROOT = 1        # class 99
MARK = "m3reg"
NEW_DOMAIN = "m4smoke-test.example"
# 与 api auth_http/register_gates.rs::ip_hash 同算法（sha3-256 hex）
H_LOCAL = hashlib.sha3_256(b"127.0.0.1").hexdigest()


def sh(cmd: str) -> str:
    return subprocess.run(
        cmd, shell=True, capture_output=True, text=True
    ).stdout.strip()


def psql(sql: str) -> str:
    """经 stdin 喂 SQL：`-c "..."` 会把 JSON 里的双引号喂给 shell 拆词
    （reg_window 的 JSON 因此报 unterminated quoted string）。"""
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql",
         "-U", "flux", "-d", "fluxtorrent", "-t", "-A", "-q"],
        input=sql,
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        print(f"    ! psql 失败：{r.stderr.strip()[:200]}")
    return "\n".join(r.stdout.splitlines()).strip()


def secret() -> str:
    return sh("docker exec flux-api printenv JWT_SECRET")


def b64url(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


def token(uid: int, cls: int) -> str:
    now = int(time.time())
    head = b64url(json.dumps({"alg": "HS256", "typ": "JWT"}).encode())
    body = b64url(
        json.dumps(
            {"sub": uid, "class_id": cls, "iat": now, "exp": now + 3600}
        ).encode()
    )
    sig = hmac.new(
        secret().encode(), f"{head}.{body}".encode(), hashlib.sha256
    ).digest()
    return f"{head}.{body}.{b64url(sig)}"


T_ROOT = token(ROOT, 99)
FAILED = []


def check(name: str, cond: bool, extra: str = "") -> None:
    mark = "✓" if cond else "✗"
    print(f"  {mark} {name}" + (f" — {extra}" if extra else ""))
    if not cond:
        FAILED.append(name)


def call(
    method: str, path: str, tok: str | None, body: dict | None = None
) -> tuple[int, object]:
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


def msg(r: object) -> str:
    return str((r or {}).get("message", ""))[:40]


def set_setting(name: str, value: str) -> None:
    psql(
        "INSERT INTO site_settings (name, value) VALUES "
        f"('{name}', '{value}') ON CONFLICT (name) DO UPDATE "
        "SET value = EXCLUDED.value"
    )


def clear_throttle() -> None:
    pw = sh("docker exec flux-redis printenv REDIS_PASSWORD")
    keys = sh(
        "docker exec flux-redis redis-cli -a "
        f'{pw} --no-auth-warning --scan --pattern "rl:register-ip:*"'
    ).split()
    if keys:
        sh(
            f"docker exec flux-redis redis-cli -a {pw} --no-auth-warning "
            f"DEL {' '.join(keys)}"
        )


def captcha() -> tuple[str, int]:
    st, r = call("GET", "/api/v1/auth/captcha", None)
    if st != 200:
        return "", 0
    d = (r or {}).get("data") or {}
    q = str(d.get("question", ""))
    m = re.match(r"\s*(\d+)\s*\+\s*(\d+)", q)
    ans = int(m.group(1)) + int(m.group(2)) if m else 0
    return str(d.get("captcha_id", "")), ans


def register(name: str, email: str) -> tuple[int, object]:
    cid, ans = captcha()
    return call(
        "POST",
        "/api/v1/auth/register",
        None,
        {
            "username": name,
            "email": email,
            "password": "Passw0rd!m4",
            "invite_code": "",
            "captcha_id": cid,
            "captcha_answer": ans,
            "fields": {},
        },
    )


def main() -> int:
    print("== 0. 备份设置 + 清理 ==")
    saved = {
        n: psql(f"SELECT value FROM site_settings WHERE name='{n}'")
        for n in (
            "registration_mode",
            "reg_ip_limit",
            "reg_ip_window_hours",
            "reg_window",
            "maxusers",
        )
    }
    psql(
        "UPDATE users SET username = 'zzdel_' || id, "
        "email = 'zzdel_' || id || '@invalid.test', status = 2 "
        f"WHERE username LIKE '{MARK}%'; "
        "DELETE FROM reg_ip_signups WHERE user_id IN "
        f"(SELECT id FROM users WHERE username LIKE 'zzdel%'); "
        f"DELETE FROM disposable_email_domains WHERE domain = '{NEW_DOMAIN}';"
    )
    set_setting("registration_mode", "open")
    set_setting("reg_window", "")
    psql(f"DELETE FROM reg_ip_signups WHERE ip_hash = '{H_LOCAL}'")
    st, r = call("GET", "/api/v1/register-mode", None)
    print(f"  · 注册模式接口 HTTP {st}")

    print("== 1. 一次性邮箱闸 ==")
    ts = str(int(time.time()))
    psql(
        f"INSERT INTO disposable_email_domains (domain, source) VALUES "
        f"('{NEW_DOMAIN}', 'import') ON CONFLICT DO NOTHING"
    )
    clear_throttle()
    st, r = register(f"{MARK}disp{ts}", f"abc@{NEW_DOMAIN}")
    check("一次性邮箱被拒", st == 400 and "一次性" in msg(r),
          f"HTTP {st} {msg(r)}")
    # 子域后缀匹配
    clear_throttle()
    st2, r2 = register(f"{MARK}disp2{ts}", f"abc@mail.{NEW_DOMAIN}")
    check("子域命中拒绝", st2 == 400 and "一次性" in msg(r2),
          f"HTTP {st2} {msg(r2)}")

    print("== 2. 同 IP 限额闸 ==")
    set_setting("reg_ip_limit", "1")
    clear_throttle()
    st, r = register(f"{MARK}one{ts}", f"one{ts}@example.com")
    ok1 = st == 200
    uid1 = ((r or {}).get("data") or {}).get("user_id") if ok1 else None
    check("首个注册成功（限额 1）", ok1, f"HTTP {st} {msg(r)}")
    if ok1:
        row = psql(
            "SELECT length(ip_hash) || '|' || (ip_hash ~ '^[0-9a-f]{64}$')"
            "::text FROM reg_ip_signups WHERE user_id = "
            f"{uid1}"
        )
        check("摘要留痕（64 位 hex，无明文 IP）", row == "64|true", f"{row}")
    clear_throttle()
    st, r = register(f"{MARK}two{ts}", f"two{ts}@example.com")
    check("同 IP 第 2 个被拒", st == 400 and "网络环境" in msg(r),
          f"HTTP {st} {msg(r)}")

    print("== 3. 注册窗口闸 ==")
    set_setting("reg_ip_limit", "0")
    set_setting(
        "reg_window",
        '{"open":"2020-01-01T00:00:00Z","close":"2020-01-02T00:00:00Z",'
        '"slots":0,"label":"M4 冒烟旧窗口"}',
    )
    clear_throttle()
    st, r = register(f"{MARK}win1{ts}", f"w1{ts}@example.com")
    check("窗口已结束被拒", st == 400 and "结束" in msg(r),
          f"HTTP {st} {msg(r)}")
    set_setting(
        "reg_window",
        '{"open":"2099-01-01T00:00:00Z","close":"2099-01-02T00:00:00Z",'
        '"slots":0,"label":"M4 冒烟未来窗口"}',
    )
    clear_throttle()
    st, r = register(f"{MARK}win2{ts}", f"w2{ts}@example.com")
    check("窗口未开始被拒", st == 400 and "尚未开始" in msg(r),
          f"HTTP {st} {msg(r)}")
    set_setting(
        "reg_window",
        '{"open":"2020-01-01T00:00:00Z","close":"2099-01-01T00:00:00Z",'
        '"slots":1,"label":"M4 冒烟名额窗口"}',
    )
    clear_throttle()
    st, r = register(f"{MARK}win3{ts}", f"w3{ts}@example.com")
    check("名额已满被拒", st == 400 and "名额已满" in msg(r),
          f"HTTP {st} {msg(r)}")

    print("== 4. 用户总量闸（maxusers）==")
    set_setting("reg_window", "")
    used = psql("SELECT count(*) FROM users WHERE status < 2")
    set_setting("maxusers", used)
    clear_throttle()
    st, r = register(f"{MARK}cap{ts}", f"cap{ts}@example.com")
    check("达上限被拒", st == 400 and "上限" in msg(r),
          f"HTTP {st} {msg(r)}（现役 {used}）")

    print("== 5. 域名库 admin API ==")
    st, r = call(
        "GET", f"/api/v1/admin/email-domains?q=mailinator", T_ROOT
    )
    items = ((r or {}).get("data") or {}).get("items") or []
    check("列表可查（含内置库）", st == 200 and len(items) > 0,
          f"HTTP {st} n={len(items)}")
    st, r = call(
        "POST",
        "/api/v1/admin/email-domains/import",
        T_ROOT,
        {"domains": [NEW_DOMAIN, "不是域名", "x"], "text": f"y.{NEW_DOMAIN}"},
    )
    d = (r or {}).get("data") or {}
    check("导入：合法入库 / 非法点名",
          st == 200 and d.get("invalid", 0) >= 2,
          f"HTTP {st} added={d.get('added')} invalid={d.get('invalid')}")
    st, r = call(
        "GET", f"/api/v1/admin/email-domains?q={NEW_DOMAIN}", T_ROOT
    )
    items = ((r or {}).get("data") or {}).get("items") or []
    check("导入后可查（含子域行）", len(items) >= 2, f"n={len(items)}")
    st, r = call(
        "GET", "/api/v1/admin/email-domains/export", T_ROOT
    )
    doms = ((r or {}).get("data") or {}).get("domains") or []
    check("导出含内置+导入", NEW_DOMAIN in doms and "mailinator.com" in doms,
          f"count={len(doms)}")
    st, r = call(
        "POST",
        "/api/v1/admin/email-domains/delete",
        T_ROOT,
        {"domains": [NEW_DOMAIN, f"y.{NEW_DOMAIN}"]},
    )
    check("删除 2 行", st == 200 and ((r or {}).get("data") or {}).get(
        "deleted") == 2, f"HTTP {st}")

    print("== 6. 无权限拒绝 ==")
    # uid1 是第 2 步注册出来的真实普通用户（class 1）——不能用
    # token(1, 1)：守卫按 sub 查库取档，sub=1 就是站长，照样 200。
    if uid1:
        T_USER = token(uid1, 1)
        st, r = call("GET", "/api/v1/admin/email-domains", T_USER)
        check("普通用户 403", st == 403, f"HTTP {st}")
    else:
        check("普通用户 403", False, "第 2 步未拿到 uid1，跳过")

    print("== 7. 注册摘要清理 cron ==")
    psql(
        "INSERT INTO reg_ip_signups (ip_hash, user_id, created_at) VALUES "
        f"('{'0' * 64}', NULL, now() - interval '40 days')"
    )
    done = False
    for _ in range(4):
        st, r = call("POST", "/api/v1/admin/jobs/run", T_ROOT,
                     {"job": "reg_cleanup"})
        tid = ((r or {}).get("data") or {}).get("id") if st == 200 else None
        if not tid:
            continue
        for _ in range(20):
            time.sleep(4)
            s = psql(f"SELECT status FROM job_triggers WHERE id = {tid}")
            if s == "done":
                done = True
                break
            if s == "failed":
                break
        if done:
            break
    left = psql(
        f"SELECT count(*) FROM reg_ip_signups WHERE ip_hash = '{'0' * 64}'"
    )
    check("40 天前摘要被清理", done and left == "0",
          f"trigger={done} 残留={left}")

    print("== 8. 还原设置 + 清理账号 ==")
    for n, v in saved.items():
        set_setting(n, v)
    ids = psql(
        "SELECT string_agg(id::text, ',') FROM users WHERE username LIKE "
        f"'{MARK}%'"
    )
    if ids:
        psql(
            f"DELETE FROM messages WHERE receiver_id IN ({ids}) OR "
            f"sender_id IN ({ids}); "
            f"DELETE FROM user_field_values WHERE user_id IN ({ids}); "
            f"DELETE FROM reg_ip_signups WHERE user_id IN ({ids});"
        )
        # 硬删不可能：注册即写 audit_log(actor_id=自己)，而 0266 的
        # 哈希链触发器禁 UPDATE/DELETE audit_log，FK 又无 ON DELETE
        # ——产品的处置是墓碑化（user_del.rs 注释同口径）。
        psql(
            "UPDATE users SET username = 'zzdel_' || id, "
            "email = 'zzdel_' || id || '@invalid.test', status = 2 "
            f"WHERE id IN ({ids})"
        )
    psql(
        f"DELETE FROM disposable_email_domains WHERE domain = '{NEW_DOMAIN}'"
        f" OR domain = 'y.{NEW_DOMAIN}'; "
        f"DELETE FROM reg_ip_signups WHERE ip_hash = '{H_LOCAL}';"
    )
    left_u = psql(
        f"SELECT count(*) FROM users WHERE username LIKE '{MARK}%'"
    )
    left_s = psql(
        "SELECT count(*) FROM reg_ip_signups WHERE created_at > "
        "now() - interval '10 minutes'"
    )
    check("测试账号清理归零", left_u == "0",
          f"残留 {left_u}（uid1={uid1}）")
    check("测试摘要清理归零", left_s == "0", f"残留 {left_s}")
    back = psql(
        "SELECT value FROM site_settings WHERE name = 'registration_mode'"
    )
    check("registration_mode 已还原", back == saved["registration_mode"],
          f"{back}")

    print()
    if FAILED:
        print(f"✗ {len(FAILED)} 项失败：")
        for f in FAILED:
            print(f"   - {f}")
        return 1
    print("✓ 全部通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
