# -*- coding: utf-8 -*-
"""常驻闸门：/push 三个写口必须有频率上限（P-454）+ 一张「没人管的写口」台账。

会员/运维视角：`/push/test` 是全站**唯一会替会员向外发 HTTP POST** 的口
（每次最多打 5 条订阅），`/push/subscribe` 每次要做一次 DNS 解析 + 一行
upsert。这仓没有全局限流层——`http/throttle.rs` 的 `rate_window` 是**逐口挂**
的（附件阅读 60/时、签发 Token 5/分都是这么写的），而 /push 三个口
一个都没挂：登录态下可以无限刷。现网实测（旧镜像）连打 40 次退订，
400 到底、一次 429 都没有。

修法（`push_http.rs`）：subscribe 30/分、unsubscribe 30/分（成对刷要同档，
否则「订阅→退订→订阅」绕开单口配额）、test 5/分（与签发 Token 同档，
它是真出网的那一步）。键只传业务段——`rl:` 前缀由 throttle 统一加（P-72：
调用方自带前缀会造成 `rl:rl:` 双前缀，运维照文档 scan 一条都数不到）。

判据：
  R0 正控   修前 push_http.rs 里没有 rate_window，且 throttle 不是全局层
  R1        三口各挂一次；键名不带 rl:；test 档必须比读写档紧
  R2        活体·自升级：连打 40 次退订 ⇒ 新镜像应在第 31 次起 429（PASS）；
            旧镜像 40 次全 400（WARN 记部署账，同时就是缺陷本体的现场）
  R3        台账读数（不判红）：会员面写口里一个限流都没挂的路由数与前 15 条
            ——便宜的写口不需要限流，所以这是**待裁清单**不是违例名单

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_push_rate.py
"""
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import BASE, ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PUSH = "apps/api/src/push_http.rs"
FIX = "9f16f020"  # 本批修复的提交（R0 从它的父提交取旧形状）
SRC = os.path.join(ROOT, "apps", "api", "src")
MACRO = re.compile(r'#\[(post|put|delete|patch)\(\s*"([^"]+)"')
LIMITERS = ("rate_window(", "throttle_limited(", "check_rate(",
            "counter_peek(", "counter_bump(")


def read(rel):
    return open(os.path.join(ROOT, rel.replace("/", os.sep)),
                encoding="utf-8", errors="replace").read()


def call(method, path, body=None, token=None):
    r = urllib.request.Request(BASE + path, method=method)
    r.add_header("Content-Type", "application/json")
    if token:
        r.add_header("Authorization", "Bearer " + (token or ""))
    data = ('{"endpoint":"https://127.0.0.1/rlprobe%d"}' % RUN).encode() \
        if body is None else body
    try:
        with urllib.request.urlopen(r, data, timeout=15) as f:
            return f.status
    except urllib.error.HTTPError as e:
        return e.code
    except Exception:
        return -1


RUN = int(time.time())
print("== R0 正控（修前形状）==")
old = subprocess.run(["git", "show", "%s^:%s" % (FIX, PUSH)],
                     capture_output=True, text=True, encoding="utf-8",
                     errors="replace", cwd=ROOT)
if old.returncode != 0:
    raise SystemExit("取文失败：%s" % (old.stderr or "")[:150])
old_s = old.stdout or ""
ok("R0a 修前 /push 三个口一个限流都没挂（尺抓得到旧形状）",
   "rate_window" not in old_s
   and old_s.count("async fn ") >= 3)
th = read("apps/api/src/http/throttle.rs")
ok("R0b 这仓没有全局限流层：rate_window 是逐口调用的工具函数",
   "pub async fn rate_window" in th
   and "fn config" not in th and "wrap_service" not in th)

print("== R1 现网接线 ==")
new = read(PUSH)
hits = re.findall(r"rate_window\(&state, &format!\(\"([a-z]+):\{\}\", "
                  r"auth\.id\), (\d+), (\d+)\)", new)
got = dict((k, (int(n), int(w))) for k, n, w in hits)
ok("R1a 三口各挂一次且键用业务段（不带 rl: 前缀，P-72）：%s" % sorted(got),
   set(got) == {"pushsub", "pushunsub", "pushtest"}
   and "rl:" not in "".join(got),
   "实挂=%s" % got)
ok("R1b 真出网的那一口（test）配额必须比读写口紧：%s" % got,
   got.get("pushtest", (99,))[0] < got.get("pushsub", (0,))[0]
   and got.get("pushsub") == got.get("pushunsub"),
   "订阅/退订不同档＝成对刷能绕开单口配额")

print("== R2 活体：连打 40 次退订 ==")
tok = (subprocess.run(
    [sys.executable, os.path.join(ROOT, "_mint_jwt.py"),
     "--uid", "1", "--class", "99"], capture_output=True, text=True,
    encoding="utf-8", errors="replace").stdout or "")
m = re.search(r"[\w\-]+\.[\w\-]+\.[\w\-]+", tok)
token = m.group(0) if m else None
if not token:
    print("   [跳过 R2] 探针令牌取不到")
else:
    codes = [call("POST", "/push/unsubscribe", token=token) for _ in range(40)]
    n429 = codes.count(429)
    first429 = codes.index(429) + 1 if n429 else 0
    print("   状态码分布=%s" % {c: codes.count(c) for c in set(codes)})
    if n429:
        ok("R2a 第 %d 次起 429（配额 30/分）" % first429,
           30 <= first429 <= 32, "首次 429 位次=%d" % first429)
        ok("R2b 429 之后不再放行", n429 >= len(codes) - 32 - 1,
           "429 共 %d 次" % n429)
    else:
        print("   [WARN 不判红] 40 次全是 %s，一次 429 都没有＝现网镜像"
              "仍未挂限流（本批修复没进镜像；这就是缺陷本体的现场）"
              % sorted(set(codes)))

print("== R3 台账读数：会员面一个限流都没挂的写口 ==")
bare, total = [], 0
for dp, _d, fs in os.walk(SRC):
    if "node_modules" in dp or ".next" in dp:
        continue
    for f in fs:
        if not f.endswith(".rs"):
            continue
        rel = os.path.relpath(os.path.join(dp, f), ROOT).replace("\\", "/")
        s = read(rel)
        for m in MACRO.finditer(s):
            path = m.group(2)
            if path.startswith("/admin") or path.startswith("/staff"):
                continue
            total += 1
            tail = s[m.end():m.end() + 1400]
            if not any(k in tail for k in LIMITERS):
                bare.append(path)
print("   会员/社区面写口 %d 条，其中本口处理器内看不到限流调用的 %d 条"
      % (total, len(bare)))
print("   （这把尺只扫**本口处理器内**的字面调用：按 IP 的频控、跨模块的")
print("    闸门它看不见——注册就有 24h/2IP 的频控却仍在名单里，")
print("    所以 %d 是**上界**。便宜的纯写库口本来也不需要限流："
      "这是待裁清单，不是违例名单）" % len(bare))
print("   前 15 条：%s" % sorted(set(bare))[:15])

summary()
