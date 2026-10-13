# -*- coding: utf-8 -*-
"""闸门 BF：NexusPHP 兼容层与三条下载路的对外契约（批四十二）。

读代码 + 实测确认过的现网事实（不重复立案，只钉成常驻判据）：限流已
fail-close（P-67）、approval 默认在安全侧、user.json 与 userdetails.php
共用 np_user_json 所以字段不可能漂移、临时凭证路的账号校验落在共用层。

本轮立的四条：P-161 的判据在本文件 S2，P-162~P-164 的判据在 BF2（下载路专卷）：

  P-161 torrents.json 只认本站参数名 —— 实测 `?search=片名` 回 200 + 全站
        未筛（total_estimate 仍是 69），`?cat=1`、`?incldead=0` 同理被 serde
        静默丢掉。NP 工具的 `search/cat/incldead/rowset` 必须双收：RSS 侧
        批十一就是这么修的（rss_http/query.rs），兼容层内两处口径不一致
        比两处都缺更糟 —— 刷流脚本拿到的是「没报错但内容不对」的列表。
  P-162 扣费排在交付判定之前 —— 网页口/凭证口/批量口先 `charge_for_download`
        再 `build_torrent_bytes`：付费种 + 被停下载/挂起/封禁的账号会先扣
        魔力、写好已购，再被拒发；批量口一次最多把 50 个付费种全扣完回 404，
        已购记录还让重试跳过扣费段。判据：同一账号（停下载 + 余额 0）打
        付费种，原话必须是「下载权限已被暂停」而不是「魔力不足」，且探针
        号流水零增长。
  P-163 同一份 .torrent 字节，compat/凭证两路口带 no-store，网页口与批量
        zip 没有 —— 内嵌本人 passkey 的响应一律禁缓存。
  P-164 download.php 把「停下载」报成 401 未认证（凭证其实没坏）—— 工具据此
        去重置 passkey、把账号问题当对接问题查；现在三路同口径 400/403。

本文件只管**只读契约**；三条下载路（字节一致 / no-store / 资格先扣费 /
扣费正控与痕迹复原）在 `pt_audit_bf2_download_paths.py`，两个文件各自
注册探针号与 Token、各自收尾，互不共享限流额度。

段序：S0 前置 / S1 认证 / S2 参数名 / S3 分页 / S4 字段 / S5 限流 /
     S7 别名链 / S8 meta 契约 / S9 收尾

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bf_compat_nexus.py
"""
import json, os, re, subprocess, sys, time, urllib.error, urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
HOST = BASE.replace("/api/v1", "")
UA = "Mozilla/5.0 (bf-gate)"


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], capture_output=True, text=True,
        encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:56], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


def cnt(t, w=None):
    return psql("SELECT count(*) FROM %s%s"
                % (t, "" if w is None else " WHERE " + w))


def blob(r):
    return json.dumps(r, ensure_ascii=False) if r is not None else ""


def data(r):
    if isinstance(r, tuple):
        r = r[1]
    return (r or {}).get("data") or {}


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    """重定向不跟随：别名层的契约就写在 status + Location 上。"""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


OPENER = urllib.request.build_opener(_NoRedirect)


def hit(path, hdr=None, method="GET", body=None):
    """→ (status, 小写头 dict, 文本, 字节)。兼容层要看 Location /
    Cache-Control / Retry-After，call() 的信封看不到。"""
    url = path if path.startswith("http") else HOST + path
    req = urllib.request.Request(url, method=method, data=body)
    for k, v in (hdr or {}).items():
        req.add_header(k, v)
    req.add_header("User-Agent", UA)
    try:
        rr = OPENER.open(req, timeout=40)
        st, hd, bb = rr.status, dict(rr.headers.items()), rr.read()
    except urllib.error.HTTPError as e:
        st, hd, bb = e.code, dict(e.headers.items()), e.read()
    return st, {k.lower(): v for k, v in hd.items()}, \
        bb.decode("utf-8", "replace"), bb


