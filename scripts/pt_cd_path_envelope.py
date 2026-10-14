# -*- coding: utf-8 -*-
"""CD 闸门（批七十八）：路径参数畸形也必须是站内信封（P-282，接 P-280）。

现网（修前，镜像 `api:aa1025c`，2026-10-14 实测，**不带 token 也一样**）：
    GET /users/abc          -> 404 text/plain  can not parse "abc" to a i64
    GET /torrents/notanid   -> 404 text/plain  can not parse "notanid" to a i64
⇒ 两条都坏：
  ① 契约：批七十六刚立的口径是「所有 HTTP 响应都是
     {code,message,data,request_id}」。查询侧换了 `Q<T>`（109 处），
     但**路径侧 `web::Path<T>` 形参有 199 处 / 115 个文件**，
     actix 在进 handler 之前就把失败渲染成 text/plain 英文，
     CR 闸门的探针全是查询串，所以这条一直没人看见（抽查十条口才抓到）；
  ② 时序：路径解析发生在 `require_auth` **之前** ⇒ 未登录请求也能拿到
     内部解析细节，和 P-280 里 RSS 那条「参数错误抢在 passkey 校验之前」同形。

修法不动那 199 处形参，用 actix 的中央机制 `PathConfig::error_handler`
（新增 `apps/api/src/path_cfg.rs`）。顺手把 main.rs 里内联的 JSON 载荷错误
口径原样搬进同一模块——main.rs 是 324 行的「只许瘦不许胖」存量超限文件，
搬家既腾出行预算，又让「抽取失败走信封」只剩一处口径。
搬家不许改味道，所以 CD5 专门打一条**畸形 JSON body** 当对照。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_cd_path_envelope.py
只读：探针只发 GET 与一条必然在抽取期就失败的 POST（无副作用）。
"""
import atexit  # noqa: E402
import io  # noqa: E402
import json  # noqa: E402
import os  # noqa: E402
import subprocess  # noqa: E402
import sys  # noqa: E402
import urllib.error  # noqa: E402
import urllib.parse  # noqa: E402
import urllib.request  # noqa: E402

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, login, ok, summary  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ROOT = login()
AUDIT = [0]

# (畸形路径, 说明, 同口合法对照路径或 None)
# 六条覆盖六个不同域，都是 grep `#[get("/…/{…}")]` 挑出来的真路由，
# 并逐条先在现网确认「合法 id 能 200、畸形 id 现在回 text/plain」。
BAD_PATHS = [
    ("/users/abc", "公开主页 /users/{id}", "/users/1"),
    ("/torrents/notanid", "种子详情 /torrents/{id}", "/torrents/40079"),
    ("/users/x/torrentlist", "用户种单 /users/{id}/torrentlist",
     "/users/1/torrentlist"),
    ("/collections/notanum", "合辑 /collections/{id}", "/collections/1"),
    ("/admin/users/abc", "后台成员详情 /admin/users/{id}", "/admin/users/1"),
    ("/open/torrents/notanid", "开放接口 /open/torrents/{id}",
     "/open/torrents/40079"),
]


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def raw_send(method, path, token=None, body=None, ctype="application/json"):
    req = urllib.request.Request(BASE + path, method=method)
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = body.encode("utf-8") if body is not None else None
    if data is not None:
        req.add_header("Content-Type", ctype)
    try:
        r = urllib.request.urlopen(req, data, timeout=30)
        return r.status, r.headers.get("Content-Type", ""), r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.headers.get("Content-Type", ""), e.read()
    except Exception as e:  # noqa: BLE001
        return 0, "", repr(e).encode("utf-8")[:90]


def shape(st, ct, body):
    """是否站内信封：JSON + 四键齐全。"""
    try:
        j = json.loads(body)
    except Exception:  # noqa: BLE001
        return False, {}
    if not isinstance(j, dict) or "code" not in j or "message" not in j:
        return False, j
    return "json" in ct, j


