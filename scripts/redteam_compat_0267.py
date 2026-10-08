"""0267 第三方对接修复批 —— 逐项真机验收。

对本地 docker 栈打真实请求，逐条核对报告里的 R-01..R-17 与安全项 S-1..S-7。
用法：python _verify_compat_0267.py
"""
import base64
import os
import hashlib
import hmac
import json
import re
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET

API = "http://127.0.0.1:8080/api/v1"
TRK = "http://127.0.0.1:7070"
ROOT = os.environ.get(
    "FLUX_ROOT",
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
)

ok_n = 0
bad = []


def chk(item, cond, detail=""):
    global ok_n
    if cond:
        ok_n += 1
        print(f"PASS  {item}" + (f"  | {detail}" if detail else ""))
    else:
        bad.append(item)
        print(f"FAIL  {item}  | {detail}")


def req(method, url, headers=None, body=None, follow=False):
    r = urllib.request.Request(url, method=method, data=body,
                               headers=headers or {})
    opener = urllib.request.build_opener()
    if not follow:
        class NoRedirect(urllib.request.HTTPRedirectHandler):
            def redirect_request(self, *a, **k):
                return None
        opener = urllib.request.build_opener(NoRedirect)
    try:
        with opener.open(r, timeout=20) as resp:
            return resp.status, lc(resp.headers), resp.read()
    except urllib.error.HTTPError as e:
        return e.code, lc(e.headers), e.read()


def lc(h):
    """响应头 → 全小写键，避免大小写差异造成假失败"""
    return {k.lower(): v for k, v in h.items()}


def jwt_root():
    secret = re.search(r"^JWT_SECRET=(.+)$",
                       open(f"{ROOT}/docker/.env", encoding="utf-8").read(),
                       re.M).group(1).strip().strip('"').strip("'")
    b64 = lambda b: base64.urlsafe_b64encode(b).rstrip(b"=")
    now = int(time.time())
    h = b64(json.dumps({"alg": "HS256", "typ": "JWT"},
                       separators=(",", ":")).encode())
    p = b64(json.dumps({"sub": 1, "class_id": 1, "iat": now,
                        "exp": now + 21600},
                       separators=(",", ":")).encode())
    s = b64(hmac.new(secret.encode(), h + b"." + p, hashlib.sha256).digest())
    return (b".".join([h, p, s])).decode()


def issue_token(jwt, name, scopes, rpm=600):
    """签发 Token；失败（如 3 枚上限）返回 (None, None) 并打印原因"""
    st, _, b = req("POST", f"{API}/me/tokens",
                   {"Authorization": f"Bearer {jwt}",
                    "Content-Type": "application/json"},
                   json.dumps({"name": name, "scopes": scopes,
                               "rate_per_min": rpm}).encode())
    try:
        j = json.loads(b)
    except Exception:
        j = {}
    if st != 200 or not j.get("data"):
        print(f"[env] Token {name} 签发失败: HTTP {st} {str(b)[:90]}")
        return None, None
    d = j["data"]
    return d["token"], d.get("id")


def jget(url, tok):
    st, h, b = req("GET", url, {"Authorization": f"Token {tok}"})
    try:
        return st, h, json.loads(b)
    except Exception:
        return st, h, b


jwt = jwt_root()

# 先清掉上一轮的验收 Token（每用户 3 枚上限；只动本脚本名下的）
st, _, b = req("GET", f"{API}/me/tokens", {"Authorization": f"Bearer {jwt}"})
if st == 200:
    for t in json.loads(b)["data"]:
        if t["name"].startswith("verify-") and t.get("revoked_at") is None:
            req("POST", f"{API}/me/tokens/revoke",
                {"Authorization": f"Bearer {jwt}",
                 "Content-Type": "application/json"},
                json.dumps({"id": t["id"]}).encode())
    print(f"[env] 清理历史验收 Token 完成")

RTOK, RTOK_ID = issue_token(jwt, "verify-rl", ["read"], rpm=1)
TOK, TOK_ID = issue_token(jwt, "verify-main", ["read"])


