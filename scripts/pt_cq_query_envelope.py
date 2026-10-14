# -*- coding: utf-8 -*-
"""CR 闸门（批七十六）：查询参数畸形必须回站内信封，
不能回 text/plain 英文（P-280）。

现网（修前，镜像 `api:aa1024a`，2026-10-14 实测）：
    GET /admin/user-roles?user_id=abc  -> 400 text/plain
        "Query deserialize error: invalid digit found in string"
    GET /rss/{passkey}?showrows=abc    -> 同上（且这条在鉴权之前就炸）
⇒ 破坏「所有 HTTP 响应都是 {code, message, request_id}」这条契约：
  · 前端（`/torrents` 那套 paged/api-client）读的是 code+message，
    拿到 text/plain 只能显示「未知错误」——2026-10-10 批九在 /torrents
    上已实测并逐口改过；
  · 对外脚本（RSS / Torznab / NexusPHP 兼容层）按 JSON 解析直接崩，
    而 RSS 是**passkey 端点**：参数错误抢在 passkey 校验之前返回，
    等于让未鉴权请求探到内部解析细节。

修法：`http/qenv.rs` 的 `Q<T>`（用法与 `web::Query<T>` 一致，失败交回
`DomainError::Validation`）。本批**收全**：会员侧/对外侧 39 文件 50 处起步，
续批 `d08a9b22` 扫完余量，HEAD 上 `: crate::http::Q<` 形参 108 处 + 简写 1 处
共 109 处分布在 87 个文件，`: web::Query<` 形参余量 **0**
（CR5 断言余量必须为 0，
CR7 断言 `qenv.rs` 与 `mod qenv;` 确实在 HEAD 里——上一批就栽在「新文件没提交，
构建树里没功能」）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_cq_query_envelope.py
只读：探针只发 GET；令牌临时签、收尾删。
"""
import io
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOKEN_NAME = "cr76_probe_token"


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def raw_get(path, token=None, auth_header=None):
    """看得到 content-type 的 GET（本闸门断言的正是 content-type）。"""
    req = urllib.request.Request(BASE + path)
    if auth_header:
        req.add_header("Authorization", auth_header)
    elif token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req, timeout=25) as r:
            return r.status, (r.headers.get("Content-Type") or ""), r.read()
    except urllib.error.HTTPError as e:
        return e.code, (e.headers.get("Content-Type") or ""), e.read()
    except Exception as e:  # 超时/连接失败也要记账，别让整闸崩掉
        return 0, "exception", str(e).encode()[:80]


def envelope(status, ctype, body):
    """站内信封三要件：JSON、有 code、有中文 message。"""
    try:
        d = json.loads(body)
    except Exception:
        return False, "非 JSON（%s）：%s" % (ctype, body[:70])
    okj = (isinstance(d, dict) and "code" in d and "message" in d
           and "request_id" in d)
    return okj, ("code=%s message=%s" % (d.get("code"),
                                          str(d.get("message"))[:52]))


clear_register_rl()

# ===== CR0 对照：合法参数照常出数据（收紧不许打断正常路径）=====
s0, b0 = call("GET", "/forums/feed?limit=2", None, token=ROOT)
s0b, _b0b = call("GET", "/me/orders?page=1", None, token=ROOT)
s0c, _b0c = call("GET", "/torrents/40079/related?limit=3", None, token=ROOT)
ok("CR0a 合法查询参数必须照常 200：forums/feed=%s、me/orders=%s、" 
   "related=%s（收紧不许打断正常路径）" % (s0, s0b, s0c),
   s0 == 200 and s0b == 200 and s0c == 200,
   "http=%s/%s/%s" % (s0, s0b, s0c))