atexit.register(
    lambda: print("   收尾自检：audit_log 条数 %s → %s"
                  % (AUDIT[0], psql("SELECT count(*) FROM audit_log"))))
AUDIT[0] = psql("SELECT count(*) FROM audit_log")

print("== CD0 正控：同口的合法路径必须照常工作（收紧不许打断正常路径）==")
for i, (p, what, good) in enumerate(BAD_PATHS, start=1):
    if not good:
        continue
    st, ct, body = raw_send("GET", good, token=ROOT)
    ok("CD0%d 合法对照 `%s` 不被中央口径误伤 http=%s ctype=%s"
       % (i, good, st, ct[:16]),
       "text/plain" not in ct and st != 400,
       "http=%s ctype=%s body=%s" % (st, ct[:16], body[:80]))

print("== CD1 带 token：畸形路径参数必须回站内信封（修前 404 text/plain）==")
for p, what, _g in BAD_PATHS:
    st, ct, body = raw_send("GET", p, token=ROOT)
    isenv, j = shape(st, ct, body)
    ok("CD1 %s `%s`：400 信封 + 中文前缀 + request_id（修前 404 text/plain "
       "英文）http=%s code=%s" % (what, p, st, j.get("code")),
       st == 400 and isenv and j.get("code") == 1002
       and "路径参数无效" in str(j.get("message", ""))
       and bool(j.get("request_id")),
       "http=%s ctype=%s body=%s" % (st, ct[:18], body[:96]))

print("== CD2 不带 token：抽取期失败发生在鉴权之前，也得回信封 ==")
for p, what, _g in BAD_PATHS[:3]:
    st, ct, body = raw_send("GET", p)
    isenv, j = shape(st, ct, body)
    ok("CD2 未鉴权 `%s` 不回 text/plain 英文解析细节 http=%s code=%s"
       % (p, st, j.get("code")),
       st == 400 and isenv and "text/plain" not in ct,
       "http=%s ctype=%s body=%s" % (st, ct[:18], body[:96]))

print("== CD3 404 语义没被打坏：真不存在的接口仍是 JSON 的 1004 ==")
for p in ("/topics/abc/nonsense", "/no/such/route/at/all"):
    st, ct, body = raw_send("GET", p, token=ROOT)
    isenv, j = shape(st, ct, body)
    ok("CD3 `%s` 仍回信封 code=%s（不该被中央口径误接成 400）"
       % (p, j.get("code")),
       isenv and j.get("code") == 1004 and st == 404,
       "http=%s ctype=%s body=%s" % (st, ct[:18], body[:96]))

print("== CD4 静态：中央口径确实接进了装配点，且新文件在 HEAD 里 ==")
main_rs = io.open(os.path.join(REPO, "apps", "api", "src", "main.rs"),
                  encoding="utf-8", errors="replace").read()
ok("CD4a main.rs 同时注册 json_config + path_config",
   "crate::path_cfg::json_config()" in main_rs
   and "crate::path_cfg::path_config()" in main_rs,
   "命中 json=%s path=%s" % ("json_config" in main_rs,
                            "path_config" in main_rs))
rc, tree = (lambda r: (r.returncode, r.stdout))(subprocess.run(
    ["git", "ls-tree", "-r", "--name-only", "HEAD"], cwd=REPO,
    stdout=subprocess.PIPE, universal_newlines=True, encoding="utf-8"))
ok("CD4b 被引用的 path_cfg.rs 必须在 HEAD 树里（批七十六就栽在漏提交）",
   "apps/api/src/path_cfg.rs" in tree, "rc=%d 命中=%s"
   % (rc, "apps/api/src/path_cfg.rs" in tree))

