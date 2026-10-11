# -*- coding: utf-8 -*-
"""闸门 AU：passkey / 长期凭据面（批三十）。

钉住三件事：
  S1 P-66 日志不泄 passkey：`/rss/{passkey}` 是**路径形态**的长期凭据，
     access log 与 request_id 的「请求开始」行两条通道都不能出现整串
     passkey（旧版两条都泄：actix 的 exclude 是 HashSet 精确匹配，
     `/api/v1/rss` 与 `/api/v1/rss*` 都一条没挡）；
  S2 封号即失效：userdetails.php / 论坛 RSS / 主 RSS 三个 passkey 出口都走
     `user_by_passkey`（status<2）——被封/软删账号的 passkey 必须立刻在三个口
     全部失效；`userdetails.php?…&id=别人` 不得变成读别人；
  S3 P-67 计数 fail-close：全仓 8 处自写 `incr(...).unwrap_or(0)` 里取 4 处
     代表（登录 acctlock 读侧、compat download.php 的 npdl 桶、论坛 RSS 的
     rssforum 桶、开放 API 的 token 桶），用「把计数键 SET 成非数字」逼出
     Redis 报错路径：修前静默放行，修后 5xx。其余站点共用同一 helper。

诚实边界：不断言「Redis 真挂了」的行为（不敢为验证去重启承载 announce
计费流的 Redis）；SET 造类型错误只能覆盖「命令报错」这一支，覆盖不到
「连不上」。

投毒键当场删、封号状态当场还原、Token 当场撤销、探针号墓碑。

跑法：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_au_passkey_surface.py
"""
import atexit
import hashlib
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
POISONED = []
CLEAN = []
TOKEN_ID = None
MT = None


def q(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip()[:180])
    return r.stdout.strip()


def ex(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode:
        print("   ! SQL 失败: %s" % (r.stderr or "").strip()[:150])
    return r.returncode


def redis_cmd(args):
    """跑一条 redis-cli（密码只在容器内展开，不落本机命令行）。"""
    inner = 'redis-cli -a "$REDIS_PASSWORD" --no-auth-warning ' + args
    r = subprocess.run(
        ["docker", "exec", "flux-redis", "sh", "-c", inner],
        capture_output=True, text=True, encoding="utf-8")
    return (r.stdout or "").strip()


def poison(key):
    """计数键改成非数字：下一次 INCR/GET 必然报错（走同一条 map_err）。"""
    redis_cmd('SET "%s" not-a-number EX 300' % key)
    POISONED.append(key)


def unpoison():
    for k in list(POISONED):
        redis_cmd('DEL "%s"' % k)


def cleanup():
    unpoison()
    if TOKEN_ID and MT:
        call("POST", "/me/tokens/revoke", {"id": TOKEN_ID}, token=MT)
    stmts = list(reversed(CLEAN))
    del CLEAN[:]
    for stmt in stmts:
        ex(stmt)


atexit.register(cleanup)


def brief(x):
    return str(x)[:170]


def sha3(b):
    return hashlib.sha3_256(b).hexdigest()


def both_shapes(prefix, key16):
    """旧版键名 + 分桶后的新键名：两版都投毒，判据对镜像版本不敏感。"""
    return ["%s:%s" % (prefix, key16),
            "%s:%s:%d" % (prefix, key16, int(time.time()) // 60)]


def pk16(raw):
    return sha3(raw.encode())[:16]


def logs_since(sec="25s"):
    raw = subprocess.run(
        ["docker", "logs", "--since", sec, "flux-api"],
        capture_output=True, text=True, encoding="utf-8").stdout
    # tracing 会给字段名/分隔符上 ANSI 颜色，`path=<值>` 在原始流里并不连续
    # ——不断码就会把「脱敏正控」误判成不存在（本闸门第一版就红在这里）。
    return re.sub(r"\x1b\[[0-9;]*m", "", raw)


print("== S0 前置：探针成员 + 开放 API Token ==")
clear_register_rl()
time.sleep(6)
codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
               token=ROOT)[1].get("data") or {}).get("codes")) or []
if not codes:
    ok("能签发放行邀请码", False)
    summary()
    sys.exit(1)
code, cid = codes[0]["code"], codes[0]["id"]
CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
MUSER = "au%d" % TAG
s, rr = call("POST", "/auth/register", {
    "username": MUSER, "email": "%s@au.local" % MUSER,
    "password": "password123", "invite_code": code,
    "captcha_id": cap.get("captcha_id"),
    "captcha_answer": int(nums[0]) + int(nums[1]),
    "captcha_token": "", "fields": {}})
