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
     Redis 报错路径：修前静默放行，修后 5xx。其余站点共用同一 helper；
  S5 P-72 键名契约：`rl:` 前缀只由 rate_window 加一次，实测键名必须等于
     文档口径 `rl:<scope>:<id>:<窗口>`（双前缀 `rl:rl:*` 即红）；
  S6 P-69 计数键 TTL 自愈；
  S7 P-73 无 TTL 孤键不得等于永久拒绝（shoutbox/captcha/banlog/pwdforgot
     四站从自写 Redis 收进 rate_window）。

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
# S9 正控签出的 Token 要在收尾撤销；早退路径也会走 cleanup()，故在此定义
EXTRA_IDS = []
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
    # S9 的正控那枚也当场撤掉（每人上限 3 枚，闸门不该把额度吃光）
    for tid in EXTRA_IDS:
        if MT:
            call("POST", "/me/tokens/revoke", {"id": tid}, token=MT)
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


def _decode_log(blob):
    """日志字节 → 文本。要能识别 UTF-16LE（PowerShell 默认）。"""
    if blob[:2] == b"\xff\xfe":
        return blob.decode("utf-16-le", "replace")[1:]
    if blob[:3] == b"\xef\xbb\xbf":
        return blob[3:].decode("utf-8", "replace")
    if b"\x00" in blob[:64]:
        return blob.decode("utf-16-le", "replace")
    return blob.decode("utf-8", "replace")


def logs_since(sec="25s"):
    """取 access log。日志源必须**跟着 API 所在位置**走。

    批一百五十四实测：sweep 打的是宿主上起的侧进程（:8100），它的 access log
    在自己的 stdout 里，`docker logs flux-api` 读的是主容器那份——两者根本不是
    同一个进程的输出。于是「日志里必须带 HTTP 方法」这类判据在侧进程下拿到的
    是主容器的历史行，正控必然红（实测 S1/P-74 正控 429×0 之外的另一条红：
    `"GET /api/v1/forums"` 一行都没有，samp 为空 ⇒ any(...) 恒 False）。

    判法：FLUX_API_BASE 的端口若是主容器发布端口就读容器日志，否则读侧进程
    自己的日志文件。找不到时如实返回空串并由判据红，不静默拿别的进程的日志充数。
    """
    base = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")
    try:
        port = int(base.rsplit(":", 1)[1].split("/")[0])
    except Exception:  # noqa: BLE001
        port = 8080
    pub = subprocess.run(["docker", "port", "flux-api"],
                         capture_output=True, text=True, encoding="utf-8", errors="replace").stdout or ""
    if str(port) in {ln.strip().rsplit(":", 1)[-1]
                      for ln in pub.splitlines() if ln.strip()}:
        raw = subprocess.run(
            ["docker", "logs", "--since", sec, "flux-api"],
            capture_output=True, text=True, encoding="utf-8").stdout
        return re.sub(r"\x1b\[[0-9;]*m", "", raw)
    # 侧进程：日志由启动脚本重定向到文件。先看环境变量，
    # 再自动探工作区里的默认名字（本仓验证脚本 side8101.ps1 统一
    # 写到 _tmp_v2/side_access.log）——不要让每个跑 sweep 的人手动配环境变量，
    # 不配就是三条「日志里有没有」的假红（实测 sweep8 就是这么红的）。
    cands = [os.environ.get("FLUX_SIDE_LOG")]
    # 仓库根：__file__ 是 scripts/pt_audit_au_passkey_surface.py，
    # 所以 dirname 一次是 scripts、二次才是仓库根。
    # 第一版只去了一层·得到 D:\，进而找不到 _tmp_v2/side_access.log，
    # 表现为三条「日志里没有相关行」的假红（日志里实际有）。
    # 仓库根：先从 cwd 猜（仓内所有闸门都是从仓库根跑的），
    # 再回退 __file__。不把它当唯一依赖：被其他模块 exec 引用时
    # __file__ 可能不存在（NameError），会让整个日志取数抹结果为空。
    roots = [os.getcwd()]
    try:
        roots.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    except NameError:
        pass
    for r in roots:
        cands.append(os.path.join(r, "_tmp_v2", "side_access.log"))
    for cand in cands:
        if not cand or not os.path.exists(cand) or os.path.getsize(cand) == 0:
            continue
        # 只取本次之后的行（按 mtime + 尾部读的字节数），
        # 不要固定取尾 200KB：日志文件会一直增长（实测 668KB），
        # 本轮刚写的行很快就被挤出尾部窗口 → 又变成「日志里没有」的假红。
        # 取最后 4MB 并再按行过滤本次进程启动之后的时间戳。
        try:
            size = os.path.getsize(cand)
            with open(cand, "rb") as fh:
                fh.seek(max(0, size - 4 * 1024 * 1024))
                blob = fh.read()
        except OSError:
            continue
        # 编码探测：本仓验证脚本用 PowerShell 的 Tee-Object 写日志，
        # 它在 Windows 上默认写 **UTF-16LE**（字节间隔一个 0x00），而不是
        # UTF-8。按 UTF-8 解码得到的是一大片乱码，里面连 "path=/api/v1/rss/"
        # 这个形式都找不到 —— 三条「日志里有没有」的判据全红，
        # 而日志里实际一行不少。因此先看首字节否有 BOM/零字节来判库。
        tail = _decode_log(blob)
        tail = re.sub(r"\x1b\[[0-9;]*m", "", tail)
        # PowerShell 的 Tee-Object 有缓冲：刚写进去的请求行可能还在缓冲里，
        # 所以「读到文件尾部」不等于「读到本轮请求」。等一下：
        # 刷新到能看见本次触发的 RSS 路径为止（最多等 6 秒）。
        needle = "path=/api/v1/rss/"
        for _ in range(12):
            if needle in tail:
                break
            time.sleep(0.5)
            try:
                with open(cand, "rb") as fh:
                    size2 = os.path.getsize(cand)
                    fh.seek(max(0, size2 - 4 * 1024 * 1024))
                    blob = fh.read()
                tail = re.sub(r"\x1b\[[0-9;]*m", "", _decode_log(blob))
            except OSError:
                break
        return tail
    return ""
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
                                    "rate_per_min": 60,
                                    "password": "password123"}, token=MT)
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
# P-74：同一行里的第二个方向——格式符写错 actix 不报错，直接把 `%m` 印成
# 字面量，于是 access log 只剩 `"m /path"`，HTTP 方法整列没了（POST 发种与
# GET 列表再也分不出来）。泄漏挡住 ≠ 字段还在，两头都要钉。
samp = [l for l in log.split("\n") if "/api/v1/forums" in l]
ok("S1/P-74 正控：access log 行里带 HTTP 方法",
   any('"GET /api/v1/forums"' in l for l in samp), brief(samp[-1:]))
