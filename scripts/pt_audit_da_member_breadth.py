# -*- coding: utf-8 -*-
"""闸门 DA：以「低档成员」身份把全站 GET 实跑一遍（路由可达 / 5xx / 越权）。

为什么要有这一支（批一百五十一）：
  本仓踩过两次「csrf 的 configure 把整条 /api/v1 链吃掉 ⇒ 除最后挂载的
  服务外全站 404」，编译、单测、route_manifest_guard 三样都绿，只有真打
  一次才看得见（见 community_http::scope_tree_tests 的注释）。而清单门禁
  只数条数、不验可达，所以这一支把「清单里的每条 GET」逐条打一遍。

同时把「成员看得见什么」钉死：
  - 5xx 必须为零（现网 8faa2033 镜像早于批一百五十一，`/me/torrentlist`
    仍 500 —— 见 PENDING 台账，本批在侧进程 A/B 过：现网 1 条 / 本批 0 条）
  - `/admin/**`、`/staff/**` 对 class 1 成员一律 401/403，一条 200 都不许
  - 核心成员口（首页/我的/H&R/种子详情…）必须真 200，红了就是链路断

跑法（现网）：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_da_member_breadth.py
跑法（侧进程）：FLUX_API_BASE=http://127.0.0.1:8099/api/v1 PYTHONIOENCODING=utf-8 \\
  python -u scripts/pt_audit_da_member_breadth.py

取证备忘（本轮实测，别按猜测改判据）：
  - 未挂载口的 404 长这样：{"code":1004,"message":"接口不存在"}（default_service）；
    业务「资源不存在」是 1004 + 别的文案，两者不能混判 ⇒ 判据只比这句字面量。
  - `/api/v1/metrics` 未配 ANN_METRICS_TOKEN 时回 404 **空体**，是刻意的隐藏门禁，
    不算「路由不存在」。
  - 模块网关关着的域回 404 + 「本站未开放此功能」（farm/wiki/jixiao/contests/
    textbooks/social/gomoku 等），这是产品口径不是洞。
  - `/messages/staff` 名字像职员信箱，SQL 其实是 `sender_id=$1 OR receiver_id=$1`
    的「我与职员的往来」⇒ 成员 200 + 空数组是对的（曾被我记成越权，已排除）。
  - `/shoutbox/bot`、`/shoutbox/bot/exec` 文案写着「需 staff 权」，实测全员可用
    且喊话界面就走这一口 ⇒ P-336 修的是文案不是代码。

DA12 把「前端↔API 参数契约」也钉进来：活体回 400 且文案写着「缺少必填字段：X」
的口，apps/web 里必须有一个同时出现该路径与 `X=` 的调用点；确实服务外部客户端的
（NexusPHP 兼容入口、邮件链接、辅助下载 token、网关回跳）进 EXTERNAL 写明读者。
本轮实测 15 条要求必填参数：外部读者 5 条、前端 10 条全部传对（负控：把
forum-follow.tsx 的 `target_type=` 改成 `type_=` ⇒ DA12 红在
`/follows/status?target_type`，还原后绿）。
"""
import io
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
INV = []
SENTINEL_MSG = None


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], capture_output=True, text=True,
        encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:60], (r.stderr or "").strip()[:90]))
    return (r.stdout or "").strip()


def one(q):
    return psql(q) or "1"


def data(r):
    if isinstance(r, tuple):
        r = r[1]
    return (r or {}).get("data") or {}


# ---------- 清单：GET 全集（route-manifest 是门禁产物，与源码同提交） ----------
MANIFEST = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                        "docs", "api", "route-manifest.json")
ALL = json.load(open(MANIFEST, encoding="utf-8"))["routes"]
GETS = sorted({r["path"] for r in ALL if r["method"] == "GET"})
SKIP = {"/me/messages/stream"}  # SSE 会挂着不回，30s 超时把它算成异常

ok("DA0 清单拿到 GET 全集（%d 条，下限 350）" % len(GETS), len(GETS) >= 350)

# ---------- 路径参数取真值（取不到就用 1，红账里会写明来源） ----------
# 取样必须定序：不带 ORDER BY 时 PG 可以回任意一行，探到的种就随运行漂
# （曾漂到无存档的探针种上，下载口回「种子文件缺失」这条业务 400）。
TORRENT = one("SELECT id FROM torrents WHERE approval_status = 1 "
              "ORDER BY id LIMIT 1")