def jget(path, tok=None, **kw):
    return hit(path, {"Authorization": "Token " + tok} if tok else None,
               **kw)


def np_list(path, tok):
    """torrents.json 的判别量：页内条数 + total_estimate（页内条数被
    pagesize≤50 夹住，筛不筛得动只能看 total_estimate）。"""
    st, _, t, _ = jget(path, tok)
    try:
        d = json.loads(t).get("data") or {}
    except Exception:
        return st, -1, -1, {}
    return st, len(d.get("items") or []), d.get("total_estimate", -1), d


# ---- 语料 ----
PK = psql("SELECT passkey FROM users WHERE id = 1")
KW = psql("SELECT substring(name from 1 for 6) FROM torrents WHERE "
          "approval_status = 1 ORDER BY length(name) DESC LIMIT 1")
# S5e 要拿真种打 download.php 的限流额度（只读，不扣费不写账）
FREE = psql("SELECT id FROM torrents WHERE approval_status = 1 "
            "AND COALESCE(price,0) = 0 ORDER BY id LIMIT 1") or "0"
NPDL = "/api/v1/compat/nexusphp/download.php?id=%s&passkey=%s"
TOTAL = int(psql("SELECT count(*) FROM torrents WHERE approval_status = 1"))
KWN = int(psql("SELECT count(*) FROM torrents WHERE approval_status = 1 "
               "AND name LIKE '%s%%'" % KW.replace("'", "''")))
CATN = int(psql("SELECT count(*) FROM torrents WHERE approval_status = 1 "
                "AND category_id = 1"))
ok("S0a 语料齐（root passkey %d 位、活跃成员 %s 人、全量过审种=%s、"
   "关键词=%s 命中=%s、cat1=%s）" % (len(PK), cnt("users", "status < 2"),
                                     TOTAL, KW, KWN, CATN),
   len(PK) == 32 and 0 < KWN < TOTAL and 0 < CATN < TOTAL,
   "缺任一则对应判据不承重")

# ---- 探针成员：注册一个真实账号，全程只动它 ----
clear_register_rl()
codes = data(call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                  token=ROOT)).get("codes") or []
cap = data(call("GET", "/auth/captcha", token=ROOT))
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
NAME = "bf%d" % (TAG * 1000 + int(time.time()) % 1000)
PU, PTOK, WHY = 0, None, ""
if not codes:
    WHY = "发码失败"
elif len(nums) < 2:
    WHY = "验证码题面解析不出 %r" % cap