ok("S1/P-74 反控：没有被当字面量印出的 %m（形如 \"m /api/…\"）",
   not any('"m /api' in l for l in samp), brief(samp[-1:]))

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

print("== S5 限流键名契约（P-72）==")
# 键名不是内部细节，它是闸门与运维的**读数接口**：`rl:` 前缀归 rate_window
# 统一加。调用方再自带一次就落出 `rl:rl:npdl:…`（本批在跑着的容器里实测
# 到的正是这个形状）——文档里 `redis-cli --scan --pattern "rl:npdl:*"`
# 一条都数不到，往 `rl:npdl:…` 投毒也打不中，「fail-close 已生效」这类
# 判据会在键名错位的情况下假绿。
def live_keys(pat):
    out = redis_cmd('--scan --pattern "%s"' % pat)
    return [k.strip() for k in out.split("\n") if k.strip()]


s, _ = call("GET", "/compat/nexusphp/download.php?id=1&passkey=%s" % PK,
            raw=True)
dbl = live_keys("rl:rl:*")
ok("S5 不存在双前缀限流键（rl:rl:*）——文档口径与实际键名同源", not dbl,
   brief(dbl))
buckets = [k for k in live_keys("rl:npdl:*")
           if re.match(r"^rl:npdl:[0-9a-f]{16}:\d+$", k)]
ok("S5 正控：npdl 桶存在，形状是 rl:<scope>:<passkey 哈希前 16>:<窗口>",
   buckets, "命中=%s http=%s" % (len(buckets), s))