TOPIC = one("SELECT id FROM topics ORDER BY id LIMIT 1")
BOARD = one("SELECT id FROM forums ORDER BY id LIMIT 1")
ARTIST = one("SELECT id FROM artists ORDER BY id LIMIT 1")
NETWORK = one("SELECT id FROM content_networks ORDER BY id LIMIT 1")
MATCH = one("SELECT id FROM sport_matches ORDER BY id LIMIT 1")
COLL = one("SELECT id FROM collections ORDER BY id LIMIT 1")
SUB = one("SELECT id FROM subtitles ORDER BY id LIMIT 1")
BANNER = one("SELECT id FROM gacha_banners ORDER BY id LIMIT 1")
CHEAT = one("SELECT id FROM cheat_events ORDER BY id LIMIT 1")
RULE = one("SELECT id FROM auto_promo_rules ORDER BY id LIMIT 1")
SHA = psql("SELECT sha256 FROM attachments ORDER BY id LIMIT 1") or "deadbeef"
GAME = psql("SELECT key FROM arcade_games ORDER BY key LIMIT 1") or "jgg"
PAGE_SLUG = psql("SELECT slug FROM custom_pages ORDER BY id LIMIT 1") or "x"
WIKI_SLUG = psql("SELECT slug FROM wiki_pages ORDER BY id LIMIT 1") or "x"


