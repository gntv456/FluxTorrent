# -*- coding: utf-8 -*-
"""常驻闸门：Web Push 的 endpoint 是会员控制的出网口，必须按解析后的 IP 判（P-453）。

资深会员视角：站点允许我把浏览器算出来的 push endpoint 存进库里，之后
**服务器会主动往这个 URL POST**（E2E 加密载荷）。这就是一条由会员控制的
出网口，而旧写法只比字符串前缀：

    https://127.0.0.1/p…          → 拦下（旧白名单唯一真的挡住的一种）
    https://169-254-169-254.nip.io/…  → **穿过白名单**（公共通配 DNS，A 记录
                                    就是云元数据地址）
    https://127-0-0-1.nip.io/…    → 穿过白名单
    https://rebind.<n>.example.invalid/x → 穿过白名单（自建域指回 127.0.0.1）

现网实测（2026-10-20，旧镜像）：后三条都走到 p256dh 校验才被拒——
说明 endpoint 这一关放行。危害不是「读到响应体」（载荷是加密的），而是
① 站点变成往内网/元数据地址 POST 的代理；② `/push/test` 把
delivered / gone / 失败分三类回给调用方 ⇒ 一个**状态码 oracle**；
③ 旧投递客户端**没有超时**，对方挂住就长期占着 worker 线程。

修法（`push_http/endpoint.rs` 一份，subscribe 与 deliver 都用它）：
  · `endpoint_host_port`：scheme 必须 https，**解析不出来就拒**
    （旧写法 `if let Ok(u) = Url::parse(..)` 解析失败时跳过检查＝放行）；
  · `guard_endpoint`：字面量 IP 或 DNS 解析，**每一个**地址都要公网
    （127/8、RFC1918、169.254/16 链路本地、100.64/10 CGNAT、0/8、
    240/4、组播、广播、未指定、IPv6 ULA fc00::/7、`::ffff:` v4-mapped）；
  · deliver 把校验过的地址 **钉进 HTTP client**（`resolve`）并加 10s 超时
    ——不钉就有「校验时一个答案、发送时另一个答案」的 DNS 重绑定窗口。

判据：
  S0 正控   修前：subscribe 是字符串前缀白名单、client 没有 resolve/timeout
  S1        现网接线：两处都过 guard_endpoint；闸门本体只有一份
  S2        纯函数行为：cargo test push_http::endpoint（4 用例 24 条断言）
  S3        活体·可判红：字面量内网 endpoint 必须被拒（两个镜像都得拒）
  S4        活体·自升级：通配 DNS / 自建域三条——新镜像拒（PASS），
            旧镜像穿到 p256dh 才拒（WARN 记部署账）；跑完删净自己造的订阅行

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_push_endpoint_ssrf.py
"""
import json
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
EP = "apps/api/src/push_http/endpoint.rs"
PUSH = "apps/api/src/push_http.rs"
CRY = "apps/api/src/push_http/crypto.rs"
FIX = "bb5f9c4f"          # 本批修复的提交（S0 从它的父提交取旧形状）
FAKE_P256 = "BG" + "A" * 86          # 65 字节 0x04 开头的假非压缩点
RUN = int(time.time())
PSQL = ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux", "-q",
        "-d", "fluxtorrent", "-tA", "-c"]
LITERAL = "https://127.0.0.1/p%d" % RUN
BYPASS = ["https://169-254-169-254.nip.io/p%d" % RUN,
          "https://127-0-0-1.nip.io/p%d" % RUN,
          "https://rebind%d.example.invalid/x" % RUN]


def read(rel):
    return open(os.path.join(ROOT, rel.replace("/", os.sep)),
                encoding="utf-8", errors="replace").read()


def git_show(rev_path):
    p = subprocess.run(["git", "show", rev_path], capture_output=True,
                       text=True, encoding="utf-8", errors="replace",
                       cwd=ROOT)
    if p.returncode != 0:
        raise SystemExit("取文失败 %s / %s" % (rev_path,
                                               (p.stderr or "")[:150]))
    return p.stdout or ""


def git_grep(pat):
    """全仓命中行（用来数「同一份判定有没有被抄第二遍」）。"""
    p = subprocess.run(
        ["git", "grep", "-n", "--untracked", pat, "--", "apps"],
        capture_output=True, text=True, encoding="utf-8",
        errors="replace", cwd=ROOT)
    return [l for l in (p.stdout or "").splitlines() if l.strip()]


def sql(q):
    return (subprocess.run(PSQL + [q], capture_output=True, text=True,
                           encoding="utf-8").stdout or "").strip()


def call(method, path, body=None, token=None):
    r = urllib.request.Request(BASE + path, method=method)
    r.add_header("Content-Type", "application/json")
    if token:
        r.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(r, data, timeout=25) as f:
            return f.status, json.loads(f.read())
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"raw": raw[:100].decode("utf-8", "replace")}