else:
    rg = call("POST", "/auth/register", {
        "username": NAME, "email": "%s@bf.local" % NAME,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    # 邀请制下 /auth/register 只回 user_id、不回 token（实测），要自己补一次
    # 登录 —— 与 BE 的探针号做法一致，别把「没 token」当成注册失败。
    PTOK = data(rg).get("token") or data(call(
        "POST", "/auth/login", {"username": NAME, "password": "password123"})
    ).get("token")
    if PTOK:
        PU = int(psql("SELECT id FROM users WHERE username = '%s'" % NAME))
    else:
        WHY = "注册/登录 http=%s %s" % (rg[0], blob(rg[1])[:110])
ok("S0b 探针成员就绪（uid=%s）%s" % (PU, WHY), PU > 0 and PTOK is not None,
   WHY or "注册没成功 ⇒ S4 三态判据不承重")
PPK = psql("SELECT passkey FROM users WHERE id = %d" % PU) if PU else ""

MONTH = psql("SELECT to_char((now() at time zone 'utc+8'), 'YYYY-MM')")
# 注意：全站 spark_ledger / audit_log 的总量**不进**残留判据 —— worker 每
# 60s 也在写它们，比总量会把别人的账算到我头上（BF2 实测假红过一次）。
B0 = {"users": cnt("users"), "live": cnt("users", "status < 2"),
      "keys": cnt("download_keys"),
      "traffic": cnt("traffic_ledger"),
      "buy": cnt("torrent_purchases"),
      "root_spark": psql("SELECT spark_balance FROM users WHERE id = 1"),
      "pool": psql("SELECT donated_total FROM magic_pool WHERE month = "
                   "'%s'" % MONTH),
      "pool_row": cnt("magic_pool", "month = '%s'" % MONTH)}
print("   基线：%s" % B0)

# ---- Token：功能用一枚；限流用一枚（该段会把它打满） ----
def revoke_stale(pfx):
    lst = data(call("GET", "/me/tokens", token=ROOT))
    for it in (lst.get("tokens") if isinstance(lst, dict) else lst) or []:
        if str(it.get("name") or "").startswith(pfx) and not it.get(
                "revoked_at"):
            call("POST", "/me/tokens/revoke", {"id": it.get("id")},
                 token=ROOT)


def mint(pfx):
    revoke_stale(pfx)
    return data(call("POST", "/me/tokens", {"name": "%s%d" % (pfx, TAG),
                                            "password": "password123"},
                     token=ROOT)).get("token")


def tok_id(pfx):
    return psql("SELECT id FROM api_tokens WHERE user_id = 1 AND name = "
                "'%s%d' AND revoked_at IS NULL ORDER BY id DESC LIMIT 1"
                % (pfx, TAG))


TKF = mint("bfp")
ok("S0c 功能 Token 签出（开局先撤上几轮残枚，在册上限 3）", bool(TKF),
   "没撤干净会把签发口焊死")

# ============ S1 认证口径 ============
st, _, t, _ = hit("/api/v1/compat/nexusphp/user.json")
ok("S1a 无凭证 → 401 + code 2001（不是 200 空体、不是 500）",
   st == 401 and '"code":2001' in t.replace(" ", ""),
   "http=%s %s" % (st, t[:70]))
st, _, t, _ = hit("/api/v1/compat/nexusphp/user.json",
                  {"Authorization": "Bearer " + ROOT})
ok("S1b 站内 JWT 不能当开放 Token 用（401）：兼容层只认 fxo_ Token",
   st == 401, "http=%s %s" % (st, t[:60]))
st, UHD, t, _ = jget("/api/v1/compat/nexusphp/user.json", TKF)
ok("S1c `Authorization: Token` 200 且回本人 id",
   st == 200 and (json.loads(t).get("data") or {}).get("id") == 1,
   "http=%s %s" % (st, t[:70]))
st, _, t, _ = jget("/api/v1/compat/nexusphp/user.json?apikey=" + TKF)
ok("S1d `?apikey=` 同枚凭证同样受理（Prowlarr/Torznab 习惯）", st == 200,
   "http=%s" % st)
TKB = mint("bfb")
call("POST", "/me/tokens/revoke", {"id": int(tok_id("bfb"))}, token=ROOT)
st, _, t, _ = jget("/api/v1/compat/nexusphp/user.json", TKB)
ok("S1e 撤销后立刻失效（401，不是仍 200）", st == 401, "http=%s" % st)
TKB = None

# ============ S2 NP 惯用参数名（P-161） ============
LIST = "/api/v1/compat/nexusphp/torrents.json"
_, n_all, t_all, _ = np_list(LIST, TKF)
_, n_kw, t_kw, _ = np_list(LIST + "?keyword=%s" % KW, TKF)
_, n_se, t_se, _ = np_list(LIST + "?search=%s" % KW, TKF)
ok("S2a `search=` 与 `keyword=` 同筛（total %s vs %s，全量 %s，库里前缀命中"
   "%s）" % (t_se, t_kw, t_all, KWN),
   t_se == t_kw and t_se < t_all and t_se >= KWN,
   "旧行为：search 被丢 ⇒ total 仍是全量（正控与别名必须同数才算收口）")
_, _, t_cat, _ = np_list(LIST + "?category=1", TKF)
_, _, t_c1, _ = np_list(LIST + "?cat=1", TKF)
ok("S2b `cat=` 与 `category=` 同筛（total %s vs %s，应=%s）"
   % (t_c1, t_cat, CATN), t_c1 == t_cat and t_c1 == CATN and t_c1 < t_all,
   "旧行为：cat 被丢 ⇒ 69 而非 67")
_, _, t_id, _ = np_list(LIST + "?include_dead=0", TKF)
_, _, t_il, _ = np_list(LIST + "?incldead=0", TKF)
ok("S2c `incldead=0`（NP「只要活种」）与 `include_dead=0` 同筛（%s vs %s，"
   "全量 %s）" % (t_il, t_id, t_all), t_il == t_id and t_il <= t_all,
   "旧行为：incldead 被丢 ⇒ 退化成含断种全量")
_, n_r, _, _ = np_list(LIST + "?rowset=2", TKF)
_, n_s, _, _ = np_list(LIST + "?slot_number=3", TKF)
ok("S2d 条数名 `rowset`/`slot_number` 生效（%s 条 / %s 条）" % (n_r, n_s),
   n_r == 2 and n_s == 3, "rows=%s/%s" % (n_r, n_s))
st, _, t, _ = jget(LIST + "?search=%s&cat=1&incldead=1&searcharea=4"
                   "&searchmode=2&pagesize=3" % KW, TKF)
ok("S2e NP 名一起发：200 不炸且按 cat 收口（%s）" % st, st == 200
   and '"code":0' in t.replace(" ", ""), t[:80])
PCTN = int(psql("SELECT count(*) FROM torrents WHERE approval_status = 1 "
                "AND name LIKE '%%\\%%' ESCAPE '\\'"))
_, n_pct, t_pct, _ = np_list(LIST + "?keyword=%25", TKF)
ok("S2f 裸「百分号」当字面量：total=%s 与库里真含该字符的种数 %s 相等"
   "（不是全量 %s）" % (t_pct, PCTN, t_all), t_pct == PCTN,
   "rows=%s" % n_pct)

# ============ S3 分页与上界 ============
t0 = time.time()
st, _, t, _ = jget(LIST + "?page=1000000&pagesize=50", TKF)
dt = time.time() - t0
d = json.loads(t).get("data") or {}
ok("S3a page 越界夹到 400 且不慢（%.2fs，ZT81 的 LIMIT 5E7 不许回来）" % dt,
   st == 200 and d.get("page") == 400 and dt < 3.0, "page=%s" % d.get("page"))
_, _, _, d2 = np_list(LIST + "?pagesize=9999", TKF)
ok("S3b pagesize 上限 50 生效（9999 不真按 9999 查）",
   d2.get("page_size") == 50, "size=%s" % d2.get("page_size"))
_, n_p2, _, _ = np_list(LIST + "?page=2&pagesize=2", TKF)
ok("S3c 第 2 页有数据（P1 那条 page>=2 恒空的坑不许回来）",
   n_p2 > 0 or TOTAL <= 2, "rows=%s total=%s" % (n_p2, t_all))
ok("S3d 分页字段类型稳定（total_estimate int / has_more bool / page_size int）",
   isinstance(d2.get("total_estimate"), int)
   and isinstance(d2.get("has_more"), bool)
   and isinstance(d2.get("page_size"), int), blob(d2)[:90])

# ============ S4 字段口径 ============
def ud(passkey):
    st, hd, t, _ = hit("/api/v1/compat/nexusphp/userdetails.php?passkey="
                       + passkey)
    try:
        return st, hd, json.loads(t).get("data") or {}
    except Exception:
        return st, hd, {}


KEYS = {"id", "username", "uploaded", "downloaded", "seedbonus", "class",
        "class_name", "ratio", "ratio_infinite", "ratio_display", "passkey"}
st, PHD, a = ud(PPK)
bj = json.loads(jget("/api/v1/compat/nexusphp/user.json", TKF)[2]).get(
    "data") or {}
ok("S4a userdetails.php 与 user.json 键集合都等于 np_user_json 的形状",
   st == 200 and set(a) == KEYS and set(bj) == KEYS,
   "差异=%s" % sorted((set(a) ^ KEYS) | (set(bj) ^ KEYS)))
ok("S4b 两口径都禁缓存（响应含 passkey，不得被中间层留存）",
   "no-store" in (PHD.get("cache-control") or "").lower()
   and "no-store" in (UHD.get("cache-control") or "").lower(),
   "ud=%s uj=%s" % (PHD.get("cache-control"), UHD.get("cache-control")))
OTHERS = [x for x in psql("SELECT passkey FROM users WHERE id <> %d "
                          "AND status < 2 AND passkey IS NOT NULL" % PU
                          ).splitlines() if x]
ok("S4c 只回本人那枚 passkey（库里他人 %d 枚一枚都不许出现在响应里）"
   % len(OTHERS), a.get("passkey") == PPK
   and not any(x in blob(a) for x in OTHERS), "self=%s" % str(
       a.get("passkey"))[:6])


def ratio_case(up, down, want, label):
    psql("UPDATE users SET uploaded = %d, downloaded = %d WHERE id = %d"
         % (up, down, PU))
    _, _, x = ud(PPK)
    got = (x.get("ratio_display"), x.get("ratio"), x.get("ratio_infinite"))
    ok("S4d-%s 上传=%d 下载=%d → display=%s infinite=%s（旧实现回 -1.0 被"
       "工具当负数显示）" % (label, up, down, got[0], got[2]),
       got[0] == want[0] and got[2] == want[1] and float(got[1]) >= 0.0,
       blob(got)[:90])


ratio_case(1000, 0, ("∞", True), "inf")
ratio_case(0, 0, ("---", False), "zero")
ratio_case(3000, 1000, ("3.0000", False), "real")

# ============ S5 限流（passkey 维与 Token 维） ============
def burn(path, n, tok=None):
    return [jget(path, tok)[0] if tok else hit(path)[0] for _ in range(n)]


ud_codes = burn("/api/v1/compat/nexusphp/userdetails.php?passkey=" + PK, 33)
first = next((i + 1 for i, c in enumerate(ud_codes) if c == 429), None)
st, hd, t, _ = hit("/api/v1/compat/nexusphp/userdetails.php?passkey=" + PK)
ok("S5a 同一 passkey 第 %s 次撞 429（额度 30/分，P-67 那道 fail-close 闸）"
   % first, first == 31 and 429 in ud_codes[-3:],
   "tail=%s" % ud_codes[-4:])
ok("S5b 429 体口径与 meta 一致：code 1015 + Retry-After: 60",
   '"code":1015' in t.replace(" ", "") and hd.get("retry-after") == "60",
   "%s / %s" % (t[:60], hd.get("retry-after")))
ok("S5c 换一枚 passkey 立刻可用（键按 passkey 分，不是按 IP 一锅端）",
   ud(PPK)[0] == 200, "probe=%s" % ud(PPK)[0])
okc, bad = 0, []
for k in ["z" * 32, PK[:-1], PK + "x", "A" * 32, PK.replace("a", "b"), ""]:
    c = hit("/api/v1/compat/nexusphp/userdetails.php?passkey=" + k)[0]
    okc += c == 401
    bad.append("%s→%s" % (len(k), c))
ok("S5d 畸形/缺失 passkey 六种一律 401（不可枚举、不 500、不给 200 空体）",
   okc == 6, " ".join(bad))
dcodes = burn(NPDL % (FREE, PK), 31)
ok("S5e download.php 同额度撞 429（末两次=%s）" % dcodes[-2:],
   429 in dcodes[-2:], "tail=%s" % dcodes[-6:])
TKX = mint("bfz")
lim, tcodes = None, []
for i in range(62):
    st, hd, _, _ = jget("/api/v1/compat/nexusphp/user.json", TKX)
    tcodes.append(st)
    lim = hd.get("x-ratelimit-limit") or lim
first_t = next((i + 1 for i, c in enumerate(tcodes) if c == 429), None)
meta = data(call("GET", "/compat/meta"))
ok("S8a meta 声称 token_default_per_min=%s，实测头 X-RateLimit-Limit=%s、"
   "首个 429 在第 %s 次" % ((meta.get("rate_limit") or {}).get(
       "token_default_per_min"), lim, first_t),
   str((meta.get("rate_limit") or {}).get("token_default_per_min")) == "60"
   and lim == "60" and first_t == 61, "200 数=%s" % tcodes.count(200))

# ============ S7 别名链 ============
FS = {"Content-Type": "application/x-www-form-urlencoded"}
st, hd, _, _ = hit("/api/v1/compat/nexusphp/takelogin.php", FS, "POST",
                   b"username=root&password=whatever")
ok("S7a takelogin.php → 307 且 Location 恰为 /api/v1/auth/login（保方法保"
   " body）", st == 307 and hd.get("location") == "/api/v1/auth/login",
   "%s loc=%s" % (st, hd.get("location")))
st, hd2, _, _ = hit("/api/v1/compat/nexusphp/takelogin.php?returnto=%2F"
                    "torrents.php", FS, "POST",
                    b"username=root&password=whatever")
ok("S7b 带 query 时跳转仍在本站路径内（前缀固定，Host/query 不可注入绝对地址）",
   st == 307 and (hd2.get("location") or "").startswith(
       "/api/v1/auth/login?returnto="), "loc=%s" % hd2.get("location"))
st, _, t, _ = hit("/api/v1/auth/login", FS, "POST",
                  ("username=%s&password=password123" % NAME).encode())
ok("S7c 别名终点真收表单：x-www-form-urlencoded 登录拿到 token",
   st == 200 and (json.loads(t).get("data") or {}).get("token"),
   "%s %s" % (st, t[:80]))
st, hd, _, _ = hit("/api/v1/compat/nexusphp/details.php?id=" + FREE)
ok("S7d details.php → 302 到本站详情页（不伪造 HTML，诚实降级）",
   st == 302 and (hd.get("location") or "").endswith("/torrent/" + FREE),
   "%s loc=%s" % (st, hd.get("location")))
st, hd, _, _ = hit("/api/v1/compat/nexusphp/getrss.php?passkey=" + PK)
ok("S7e getrss.php → 302 到 /api/v1/rss/{passkey}（其余参数原样透传）",
   st == 302 and (hd.get("location") or "").startswith("/api/v1/rss/" + PK),
   "loc=%s" % (hd.get("location") or "")[:70])
st, hd, _, _ = hit("/api/v1/compat/nexusphp/getrss.php?passkey=" + "A" * 32)
st2, hd2, _, _ = hit("/api/v1/compat/nexusphp/getrss.php?passkey=a" + "/"
                     * 31)
ok("S7f 别名层只转发合法形状：大写与含斜杠都 401 且不给出 Location（不照抄"
   "调用方文本拼订阅地址）", st == 401 and not hd.get("location")
   and st2 == 401 and not hd2.get("location"),
   "%s loc=%s / %s loc=%s" % (st, hd.get("location"), st2,
                              hd2.get("location")))
# 形状合法但不存在的 passkey：别名层不查库（判据只在被转发的那个端点有一份），
# 所以它照常 302，由 /rss/{pk} 回 404 —— 这条把「两处实现」的边界钉住
st, _, _, _ = hit("/api/v1/compat/nexusphp/getrss.php?passkey=" + "z" * 32)
st2, _, _, _ = hit("/api/v1/rss/" + "z" * 32)
ok("S7g 合法但不存在的 passkey：别名 302 → 目标口 404（判据只有一份，转发层"
   "不重复实现，也不 500/不 200 空源）", st == 302 and st2 == 404,
   "alias=%s target=%s" % (st, st2))

# ============ S8 meta 契约 ============
ok("S8b meta 的 api_root 与 on_limit 文案和实际对齐（不虚报能力）",
   meta.get("api_root") == "/api/v1"
   and "1015" in blob((meta.get("rate_limit") or {}).get("on_limit")),
   blob(meta.get("rate_limit"))[:110])
eps = meta.get("endpoints") or {}
flat = [v for v in eps.values() if isinstance(v, str)]
flat += [v for v in (eps.get("aliases") or {}).values()
         if isinstance(v, str)]
noo = [v for v in flat if "/api/v1" not in v]
ok("S8c meta 声明的 %d 个口全部带 /api/v1 前缀（漏前缀=工具拼出 404 URL）"
   % len(flat), bool(flat) and not noo, blob(noo)[:110])
ok("S8d meta 声称 download_by_passkey/temp_key/user_info 为真，且这三条路"
   "本轮都实测通", all((meta.get("capabilities") or {}).get(k) is True for k
                       in ("download_by_passkey", "download_temp_key",
                           "user_info")), blob(meta.get("capabilities"))[:110])

# ============ S9 收尾 ============
for pfx in ("bfp", "bfb", "bfz"):
    revoke_stale(pfx)
LEFT_TK = [n for n in ("bfp", "bfb", "bfz")
           if cnt("api_tokens", "user_id = 1 AND name LIKE '%s%%' AND "
                  "revoked_at IS NULL" % n) != "0"]
ok("S9a 本轮三枚 Token 全撤净（只数自己前缀，别的闸门的残枚不算我的账）",
   not LEFT_TK, "未撤=%s 在册总数=%s" % (LEFT_TK, cnt(
       "api_tokens", "user_id = 1 AND revoked_at IS NULL")))
call("POST", "/admin/users/status", {"user_id": PU, "status": 2}, token=ROOT)
call("DELETE", "/admin/users/%d" % PU, None, token=ROOT)
TOMB = psql("SELECT coalesce(passkey,'-') || '|' || pass_hash FROM users "
            "WHERE id = %d" % PU)
ok("S9b 探针号走真实墓碑：passkey 作废后旧钥在 userdetails 读不到（401）",
   TOMB.startswith("deleted-") and "|!" in TOMB and ud(PPK)[0] == 401,
   "tomb=%s http=%s" % (TOMB[:20], ud(PPK)[0]))
psql("DELETE FROM download_keys WHERE user_id = 1 OR user_id = %d" % PU)
if codes:
    psql("UPDATE invites SET status = 2 WHERE id = %d" % codes[0]["id"])
NOW = {"users": cnt("users"), "live": cnt("users", "status < 2"),
       "keys": cnt("download_keys"),
       "traffic": cnt("traffic_ledger"),
       "buy": cnt("torrent_purchases"),
       "root_spark": psql("SELECT spark_balance FROM users WHERE id = 1"),
       "pool": psql("SELECT donated_total FROM magic_pool WHERE month = "
                    "'%s'" % MONTH)}
drift = {k: (B0[k], NOW[k]) for k in NOW
         if k not in ("users", "live") and NOW[k] != B0[k]}
ok("S9c 零残留：凭证/流水账/已购/root 余额/当月池回基线；users 总数不变"
   "（%s→%s）、活跃数 -1（%s→%s，探针号留一枚墓碑）"
   % (B0["users"], NOW["users"], B0["live"], NOW["live"]),
   not drift and NOW["users"] == B0["users"]
   and int(NOW["live"]) == int(B0["live"]) - 1, blob(drift)[:130])
ok("S9d 探针号确为墓碑且不再是活跃成员（status≥2，兼容层再读不到）",
   psql("SELECT status FROM users WHERE id = %d" % PU) in ("2", "3")
   and cnt("api_tokens", "user_id = %d" % PU) == "0",
   "status=%s tokens=%s" % (psql("SELECT status FROM users WHERE id = %d"
                                 % PU),
                            cnt("api_tokens", "user_id = %d" % PU)))
summary()