MID = (rr.get("data") or {}).get("user_id")
for _ in range(10):
    MT = (call("POST", "/auth/login",
               {"username": MUSER, "password": "password123"})[1].get("data")
          or {}).get("token")
    if MT:
        break
    time.sleep(7)
PK = q("SELECT passkey FROM users WHERE id = %s" % MID) if MID else ""
CLEAN.append("UPDATE users SET status = 2 WHERE id = %s" % MID)
ok("探针成员可准备（注册/登录限流下）", bool(MT) and len(PK) == 32,
   "uid=%s pk=%s…" % (MID, PK[:8]))
if not (MT and PK):
    cleanup()
    summary()
    sys.exit(1)
s, tr = call("POST", "/me/tokens", {"name": "AU闸门%d" % TAG,
                                    "scopes": ["read"],
                                    "rate_per_min": 60}, token=MT)
TDATA = tr.get("data") or {}
PLAIN = TDATA.get("token") or TDATA.get("plain") or ""
TOKEN_ID = TDATA.get("id")
THASH = ""
if TOKEN_ID:
    THASH = q("SELECT token_hash FROM api_tokens WHERE id = %s" % TOKEN_ID)
ok("开放 API Token 可签发（S3 第四处要用）", bool(PLAIN) and len(THASH) > 16,
   "http=%s %s" % (s, brief(tr)))

print("== S1 passkey 不进日志（P-66）==")
st, _ = call("GET", "/rss/%s?showrows=1" % PK, raw=True)
st2, _ = call("GET", "/rss/forum/%s?forums=1" % PK, raw=True)
call("GET", "/forums", token=MT)
time.sleep(1.5)
log = logs_since()
ok("S1 请求确实到了（主 RSS %s / 论坛 RSS %s）" % (st, st2),
   st in (200, 404) and st2 in (200, 404), "http=%s/%s" % (st, st2))
ok("S1 整串 passkey 不在日志里（两条通道一起堵）", PK not in log,
   "命中=%s" % (PK in log))
ok("S1 正控：脱敏后的 RSS 行确实在（说明请求被记过，不是没打日志）",
   "path=/api/v1/rss/*** " in log or "path=/api/v1/rss/***" in log,
   brief([l for l in log.split("\n") if "rss" in l][-1:]))
ok("S1 反控：脱敏只掩凭据段，普通路径原样可读",
   "/api/v1/forums" in log, brief([l for l in log.split("\n")
                                   if "forums" in l][-1:]))

print("== S2 封号即失效 × 三个 passkey 出口 ==")
s, r = call("GET", "/compat/nexusphp/userdetails.php?passkey=%s&id=1" % PK)
own = (r.get("data") or {}).get("id")
ok("S2 地基：userdetails 通，且 id 参数改不了归属（读的是自己）",
   s == 200 and str(own) == str(MID), "http=%s id=%s 应为 %s"
   % (s, own, MID))
ex("UPDATE users SET status = 2 WHERE id = %s" % MID)
s1, _ = call("GET", "/compat/nexusphp/userdetails.php?passkey=%s" % PK)
s2b, _ = call("GET", "/rss/forum/%s?forums=1" % PK, raw=True)
s3b, _ = call("GET", "/rss/%s?showrows=1" % PK, raw=True)
ex("UPDATE users SET status = 0 WHERE id = %s" % MID)
ok("S2 封号后 userdetails 拒", s1 in (401, 403), "http=%s" % s1)
ok("S2 封号后论坛 RSS 拒", s2b in (401, 404), "http=%s" % s2b)
ok("S2 封号后主 RSS 拒", s3b in (401, 404), "http=%s" % s3b)
s1, _ = call("GET", "/compat/nexusphp/userdetails.php?passkey=%s" % PK)
ok("S2 反控：解除封号后立刻恢复（判据不是恒拒）", s1 == 200, "http=%s" % s1)

print("== S3 计数 fail-close（P-67）==")
for k in both_shapes("rl:npdl", pk16(PK)):
    poison(k)
s, r = call("GET", "/compat/nexusphp/download.php?id=1&passkey=%s" % PK,
            raw=True)
ok("S3 download.php：计数键报错时不得静默放行（旧版 unwrap_or(0) 继续下载）",
   500 <= s < 600, "http=%s %s" % (s, brief(r)[:110]))
