# -*- coding: utf-8 -*-
"""闸门 BF2：三条下载路的字节一致、禁缓存与「资格先于扣费」（批四十二）。

BF 管兼容层的读口契约，本文件专管下载 —— 因为批四十二的四条立案里有三条
落在「同一份 .torrent 的三条出口」上，判据需要真扣费、真冻结、真挂起，
和只读契约分开跑才不会被彼此的限流额度互相踩。

  P-162 扣费排在交付判定之前 —— 网页口 / 临时凭证口 / 批量口原先都先
        `charge_for_download` 再 `build_torrent_bytes`。实测旧行为（探针号
        余额 0 + 停下载，打 500 魔力的付费种）：
          GET /torrents/41954/download → 400「魔力不足：该种子为付费种子…」
        即扣费段先跑；批量口更狠，`for id in ids { charge(); build(); }`
        会把选中的付费种逐个扣完再因 `files.is_empty()` 回 404。
        修后同一条请求必须回 400「您的下载权限已被暂停，请联系管理组」，
        且探针号 spark_ledger 零增长。
  P-163 同一份字节：compat / 凭证两路口带 `private, no-store`，网页口与
        批量 zip 原先没有 —— 内嵌本人 passkey 的响应一律禁缓存。
  P-164 download.php 原先把「停下载/挂起」报成 401 未认证（凭证其实没坏），
        工具据此去重置 passkey、把账号问题当对接问题查。现在三路同口径。

正控（S6k/S6l）：解冻 + 给足火花后，付费种照常扣费、卖方照常入账、重放不
二次扣费 —— 修顺序不许把 0086 的扣费链改坏。本文件是批四十二唯一会真的
动钱的闸门，收尾把 root 余额、当月魔法池、双方流水、已购行逐项复原并回读
证明（S6o）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bf2_download_paths.py
"""
import json, os, re, subprocess, sys, time, urllib.error, urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
HOST = BASE.replace("/api/v1", "")
DL = "/api/v1/torrents/%s/download"
NPDL = "/api/v1/compat/nexusphp/download.php?id=%s&passkey=%s"
DKEY = "/api/v1/downloads/%s?token=%s"
BATCH = "/api/v1/torrents/batch-download"
JS = {"Content-Type": "application/json"}


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


def hit(path, hdr=None, method="GET", body=None):
    url = path if path.startswith("http") else HOST + path
    req = urllib.request.Request(url, method=method, data=body)
    for k, v in (hdr or {}).items():
        req.add_header(k, v)
    try:
        rr = urllib.request.urlopen(req, timeout=40)
        st, hd, bb = rr.status, dict(rr.headers.items()), rr.read()
    except urllib.error.HTTPError as e:
        st, hd, bb = e.code, dict(e.headers.items()), e.read()
    return st, {k.lower(): v for k, v in hd.items()}, \
        bb.decode("utf-8", "replace"), bb


def cc(h):
    return (h.get("cache-control") or "").lower()


# ---- 语料 ----
PK = psql("SELECT passkey FROM users WHERE id = 1")
FREE = psql("SELECT id FROM torrents WHERE approval_status = 1 "
            "AND COALESCE(price,0) = 0 AND owner_id <> 1 ORDER BY id LIMIT 1")
PAID = psql("SELECT id, price FROM torrents WHERE approval_status = 1 "
            "AND COALESCE(price,0) > 0 AND owner_id IS NOT NULL "
            "ORDER BY price DESC LIMIT 1").split("|")
UN = psql("SELECT id FROM torrents WHERE approval_status <> 1 AND id > 0 "
          "ORDER BY id LIMIT 1")
if len(PAID) != 2:
    PAID = ["0", "0"]
PRICE = int(PAID[1] if PAID[1].isdigit() else 0)
ok("S0a 语料齐（root passkey %d 位、免费种=%s、付费种=%s@%d、未过审=%s）"
   % (len(PK), FREE, PAID[0], PRICE, UN),
   len(PK) == 32 and FREE.isdigit() and UN.isdigit() and PRICE > 0,
   "缺任一则扣费判据不承重")

# ---- 探针成员（全程只动它） ----
clear_register_rl()
codes = data(call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                  token=ROOT)).get("codes") or []
cap = data(call("GET", "/auth/captcha", token=ROOT))
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
NAME = "bf2d%d" % (TAG * 1000 + int(time.time()) % 1000)
PU, PTOK, WHY = 0, None, ""
if not codes:
    WHY = "发码失败"