# ---------- 取 passkey ----------
st, h, d = jget(f"{API}/compat/nexusphp/user.json", TOK)
PK = d["data"]["passkey"]
# TOR's info_hash 现取（TOK 在发种段会被吊销腾名额，后取会 401）
_, _, d_ih = jget(f"{API}/compat/nexusphp/torrent/40.json", TOK)
IH_HEX = (d_ih.get("data") or {}).get("info_hash")
print(f"\n[env] passkey={PK} info_hash={IH_HEX} tokens issued\n")

# ================= R-02 Torznab caps 合规 =================
st, h, b = req("GET", f"{API}/torznab")
xml = b.decode()
chk("R-02 caps 根节点为 <caps>", "<caps>" in xml and "torznab:search" not in xml)
chk("R-02 caps 含 <searching> 能力声明",
    "<searching>" in xml and "tv-search" in xml and "movie-search" in xml)
chk("R-02 caps 可被 XML 解析", ET.fromstring(xml) is not None)
chk("R-02 caps 分类已映射（含 2000/5000，非只有 8000）",
    'id="2000"' in xml and 'id="5000"' in xml, xml.count("<category"))

# ================= R-03/R-04/R-05 Torznab search =================
st, h, b = req("GET", f"{API}/torznab/search?apikey={TOK}&t=search&q=e2e")
sx = b.decode()
chk("R-03 item 含 torznab:attr seeders",
    'torznab:attr name="seeders"' in sx)
chk("R-03 item 含 downloadvolumefactor", "downloadvolumefactor" in sx)
chk("R-03 item 含 infohash", 'name="infohash"' in sx)
chk("R-04 item 分类非硬编码 8000",
    re.search(r'<category id="(\d+)"', sx).group(1) in {"2000", "4000", "5000"})
chk("R-03 enclosure 带真实 passkey", f"passkey={PK}" in sx, "且 link 同址")

st2, _, b2 = req(
    "GET", f"{API}/torznab/search?apikey={TOK}&t=tvsearch&q=e2e&season=1&ep=2")
n_tv = len(re.findall(r"<item>", b2.decode()))
n_s = len(re.findall(r"<item>", sx))
chk("R-05 tvsearch+season/ep 真的收窄（不再静默忽略）",
    n_tv == 0 and n_s == 1, f"tv={n_tv} search={n_s}")

st3, _, b3 = req("GET", f"{API}/torznab/search?apikey={TOK}&cat=7020")
chk("R-05 命不中的 cat 返回空 feed（非忽略分类）",
    len(re.findall(r"<item>", b3.decode())) == 0)

st4, _, b4 = req("GET", f"{API}/torznab/search?apikey={TOK}&cat=4000")
n_pc = len(re.findall(r"<item>", b4.decode()))
st5, _, b5 = req("GET", f"{API}/torznab/search?apikey={TOK}&cat=7020")
n_nz = len(re.findall(r"<item>", b5.decode()))
chk("R-05 cat 过滤生效（命中分类有结果 / 未知分类为空）",
    n_pc == 1 and n_nz == 0, f"cat=4000→{n_pc} cat=7020→{n_nz} 全量{n_s}")

# ================= R-06 零做种新种可见 =================
st, _, d = jget(f"{API}/compat/nexusphp/torrents.json?page=1", TOK)
items = d["data"]["items"]
chk("R-06 零做种新种默认可见", len(items) >= 1, f"items={len(items)}")
if items:
    it = items[0]
    chk("R-09 列表含 info_hash", bool(it.get("info_hash")))
    chk("R-09 列表含 pieces_hash", it.get("pieces_hash") is not None)
    chk("R-10 列表含 category_name", bool(it.get("category_name")))
    chk("R-10 列表含 category_np / category_newznab",
        it.get("category_np") and it.get("category_newznab"))
    chk("R-10 列表含 medium_name 字段", "medium_name" in it)
    chk("R-10 列表含 promotion_name 字段", "promotion_name" in it)

st, _, d = jget(f"{API}/compat/nexusphp/torrents.json?page=1&alive=1", TOK)
alive1 = d["data"]["items"]
# alive=1 必须是默认（全量）的子集，且逐条 seeders>0
chk("R-06 alive=1 只要活种（子集 + 逐条校验）",
    len(alive1) <= len(items) and all(t["seeders"] > 0 for t in alive1),
    f"全量={len(items)} 活种={len(alive1)}")

