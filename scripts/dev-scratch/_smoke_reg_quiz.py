#!/usr/bin/env python3
"""M5 冒烟（0372）：注册答题题库/抽题判分/票据一次性 + PoW 注册验证码
+ 反作弊规则准确率统计。

用法：python _smoke_reg_quiz.py
前提：api 已部署（含 0372）；自签 staff JWT。
副作用：临时改 6 个 settings 键、建 3 个测试账号与若干假证据，结束全部还原。
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
ROOT = 1
MARK = "m5quiz"
CAT = "smoketest"
Q_TEXT = "M5 冒烟：本站对刷上传量是否违规？"
Q_OPTS = ["不违规，是技巧", "违规，会被封号", "视情况", "不清楚"]
Q_ANS = 1


def sh(cmd: str) -> str:
    return subprocess.run(
        cmd, shell=True, capture_output=True, text=True
    ).stdout.strip()


def psql(sql: str) -> str:
    """经 stdin 喂 SQL：`-c "..."` 会被 shell 拆词（JSON 值里的双引号）。"""
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
    print(f"  {'✓' if cond else '✗'} {name}" + (f" — {extra}" if extra else ""))
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


def data_of(r: object) -> dict:
    return (r or {}).get("data") or {}


def msg(r: object) -> str:
    return str((r or {}).get("message", ""))[:44]


def set_setting(name: str, value: str) -> None:
    psql(
        "INSERT INTO site_settings (name, value) VALUES "
        f"('{name}', '{value}') ON CONFLICT (name) DO UPDATE "
        "SET value = EXCLUDED.value"
    )


def clear_throttle() -> None:
    pw = sh("docker exec flux-redis printenv REDIS_PASSWORD")
    for pat in ("rl:register-ip:*", "rl:quiz:*", "rl:pow:*"):
        keys = sh(
            "docker exec flux-redis redis-cli -a "
            f'{pw} --no-auth-warning --scan --pattern "{pat}"'
        ).split()
        if keys:
            sh(
                f"docker exec flux-redis redis-cli -a {pw} --no-auth-warning "
                f"DEL {' '.join(keys)}"
            )


def captcha() -> tuple[str, int]:
    st, r = call("GET", "/api/v1/auth/captcha", None)
    d = data_of(r)
    m = re.match(r"\s*(\d+)\s*\+\s*(\d+)", str(d.get("question", "")))
    return (
        str(d.get("captcha_id", "")),
        (int(m.group(1)) + int(m.group(2))) if m else 0,
    )


def register(name: str, email: str, extra: dict | None = None) -> tuple[int, object]:
    cid, ans = captcha()
    payload = {
        "username": name,
        "email": email,
        "password": "Passw0rd!m5",
        "invite_code": "",
        "captcha_id": cid,
        "captcha_answer": ans,
        "fields": {},
    }
    payload.update(extra or {})
    return call("POST", "/api/v1/auth/register", None, payload)


def quiz_ticket() -> tuple[str, object]:
    st, r = call("GET", "/api/v1/auth/quiz-challenge", None)
    return (str(data_of(r).get("ticket", "")), r)


def meets(hexs: str, bits: int) -> bool:
    """与 api pow.rs::meets_difficulty 同口径（前导零 bit 数）"""
    if bits == 0 or bits > len(hexs) * 4:
        return False
    full = bits // 4
    if any(c != "0" for c in hexs[:full]):
        return False
    rem = bits % 4
    if rem == 0:
        return True
    mask = 0xF >> rem
    ch = hexs[full]
    v = int(ch, 16)
    return v <= mask


def solve_pow(challenge: str, bits: int, limit: int = 8_000_000) -> str:
    n = 0
    while n < limit:
        s = str(n)
        h = hashlib.sha256(f"{challenge}:{s}".encode()).hexdigest()
        if meets(h, bits):
            return s
        n += 1
    return ""


def main() -> int:
    print("== 0. 备份设置 + 清理 ==")
    keys = (
        "registration_mode",
        "reg_quiz_enabled",
        "reg_quiz_category",
        "reg_quiz_pass_required",
        "reg_quiz_ttl_minutes",
        "captcha_provider",
        "pow_difficulty_bits",
        "reg_ip_limit",
        "reg_window",
    )
    saved = {
        k: psql(f"SELECT value FROM site_settings WHERE name='{k}'")
        for k in keys
    }
    psql(
        f"DELETE FROM reg_quiz_questions WHERE category = '{CAT}'; "
        f"DELETE FROM cheat_events WHERE agent LIKE 'm5fam%'; "
        f"DELETE FROM users WHERE username LIKE 'zzdel_{MARK}%'; "
        "UPDATE users SET username = 'zzdel_" + MARK + "' || id, "
        "email = 'zzdel_" + MARK + "' || id || '@invalid.test', status = 2 "
        f"WHERE username LIKE '{MARK}%';"
    )
    set_setting("registration_mode", "open")
    set_setting("reg_ip_limit", "0")
    set_setting("reg_window", "")

    print("== 1. 题库 admin CRUD ==")
    st, r = call("GET", "/api/v1/admin/reg-quiz", T_ROOT)
    d = data_of(r)
    check("题库列表 + 内置题", st == 200 and d.get("total", 0) >= 14,
          f"HTTP {st} total={d.get('total')} stats={d.get('stats')}")
    st, r = call(
        "POST",
        "/api/v1/admin/reg-quiz",
        T_ROOT,
        {
            "category": CAT,
            "question": Q_TEXT,
            "options": Q_OPTS,
            "answer_index": Q_ANS,
            "explanation": "M5 冒烟用题",
        },
    )
    qid = int(data_of(r).get("id") or 0)
    check("新增题目", st == 200 and qid > 0, f"HTTP {st} id={qid}")
    st, r = call(
        "POST",
        "/api/v1/admin/reg-quiz",
        T_ROOT,
        {"category": CAT, "question": "坏题", "options": ["只有一项"],
         "answer_index": 5},
    )
    check("非法题目被拒", st == 400, f"HTTP {st} {msg(r)}")
    st, r = call(
        "POST",
        "/api/v1/admin/reg-quiz/update",
        T_ROOT,
        {"id": qid, "enabled": False},
    )
    check("停用题目", st == 200, f"HTTP {st}")
    st, r = call(
        "POST",
        "/api/v1/admin/reg-quiz/update",
        T_ROOT,
        {"id": qid, "enabled": True},
    )
    check("重新启用", st == 200, f"HTTP {st}")
    st, r = call(
        "POST",
        "/api/v1/admin/reg-quiz/import",
        T_ROOT,
        {
            "questions": [
                {"category": CAT, "question": Q_TEXT, "options": Q_OPTS,
                 "answer_index": Q_ANS},
                {"category": CAT, "question": "M5 冒烟第二题：H&R 会怎样？",
                 "options": ["无事", "被扣分甚至封禁"], "answer_index": 1},
                {"category": CAT, "question": "x", "options": ["a", "b"],
                 "answer_index": 0},
            ]
        },
    )
    d = data_of(r)
    check("导入：去重 + 非法点名",
          st == 200 and d.get("added") == 1 and d.get("invalid") == 1,
          f"HTTP {st} added={d.get('added')} invalid={d.get('invalid')}")

    print("== 2. 答题注册：答对通过 ==")
    set_setting("reg_quiz_enabled", "yes")
    set_setting("reg_quiz_category", CAT)
    set_setting("reg_quiz_pass_required", "yes")
    ts = str(int(time.time()))
    st, r = call("GET", "/api/v1/auth/quiz-challenge", None)
    d = data_of(r)
    check("抽题返回选项但不含答案",
          st == 200 and len(d.get("options") or []) >= 2
          and "answer" not in json.dumps(d),
          f"HTTP {st} options={len(d.get('options') or [])}")
    tick = str(d.get("ticket", ""))
    clear_throttle()
    st, r = register(f"{MARK}ok{ts}", f"ok{ts}@example.com",
                     {"quiz_ticket": tick, "quiz_answer": Q_ANS})
    check("答对即注册成功", st == 200, f"HTTP {st} {msg(r)}")

    print("== 3. 答错拦截 + 票据一次性 ==")
    clear_throttle()
    tick2, _ = quiz_ticket()
    st, r = register(f"{MARK}bad{ts}", f"bad{ts}@example.com",
                     {"quiz_ticket": tick2, "quiz_answer": 0})
    check("答错被拒", st == 400 and "答错" in msg(r), f"HTTP {st} {msg(r)}")
    left = psql(
        "SELECT count(*) FROM reg_quiz_attempts WHERE passed IS FALSE "
        "AND created_at > now() - interval '10 minutes'"
    )
    check("答题留痕（失败题）", left != "0", f"{left} 条")
    clear_throttle()
    st, r = register(f"{MARK}re{ts}", f"re{ts}@example.com",
                     {"quiz_ticket": tick2, "quiz_answer": Q_ANS})
    check("票据用后即焚（重放被拒）",
          st == 400 and "票据" in msg(r), f"HTTP {st} {msg(r)}")

    print("== 4. pass_required=no：只记录不拦截 ==")
    set_setting("reg_quiz_pass_required", "no")
    clear_throttle()
    tick3, _ = quiz_ticket()
    st, r = register(f"{MARK}soft{ts}", f"soft{ts}@example.com",
                     {"quiz_ticket": tick3, "quiz_answer": 0})
    check("答错仍可注册（软闸）", st == 200, f"HTTP {st} {msg(r)}")

    print("== 5. 未答题直接提交 ==")
    clear_throttle()
    st, r = register(f"{MARK}none{ts}", f"none{ts}@example.com")
    check("缺票据被拒", st == 400 and "答题" in msg(r),
          f"HTTP {st} {msg(r)}")

    print("== 6. PoW 注册验证码 ==")
    set_setting("reg_quiz_enabled", "no")
    set_setting("captcha_provider", "pow")
    set_setting("pow_difficulty_bits", "12")
    st, r = call(
        "GET", "/api/v1/auth/pow-challenge?scope=register", None
    )
    d = data_of(r)
    ch, bits = str(d.get("pow_challenge", "")), int(
        d.get("pow_difficulty", 0) or 0
    )
    check("注册 PoW 挑战（无需失败计数）",
          st == 200 and len(ch) == 64 and bits == 12,
          f"HTTP {st} bits={bits}")
    sol = solve_pow(ch, bits)
    clear_throttle()
    st, r = register(f"{MARK}pow{ts}", f"pow{ts}@example.com",
                     {"pow_token": d.get("pow_token", ""),
                      "pow_solution": sol})
    check("PoW 解得对即注册成功", st == 200, f"HTTP {st} {msg(r)}")
    clear_throttle()
    st2, r2 = call(
        "GET", "/api/v1/auth/pow-challenge?scope=register", None
    )
    d2 = data_of(r2)
    st, r = register(f"{MARK}powb{ts}", f"powb{ts}@example.com",
                     {"pow_token": d2.get("pow_token", ""),
                      "pow_solution": "0"})
    bad_ok = st == 400 and "PoW" in msg(r)
    if not bad_ok:
        # 运气好第一个解就是 0：再试一个必然不满足的
        st, r = register(f"{MARK}powc{ts}", f"powc{ts}@example.com",
                         {"pow_token": d2.get("pow_token", "")})
        bad_ok = st == 400
    check("PoW 解不满足/缺失被拒", bad_ok, f"HTTP {st} {msg(r)}")

    print("== 7. 规则准确率统计 ==")
    psql(
        "INSERT INTO cheat_events (user_id, agent, reason, hits, "
        "resolved_at, false_positive) VALUES "
        "(39, 'm5fam:a', 'M5 冒烟证据', 1, now(), true), "
        "(39, 'm5fam:b', 'M5 冒烟证据', 1, now(), false)"
    )
    st, r = call("GET", "/api/v1/admin/cheat-accuracy?days=7", T_ROOT)
    d = data_of(r)
    fams = {f["family"]: f for f in (d.get("families") or [])}
    f = fams.get("m5fam")
    check("家族级误判率", st == 200 and f is not None
          and f["false_positive"] == 1 and f["accuracy"] == 0.5,
          f"HTTP {st} {f}")
    check("处置结论分布返回", st == 200 and isinstance(
        d.get("sanctions"), list), f"{d.get('sanctions')}")

    print("== 8. 还原设置 + 清理 ==")
    for k, v in saved.items():
        set_setting(k, v)
    psql(
        f"DELETE FROM reg_quiz_questions WHERE category = '{CAT}'; "
        f"DELETE FROM cheat_events WHERE agent LIKE 'm5fam%'; "
        f"DELETE FROM reg_ip_signups WHERE user_id IN "
        f"(SELECT id FROM users WHERE username LIKE '{MARK}%');"
    )
    ids = psql(
        "SELECT string_agg(id::text, ',') FROM users WHERE username LIKE "
        f"'{MARK}%'"
    )
    if ids:
        psql(
            f"DELETE FROM messages WHERE receiver_id IN ({ids}) OR "
            f"sender_id IN ({ids}); "
            f"DELETE FROM user_field_values WHERE user_id IN ({ids});"
        )
        # 硬删不可能：注册即写 audit_log(actor_id=自己)，0266 哈希链触发器
        # 禁 audit_log 的 UPDATE/DELETE，FK 又无 ON DELETE —— 墓碑化。
        psql(
            "UPDATE users SET username = 'zzdel_" + MARK + "' || id, "
            "email = 'zzdel_" + MARK + "' || id || '@invalid.test', "
            f"status = 2 WHERE id IN ({ids})"
        )
    left_u = psql(f"SELECT count(*) FROM users WHERE username LIKE '{MARK}%'")
    left_q = psql(
        f"SELECT count(*) FROM reg_quiz_questions WHERE category = '{CAT}'"
    )
    check("测试账号清理归零", left_u == "0", f"残留 {left_u}")
    check("测试题目清理归零", left_q == "0", f"残留 {left_q}")
    back = psql(
        "SELECT value FROM site_settings WHERE name = 'captcha_provider'"
    )
    check("captcha_provider 已还原", back == saved["captcha_provider"],
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