elif len(nums) < 2:
    WHY = "验证码题面解析不出 %r" % cap
else:
    rg = call("POST", "/auth/register", {
        "username": NAME, "email": "%s@bf2.local" % NAME,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    # 邀请制下 /auth/register 只回 user_id、不回 token（实测），补一次登录
    PTOK = data(rg).get("token") or data(call(
        "POST", "/auth/login", {"username": NAME, "password": "password123"})
    ).get("token")
    if PTOK:
        PU = int(psql("SELECT id FROM users WHERE username = '%s'" % NAME))
    else:
        WHY = "注册/登录 http=%s %s" % (rg[0], blob(rg[1])[:110])
ok("S0b 探针成员就绪（uid=%s）%s" % (PU, WHY), PU > 0 and PTOK is not None,
   WHY or "注册没成功 ⇒ 扣费段整段不承重")
PPK = psql("SELECT passkey FROM users WHERE id = %d" % PU) if PU else ""
JH = {"Authorization": "Bearer " + (PTOK or "")}
RH = {"Authorization": "Bearer " + ROOT}


def tok_id(pfx):
    return psql("SELECT id FROM api_tokens WHERE user_id = 1 AND name = "
                "'%s%d' AND revoked_at IS NULL ORDER BY id DESC LIMIT 1"
                % (pfx, TAG))


def mint(pfx, tok=None):
    revoke_my(pfx, tok)
    return data(call("POST", "/me/tokens", {
        "name": "%s%d" % (pfx, TAG), "password": "password123"},
        token=tok or ROOT)).get("token")


LAST_ISSUE = {"st": 0, "body": ""}


def issue_key(tok, tid):
    """签不出凭证时把 http 状态与响应体留档：S2c 要用它区分「闸门拒」与
    「签发口坏了」——只看 None 分不清。每次调用先清空，免得上一次的状态
    被这一次读成判决。"""
    LAST_ISSUE.update(st=0, body="")
    rq = urllib.request.Request(BASE + "/downloads/keys", method="POST")
    rq.add_header("Authorization", "Token " + (tok or ""))
    rq.add_header("Content-Type", "application/json")
    try:
        body = urllib.request.urlopen(
            rq, json.dumps({"torrent_id": int(tid)}).encode(),
            timeout=30).read()
        return json.loads(body)["data"]["key"]
    except urllib.error.HTTPError as e:
        LAST_ISSUE["st"] = e.code
        try:
            LAST_ISSUE["body"] = e.read().decode("utf-8", "replace")[:300]
        except Exception:
            pass
        print("   签发凭证异常：http=%s %s"
              % (e.code, LAST_ISSUE["body"][:80]))
        return None
    except Exception as e:
        LAST_ISSUE["st"] = -1
        LAST_ISSUE["body"] = "%s: %s" % (type(e).__name__, str(e)[:120])
        print("   签发凭证异常：%s" % LAST_ISSUE["body"][:80])
        return None


def my_tokens(pfx, tok=None):
    """按名字前缀取本轮自己签的 Token（列表回显是掩码，只能按 name 找 id）。
    必须只挑未撤销的那枚：同名旧行还留在表里。"""
    lst = data(call("GET", "/me/tokens", token=tok or ROOT))
    return [it for it in (lst.get("tokens") if isinstance(lst, dict) else
                          lst) or []
            if str(it.get("name") or "").startswith(pfx)
            and not it.get("revoked_at")]


def revoke_my(pfx, tok=None):
    for it in my_tokens(pfx, tok):
        call("POST", "/me/tokens/revoke", {"id": it.get("id")},
             token=tok or ROOT)
    return len(my_tokens(pfx, tok))


TKF = mint("bf2t")
# 探针号自己的开放 Token：凭证下载口必须用**被冻那个账号**的 Token 签发，
# 否则签出来的是 root 的凭证，冻探针号根本管不住它（第一轮实测就把 200
# 当成「凭证口没闸门」，其实是判据打错了对象）。
PTK = data(call("POST", "/me/tokens", {"name": "bf2p%d" % TAG,
                                       "password": "password123"},
                token=PTOK)).get("token")
ok("S0d 探针号自己的 Token 签出（凭证口要用它的身份）", bool(PTK),
   "没签出来 ⇒ S2c 判据不承重")
MONTH = psql("SELECT to_char((now() at time zone 'utc+8'), 'YYYY-MM')")
B0 = {"users": cnt("users"), "live": cnt("users", "status < 2"),
      "keys": cnt("download_keys"), "ledger": cnt("spark_ledger"),
      "buy": cnt("torrent_purchases"),
      # traffic 不再用全表计数：worker 每 60s 就在写 traffic_ledger（与同文件
      # S7e 对 spark_ledger 的判据同一个根因），收尾时全表计数必然比基线多
      # ——实测并跑时凭空多出 171/172 两行 2026-10-06 的历史流水，被算成残留。
      # 改成「本闸门探针号自己的行数」，别人的写入一概不算。
      "traffic": cnt("traffic_ledger", "user_id = %d" % PU),
      "snatch": cnt("snatches"),
      "root_spark": psql("SELECT spark_balance FROM users WHERE id = 1"),
      "pool": psql("SELECT donated_total FROM magic_pool WHERE month = "
                   "'%s'" % MONTH),
      "pool_row": cnt("magic_pool", "month = '%s'" % MONTH)}
print("   基线：%s" % B0)
ok("S0c 开放 Token 签出（凭证口要用；先撤上几轮同名残枚）", bool(TKF),
   "在册上限 3 枚，攒满会把签发口焊死")

# ============ S1 三路字节一致 + 禁缓存（P-163） ============
st, h1, _, wb = hit(DL % FREE, RH)
st2, h2, _, nb = hit(NPDL % (FREE, PK))
K1 = issue_key(TKF, FREE)
st3, h3, _, kb = hit(DKEY % (FREE, K1))
ok("S1a 三路产出同一份 .torrent（网页 %d / 兼容 %d / 凭证 %d 全等，bencode "
   "起手）" % (len(wb), len(nb), len(kb)),
   st == st2 == st3 == 200 and wb and wb == nb == kb and wb[:1] == b"d",
   "%s/%s/%s" % (st, st2, st3))
ok("S1b 内嵌本人 announce（passkey 就在字节里）⇒ 三路都 no-store",
   PK.encode() in nb and all("no-store" in cc(h) for h in (h1, h2, h3)),
   "web=%s np=%s key=%s" % (cc(h1)[:16], cc(h2)[:16], cc(h3)[:16]))
st, hb, _, zb = hit(BATCH, dict(RH, **JS), "POST",
                    json.dumps({"ids": [int(FREE)]}).encode())
ok("S1c 批量 zip 同样 no-store（包里每颗 .torrent 都带本人 passkey）",
   st == 200 and zb[:2] == b"PK" and "no-store" in cc(hb),
   "%s ct=%s cc=%s" % (st, hb.get("content-type"), cc(hb)[:24]))
ok("S1d zip 里那颗 .torrent 确是本人事儿的字节（announce 含 root passkey）",
   PK.encode() in zb, "size=%d" % len(zb))

# ============ S2 资格先于扣费（P-162 / P-164） ============
psql("UPDATE users SET spark_balance = 0 WHERE id = %d" % PU)
call("PUT", "/admin/users/flags", {"user_id": PU, "download_enabled": False},
     token=ROOT)
L0 = cnt("spark_ledger", "user_id = %d" % PU)
LOCK = psql("SELECT coalesce(download_locked_by,'-') FROM users WHERE id = %d"
            % PU)
st, _, t, _ = hit(DL % PAID[0], JH)
ok("S2a 停下载 + 余额 0 的账号打**付费**种（网页口）：原话是「下载权限已被"
   "暂停」而不是「魔力不足」（%s）" % st,
   st == 400 and "已被暂停" in t and "魔力不足" not in t, t[:120])
ok("S2b 冻结走管理接口：download_locked_by=staff（ratio_watch 巡检不许误"
   "解）", LOCK == "staff", "lock=%s" % LOCK)
K2 = issue_key(PTK, PAID[0])
if K2 is None:
    # P-344 之后签发口自己先过 ensure_downloadable（download_keys.rs:67），
    # 冻着的账号根本签不出凭证——这是合格行为，不是「签发口坏了」。判据钉
    # 「拒的理由」：http=400 且原话是停下载；若回 401/404/500 仍红。
    ok("S2c 临时凭证口同判据：签发口先拒冻结账号（http=%s，原话「已被暂停」）"
       % LAST_ISSUE["st"],
       LAST_ISSUE["st"] == 400 and "已被暂停" in LAST_ISSUE["body"]
       and "魔力不足" not in LAST_ISSUE["body"], LAST_ISSUE["body"][:120])
else:
    st, _, t, _ = hit(DKEY % (PAID[0], K2))
    ok("S2c 临时凭证口同判据：%s" % st,
       st == 400 and "已被暂停" in t and "魔力不足" not in t, t[:100])
st, _, t, _ = hit(NPDL % (PAID[0], PPK))
ok("S2d 兼容口同判据（P-164）：凭证没坏就不再报 401，回 400 + 可行动原话",
   st == 400 and "已被暂停" in t, "%s %s" % (st, t[:100]))
st, _, t, _ = hit(BATCH, dict(JH, **JS), "POST",
                  json.dumps({"ids": [int(PAID[0]), int(FREE)]}).encode())
L1 = cnt("spark_ledger", "user_id = %d" % PU)
ok("S2e 批量口整单只判一次资格（旧行为：先把选中的付费种逐个扣完再回 404）"
   "：%s 且探针流水 %s→%s" % (st, L0, L1),
   st == 400 and "已被暂停" in t and L1 == L0, blob(t)[:90])
ok("S2f 三条路都没扣到钱（探针 spark_ledger / torrent_purchases 零增长）",
   L1 == L0 and cnt("torrent_purchases", "user_id = %d" % PU) == "0",
   "流水=%s 已购=%s" % (L1, cnt("torrent_purchases",
                                "user_id = %d" % PU)))
call("PUT", "/admin/users/flags", {"user_id": PU, "suspended": True},
     token=ROOT)
st, _, _, _ = hit(NPDL % (FREE, PPK))
st2, _, _, _ = hit(DL % FREE, JH)
st3, _, _, _ = hit(BATCH, dict(JH, **JS), "POST",
                   json.dumps({"ids": [int(FREE)]}).encode())
ok("S3a 挂起账号三路都 403 Forbidden（不是 401、不是 200、不是 500）",
   st == 403 and st2 == 403 and st3 == 403, "%s/%s/%s" % (st, st2, st3))
call("PUT", "/admin/users/flags", {"user_id": PU, "suspended": False},
     token=ROOT)
call("PUT", "/admin/users/flags", {"user_id": PU, "download_enabled": True},
     token=ROOT)

# ============ S4 未过审与非法 id（拒得对，且不退化成 401） ============
st, _, t, _ = hit(NPDL % (UN, PK))
st2, _, _, _ = hit(DL % UN, RH)
st3, _, t3, _ = hit(BATCH, dict(RH, **JS), "POST",
                    json.dumps({"ids": [int(UN)]}).encode())
ok("S4a 未过审种：兼容口/网页口/批量口都 404（不是 200 空壳、不是 401）",
   st == 404 and st2 == 404, "np=%s web=%s batch=%s %s" % (
       st, st2, st3, t[:40]))
ok("S4b 批量口全项失败时不静默回空 zip（400 + 点名「都没能打包」，不是 "
   "200+0 条目；P-432/HX 钉的就是这条文案）",
   st3 == 400 and "都没能打包" in t3, "batch=%s %s" % (st3, t3[:120]))
st, _, t, _ = hit(NPDL % ("-1", PK))
st2, _, _, _ = hit(NPDL % ("99999999", "z" * 32))
ok("S4c 非法 id / 畸形 passkey 安全拒（400/401/404，不 500）",
   st in (400, 404) and st2 in (400, 401, 404), "%s / %s" % (st, st2))

# ============ S5 正控：解冻后照常扣费（修顺序不许改坏 0086） ============
psql("UPDATE users SET spark_balance = 200000 WHERE id = %d" % PU)
IDEM_S = "torrent-sell:%d:%s" % (PU, PAID[0])
IDEM_B = "torrent-buy:%d:%s" % (PU, PAID[0])
PRE_B = cnt("spark_ledger", "user_id = %d AND kind = 'torrent_buy'" % PU)
st, hj, _, body = hit(NPDL % (PAID[0], PPK))
DEBT = psql("SELECT coalesce(sum(amount),0) FROM spark_ledger WHERE "
            "idempotency_key = '%s'" % IDEM_B)
NET = psql("SELECT amount FROM spark_ledger WHERE idempotency_key = '%s'"
           % IDEM_S)
BUYROWS = cnt("spark_ledger", "user_id = %d AND kind = 'torrent_buy'" % PU)
BUY = cnt("torrent_purchases", "user_id = %d" % PU)
ok("S5a 正控：解冻后 compat 下付费种 200，买方扣 -%d、卖方入账 %s、已购 1 行"
   % (PRICE, NET),
   st == 200 and body[:1] == b"d" and BUYROWS == str(int(PRE_B) + 1)
   and DEBT == "-%d" % PRICE and BUY == "1" and int(NET or 0) > 0,
   "st=%s 买方流水 %s→%s 扣=%s 入账=%s 已购=%s" % (st, PRE_B, BUYROWS, DEBT,
                                                  NET, BUY))
ok("S5b 凭证那一路响应也 no-store（同一份字节）", "no-store" in cc(hj),
   "cc=%s" % cc(hj)[:26])
st, _, _, b2 = hit(NPDL % (PAID[0], PPK))
st2, _, _, _ = hit(DKEY % (PAID[0], issue_key(TKF, PAID[0])))
BUYROWS2 = cnt("spark_ledger", "user_id = %d AND kind = 'torrent_buy'" % PU)
ok("S5c 重放不二次扣费（已购记录跳过扣费段：买方流水仍 %s 条、已购仍 1 行）"
   % BUYROWS,
   st == 200 and st2 == 200 and b2[:1] == b"d" and BUYROWS2 == BUYROWS
   and cnt("torrent_purchases", "user_id = %d" % PU) == "1",
   "%s/%s 流水 %s→%s" % (st, st2, BUYROWS, BUYROWS2))

# 复原本轮真实扣费痕迹：双方流水、已购行、root 余额、当月魔法池
psql("DELETE FROM spark_ledger WHERE user_id = %d OR idempotency_key = '%s'"
     % (PU, IDEM_S))
psql("DELETE FROM torrent_purchases WHERE user_id = %d" % PU)
psql("UPDATE users SET spark_balance = %s WHERE id = 1" % B0["root_spark"])
if B0["pool_row"] == "0":
    psql("DELETE FROM magic_pool WHERE month = '%s'" % MONTH)
else:
    psql("UPDATE magic_pool SET donated_total = %s WHERE month = '%s'"
         % (B0["pool"], MONTH))
POOL = psql("SELECT donated_total FROM magic_pool WHERE month = '%s'" % MONTH)
ok("S5d 扣费痕迹复原并回读证明（root 余额/当月池/双边流水/已购全回基线）",
   psql("SELECT spark_balance FROM users WHERE id = 1") == B0["root_spark"]
   and cnt("spark_ledger", "user_id = %d OR idempotency_key = '%s'"
           % (PU, IDEM_S)) == "0"
   and cnt("torrent_purchases", "user_id = %d" % PU) == "0"
   and (POOL == B0["pool"] or (B0["pool_row"] == "0"
                               and cnt("magic_pool", "month = '%s'" % MONTH)
                               == "0")),
   "root=%s/%s 池=%s/%s" % (psql("SELECT spark_balance FROM users "
                                 "WHERE id = 1"), B0["root_spark"], POOL,
                            B0["pool"]))

# ============ S6 凭证生命周期 ============
K3 = issue_key(TKF, FREE)
st, _, _, _ = hit(DKEY % ("99999999", K3))
st2, _, _, _ = hit(DKEY % (FREE, K3 + "x"))
st3, _, _, _ = hit(DKEY % (FREE, ""))
ok("S6a 凭证绑种子/绑原文：错种子、改一字符、空 token 全 401",
   st == 401 and st2 == 401 and st3 == 401, "%s/%s/%s" % (st, st2, st3))
psql("UPDATE download_keys SET expires_at = now() - interval '1 hour' "
     "WHERE user_id = 1 AND torrent_id = %s" % FREE)
st, _, _, _ = hit(DKEY % (FREE, K3))
ok("S6b 过期凭证 401（30 分钟窗口是真判据，不是只写在响应字段里）",
   st == 401, "%s" % st)
st, _, t, _ = hit(BATCH, dict(RH, **JS), "POST",
                  json.dumps({"ids": [int(FREE), int(FREE), -3,
                                      99999999]}).encode())
ok("S6c 批量口去重且丢非法 id：仍 200（%s）不 500" % st, st == 200,
   "%s %s" % (st, t[:60]))
codes2 = []
for _ in range(11):
    rq = urllib.request.Request(BASE + "/downloads/keys", method="POST")
    rq.add_header("Authorization", "Token " + (TKF or ""))
    rq.add_header("Content-Type", "application/json")
    try:
        codes2.append(urllib.request.urlopen(
            rq, json.dumps({"torrent_id": int(FREE)}).encode(),
            timeout=30).status)
    except urllib.error.HTTPError as e:
        codes2.append(e.code)
ok("S6d 签发限流每用户 10 次/分（末两次=%s，P-67 fail-close 那道）"
   % codes2[-2:], 429 in codes2[-2:], "%s" % codes2)

# ============ S7 收尾 ============
revoke_my("bf2t")
revoke_my("bf2p", PTOK)
ok("S7a 本轮自己签的 Token 全撤净（只数自己前缀，别把别的闸门算进来）",
   not my_tokens("bf2t") and not my_tokens("bf2p", PTOK),
   "root 侧=%s 探针侧=%s" % (len(my_tokens("bf2t")),
                             len(my_tokens("bf2p", PTOK))))
call("POST", "/admin/users/status", {"user_id": PU, "status": 2}, token=ROOT)
sban, rban = call("POST", "/me/tokens", {"name": "bf2x%d" % TAG,
                                         "password": "password123"},
                  token=PTOK)
ok("S7b 被封禁的账号不能再签长期凭证（%s，探针 Token 的签发口也吃状态闸门）"
   % sban, sban != 200 or not data(rban).get("token"), blob(rban)[:100])
call("DELETE", "/admin/users/%d" % PU, None, token=ROOT)
TOMB = psql("SELECT coalesce(passkey,'-') || '|' || pass_hash FROM users "
            "WHERE id = %d" % PU)
st, _, _, _ = hit(NPDL % (FREE, PPK))
ok("S7c 探针号走真实墓碑：passkey 作废后旧钥下载 401（不是仍发文件）",
   TOMB.startswith("deleted-") and "|!" in TOMB and st == 401,
   "tomb=%s dl=%s" % (TOMB[:20], st))
psql("DELETE FROM download_keys WHERE user_id = 1 OR user_id = %d" % PU)
if codes:
    psql("UPDATE invites SET status = 2 WHERE used_by IS NULL AND id = %d" % codes[0]["id"])
psql("DELETE FROM traffic_ledger WHERE user_id = %d" % PU)
psql("DELETE FROM snatches WHERE user_id = %d" % PU)
# 墓碑那一步产品自己会写一条 `user-purge-<uid>` 流水把余额冲平（实测
# -199500），所以火花流水必须**在墓碑之后**删，否则判据数到的残留是
# 产品收尾动作，不是我的账。
psql("DELETE FROM spark_ledger WHERE user_id = %d OR idempotency_key IN "
     "('%s','%s')" % (PU, IDEM_B, IDEM_S))
NOW = {"users": cnt("users"), "live": cnt("users", "status < 2"),
       "keys": cnt("download_keys"),
       "buy": cnt("torrent_purchases"),
       "traffic": cnt("traffic_ledger", "user_id = %d" % PU),
       "snatch": cnt("snatches"),
       "root_spark": psql("SELECT spark_balance FROM users WHERE id = 1"),
       "pool": psql("SELECT donated_total FROM magic_pool WHERE month = "
                    "'%s'" % MONTH)}
drift = {k: (B0[k], NOW[k]) for k in NOW
         if k not in ("users", "live") and NOW[k] != B0[k]}
# 基线是在探针号**还活跃**时取的，收尾它变成墓碑：总数不变、活跃数 -1。
# （把「+1/-1」写反过一次假红，所以两个量都摆在名字里，不靠脑补。）
ok("S7d 零残留：凭证/已购/流水账/快照/root 余额/当月池回基线；users 总数不变"
   "（%s→%s）、活跃数 -1（%s→%s，探针号留一枚墓碑）"
   % (B0["users"], NOW["users"], B0["live"], NOW["live"]),
   not drift and NOW["users"] == B0["users"]
   and int(NOW["live"]) == int(B0["live"]) - 1, blob(drift)[:140])
LEFT = cnt("spark_ledger", "user_id = %d OR idempotency_key IN ('%s','%s')"
           % (PU, IDEM_B, IDEM_S))
ok("S7e 本轮动过的火花流水清干净（探针名下含产品自写的 user-purge 行 + "
   "两条幂等键，共 0 条）——全站 spark_ledger 总量不放进判据：worker 每 60s "
   "也在写它（实测假红过一次）", LEFT == "0", "left=%s" % LEFT)
summary()
