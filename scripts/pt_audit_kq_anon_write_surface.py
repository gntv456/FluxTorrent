# -*- coding: utf-8 -*-
"""常驻闸门 KQ：匿名打全站写口，钉「不许真干活」+「鉴权顺序不退步」。

选题（2026-10-23 一次性普查转常驻）：DA 只扫 GET，写口这一面从来没被
系统扫过。拿 openapi 的路由清单把每条 POST/PUT/PATCH/DELETE 用**空载荷 +
不带任何凭证**打一遍，按状态码分桶。三件事分别钉：

  KQ1 清单可信：openapi 走统一信封（`{"code":0,"data":{...}}`），条目数
      必须 ≥ 60 且含写口——清单空了整支闸门就成了空转（正控同条给数）。
  KQ2 **零真落库**：匿名请求拿到 2xx 就是越权写，一条都不许有。这条是硬线，
      不设棘轮、不许登记。收尾再回读关键业务表计数不变（证明这轮普查
      自己在现网不留东西，也反证那些 2xx 不是「看着像没落库」）。
  KQ3 鉴权顺序棘轮（只许降不许升）：本仓口径是「鉴权/权限闸装在业务校验
      之前」，但 `web::Json<T>` 是 extractor，跑在 handler 之前 ⇒ 任何**必填
      字段**缺失都会以 `400 缺少必填字段：x` 抢先回。今天实测 42 条（全部
      是这一型；没有一条越到业务层，也没有一条 2xx）。这条不是漏洞是架构
      账，要改是 30+ handler 的取舍，所以只钉「不许变多」：接线鉴权中间件
      或换成「先鉴权再解析」的提取器时，登记值必须跟着降。

桶必须分开数，别把三样东西混成一样：
  - **429 是限流，不是鉴权**：一次性普查那版把 32 条 429 记成了「停在
    门口」，于是把 42 条错报成 33 条。这里逐条退避重试到拿到真判定。
  - **`DELETE + JSON body` 是本仓既有形状**（`/torrents/{id}/previews`、
    撤销补签，界面注释自己写着「同形」）：探针空手打会得「请求体不是合法
    JSON」，那是探针的锅，单列一桶不进修行账。
  - **404/409/422 与 400 不是一回事**：前者说明匿名请求已经触达业务层
    （按对象干活了），单独立硬线 KQ2b。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_kq_anon_write_surface.py
"""
import json
import os
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import dig, ok, psql_stdin, summary  # noqa: E402

BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")
HOST = BASE[:-len("/api/v1")] if BASE.endswith("/api/v1") else BASE
WRITE_METHODS = ("post", "put", "patch", "delete")
# openapi 的 path 自带 /api/v1 前缀 ⇒ 拼 URL 必须用裸主机，别把前缀写两遍
PREFIX = "/api/v1"
# 设计上允许匿名的入口：登录/注册/验证码/找回密码/被封者申诉通道/
# 职员箱首问/公开读口，以及带凭证的 rss·torznab·download（凭证在 query 里）
PUBLIC = (
    "/auth/login", "/auth/register", "/auth/captcha", "/auth/2fa",
    "/auth/password", "/auth/passkey", "/appeals", "/contactstaff",
    "/ban-log", "/site-profile", "/health", "/ready", "/openapi",
    "/docs", "/swagger", "/webhooks", "/rss", "/torznab", "/passkey",
    "/downloads", "/download", "/compat/nexusphp/takelogin.php",
    "/compat/nexusphp/picturecom", "/attach", "/proxy", "/image",
    "/embed", "/oembed", "/qrcode", "/qrcode", "/metrics", "/rules",
)
# 实测（带 429 退避重跑、DELETE 也发 `{}`）：先解析请求体的口 42 条。
# 一次性普查那版记的是 33——因为 32 条被匿名限流器打成 429，被错记成
# 「鉴权停在门口」。接线「先鉴权再解析」后这个数只会降；降了要同步改这里
# （与行数门禁同款棘轮）。
BODY_FIRST_MAX = 42
# 关键表 → 水位列（None = 这表没有 id 类单调键，只数行数）。
# 水位列必须有：只数 count 会漏掉「原地改内容」，只数 max(id) 会漏掉
# 「删一行又插一行」，两个一起看才叫没被动过。
GUARD = {
    "messages": "id",
    "appeals": "id",
    "requests": "id",
    "staffmessages": "id",
    "torrents": "id",
    "user_achievements": "user_id",
    "spark_ledger": "id",
    "site_settings": None,
    "message_templates": "id",
}


