# -*- coding: utf-8 -*-
"""闸门 PN：踢出其它会话必须连「更晚登录」的会话一起踢（P-564）。

文案承诺「将使其它所有设备上的登录立即失效（本设备不受影响）」，旧实现
nbf=本 token 的 iat-1 只能判死更早签发的 token：比本设备更晚登录的会话
反而幸存——账号被盗后，攻击者登录在你之后，「踢出其它会话」踢不掉他。

修复：nbf=now-1 让所有既有凭证失效，同时给本设备重签一枚新 token
（iat=now > nbf）随响应写回 flux_token cookie；本设备靠新 token 存活。

本闸门用三枚真 token（C 最旧 / A 当前 / B 最新）走真 API：
  PN0 三枚令牌齐备
  PN1 撤销后 C、A、B 全失效；响应带新 token；新 token 可用；
      响应带 Set-Cookie（浏览器路径）
  PN2 源码：nbf=now-1 + issue + Set-Cookie；旧形状负控
"""
import atexit
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
TAG = int(time.time()) % 100000
UNAME = "pn%d" % TAG
PW = "pw-old-564"
UID = None
CID = None


def ex(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   ! SQL: %s" % (r.stderr or "").strip()[:150])
    return r.returncode


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError(r.stderr.strip()[:200])
    return r.stdout.strip()


def cleanup():
    if UID:
        for t, col in (("messages", "receiver_id"),
                       ("token_revocations", "user_id"),
                       ("login_events", "user_id"),
                       ("spark_ledger", "user_id"),
                       ("password_resets", "user_id")):
            ex("DELETE FROM %s WHERE %s = %d" % (t, col, UID))
        ex("UPDATE users SET status = 2 WHERE id = %d" % UID)
    if CID:
        ex("DELETE FROM invites WHERE id = %s" % CID)
    if UID:
        print("   [cleanup] 探针=%s 邀请=%s"
              % (UID, "已删" if CID else "-"))


atexit.register(cleanup)


def tok():
    s, r = call("POST", "/auth/login", {"username": UNAME, "password": PW})
    t = (r.get("data") or {}).get("token")
    if not t:
        print("   ! login http=%s msg=%s"
              % (s, (r.get("message") or "")[:90]))
    return t


def raw_post(path, token, payload=None):
    req = urllib.request.Request(
        BASE + path, method="POST",
        data=json.dumps(payload or {}).encode(),
        headers={"Content-Type": "application/json",
                 "Authorization": "Bearer " + token})
    try:
        with urllib.request.urlopen(req, timeout=15) as r:
            return r.status, r.read().decode(), dict(r.headers)
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode(), dict(e.headers)


def set_cookie(headers):
    for k, v in headers.items():
        if k.lower() == "set-cookie":
            return v
    return ""


def me(t):
    # 令牌缺失绝不能伪装成 401（否则 429 级联能让「必须失效」判据假绿）。
    if not t:
        return -1
    return call("GET", "/me", token=t)[0]


def clear_login_rl():
    """只清本探针号的登录限流桶（用户名档 5 次/分，本闸门要登 6 次）。"""
    cmd = ('redis-cli -a "$REDIS_PASSWORD" --no-auth-warning --scan '
           '--pattern "rl:login:%s:*" '
           '| xargs -r redis-cli -a "$REDIS_PASSWORD" --no-auth-warning del'
           % UNAME)
    subprocess.run(["docker", "exec", "flux-redis", "sh", "-c", cmd],
                   capture_output=True, text=True, encoding="utf-8")


def reg_probe():
    global UID, CID
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return False
    code, CID = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    s, r = call("POST", "/auth/register", {
        "username": UNAME, "email": "%s@pn.local" % UNAME,
        "password": PW, "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": (int(nums[0]) + int(nums[1])
                           if len(nums) >= 2 else 0),
        "captcha_token": "", "fields": {}})
    uid = (r.get("data") or {}).get("user_id")
    if uid:
        UID = int(uid)
    print("   注册 http=%s uid=%s" % (s, UID))
    return bool(UID)


print("== PN0 探针号与三枚令牌 ==")
ok("PN0a 探针号可建成", reg_probe())
if not UID:
    summary()
    sys.exit(1)
C = tok()
time.sleep(1.5)
A = tok()
time.sleep(1.5)
B = tok()
ok("PN0b 三枚令牌齐备（C 最旧 / A 当前 / B 最新）",
   bool(C and A and B))
# 撤销线是秒级 iat：B 必须落在 now-1 之前，否则同秒签发本就该幸存。
time.sleep(1.5)

print("== PN1 撤销后 C、A、B 全失效，本设备换新 token ==")
s, body, hdrs = raw_post("/me/sessions/revoke-others", A)
ok("PN1a 撤销请求成功", s == 200, "http=%s %s" % (s, body[:80]))
try:
    nd = ((json.loads(body).get("data") or {}).get("token") or "")
except Exception:  # noqa: BLE001
    nd = ""
ok("PN1b 更早登录的会话已失效", me(C) == 401)
ok("PN1c 更晚登录的会话也必须失效（红线：旧版 200，攻击者踢不掉）",
   me(B) == 401)
ok("PN1d 本次请求所持旧 token 由新 token 接替（应已失效）",
   me(A) == 401)
ok("PN1e 响应下发新 token 且可用（本设备不受影响）",
   bool(nd) and me(nd) == 200, "new=%s" % nd[:16])
sc = set_cookie(hdrs)
ok("PN1f 新 token 同时写回 flux_token cookie（浏览器路径）",
   "flux_token=" in sc, sc[:60])

print("== PN2 源码：撤销线口径 ==")
src = open(os.path.join(REPO, "apps", "api", "src", "auth_http",
                        "me_sessions.rs"),
           encoding="utf-8", errors="replace").read()
body_fn = src.split("pub async fn revoke_other_sessions")[-1]


def new_shape(b):
    return ("timestamp() - 1" in b and ".issue(" in b
            and "set-cookie" in b and "auth.iat - 1" not in b)


ok("PN2a nbf 用 now-1 且重签 token / 写 cookie", new_shape(body_fn))
old = ("let auth = require_auth(&req, &state).await?;\n"
       "    .bind(auth.iat - 1)\n"
       '    Ok(ok(serde_json::json!({ "revoked": "others" })))')
ok("PN2b 承重自证：旧形状必须判红", new_shape(old) is False)
ok("PN2c 撤销线已写入 DB 权威表（与 Redis 双轨）",
   "token_revocations" in body_fn and "logout_nbf:" in body_fn)
nbf = psql("SELECT COALESCE(max(nbf), 0) FROM token_revocations "
           "WHERE user_id = %d" % UID)
ok("PN2d 撤销线落在当前秒附近（now-2 … now）",
   abs(int(nbf) - int(time.time())) <= 2 if nbf.isdigit() else False,
   "nbf=%s now=%d" % (nbf, int(time.time())))

print("== PN3 改密同口径：更晚登录的会话也必须失效（P-565）==")
clear_login_rl()
C2 = tok()
time.sleep(1.5)
A2 = tok()
time.sleep(1.5)
B2 = tok()
ok("PN3-0 三枚新令牌齐备（C2/A2/B2）",
   bool(C2 and A2 and B2 and len({C2, A2, B2}) == 3))
# 同 PN0：B2 要在改密时刻的上一秒之前签发。
time.sleep(1.5)
NEWPW = "pw-new-565"
s, pbody, hdrs = raw_post("/me/password/change", A2,
                          {"old_password": PW, "new_password": NEWPW})
ok("PN3a 改密请求成功", s == 200, "http=%s %s" % (s, pbody[:80]))
try:
    nd = ((json.loads(pbody).get("data") or {}).get("token") or "")
except Exception:  # noqa: BLE001
    nd = ""
ok("PN3b 更早登录的会话已失效", me(C2) == 401)
ok("PN3c 更晚登录的会话也必须失效（红线：旧版 200）", me(B2) == 401)
ok("PN3d 改密所持旧 token 由新 token 接替（应已失效）", me(A2) == 401)
ok("PN3e 响应下发新 token 且可用（本设备不受影响）",
   bool(nd) and me(nd) == 200, "new=%s" % nd[:16])
sc = set_cookie(hdrs)
ok("PN3f 新 token 同时写回 flux_token cookie（浏览器路径）",
   "flux_token=" in sc, sc[:60])

print("== PN4 源码：改密撤销线口径（P-565）==")
msec = open(os.path.join(REPO, "apps", "api", "src", "auth_http",
                         "me_security.rs"),
            encoding="utf-8", errors="replace").read()
pw_fn = msec.split("pub async fn me_password_change")[-1]
ok("PN4a nbf 用 now-1 且重签 token / 写 cookie",
   "timestamp() - 1" in pw_fn and ".issue(" in pw_fn
   and "set-cookie" in pw_fn and "auth.iat - 1" not in pw_fn)
oldpw = ("let auth = require_auth(&req, &state).await?;\n"
         "    .bind(auth.iat - 1)\n"
         '    Ok(ok(serde_json::json!({ "changed": true })))')
ok("PN4b 承重自证：旧形状必须判红", new_shape(oldpw) is False)

print("== PN-Z 收尾 ==")
cleanup()
ok("PN-Z1 撤销行已回收",
   psql("SELECT count(*) FROM token_revocations WHERE user_id = %d"
        % UID) == "0")
summary()