st, _, d = jget(f"{API}/compat/nexusphp/torrent/40.json", TOK)
chk("R-09 详情含双指纹",
    bool(d["data"].get("info_hash")) and "pieces_hash" in d["data"])

# ================= R-01 RSS enclosure =================
st, h, b = req("GET", f"{API}/rss/{PK}")
rss = b.decode()
chk("R-01 RSS item 含 <enclosure>", "<enclosure" in rss)
# 断言不绑死 id=40：RSS 按 id DESC 只出最新 50 条，开发库种子流动后旧 id 不在窗口内
chk("R-01 enclosure 指向 download.php + passkey",
    f"download.php?id=" in rss and f"passkey={PK}" in rss)
chk("R-01 RSS 响应禁缓存",
    "no-store" in h.get("cache-control", ""), h.get("cache-control"))
st, _, b = req("GET", f"{API}/rss/{PK}?linktype=page")
chk("R-01 linktype=page 保留详情页链接",
    "/torrent/" in b.decode() and "<enclosure" in b.decode())

# ================= R-12 限流头与 Retry-After =================
st, h, _ = jget(f"{API}/plugins/ptppUserInfo", TOK)
chk("R-12 正常响应带 X-RateLimit-*",
    h.get("x-ratelimit-limit") == "600" and "x-ratelimit-remaining" in h,
    f"limit={h.get('x-ratelimit-limit')} rem={h.get('x-ratelimit-remaining')}")
if RTOK:
    req("GET", f"{API}/plugins/ptppUserInfo",
        {"Authorization": f"Token {RTOK}"})
    st, h, b = req("GET", f"{API}/plugins/ptppUserInfo",
                   {"Authorization": f"Token {RTOK}"})
else:
    st, h, b = 0, {}, b""
chk("R-12 超限 429 + Retry-After",
    st == 429 and h.get("retry-after") == "60",
    f"HTTP {st} retry-after={h.get('retry-after')}")

# ================= R-13 compat/meta =================
st, _, b = req("GET", f"{API}/compat/meta")
d = json.loads(b)["data"]
chk("R-13 meta 含 capabilities", "capabilities" in d and d["capabilities"]["torznab"])
chk("R-13 meta 列出 torznab/rss/open 端点",
    "torznab_search" in d["endpoints"] and "rss" in d["endpoints"]
    and "open_announces" in d["endpoints"])
chk("R-13 meta 声明限流头契约", "rate_limit" in d)

# ================= R-14 NP 别名 =================
st, h, _ = req("GET", f"{API}/compat/nexusphp/getrss.php?passkey={PK}")
chk("R-14 getrss.php → 302 且 Location 正确",
    st in (301, 302) and f"/api/v1/rss/{PK}" in h.get("location", ""),
    f"HTTP {st} {h.get('location')}")
st, h, _ = req("GET", f"{API}/compat/nexusphp/getrss.php?passkey=bad")
chk("R-14 getrss.php 拒绝畸形 passkey", st == 401, f"HTTP {st}")
st, _, b = req("GET", f"{API}/compat/nexusphp/userdetails.php?passkey={PK}")
chk("R-14 userdetails.php 返回 NP 口径 JSON",
    st == 200 and json.loads(b)["data"].get("passkey") == PK, f"HTTP {st}")
st, h, _ = req("GET", f"{API}/compat/nexusphp/details.php?id=40")
chk("R-14 details.php → 302", st in (301, 302), f"HTTP {st}")
st, h, _ = req("POST", f"{API}/compat/nexusphp/takelogin.php",
               {"Content-Type": "application/x-www-form-urlencoded"},
               b"username=root&password=x")
chk("R-14 takelogin.php → 307", st == 307, f"HTTP {st}")

# ================= R-08 表单登录 =================
st, _, b = req("POST", f"{API}/auth/login",
               {"Content-Type": "application/x-www-form-urlencoded"},
               b"username=root&password=definitely-wrong")
chk("R-08 表单登录被接受（不再是 Content type error）",
    "Content type error" not in b.decode(), f"HTTP {st} {b[:90]}")
st, _, _ = req("POST", f"{API}/auth/login",
               {"Content-Type": "application/x-www-form-urlencoded",
                "Origin": "https://evil.example"},
               b"username=root&password=x")