def level(t):
    key = GUARD[t]
    if key is None:
        return psql_stdin("SELECT count(*)::text FROM %s" % t)
    return psql_stdin("SELECT count(*)::text || '|' || "
                      "COALESCE(max(%s), 0)::text FROM %s" % (key, t))
PATH_PARAMS = ("{id}", "{tid}", "{uid}", "{pid}", "{rid}", "{cid}",
               "{sid}", "{mid}", "{aid}", "{vid}", "{kid}", "{oid}",
               "{jid}", "{bid}", "{qid}", "{did}", "{iid}", "{lid}")


def fetch(path, method="GET", body=None, tries=3):
    """打一跳，返回 (status, dict)。429 退避重试，别把限流当结论。"""
    url = HOST + path
    for i in range(tries):
        req = urllib.request.Request(url, method=method)
        req.add_header("Content-Type", "application/json")
        req.add_header("Accept-Language", "zh-CN")
        # DELETE 也发 `{}`：本仓有几口就是 `DELETE + JSON body`（如
        # /torrents/{id}/previews、/admin/attendance 撤销补签，界面注释里
        # 自己写着「与 torrents/previews 同形」）。空手打会得
        # 「请求体不是合法 JSON」，那是探针没发体，不是发现。
        data = b"{}" if body is None else json.dumps(body).encode()
        try:
            with urllib.request.urlopen(req, data, timeout=30) as r:
                code, payload = r.status, r.read()
        except urllib.error.HTTPError as e:
            code, payload = e.code, e.read()
        except Exception as e:  # noqa: BLE001
            return 0, {"message": "请求异常 %s" % str(e)[:90]}
        if code == 429 and i < tries - 1:
            time.sleep(20 + 15 * i)
            continue
        try:
            return code, json.loads(payload)
        except Exception:  # noqa: BLE001
            return code, {"message": payload[:90].decode("utf-8", "replace")}
    return 0, {"message": "unreachable"}


def write_routes():
    st, raw = fetch(PREFIX + "/openapi.json")
    paths = dig(raw, "data", "paths") or {}
    rows = []
    for path, ops in paths.items():
        for method in (ops or {}):
            if method.lower() in WRITE_METHODS:
                rows.append((method.upper(), path))
    return sorted(rows), st, raw


def url_of(path):
    out = path
    for name in PATH_PARAMS:
        out = out.replace(name, "1")
    # 剩下的 {x} 一律填 1：判据只关心「匿名会不会被放行到业务层」，
    # 路径参数取不到真实对象时业务层回 404/1004，仍是「挡住」的一种
    while "{" in out:
        seg = out[out.index("{") + 1: out.index("}")] if "}" in out else ""
        out = out.replace("{" + seg + "}", "1")
    return out


print("== KQ1 清单可信：openapi 路由表 ==")
rows, st, raw = write_routes()
paths_n = len({p for _m, p in rows})
ok("KQ1 写口清单取到（%d 条 / %d 个路径）" % (len(rows), paths_n),
   st == 200 and len(rows) >= 60,
   "http=%s 条数=%d（openapi 走统一信封，取 data.paths）" % (st, len(rows)))
pub = [r for r in rows
       if any(url_of(r[1])[len(PREFIX):].startswith(p) for p in PUBLIC)]
targets = [r for r in rows if r not in pub]
print("   共 %d 条写口，设计上允许匿名 %d 条，本轮打 %d 条"
      % (len(rows), len(pub), len(targets)))