# ---------- 成员探针（走产品自己的注册口：邀请码 + 算术验证码） ----------
def mkprobe(prefix):
    codes = data(call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                      token=ROOT)).get("codes") or []
    cap = data(call("GET", "/auth/captcha", token=ROOT))
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(nums) < 2:
        return 0, None, "发码/验证码没准备好"
    INV.append(codes[0]["id"])
    name = "%s%d" % (prefix, TAG)
    rg = call("POST", "/auth/register", {
        "username": name, "email": "%s@da.local" % name,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    tok = data(rg).get("token") or data(call(
        "POST", "/auth/login", {"username": name,
                               "password": "password123"})).get("token")
    if not tok:
        return 0, None, "注册/登录 http=%s %s" % (rg[0], str(rg[1])[:90])
    uid = int(one("SELECT id FROM users WHERE username = '%s'" % name))
    return uid, tok, ""


clear_register_rl()
ME, MT, ERR = mkprobe("damb")
CLASS = psql("SELECT class_id::text FROM users WHERE id = %d"
                 % ME) if ME else "?"
ok("DA1 低档成员探针就绪（uid=%s class=%s）%s" % (ME, CLASS, ERR),
   ME > 0 and MT and CLASS == "1", ERR or "class=%s" % CLASS)

FILL = {
    "/admin/users/{id}": str(ME), "/admin/users/{id}/snatches": str(ME),
    "/admin/users/{id}/fields": str(ME), "/admin/users/{id}/risk": str(ME),
    "/users/{id}": str(ME), "/users/{id}/torrentlist": str(ME),
    "/admin/cheat-cases/{id}": CHEAT, "/admin/rules/{id}/revisions": RULE,
    "/admin/wiki/{id}/revisions": "1", "/artists/{id}": ARTIST,
    "/networks/{id}": NETWORK, "/matches/{id}": MATCH,
    "/collections/{id}": COLL, "/gacha/banner/{id}/rates": BANNER,
    "/gomoku/games/{id}": "1", "/subtitles/{id}/detail": SUB,
    "/subtitles/{id}/download": SUB, "/forums/topics/{id}": TOPIC,
    "/forums/{id}/topics": BOARD, "/open/torrents/{id}": TORRENT,
    "/compat/nexusphp/torrent/{id}.json": TORRENT,
    "/torrents/{id}/logs/{ordinal}": TORRENT, "/attachments/{sha}": SHA,
    "/games/collection/{game}": GAME, "/rss/{passkey}": "probekey",
    "/rss/forum/{passkey}": "probekey",
}
PASSKEY = psql("SELECT passkey FROM users WHERE id = %d" % ME)
if PASSKEY:
    FILL["/rss/{passkey}"] = PASSKEY
    FILL["/rss/forum/{passkey}"] = PASSKEY


def build(p):
    out = p.replace("{id}", FILL.get(p, TORRENT if p.startswith("/torrents/")
                                     else "1"))
    out = out.replace("{slug}", PAGE_SLUG if p.startswith("/pages/")
                      else WIKI_SLUG)
    for k, v in (("{topic_id}", TOPIC), ("{torrent_id}", TORRENT),
                 ("{passkey}", PASSKEY or "probekey"), ("{sha}", SHA),
                 ("{game}", GAME), ("{ordinal}", "1")):
        out = out.replace(k, v)
    return out


def sweep(tok):
    rows = []
    for p in GETS:
        if p in SKIP:
            continue
        st, payload = call("GET", build(p), token=tok, raw=True)
        try:
            body = json.loads(payload)
        except Exception:  # noqa: BLE001：二进制/XML 出口本来就不是 JSON
            body = {"nonjson": (payload or b"")[:48]}
        rows.append((p, st, body))
    return rows


sent = call("GET", "/zz-definitely-not-a-route")
SENTINEL_MSG = (sent[1] or {}).get("message")
ok("DA2a 正控：不存在的口必须回哨兵签名（拿到「%s」）" % SENTINEL_MSG,
   sent[0] == 404 and SENTINEL_MSG, "http=%s" % sent[0])

MINE = sweep(MT)
ANON = sweep(None)


def missing(rows):
    return [p for p, _st, b in rows
            if isinstance(b, dict) and b.get("message") == SENTINEL_MSG]


ok("DA2b 清单里 %d 条 GET 全部活在现网路由树（漏挂载 0 条，实测漏 %d）"
   % (len(MINE), len(missing(MINE))), not missing(MINE),
   "、".join(missing(MINE)[:6]))

# 本批代码在不在跑：只有新码认 40 个汉字的心愿单关键词（P-329 同族判据）
KW40 = "甲" * 40
probe = call("POST", "/wishlist", {"keyword": KW40}, token=MT)
NEWCODE = probe[0] == 200
if NEWCODE:
    call("DELETE", "/wishlist/%s" % data(probe).get("id"), None, token=MT)
print("   本次跑的是%s二进制（心愿单 40 汉字探针 http=%s）"
      % ("**新批**" if NEWCODE else "现网**旧**", probe[0]))

PENDING = set() if NEWCODE else {"/me/torrentlist"}
print("   旧码待部署台账：%s" % ("空" if NEWCODE else "、".join(PENDING)))
for label, rows in (("成员", MINE), ("匿名", ANON)):
    bad = [(p, st, b) for p, st, b in rows if st >= 500 or st <= 0]
    real = [x for x in bad if x[0] not in PENDING]
    for p, st, b in bad:
        blob = json.dumps(b, ensure_ascii=False)[:110]
        print("   [%s] %s http=%s %s" % (label, p, st, blob))
    ok("DA3%s 身份零 5xx（5xx=%d，其中未部署台账 %d，真红 %d）"
       % (label, len(bad), len(bad) - len(real), len(real)), not real,
       "、".join(x[0] for x in real[:6]))

STAFF = [x for x in MINE
         if x[0].startswith("/admin/") or x[0].startswith("/staff/")]
leak = [p for p, st, _b in STAFF if st == 200]
ok("DA4 职员面前缀（/admin/、/staff/）对 class 1 成员一律不放行："
   "%d 条里 200 = %d" % (len(STAFF), len(leak)), not leak,
   "、".join(leak[:8]))
other = {}
for p_, st_, _b in STAFF:
    if st_ not in (401, 403):
        other[st_] = other.get(st_, 0) + 1
print("   职员面里非 401/403 的形状：%s（400=actix 抽取器在鉴权之前缺必填 query；"
      "404=模块网关）" % sorted(other.items()))
denied = [x for x in STAFF if x[1] in (401, 403)]
ok("DA4b 正控：这些口确实是被鉴权挡回去的（401/403 %d/%d，不是全 404 蒙过去）"
   % (len(denied), len(STAFF)), len(denied) >= len(STAFF) * 0.9,
   "其余状态见 DA6 台账")

# 台账一律写清单里的**原始路径**（got 的键就是它，别在这里手拼 id）
CORE = ["/home", "/me", "/me/overview", "/me/hr", "/me/spark", "/me/medals",
        "/me/seedbox/status", "/me/class-progress", "/me/bookmarks",
        "/me/torrentlist", "/torrents", "/torrents/suggest",
        "/torrents/{id}/detail", "/torrents/{id}/aggregate",
        "/torrents/{id}/comments", "/torrents/{id}/peers",
        "/users/{id}", "/users/{id}/torrentlist", "/forums", "/forums/boards",
        "/forums/recent", "/wishlist", "/requests", "/preserve",
        "/resurrections", "/stats", "/classes", "/medals", "/bank/overview",
        "/shop/items", "/messages/inbox", "/shoutbox", "/invites/status",
        "/site-profile", "/rss-info", "/fun/polls", "/fundings"]
got = dict((p, st) for p, st, _b in MINE)
lag = [p for p in CORE if p in PENDING]
no_core = [p for p in CORE if got.get(p) != 200 and p not in PENDING]
ok("DA6 成员核心口台账全 200（%d 条，非 200 = %d%s）"
   % (len(CORE), len(no_core),
      "；旧码未部署豁免 %d" % len(lag) if lag else ""),
   not no_core, "、".join("%s=%s" % (p, got.get(p)) for p in no_core[:6]))
if lag:
    print("   WARN 旧码镜像上这几条核心口本来就是坏的（豁免，不是通过）：%s"
          % "、".join("%s=%s" % (p, got.get(p)) for p in lag))

SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                   "apps", "api", "src", "community_http", "leak_bot.rs")
txt = open(SRC, encoding="utf-8").read()
CLAIM = "需 staff 权"


def block_of(route_path):
    """取 #[utoipa::path( … )] 里指向 route_path 的那一段（不能整文件比：
    同文件的 /staff/leaks 确实需要 staff，整文件 substring 会误红）。
    取不到就返回 None 让 DA7 红并写明原因——闸门绝不许用 Traceback 交卷
    （rc=1 而 FAIL=0 的两种死法之一，见记忆卡）。"""
    anchor = 'path = "/api/v1%s"' % route_path
    try:
        i = txt.index(anchor)
        start = txt.rindex("#[utoipa::path(", 0, i)
        return txt[start:txt.index("#[get(", i)]
    except ValueError:
        return None


r_stats = call("GET", "/shoutbox/bot/exec?cmd=/stats", token=MT)
BLK = dict((r, block_of(r)) for r in ("/shoutbox/bot", "/shoutbox/bot/exec"))
lost = [r for r, v in BLK.items() if v is None]
LIE = [r for r, v in BLK.items() if v and CLAIM in v]
ok("DA7 P-336 防回归：喊话机器人两条口是全员只读，契约文案不许写「%s」"
   "（违规 %d、认不出 %d）" % (CLAIM, len(LIE), len(lost)),
   not LIE and not lost, "违规=%s 锚点丢失=%s" % ("、".join(LIE), "、".join(lost)))
ok("DA7b 活体同一句：class 1 成员打得动且回话是真的（http=%s，回话含「站点现状」%s）"
   % (r_stats[0], "站点现状" in json.dumps(r_stats[1], ensure_ascii=False)),
   r_stats[0] == 200 and "站点现状" in json.dumps(r_stats[1], ensure_ascii=False),
   json.dumps(r_stats[1], ensure_ascii=False)[:100])

# 400 一族：都是「缺必填 query 参数」的兼容/管理口，路径集合只许缩不许长
R400 = {"/auth/pow-challenge", "/compat/nexusphp/details.php",
        "/compat/nexusphp/download.php", "/admin/testip",
        "/admin/content-packs/export", "/admin/settings/history",
        "/forums/search", "/follows/status", "/auth/email/verify",
        "/downloads/{torrent_id}", "/admin/user-vouchers", "/ptgen",
        "/admin/user-permissions", "/games/pet/of", "/auth/ban-log",
        "/donate/order-status", "/games/collection/{game}"}
MISS = "缺少必填字段"
bad400 = [p for p, st, b in MINE if st == 400 and p not in R400
          and MISS in json.dumps(b, ensure_ascii=False)]
biz400 = [(p, json.dumps(b, ensure_ascii=False)[:70]) for p, st, b in MINE
          if st == 400 and p not in R400
          and MISS not in json.dumps(b, ensure_ascii=False)]
if biz400:
    print("   [业务 400（不是缺参），只报不判红] %s" % biz400)
ok("DA8 400 一族没长出新面孔（缺参台账 %d 条，新增 %d 条）"
   % (len(R400), len(bad400)), not bad400, "、".join(bad400[:6]))

# ---------- DA12 前端↔API 参数契约对账 ----------
# 活体回 400 且文案是「缺少必填字段：X」的口，apps/web 里必须有一个同时出现
# 该路径与 `X=` 的调用点；少传 = 用户点开就是错误页/空数据。查不到调用点又
# 确实服务外部客户端的，进 EXTERNAL 写明读者是谁（不许用白名单蒙过去）。
EXTERNAL = {
    "/compat/nexusphp/details.php": "NexusPHP 系下载器/客户端的兼容入口",
    "/compat/nexusphp/download.php": "同上（?id=&passkey= 由客户端拼）",
    "/auth/email/verify": "读者是邮件里的链接，不经过前端",
    "/downloads/{torrent_id}": "辅助下载/cross-seed 的一次性 token 链接",
    "/donate/order-status": "支付网关同步回跳查单；页面轮询走 /donate/state",
}
WEB_DIR = os.path.join(ROOTDIR, "apps", "web")
WEB_BLOB = []
for _dp, dn, _fn in os.walk(WEB_DIR):
    dn[:] = [d for d in dn if d not in (".next", "node_modules")]
    for _f in _fn:
        if _f.endswith((".ts", ".tsx")):
            WEB_BLOB.append(io.open(os.path.join(_dp, _f), encoding="utf-8",
                                    errors="replace").read())
WEB_TEXT = "\n".join(WEB_BLOB)


def web_sends(path, field):
    bare = path.split("{")[0].rstrip("/")
    hit = [t for t in WEB_BLOB if bare in t]
    return bool(hit) and any(bare in t and (field + "=") in t for t in hit)


needles = []
for p, st, b in MINE:
    if st != 400 or not isinstance(b, dict):
        continue
    msg = str(b.get("message") or "")
    m = re.search(r"缺少必填字段：([A-Za-z0-9_]+)|缺少\s+([A-Za-z0-9_]+)", msg)
    if not m:
        continue
    field = m.group(1) or m.group(2)
    needles.append((p, field))
broken = [(p, f) for p, f in needles
          if p not in EXTERNAL and not web_sends(p, f)]
outs = [(p, f) for p, f in needles if p in EXTERNAL]
print("   活体要求必填参数的口 %d 条；外部读者 %d 条；前端契约检查 %d 条"
      % (len(needles), len(outs), len(needles) - len(outs)))
for p, f in sorted(outs):
    print("      外部：%s 需要 %s —— %s" % (p, f, EXTERNAL[p]))
ok("DA12 每条「必填 query 参数」的口，前端都有传这个键（违例 %d）" % len(broken),
   not broken, "、".join("%s?%s" % b for b in broken[:6]))
ok("DA12b 正控：契约检查确实能认出「没传键」的形状（拿一个前端不存在的路径试）",
   not web_sends("/api/v1/zz-not-a-route", "whatever") and web_sends(
       "/forums/search", "q"), "判据本身要能红也能绿")

print("\n== DA13 匿名准入：会员身份数据口不许对访客开门 ==")
# 成文口径在 `apps/web/middleware.ts`：「站点准入（PT 惯例）：未登录只放行
# 登录页与静态资源，其余一律 302 → /login」。页面侧有这条，API 侧当时没有：
# 批二百五十匿名扫 321 条 GET 抓到 5 条对访客回真数据的口（P-518/521/522/523）。
# 对照口一起钉，防这把尺把自己跑成全站红（/subtitles 是刻意 optional-auth，
# 见 content_http/subtitles_list.rs 的 P-46/P-47 注释，不在必拒清单里）。
MUST_DENY = ["/preserve", "/pool/honor", "/offers", "/medal-wall",
             "/resurrections"]
MUST_OPEN = ["/health", "/register-mode", "/auth/captcha", "/about"]
amap = {p: (st, b) for p, st, b in ANON}


def anon_pair(p):
    st, b = amap.get(p, (None, None))
    return st, (b or {}).get("code") if isinstance(b, dict) else None


absent = [p for p in MUST_DENY if p not in amap]
leaks = [p for p in MUST_DENY
         if p not in absent and anon_pair(p) != (401, 2001)]
ok("DA13 会员身份数据口匿名一律 401/2001（违例 %d）" % len(leaks),
   not leaks and not absent,
   "还开着的=%s 没扫到=%s（现网旧镜像必红：P-518/521/522/523 等重建）"
   % ("、".join("%s=%s" % (p, anon_pair(p)) for p in leaks),
      "、".join(absent)))
openers = [p for p in MUST_OPEN if anon_pair(p)[0] != 200]
ok("DA13b 正控：该公开的对照口不许被同一把尺误伤（违例 %d）" % len(openers),
   not openers, "非 200 的=%s" % "、".join(
       "%s=%s" % (p, anon_pair(p)) for p in openers))

print("\n== DA14 /me/overview 的等级卡字段（P-515/516/517）==")
# 三条都是「界面读得出、静态门全绿」的形状：text 列按 JSON 解码被吞成 null、
# 分子（做种数×100）与分母（字节门槛）不同量纲、int4 按 i64 读被吞成 0
# ⇒ 下一档恒 LV2。拿探针会员打真接口，字段逐一对库里读数。
ov = data(call("GET", "/me/overview", token=MT)) or {}
DB_NAME = one("SELECT c.name FROM users u LEFT JOIN user_classes c "
              "ON c.id = u.class_id WHERE u.id = %d" % ME)
ok("DA14 class_name 是真名（旧写法对每个会员恒 null）",
   ov.get("class_name") == DB_NAME and bool(DB_NAME),
   "api=%r 库里=%r" % (ov.get("class_name"), DB_NAME))
nc14 = ov.get("next_class")
KEYS14 = ("uploaded_need", "download_count_need", "seed_hours_need",
          "account_age_days_need")
NEXTRA = one("SELECT count(*) FROM class_rules WHERE class_id > "
             "(SELECT class_id FROM users WHERE id = %d) "
             "AND class_id < 90 AND min_uploaded > 0" % ME)
if nc14 is None:
    ok("DA14b 没有下一档时回 null（不许假装一个 required）",
       NEXTRA == "0", "next_class=None 而库里还有 %s 个候选档" % NEXTRA)
else:
    ok("DA14b next_class 是四项条件（旧形状只有 name+required，required 是字节）",
       NEXTRA != "0"
       and all(isinstance(nc14.get(k), int) for k in KEYS14)
       and "required" not in nc14,
       "class_id=%s 候选档=%s keys=%s" % (
           one("SELECT class_id FROM users WHERE id = %d" % ME),
           NEXTRA, sorted(nc14.keys())))
ok("DA14c 假积分键不许回到响应里（站内没有发放/消费点）",
   "seed_points" not in ov, "keys=%s" % sorted(ov.keys())[:12])

print("\n== DA15 我的发布：分桶与总数自洽（3 档谁都不可见，不许漏给作者）==")
# `torrents::visibility` 的成文口径：approval_status=3 对**任何人**都不可见
# （含作者与职员）。旧写法「我的发布」只按 owner_id 过滤 ⇒ 3 档种连同名称、
# 分类、大小、发布时间一起发给作者，而 stats 的三档只数 0/1/2，
# 于是现网 root 看到 total=170 与 162+0+0 对不上——差值就是漏出去的条数。
# 若哪天出现 4 档（回收站/暂缓）在册，这条同样会红，逼出「分桶缺一名」的
# 那个待裁项（见任务 #6 ④），不是误伤。
for who, tk in (("root", ROOT), ("探针会员", MT)):
    dlt = data(call("GET", "/me/torrentlist?kind=uploads&per=1", token=tk)) or {}
    slt = dlt.get("stats") or {}
    tot15 = int(dlt.get("total") or 0)
    sum15 = sum(int(slt.get(k) or 0) for k in ("pending", "approved", "denied"))
    ok("DA15 %s 的「我的发布」total=%s 等于三档之和=%s" % (who, tot15, sum15),
       tot15 == sum15,
       "差的 %d 条是 visibility 口径里谁都不可见的 3 档；rows 里出现的档=%s"
       % (tot15 - sum15, sorted({int(r.get("approval_status") or -1)
                                 for r in (dlt.get("rows") or [])})))

print("\n== 状态分布（成员 / 匿名）==")
for label, rows in (("成员", MINE), ("匿名", ANON)):
    d = {}
    for _p, st, _b in rows:
        d[st] = d.get(st, 0) + 1
    print("   %s：%s" % (label, sorted(d.items())))

# ---------- 收尾：探针走产品自己的墓碑，并断言本轮专属前缀无残留 ----------
if ME:
    call("POST", "/admin/users/status", {"user_id": ME, "status": 2},
         token=ROOT)
    call("DELETE", "/admin/users/%d" % ME, None, token=ROOT)
if INV:
    psql("UPDATE invites SET status = 2 WHERE used_by IS NULL AND id IN (%s)"
         % ",".join(str(int(i)) for i in INV))
TB = psql("SELECT count(*) FROM users WHERE id = %d AND status >= 2 "
          "AND passkey LIKE 'deleted-%%'" % ME) if ME else "0"
ok("DA9 探针已墓碑（tomb=%s/1）" % TB, TB == "1", "tomb=%s" % TB)
REST = psql("SELECT count(*) FROM users WHERE username LIKE 'damb%d'" % TAG)
ok("DA9b 本轮专属前缀无残留（damb%d* = %s）" % (TAG, REST), REST == "0",
   "rest=%s" % REST)

summary()