chk("S-3 跨站 Origin 的表单登录被拒（login CSRF）", st == 403, f"HTTP {st}")
st, _, _ = req("POST", f"{API}/auth/login",
               {"Content-Type": "application/json"}, 
               json.dumps({"username": "root", "password": "x"}).encode())
chk("R-08 JSON 登录路径仍可用（不再 Content type error）",
    "Content type error" not in b.decode() and st in (400, 401, 429),
    f"HTTP {st}")

# ================= R-15 增量新种流 =================
st, h, d = jget(f"{API}/open/announces?since_id=0&limit=10", TOK)
chk("R-15 announces 返回 items + 游标",
    "items" in d["data"] and "next_since_id" in d["data"],
    f"count={d['data'].get('count')}")
st, _, d = jget(f"{API}/open/announces?since_id=999999", TOK)
chk("R-15 无新种返回空数组（非 404）",
    st == 200 and d["data"]["items"] == [])
st, _, d = jget(f"{API}/open/recent?limit=5", TOK)
chk("R-09/R-13 open/recent 有数据且带 hash",
    isinstance(d["data"], list) and d["data"]
    and "pieces_hash" in d["data"][0],
    f"n={len(d['data'])}")

# ================= R-16 Token 续期 =================
st, _, b = req("POST", f"{API}/me/tokens/refresh",
               {"Authorization": f"Bearer {jwt}",
                "Content-Type": "application/json"},
               json.dumps({"id": TOK_ID}).encode())
chk("R-16 Token 续期可用", st == 200 and json.loads(b)["data"]["ttl_days"] == 180,
    f"HTTP {st}")

# ================= 0268 API 全面性补齐 =================
st, _, b = req("GET", f"{API}/openapi.json")
spec = json.loads(b)["data"]
paths = sorted(spec["paths"].keys())
chk("D-1 openapi.json 覆盖兼容层/Torznab/RSS（不再半张图）",
    len(paths) >= 22 and "/compat/nexusphp/torrents.json" in paths
    and "/torznab/search" in paths and "/rss/{passkey}" in paths,
    f"paths={len(paths)}")

st, _, d = jget(f"{API}/open/categories", TOK)
cd = d["data"]
chk("D-2 分类字典含三种 id + 媒介表",
    cd["categories"] and cd["categories"][0].get("newznab_id")
    and cd["categories"][0].get("legacy_id") and cd["media"],
    f"cat={len(cd['categories'])} media={len(cd['media'])}")

st, _, d = jget(f"{API}/compat/nexusphp/torrents.json?page=1&min_seeders=999", TOK)
chk("D-3 高级筛选 min_seeders 生效", len(d["data"]["items"]) == 0)
st, _, d = jget(f"{API}/compat/nexusphp/torrents.json?page=1&size_min=1TB", TOK)
chk("D-3 高级筛选 size_min=1TB 过滤掉小种",
    len(d["data"]["items"]) == 0)
st, _, d = jget(f"{API}/compat/nexusphp/torrents.json?page=1&sort=seeders", TOK)
chk("D-3 sort=seeders 可用（HTTP 200）", st == 200 and "items" in d["data"])
st, _, d = jget(
    f"{API}/compat/nexusphp/torrents.json?page=1&promo=free,x2&official=1", TOK)
chk("D-3 promo/official 组合筛选可用", st == 200, f"items={len(d['data']['items'])}")

st, _, d = jget(f"{API}/open/torrents/40?with_nfo=1", TOK)
td = d["data"]
chk("D-4 深详情含文件清单/MediaInfo/倍率",
    "files" in td and "mediainfo" in td
    and "downloadvolumefactor" in td and "sections" in td,
    f"numfiles={td.get('numfiles')}")

for name, url in [("D-5 overview", "/open/me/overview"),
                  ("D-5 seeding", "/open/me/seeding"),
                  ("D-5 history", "/open/me/history"),
                  ("D-5 hr", "/open/me/hr"),
                  ("D-5 messages", "/open/me/messages?unread=1")]:
    st, _, d = jget(f"{API}{url}", TOK)
    chk(name, st == 200 and "data" in d, f"HTTP {st}")