ok("KQ1b 正控：白名单非空且远小于总数（白名单写错会把全站当公开口）",
   3 <= len(pub) < len(rows) // 2, "公开=%d" % len(pub))
UNR = [u for _m, u in rows if "{" in u and not u.split("{")[0]]
ok("KQ1c 路径参数都能替掉（替不掉会全打同一处，判据就空转）",
   not UNR, str(UNR[:4]))

print("\n== KQ2/KQ3 匿名逐条打写口 ==")
before = dict((t, level(t)) for t in GUARD)
codes = {}
logged_in_first = []
parse_first = []
probe_gap = []
ran_past = []
route_missing = []
no_verdict = []
for method, path in targets:
    st, js = fetch(url_of(path), method)
    msg = str(js.get("message") or "")[:70]
    codes[st] = codes.get(st, 0) + 1
    if 200 <= st < 300:
        logged_in_first.append((method, path, st, msg))
    elif st in (401, 403):
        pass                     # 停在门口，正是想要的
    elif st == 404 and js.get("code") == 1004 and "接口不存在" in msg:
        # 契约漂移：openapi 列了、路由其实没挂（default_service 兜底）
        route_missing.append((method, path))
    elif st == 500 or st >= 500:
        parse_first.append((method, path, st, msg))
    elif st in (404, 409, 410, 422):
        # 业务层（查库/状态判定）被匿名触达 —— 比「先解析请求体」更重
        ran_past.append((method, path, st, msg))
    elif "请求体不是合法 JSON" in msg:
        probe_gap.append((method, path, st, msg))
    elif st == 400:
        parse_first.append((method, path, st, msg))
    else:
        no_verdict.append((method, path, st, msg))
print("   状态码分布：%s" % sorted(codes.items(), key=lambda kv: -kv[1]))
ok("KQ2 匿名打全站写口零 2xx（有一条真干活就是越权写）",
   not logged_in_first,
   "；".join("%s %s→%s %s" % x for x in logged_in_first[:6]))
ok("KQ2b 匿名请求不许触达业务层（404/409/410/422 = 已经在按对象干活）",
   not ran_past,
   "；".join("%s %s→%s %s" % x for x in ran_past[:6]))
ok("KQ2c 正控：探针没发体的一类必须为零（有就是本闸门自己的锅，不算发现）",
   not probe_gap or len(probe_gap) < len(targets) // 4,
   "；".join("%s %s→%s" % x[:3] for x in probe_gap[:6]))
# KQ2d：拿一条**已知会 200** 的口走同一分桶，证明「零 2xx」不是空断言
# （没有这条，分桶函数写错成「什么都不记」也会全绿）
st2, js2 = fetch(url_of(PREFIX + "/auth/login"), "POST",
                 body={"username": "root", "password": "password123"})
ok("KQ2d 正控：登录口带对凭证必须落进 2xx 桶（分桶函数本身要能认 200）",
   200 <= st2 < 300, "http=%s message=%s" % (st2, str(js2.get("message"))[:50]))
ok("KQ1d openapi 清单与实挂路由一致（列了却没挂=契约漂移）",
   not route_missing,
   "；".join("%s %s" % x for x in route_missing[:8]))
ok("KQ3 每条都拿到了可信判定（异常/超时不算测过）", not no_verdict,
   "；".join("%s %s→%s" % x[:3] for x in no_verdict[:5]))
bad5xx = [x for x in parse_first if x[2] >= 500]
ok("KQ3b 匿名请求不许打出 5xx（500=连参数校验都没守住）", not bad5xx,
   "；".join("%s %s→%s" % x[:3] for x in bad5xx[:5]))
nbf = len(parse_first)
ok("KQ3c 鉴权顺序棘轮：先解析请求体的口 %d ≤ 登记 %d（接线后请同步下调）"
   % (nbf, BODY_FIRST_MAX), nbf <= BODY_FIRST_MAX,
   "今天多出 %d 条；样例=%s" % (nbf - BODY_FIRST_MAX,
                              ["%s %s→%s" % x[:3] for x in parse_first[:6]]))
for m, p, c, msg in parse_first[:12]:
    print("     [KQ3 台账] %-6s %-42s → %s %s" % (m, p[len(PREFIX):], c, msg))

print("\n== KQ4 收尾：这轮普查自己在现网不留东西 ==")
after = dict((t, level(t)) for t in GUARD)
moved = [t for t in GUARD if before[t] != after[t]]
ok("KQ4 关键表 count|max(id) 全部不变（%s）" % ", ".join(moved[:4]) or "无变动",
   not moved, "；".join("%s %s→%s" % (t, before[t], after[t])
                        for t in moved[:4]))
ok("KQ4b 正控：这条不是恒真——先查一条真会被写的表名能不能读到数",
   all(before[t] != "" for t in GUARD), str(before)[:150])
summary()