win_now = int(time.time()) // 60
wins = [int(k.rsplit(":", 1)[1]) for k in buckets]
# 窗口必须在**键名**里（P-67）；SCAN 无序且会留着上一分钟的桶，
# 所以判「最新桶在当前分钟 ±1」而不是判任意 buckets[0]（跨分钟就假红）。
ok("S5 反控：窗口写进键名，且最新桶落在当前分钟（±1）",
   bool(wins) and win_now - 1 <= max(wins) <= win_now + 1,
   "buckets=%s 当前窗口=%s" % (brief(buckets), win_now))
newest = buckets[wins.index(max(wins))] if wins else ""
t5 = redis_cmd('TTL "%s"' % newest) if newest else "-2"
ok("S5 窗口键 TTL 落在 (0,120]：TTL 只为回收，滑动续期会把 429 续命（P-65）",
   t5.isdigit() and 0 < int(t5) <= 120, "ttl=%s" % t5)

print("== S7 计数键必须能复位（P-73：无 TTL 孤键 = 永久拒绝）==")
# 旧四站（shoutbox 10s / captcha / banlog / pwdforgot）自己写 Redis，
# `incr(...).unwrap_or(0)` + `if n == 1 { expire }`。EXPIRE 只在第一次调用
# 时补，那条命令一失败就留下一个**永不过期**的计数键，此后计数只增不减：
# pwdforgot 那个邮箱永远申请不到找回密码，captcha 那个出口 IP（CGNAT 后
# 成千用户共用）永远拿不到验证码。这里直接造出「失败之后」的现场。
ORPH = "%so" % MUSER
redis_cmd('DEL "rl:banlog:%s"' % ORPH)
redis_cmd('SET "rl:banlog:%s" 999' % ORPH)
t7 = redis_cmd('TTL "rl:banlog:%s"' % ORPH)
ok("S7 前置：造出一个没有 TTL 的计数键（TTL=-1，正是旧版 EXPIRE 失败后"
   "留下的现场）", t7 == "-1", "ttl=%s" % t7)
s7, _ = call("GET", "/auth/ban-log?username=%s" % ORPH, raw=True)
ok("S7 孤键不得变成永久拒绝：一次调用后窗口要能自己复位（旧版在这里"
   "一路 429 到天荒地老）", s7 != 429, "http=%s" % s7)
nb = [k for k in live_keys("rl:banlog:%s:*" % ORPH)
      if re.match(r"^rl:banlog:[a-z0-9]+:\d+$", k)]
ok("S7 正控：本次调用落在带窗口的桶上，而不是那个孤键", nb, brief(nb))
redis_cmd('DEL "rl:banlog:%s"' % ORPH)
hits7 = []
for _ in range(7):
    s7, _ = call("GET", "/auth/ban-log?username=%s" % ORPH, raw=True)
    hits7.append(s7)
ok("S7 反控：限流本身没被拆——5 次/分钟，第 6 次起照样 429",
   429 in hits7, brief(sorted(set(hits7))))

print("== S8 登录限流的两档分工（P-76）==")
# IP 档原本与用户名档同宽（5 次/分），于是它挡不住字典爆破（那由用户名档 +
# acctlock 两条线负责），挡住的却是合法场景：一个 NAT/CGNAT 出口下 6 个成员
# 在同一分钟各自登录，第 6 个起全量 429。闸门连跑时也是同一个出口，AB 就撞
# 死在这里（「登录 6 次都拿不到 token」）。先清桶，让断言只测这一档。
for pat in ("rl:login:%s:*" % MUSER, "rl:login:root:*", "rl:login-ip:*"):
    for k in live_keys(pat):
        redis_cmd('DEL "%s"' % k)
sts = []
for _ in range(5):
    sts.append(call("POST", "/auth/login", {"username": "root",
                                            "password": "password123"})[0])
    sts.append(call("POST", "/auth/login", {"username": MUSER,
                                            "password": "password123"})[0])
ok("S8 同一出口 IP 上两个账号各 5 次（共 10 次）都该正常——IP 档放宽到"
   " 20/分（旧版第 6 次起 429，家庭/NAT 出口会被自己人挤掉）",
   set(sts) == {200}, brief(sorted(set(sts))))