st, _, d = jget(f"{API}/open/me/overview", TOK)
chk("D-5 overview 字段齐全（含 hr_open/unread）",
    all(k in d["data"] for k in
        ("uploaded", "seeding", "unread_messages", "hr_open", "bonus")))

# ================= S-1 scope 强制 =================
# 先取一枚 .torrent 用作上传体
st, _, tor = req("GET",
                 f"{API}/compat/nexusphp/download.php?id=40&passkey={PK}")
BOUND = "----verifyboundary"
def multipart():
    return (f"--{BOUND}\r\n"
            f'Content-Disposition: form-data; name="file"; filename="t.torrent"\r\n'
            f"Content-Type: application/x-bittorrent\r\n\r\n").encode() \
        + tor + f"\r\n--{BOUND}--\r\n".encode()

st, _, b = req("POST", f"{API}/open/torrents?category_id=1",
               {"Authorization": f"Token {TOK}",  # read-only token
                "Content-Type": f"multipart/form-data; boundary={BOUND}"},
               multipart())
chk("S-1 read-only Token 发种被拒 403", st == 403, f"HTTP {st} {b[:80]}")

# 名额有限：read 类 token 已用完使命，吊销腾位再签 upload token
for rid in (TOK_ID, RTOK_ID):
    if rid:
        req("POST", f"{API}/me/tokens/revoke",
            {"Authorization": f"Bearer {jwt}",
             "Content-Type": "application/json"},
            json.dumps({"id": rid}).encode())
UTOK, UTOK_ID = issue_token(jwt, "verify-upload", ["read", "upload"])

if UTOK is None:
    chk("R-07 upload token 缺席（名额被占）→ 跳过", True, "skipped")
    raise SystemExit(1)
st, _, b = req("POST", f"{API}/open/torrents?category_id=1",
               {"Authorization": f"Token {UTOK}",
                "Content-Type": f"multipart/form-data; boundary={BOUND}"},
               multipart())
try:
    dd = json.loads(b)["data"]
except Exception:
    dd = {}
chk("R-07/R-11 upload scope 发种成功且幂等 duplicate=true",
    st == 200 and dd.get("duplicate") is True,
    f"HTTP {st} {str(dd)[:90]}")

st, _, b = req("POST", f"{API}/me/tokens",
               {"Authorization": f"Bearer {jwt}",
                "Content-Type": "application/json"},
               json.dumps({"name": "bad-scope", "scopes": ["admin"]}).encode())
chk("S-1 未知 scope 被拒", st == 400, f"HTTP {st} {b[:70]}")

# ================= R-17 tracker 非 compact =================
# 0267 起 tracker 拒绝未注册的 info_hash（防幽灵 swarm 无界增长），
# 探针必须用真实种子的 hash
if not IH_HEX:
    raise SystemExit("拿不到 info_hash，tracker 段无法验收")
IH = bytes.fromhex(IH_HEX)
PEER = b"-qB4650-" + b"0" * 12
q = urllib.parse.quote_from_bytes
base = (f"{TRK}/announce/{PK}?info_hash={q(IH, safe='')}"
        f"&peer_id={q(PEER, safe='')}&port=51413&uploaded=0&downloaded=0"
        f"&left=0&event=started")
st, _, b = req("GET", base + "&compact=1")
chk("R-17 compact 默认仍为二进制",
    st == 200 and b"5:peers" in b and b"5:peersl" not in b)
st, _, b = req("GET", base + "&compact=0")
chk("R-17 compact=0 回退 peer 字典列表",
    st == 200 and b"5:peersl" in b, b[:80])
chk("R-17 BEP3 键序修正（complete < downloaded < incomplete）",
    b.index(b"8:complete") < b.index(b"10:downloaded") < b.index(b"10:incomplete"))

# ================= 收尾：吊销探针 token =================
for tid in (TOK_ID, UTOK_ID, RTOK_ID):
    if tid:
        req("POST", f"{API}/me/tokens/revoke",
            {"Authorization": f"Bearer {jwt}",
             "Content-Type": "application/json"},
            json.dumps({"id": tid}).encode())

print(f"\n===== {ok_n} PASS / {len(bad)} FAIL =====")
for b_ in bad:
    print("  FAILED:", b_)
raise SystemExit(1 if bad else 0)