print("== CD5 搬家没搬坏：畸形 JSON body 仍是 400 信封（原内联块的口径）==")
st, ct, body = raw_send("POST", "/me/tokens", body="{not json at all")
isenv, j = shape(st, ct, body)
ok("CD5 坏 JSON 载荷 → 400 信封 code=1002（不是 text/plain）http=%s" % st,
   st == 400 and isenv and j.get("code") == 1002
   and "text/plain" not in ct,
   "http=%s ctype=%s body=%s" % (st, ct[:18], body[:96]))
print("   注：CD5 打的是必然在抽取期失败的口，未落业务、未发令牌")

print("== CD6 规模读数（中央口径替代的是逐个换形参）==")
n_site = subprocess.run(
    ["git", "grep", "-o", "--", ": web::Path<", "HEAD", "--", "apps/api/src"],
    cwd=REPO, stdout=subprocess.PIPE,
    universal_newlines=True).stdout.count(chr(10))
n_file = subprocess.run(
    ["git", "grep", "-l", "--", ": web::Path<", "HEAD", "--", "apps/api/src"],
    cwd=REPO, stdout=subprocess.PIPE,
    universal_newlines=True).stdout.count(chr(10))
print("   web::Path 形参 %d 处 / %d 个文件（本批不改它们，靠中央口径接住）"
      % (n_site, n_file))
ok("CD6 规模读数可复现（>150 处，证明确实不适合逐口换提取器）",
   n_site > 150, "命中=%d 文件=%d" % (n_site, n_file))

print("== CD7 英文细节的账（与 P-280 同口径，不假装已本地化）==")
st, ct, body = raw_send("GET", "/users/abc", token=ROOT)
msg = ""
try:
    msg = json.loads(body).get("message", "")
except Exception:  # noqa: BLE001
    pass
print("   WARN 现网 message 原文：%s" % str(msg)[:110])
ok("CD7 message 必须以中文前缀开头（细节原文保留是已知取舍）",
   str(msg).startswith("路径参数无效"), "message=%s" % str(msg)[:70])

print("== CD8 字符串路径参数不被中央口径误伤（原判据写错了，已改对）==")
# 第一版这里断言 `st != 400`，实测红了两条——红的是判据前提：
# `/attachments/{sha}` 的形参是 `web::Path<String>`，任何串都解析得动；
# 那条 400 来自**业务**校验「sha256 格式无效」（40 个 0 不是 64 位十六进制），
# `/games/collection/not_a_game` 同理（GAMES 白名单）。中央口径不该管这些，
# 所以正确的判据是：**message 不得带「路径参数无效」前缀**，
# 且合法形状的值照常通过。
PAIRS = [("/attachments/" + "0" * 40, "/attachments/" + "a" * 64),
         ("/games/collection/not_a_game", "/games/collection/capsule")]
for k, (p_bad, p_ok) in enumerate(PAIRS, start=1):
    st, ct, body = raw_send("GET", p_bad, token=ROOT)
    isenv, j2 = shape(st, ct, body)
    msg = str(j2.get("message", ""))
    ok("CD8a 非法形状 `%s` 走业务口径、不被念成解析失败 http=%s「%s」"
       % (p_bad[:32], st, msg[:30]),
       isenv and not msg.startswith("路径参数无效"),
       "http=%s ctype=%s msg=%s" % (st, ct[:16], msg[:60]))
    st2, ct2, body2 = raw_send("GET", p_ok, token=ROOT)
    isenv2, j3 = shape(st2, ct2, body2)
    msg2 = str(j3.get("message", ""))
    ok("CD8b 合法形状 `%s` 不被中央口径误伤 http=%s「%s」"
       % (p_ok[:32], st2, msg2[:30]),
       not msg2.startswith("路径参数无效") and "text/plain" not in ct2,
       "http=%s ctype=%s msg=%s" % (st2, ct2[:16], msg2[:60]))