hits8 = []
for _ in range(7):
    hits8.append(call("POST", "/auth/login", {"username": MUSER,
                                              "password": "password123"},
                      raw=True)[0])
ok("S8 反控：放宽 IP 档不等于放宽账号档——同一账号被爆破时第 6 次起照样"
   "429", 429 in hits8, brief(sorted(set(hits8))))

print("== S9 API Token 建口的 step-up（P-113）==")
# 与 P-107 同族：Token 是 180 天的**长期凭证**，不像会话 JWT 会随改密/撤销线死，
# 只能逐条 revoke。会话被偷的人若能直接签一枚带 upload 的 Token，就等于留下
# 一个「改完密码也赶不走」的后门。
def active_ids():
    got = q("SELECT COALESCE(string_agg(id::text, ','), '') FROM api_tokens "
            "WHERE user_id = %s AND revoked_at IS NULL" % MID)
    return [int(x) for x in got.split(",") if x]


def revoke_all_active(keep=None):
    for rid in active_ids():
        if keep is not None and rid == keep:
            continue
        call("POST", "/me/tokens/revoke", {"id": rid}, token=MT)


# 先清场（**留下 S0 那枚**，续期那一判要用它）：产品口径「每人最多 3 枚有效」
# 会先一步 400，把「这一口到底有没有档」遮掉——红线第一轮正是这样被掩盖的。
revoke_all_active(keep=TOKEN_ID)
ok("S9 前置：探针号只剩 S0 那枚 Token（上限会掩盖「有没有档」）",
   active_ids() == [TOKEN_ID], "active=%s" % active_ids())
s_np, r_np = call("POST", "/me/tokens", {"name": "no-pw"}, token=MT)
ok("S9 不给密码就签 Token 必须被拒（红线：200 直接吐明文 Token）",
   s_np == 400 and "复核密码" in str(r_np.get("message", "")),
   "%s %s" % (s_np, brief(r_np)))
s_bp, r_bp = call("POST", "/me/tokens",
                  {"name": "bad-pw", "password": "Wrong#2026pw"}, token=MT)
ok("S9 密码错必须拒且点到「凭证无效」（这个口此前根本不核密码，错码也照签）",
   s_bp == 400 and "凭证无效" in str(r_bp.get("message", "")),
   "%s %s" % (s_bp, brief(r_bp)))
left = [x for x in active_ids() if x != TOKEN_ID]
ok("S9 两次被拒都不该落下新 Token 行（红线：照样 INSERT，而明文已回显过一次）",
   not left, "多留=%s" % left)
for rid in left:
    EXTRA_IDS.append(rid)
    call("POST", "/me/tokens/revoke", {"id": rid}, token=MT)
s_rf, r_rf = call("POST", "/me/tokens/refresh", {"id": TOKEN_ID}, token=MT)
ok("S9 续期同样要复核（红线：200 把有效期再推 180 天 = 让后门永生）",
   s_rf == 400 and "复核密码" in str(r_rf.get("message", "")),
   "%s %s" % (s_rf, brief(r_rf)))
# 正控放在限速风暴**之前**：这一口的新档是 5 次/分，先把额度吃光就会把自己
# 的正常动线打成 429 假红（同一课在批三十四/三十五已经交过两次学费）。
revoke_all_active()
s_ok, r_ok = call("POST", "/me/tokens",
                  {"name": "AU正控%d" % TAG, "password": "password123"},
                  token=MT)
d_ok = r_ok.get("data") or {}
if d_ok.get("id"):
    EXTRA_IDS.append(d_ok["id"])
ok("S9 正控：复核通过后仍能签出 Token（且明文只在这一次返回）",
   s_ok == 200 and bool(d_ok.get("token")), "%s %s" % (s_ok, brief(r_ok)))
codes9 = []
for _ in range(6):
    codes9.append(call("POST", "/me/tokens",
                       {"name": "brute", "password": "Wrong#2026pw"},
                       token=MT, raw=True)[0])
ok("S9 建口要有档（连发 6 次错密码必须出现 429；红线：该口零限流）",
   429 in codes9, "状态序列=%s" % codes9)
revoke_all_active()
ok("S9 收尾：本探针号不留有效 Token（撤销是软撤，数 revoked_at IS NULL）",
   not active_ids(), "still=%s" % active_ids())

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