# ===== CR1 会员侧：畸形值必须回信封（修前是 text/plain 英文）=====
MEMBER = [
    ("/forums/feed?limit=abc", "会员首页订阅流"),
    ("/me/orders?page=abc", "我的订单"),
    ("/torrents/40079/related?limit=abc", "详情页相关推荐"),
    ("/games/history?limit=abc", "娱乐屋历史"),
]
# 本批续：全仓收口后，后台/开放接口家族也要有接口级证据（root 是职员档）
ADMIN = [
    ("/admin/user-roles?user_id=abc", "后台职务读口（本批最初那条红）"),
    ("/admin/torrents?page=abc", "后台种子管理列表"),
    ("/admin/users?page=abc", "后台成员名册"),
    ("/admin/shop-orders?page=abc", "后台商店订单台账"),
]
# 记账不判红：这两个口查过——`/admin/records`、`/admin/messages`、
# `/admin/audit-logs` 在现网根本不存在（404 code=1004），而
# `/admin/reports?page=abc` 的查询结构里没有 page 字段 ⇒ 未知参数被忽略回 200。
# 上一版闸门把不存在的 routes 当探针，红的是脚本自己（已改真路由）。
IGNORED_ADMIN = ["/admin/reports?page=abc"]
for path, what in MEMBER + ADMIN:
    st, ct, body = raw_get(path, token=ROOT)
    good, detail = envelope(st, ct, body)
    ok("CR1 %s `%s` 必须回站内信封（修前：400 text/plain 英文）：http=%s"
       % (what, path, st), st == 400 and "text/plain" not in ct and good,
       detail)

for p_ig in IGNORED_ADMIN:
    st_ig, ct_ig, b_ig = raw_get(p_ig, token=ROOT)
    print("   WARN CR1 读数：`%s` → http=%s（该口查询结构里没有这个字段，"
          "未知参数被忽略；不是 Q<T> 收口证据，只记账）" % (p_ig, st_ig))

# ===== CR2 对外侧：passkey / Token 端点更要紧（脚本按 JSON 解析）=====
PK = psql("SELECT passkey FROM users WHERE id=1")
st_rss, ct_rss, body_rss = raw_get("/rss/%s?showrows=abc" % PK)
good2, d2 = envelope(st_rss, ct_rss, body_rss)
ok("CR2a RSS（passkey 端点）`?showrows=abc` 必须回信封而非 text/plain："
   "http=%s ctype=%s —— 修前 400 text/plain，且炸在 passkey 校验之前"
   % (st_rss, ct_rss), st_rss == 400 and "text/plain" not in ct_rss and good2,
   d2)
st_fake, ct_fake, body_fake = raw_get("/rss/not_a_real_passkey?showrows=abc")
good3, d3 = envelope(st_fake, ct_fake, body_fake)
ok("CR2b 无效 passkey + 畸形参数：必须先按鉴权口径回信封（http=%s，"
   "不能是 text/plain）——未鉴权请求不该拿到内部解析细节" % st_fake,
   "text/plain" not in ct_fake and good3, d3)
s_mk, b_mk = call("POST", "/me/tokens",
                  {"name": TOKEN_NAME, "scopes": ["read"],
                   "password": "password123"}, token=ROOT)
dd = (b_mk or {}).get("data") or {}
PLAIN = dd.get("token") or dd.get("plaintext") or ""
st_tz, ct_tz, body_tz = raw_get("/torznab?t=tvsearch&cat=5000&limit=abc",
                                auth_header="Token " + PLAIN)
# Torznab 是给 Jackett/RSS 客户端读的**对外 XML 协议**，判据与 JSON 口不同：
# 契约是「必须是 XML、不得 text/plain、不得 5xx、不得把内部解析细节写给
# 客户端」。实测 http=200 + application/xml ⇒ 该口对畸形 limit 取「忽略」
# （按缺省行数返回），这是对外协议的有意口径，不当成收口证据，只记读数。
leak = b"Query deserialize" in body_tz or b"invalid digit" in body_tz
print("   WARN CR2c 现网读数：/torznab?limit=abc → http=%s ctype=%s "
      "（畸形 limit 被忽略而非拒绝；本批只保证不回 text/plain/5xx/内部细节）"
      % (st_tz, ct_tz[:34]))
ok("CR2c Torznab（API Token）畸形 limit 必须仍是 XML、不回 text/plain、"
   "不 5xx、不泄漏解析细节（http=%s ctype=%s）；签名令牌 http=%s"
   % (st_tz, ct_tz, s_mk),
   "text/plain" not in ct_tz and "xml" in ct_tz and st_tz < 500 and not leak,
   "http=%s ctype=%s 泄漏=%s 头=%s"
   % (st_tz, ct_tz, leak, body_tz[:60]))

# ===== CR3 中文可执行文案：message 必须带「查询参数格式无效」前缀 =====
st, ct, body = raw_get("/forums/feed?limit=abc", token=ROOT)
msg = ""
try:
    msg = json.loads(body).get("message", "")