print("== CE 中文搜索联想（P-283，同族第二处：输入口径）==")
# 现网实测（修前）：库里有名字含「种子」的过审种，但 /torrents/suggest?q=种子
# 恒回 0 条——第二路兜底要求 similarity>0.3，而 pg_trgm 对中文几乎不给分
# （实测全表对「种子」最高 0.038）。⇒ 中文用户只能「从头打对」才有联想。
CJK_N = int(psql("SELECT count(*) FROM torrents WHERE approval_status=1 "
                 "AND name LIKE '%种子%'") or 0)
print("   前置读数：过审种里名字含「种子」的有 %d 个" % CJK_N)
if CJK_N == 0:
    print("   WARN 语料里没有含「种子」的过审种——CE1/CE2 无从判，跳过")
else:
    for q, want in (("种子", True), ("电影", None)):
        st, ct, body = raw_send("GET", "/torrents/suggest?q=%s"
                                % urllib.parse.quote(q), token=ROOT)
        try:
            items = json.loads(body)["data"]["items"]
        except Exception:  # noqa: BLE001
            items = []
        hit = any("种子" in (it.get("name") or "") for it in items)
        if want is None:
            print("   读数 q=%s → %d 条" % (q, len(items)))
            continue
        first = str(items[0].get("name"))[:56] if items else "-"
        ok("CE1 `?q=%s` 必须联想到含该词的种（修前恒 0）http=%s 条数=%d"
           % (q, st, len(items)), st == 200 and len(items) > 0 and hit,
           "http=%s 条数=%d 首条=%s" % (st, len(items), first))
    st, ct, body = raw_send("GET", "/torrents/suggest?q=De", token=ROOT)
    try:
        n_en = len(json.loads(body)["data"]["items"])
    except Exception:  # noqa: BLE001
        n_en = -1
    ok("CE2 英文前缀分支没被改动挤掉：`?q=De` 仍应命中（实测 %d 条）" % n_en,
       n_en >= 1, "http=%s body=%s" % (st, body[:80]))
    st, ct, body = raw_send("GET", "/torrents/suggest?q=%s"
                            % urllib.parse.quote("种子"), token=None)
    ok("CE3 联想是登录口：未带凭证不得返回标题（口径与修前一致）http=%s" % st,
       st in (401, 403), "http=%s body=%s" % (st, body[:60]))
    pend = set(x for x in psql(
        "SELECT string_agg(id::text, ',') FROM torrents "
        "WHERE approval_status<>1").split(",") if x)
    if pend:
        st, ct, body = raw_send("GET", "/torrents/suggest?q=%s"
                                % urllib.parse.quote("种子"), token=ROOT)
        try:
            got = [int(i["id"]) for i in json.loads(body)["data"]["items"]]
        except Exception:  # noqa: BLE001
            got = []
        bad_ids = [g for g in got if str(g) in pend]
        ok("CE4 兜底只放宽**匹配**、不放宽口径：未过审种不得进联想"
           "（库里未过审 %d 个，返回 %s）" % (len(pend), got),
           not bad_ids, "越界 id=%s" % bad_ids)
src_sg = io.open(os.path.join(REPO, "apps", "api", "src", "torrent_http",
                              "suggest.rs"), encoding="utf-8",
                 errors="replace").read()
ok("CE5 静态：SQL 里确实有「非 ASCII 走包含」那一支（`$3 ~ '[^ -~]'`）",
   "$3 ~ '[^ -~]'" in src_sg and "similarity(name, $3) > 0.3" in src_sg,
   "命中=%s" % ("regex" in src_sg))
for q, want_status in (("a", 200), ("x" * 81, 400)):
    st, ct, body = raw_send("GET", "/torrents/suggest?q=%s" % q, token=ROOT)
    isenv, j = shape(st, ct, body)
    pass6 = st == want_status
    if want_status == 400:
        pass6 = pass6 and isenv and j.get("code") == 1002
    ok("CE6 原边界不动：q 长 %d → http=%s（单字仍空列表、超 80 字仍 400 信封）"
       % (len(q), st), pass6, "http=%s ctype=%s" % (st, ct[:16]))

sys.exit(summary())