print("== S0 正控（修前形状）==")
old_push, old_cry = git_show("%s^:%s" % (FIX, PUSH)), git_show(
    "%s^:%s" % (FIX, CRY))
ok("S0a 修前 subscribe 是字符串前缀白名单（且没有出网闸门）",
   'starts_with("127.")' in old_push and "guard_endpoint" not in old_push)
ok("S0b 修前投递端没钉地址也没有超时",
   ".resolve(" not in old_cry and ".timeout(" not in old_cry)

print("== S1 现网接线 ==")
push_s, cry_s, ep_s = read(PUSH), read(CRY), read(EP)
ok("S1a subscribe 与 deliver 都过同一个 guard_endpoint",
   "guard_endpoint(&body.endpoint)" in push_s
   and "guard_endpoint(endpoint).await" in cry_s)
ok("S1b 闸门本体只有一份（两份白名单必然漂移）",
   ep_s.count("pub(crate) async fn guard_endpoint") == 1
   and len(git_grep("fn is_public_ip(")) == 1
   and len(git_grep("fn guard_endpoint(")) == 1,
   "命中处=%s" % git_grep("fn is_public_ip("))
ok("S1c 投递端钉地址 + 有超时（DNS 重绑定窗口与挂死线程都关掉）",
   ".resolve(&host, *a)" in cry_s
   and ".timeout(std::time::Duration::from_secs(10))" in cry_s)
ok("S1d scheme 只认 https，解析不出来就拒（不再是 if let Ok 跳过）",
   'if u.scheme() != "https"' in ep_s and "is_empty()" in ep_s)
ok("S1e IP 分类全仓只有一处实现（push 委托 domain::webhook::is_forbidden）",
   len(git_grep("fn is_forbidden(")) == 1
   and "is_forbidden(ip)" in ep_s and "octets()" not in ep_s,
   "P-455：本文件的闸门本体一度被抄成第二份规则（两边缺段还不一样），"
   "这条就是防它再长回来")

print("== S2 纯函数行为 ==")
t = subprocess.run(["cargo", "test", "--bin", "flux-api",
                    "push_http::endpoint"],
                   capture_output=True, text=True, encoding="utf-8",
                   errors="replace", cwd=ROOT)
out = (t.stdout or "") + (t.stderr or "")
ok("S2 cargo test push_http::endpoint 通过（rc=%d）" % t.returncode,
   t.returncode == 0 and "4 passed" in out,
   "红账=%s" % [l for l in out.splitlines() if "panicked" in l][:2])

print("== S3/S4 活体 ==")
st, lg = call("POST", "/auth/login",
              {"username": "root", "password": "password123"})
tok = (lg.get("data") or {}).get("token")
if not tok:
    print("   [跳过 S3/S4] 登录不上：%s" % str(lg)[:80])
    summary()
    raise SystemExit(0)


def probe(ep):
    st, b = call("POST", "/push/subscribe",
                 {"endpoint": ep,
                  "keys": {"p256dh": FAKE_P256, "auth": "YWJjZGVmZ2hp"},
                 "topics": ["promo"]}, tok)
    return st, (b.get("message") or "")


try:
    st, msg = probe(LITERAL)
    ok("S3 字面量内网 endpoint 被拒（新旧镜像都得拒）：%s｜%s"
       % (st, msg[:46]), "endpoint" in msg and "p256dh" not in msg,
       "文案来自 endpoint 校验；出现 p256dh 说明已经进到下一步")
    穿过 = []
    for ep in BYPASS:
        st2, msg2 = probe(ep)
        穿过.append((ep.split("//")[1][:30], "p256dh" in msg2))
    未拦 = [e for e, hit in 穿过 if hit]
    if 未拦:
        print("   [WARN 不判红] %d/%d 条「解析后指回内网」的 endpoint 穿到"
              "下一步＝现网仍是字符串前缀白名单（本批修复没进镜像）：%s"
              % (len(未拦), len(穿过), 未拦))
    else:
        print("   [S4 已生效] %d 条通配 DNS/自建域 endpoint 全在边界被拒；"
              "旧镜像在这一格是 WARN，不算假绿——两条硬判据在 S3 与 S1b"
              % len(穿过))
finally:
    for ep in [LITERAL] + BYPASS:
        sql("DELETE FROM push_subscriptions WHERE endpoint = '%s';" % ep)
    left = sql("SELECT count(*) FROM push_subscriptions WHERE "
               "endpoint LIKE '%%p%d' OR endpoint LIKE 'rebind%%%%';" % RUN)
    ok("S5 本轮造的订阅行已删净（回读 %s）" % left, left == "0",
       "闸门自己留的残料要当场收掉；root 活跃订阅=%s"
       % sql("SELECT count(*) FROM push_subscriptions WHERE user_id=1 "
             "AND expired_at IS NULL;"))

summary()