except Exception:
    msg = ""
ok("CR3 message 必须是中文可读（含「查询参数格式无效」）：实测「%s」" % msg,
   "查询参数格式无效" in msg)

# ===== CR4 形状对账：凡用了 Q<T> 的文件都不许留一半 =====
# 旧写法取 `git diff HEAD~1 HEAD`，在「闸门自己也是一笔提交」时清单恒空 ⇒
# 断言永远空转（批七十六实测：CR4 报「本批改过的 0 个文件」）。改为直接扫
# 引用面本身，并把「引用面必须 ≥80 个文件」写进判据，空清单不再算过。
qfiles = []
for root, _d, fs in os.walk(os.path.join(REPO, "apps", "api", "src")):
    for fn in fs:
        if not fn.endswith(".rs"):
            continue
        p = os.path.join(root, fn)
        t = io.open(p, encoding="utf-8", errors="replace").read()
        if ": crate::http::Q<" in t or ": Q<" in t:
            qfiles.append((p.replace(REPO + os.sep, "").replace("\\", "/"),
                           t))
half = [f for f, t in qfiles
        if ": web::Query<" in t and "from_query" not in t]
ok("CR4 用了 Q<T> 的 %d 个文件里不能有「换了一半」的（同文件既留 web::Query<T> "
   "形参又不用 from_query），且引用面不得为空：%s" % (len(qfiles), half),
   len(qfiles) >= 80 and not half, "引用面=%d 半换=%s" % (len(qfiles), half))

# ===== CR5 欠账清零：类型位 web::Query<T> 形参必须为 0 =====
# （P-280 已在批七十六续里全仓收口；`from_query` 的写法不算余量——
#  那是批九起的正确姿势，抽取错误同样交回业务侧。）
n_left = 0
where = []
for root, _d, fs in os.walk(os.path.join(REPO, "apps", "api", "src")):
    for fn in fs:
        if not fn.endswith(".rs"):
            continue
        p = os.path.join(root, fn)
        c = io.open(p, encoding="utf-8").read().count(": web::Query<")
        if c:
            n_left += c
            where.append((p.replace(REPO + os.sep, "").replace("\\", "/"),
                          c))
ok("CR5 全仓「: web::Query<T>」形参余量必须为 0（当前 %d，位置 %s）—— "
   "将来谁新写一个 web::Query 形参入口，这条就红" % (n_left, where),
   n_left == 0, "余量=%d %s" % (n_left, where))

# ===== CR7 提交自洽：被引用的新文件必须在 HEAD 里（本批真栽过一次）=====
head_files = subprocess.run(
    ["git", "ls-tree", "-r", "--name-only", "HEAD", "apps/api/src"],
    cwd=REPO, stdout=subprocess.PIPE, universal_newlines=True,
    encoding="utf-8").stdout.splitlines()
have_qenv = "apps/api/src/http/qenv.rs" in head_files
modrs = subprocess.run(
    ["git", "show", "HEAD:apps/api/src/http/mod.rs"], cwd=REPO,
    stdout=subprocess.PIPE, universal_newlines=True,
    encoding="utf-8").stdout
callers = 0
for f in head_files:
    if not f.endswith(".rs"):
        continue
    t = subprocess.run(["git", "show", "HEAD:" + f], cwd=REPO,
                       stdout=subprocess.PIPE, universal_newlines=True,
                       encoding="utf-8").stdout
    callers += t.count("crate::http::Q<")
ok("CR7 HEAD 必须自洽：定义处 qenv.rs 在树里=%s、mod.rs 里 mod qenv=%s、"
   "引用 crate::http::Q 的形参=%d 处（引用 > 0 而定义缺失 = HEAD 编译不过）"
   % (have_qenv, "mod qenv;" in modrs, callers),
   have_qenv and "mod qenv;" in modrs and (callers == 0 or have_qenv),
   "定义=%s 引用=%d" % (have_qenv, callers))

# ===== CR6 残留：临时令牌删净 =====
psql("DELETE FROM api_tokens WHERE name='%s'" % TOKEN_NAME)
tk = int(psql("SELECT count(*) FROM api_tokens WHERE name='%s'" % TOKEN_NAME))
ok("CR6 临时令牌清干净（签发 http=%s，收尾残留=%d）" % (s_mk, tk), tk == 0)

summary()