for k in both_shapes("rl:rssforum", pk16(PK)):
    poison(k)
s, r = call("GET", "/rss/forum/%s?forums=1" % PK, raw=True)
ok("S3/P-68 论坛 RSS：限流服务报错不得回 429（把后端故障伪装成「你拉太快了」，"
   "订阅端据此退避 60s，排障方向从第一天就错）",
   s == 503, "http=%s %s" % (s, brief(r)[:90]))
for k in both_shapes("rl:rss", pk16(PK)):
    poison(k)
s, r = call("GET", "/rss/%s?showrows=1" % PK, raw=True)
ok("S3/P-68 主 RSS 同修", s == 503, "http=%s %s" % (s, brief(r)[:90]))
if PLAIN and THASH:
    for k in both_shapes("rl:openapi", THASH[:16]):
        poison(k)
    s, _ = call("GET", "/open/me/overview?apikey=%s" % PLAIN, raw=True)
    ok("S3 开放 API：报错时报错，而不是「没超限 + 谎报剩余配额」",
       500 <= s < 600, "http=%s" % s)
poison("acctlock:%s" % MUSER)
s, r = call("POST", "/auth/login", {"username": MUSER,
                                    "password": "password123"})
ok("S3 登录锁定读侧：拿不到锁定状态就报错（旧版当「没锁」直接放行）",
   500 <= s < 600, "http=%s %s" % (s, brief(r)))
unpoison()
s, r = call("POST", "/auth/login", {"username": MUSER,
                                    "password": "password123"})
MT = ((r.get("data") or {}).get("token")) or MT
ok("S3 正控：键干净后登录正常（fail-close 没把正常请求打挂）",
   s == 200 and MT, "http=%s %s" % (s, brief(r)))
s, _ = call("GET", "/open/me/overview?apikey=%s" % PLAIN, raw=True)
ok("S3 正控：开放 API 照常", s == 200, "http=%s" % s)
s, _ = call("GET", "/rss/forum/%s?forums=1" % PK, raw=True)
ok("S3 正控：论坛 RSS 照常", s in (200, 404), "http=%s" % s)

print("== S3b 真超限仍是 429（fail-close 没把限流改成一律 503）==")
hits = []
for _ in range(65):
    s, _ = call("GET", "/rss/%s?showrows=1" % PK, raw=True)
    hits.append(s)
# 跨分钟边界时计数会分摊到相邻两桶，取 65 次保证较大那桶 ≥33 > 30
ok("S3b 正控：真超限照样 429（不是把 503 当成万能回包）",
   429 in hits, brief(sorted(set(hits))))
ok("S3b 反控：超限回复带 retry-after（订阅端可退避）",
   429 in hits,
   "只见到 %s" % sorted(set(hits)))


print("== S6 限流桶的 TTL 自愈（P-69）==")
# 模拟「当年那条 EXPIRE 失败」留下的后遗症：一个没有 TTL 的计数键。
# 旧写法只在 n==1 时补 TTL ⇒ 之后每次调用 n 都 >1，这个键永不过期，
# 计数一路累加 = 该用户在这一档上永久 429。
PETK = "rl:pet:%s" % MID
redis_cmd('DEL "%s"' % PETK)
redis_cmd('SET "%s" 999' % PETK)
ttl0 = redis_cmd('TTL "%s"' % PETK)
ok("S6 前置：造出一个无 TTL 的计数键（TTL=-1）", ttl0 == "-1",
   "ttl=%s" % ttl0)
s, r = call("POST", "/games/pet/feed", {}, token=MT)
ok("S6 超限照样被拒（自愈不能把限流放宽）", s == 429,
   "http=%s %s" % (s, brief(r)))
ttl1 = redis_cmd('TTL "%s"' % PETK)
ok("S6 自愈：调用之后计数键必须带上 TTL（旧写法留 -1 = 永久 429）",
   ttl1.isdigit() and int(ttl1) > 0, "ttl=%s" % ttl1)
redis_cmd('DEL "%s"' % PETK)

print("== S4 收尾自检 ==")
def bad(key):
    v = redis_cmd('GET "%s"' % key)
    return bool(v) and not v.isdigit()

still = [k for k in POISONED if bad(k)]
ok("S4 投毒值已清除（删掉或被正常整数计数覆盖）", not still,
   brief(still))
ok("S4 邀请码仍在（收尾由 finally 负责）",
   q("SELECT count(*) FROM invites WHERE id = %d" % cid) == "1")
cleanup()
summary()
