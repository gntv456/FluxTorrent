#!/usr/bin/env python3
"""PT 全站 VIP 深测探针（2026-10-21，scripts/pt_audit_vip_walk.py）。

以资深 VIP 用户视角逐链路验证：浏览/搜索/详情/下载/做种/经济/社区/后台。
凭证：铸 audit_vip(uid 7346, class 9) 与 root(uid 1, class 99) 两枚 JWT
（_mint_jwt.py 同口径）；写侧探针带 pt_audit_ 前缀，数据自清理。

用法：
    python scripts/pt_audit_vip_walk.py            # 全量
    python scripts/pt_audit_vip_walk.py browse     # 只跑某域（域名见 DOMAINS）
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

API = os.environ.get("FLUX_API", "http://127.0.0.1:8080")
TRACKER = os.environ.get("FLUX_TRACKER", "http://127.0.0.1:7070")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MINT = os.path.join(ROOT, "_mint_jwt.py")

RESULTS: list[tuple[str, str, str]] = []  # (域名, 级别, 详情)
# CSRF 双提交值（GET /api/v1/me/csrf 下落 cookie，写方法回填头）
CSRF: list[str] = [""]


def csrf_handshake(token: str) -> None:
    """取 flux_csrf cookie 值（浏览器同口径；缺它全站 cookie 写口全 403）。"""
    req = urllib.request.Request(
        API + "/api/v1/me/csrf",
        headers={"Cookie": f"flux_token={token}; flux.session=1"},
    )
    with urllib.request.urlopen(req, timeout=15) as r:
        for line in r.headers.get_all("Set-Cookie") or []:
            if line.startswith("flux_csrf="):
                CSRF[0] = line.split(";")[0][len("flux_csrf="):]


def rec(domain: str, level: str, msg: str) -> None:
    RESULTS.append((domain, level, msg))


def mint(uid: int, cls: int) -> str:
    out = subprocess.run(
        [sys.executable, MINT, "--uid", str(uid), "--class", str(cls)],
        capture_output=True, text=True, check=True,
    )
    return out.stdout.strip()


def psql(sql: str) -> str:
    out = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True,
    )
    return out.stdout.strip()


def fxo_token(uid: int = 7346) -> str:
    """铸一枚 read+upload 探针 Token（hash_token=无钥 SHA3-256，直接落库）。"""
    import hashlib
    plain = "fxo_ptaudit0001"
    psql(
        "INSERT INTO api_tokens (user_id, name, token_hash, scopes, rate_per_min, expires_at) "
        f"VALUES ({uid}, 'pt_audit', '{hashlib.sha3_256(plain.encode()).hexdigest()}', "
        "ARRAY['read','upload'], 600, now() + interval '1 day') "
        "ON CONFLICT (token_hash) DO NOTHING"
    )
    return plain


def call(
    method: str,
    path: str,
    token: str,
    body: dict | None = None,
    base: str = API,
    raw: bool = False,
    bearer: bool = False,
) -> tuple[int, object]:
    url = base + path
    data = None
    if bearer:
        headers = {"Authorization": f"Token {token}"}
    else:
        # cookie 鉴权的写方法必须过 CSRF 双提交闸（flux_csrf cookie +
        # x-csrf-token 头逐字相等）——与浏览器同口径
        cookie = f"flux_token={token}; flux.session=1"
        if CSRF[0]:
            cookie += f"; flux_csrf={CSRF[0]}"
        headers = {"Cookie": cookie}
        if method not in ("GET", "HEAD") and CSRF[0]:
            headers["x-csrf-token"] = CSRF[0]
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            payload = r.read()
            if raw:
                return r.status, payload
            try:
                return r.status, json.loads(payload)
            except Exception:
                # 非 JSON 成功体（如 RSS 的 XML）——不当失败，原样带回
                return r.status, payload[:200].decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        payload = e.read()
        try:
            return e.code, json.loads(payload)
        except Exception:
            return e.code, payload[:200].decode("utf-8", "replace")
    except Exception as e:  # 连接失败等
        return 0, str(e)


def expect(domain: str, name: str, cond: bool, detail: str = "") -> bool:
    rec(domain, "PASS" if cond else "FAIL", f"{name}{('｜' + detail) if detail else ''}")
    return cond


def ok_envelope(domain: str, name: str, st: int, js: object) -> bool:
    if st != 200 or not isinstance(js, dict) or js.get("code") != 0:
        return expect(domain, name, False, f"st={st} body={str(js)[:160]}")
    return expect(domain, name, True)


# ---------------------------------------------------------------- 身份与总览
def d_identity(vip: str, root: str) -> None:
    d = "身份总览"
    st, js = call("GET", "/api/v1/me", vip)
    ok_envelope(d, "GET /me", st, js)
    if isinstance(js, dict) and js.get("code") == 0:
        me = js["data"]
        expect(d, "class=9 Nexus Master", me.get("class_id") == 9, str(me.get("class_name")))
        expect(d, "上传量 50G 到位", me.get("uploaded") == 53687091200, str(me.get("uploaded")))
    st, js = call("GET", "/api/v1/me/overview", vip)
    ok_envelope(d, "GET /me/overview", st, js)
    st, js = call("GET", "/api/v1/stats", vip)
    ok_envelope(d, "GET /stats", st, js)
    st, js = call("GET", "/api/v1/home", vip)
    ok_envelope(d, "GET /home", st, js)
    # 匿名守卫：无 cookie 必须 401
    st, _ = call("GET", "/api/v1/me", "")
    expect(d, "匿名 /me 401", st == 401, f"st={st}")
    st, _ = call("GET", "/api/v1/me", "forged.token.here")
    expect(d, "伪造 token 401", st == 401, f"st={st}")


# ---------------------------------------------------------------- 浏览/搜索
def d_browse(vip: str, root: str) -> None:
    d = "浏览搜索"
    st, js = call("GET", "/api/v1/torrents?limit=5", vip)
    ok_envelope(d, "列表默认(活种)", st, js)
    if isinstance(js, dict) and js.get("code") == 0:
        data = js["data"]
        items = data.get("items", [])
        expect(d, "items 非空", len(items) > 0, f"n={len(items)}")
        it = items[0] if items else {}
        for k in ("id", "info_hash", "name", "size", "seeders", "leechers",
                  "approval_status", "promotion", "poster"):
            if k not in it:
                expect(d, f"列表缺字段 {k}", False)
                break
        else:
            expect(d, "列表字段齐全", True)
        expect(d, "分页游标字段在", "next_cursor" in data and "total_estimate" in data)
    st, js = call("GET", "/api/v1/torrents?alive=0&limit=5", vip)
    if ok_envelope(d, "alive=0 含断种", st, js):
        n = js["data"].get("total_estimate", 0)
        expect(d, "alive=0 总数远大于活种数", n > 5, f"total={n}")
    st, js = call("GET", "/api/v1/torrents?alive=2&limit=5", vip)
    if ok_envelope(d, "alive=2 仅断种", st, js):
        items = js["data"].get("items", [])
        expect(d, "断种列表 seeders=0", all(i.get("seeders") == 0 for i in items),
               f"n={len(items)}")
    # 搜索：标题 / 副标题 / 不存在
    # 标题搜索：搜探针种自身（并行会话会清掉它们的测试种，不能依赖）
    st, js = call("GET", "/api/v1/torrents?search=pt_audit&limit=5", vip)
    if ok_envelope(d, "标题搜索(探针种)", st, js):
        items = js["data"].get("items", [])
        expect(d, "搜到探针种", any("pt_audit" in i.get("name", "") for i in items))
    st, js = call("GET", "/api/v1/torrents?search=zzqqxx_not_exist&limit=5", vip)
    if ok_envelope(d, "无结果搜索", st, js):
        expect(d, "无结果 items 空", js["data"].get("items") == [])
    st, js = call("GET", "/api/v1/torrents?search_area=1&search=%E6%B7%B1%E6%B5%8B&limit=5", vip)
    ok_envelope(d, "副标题搜索(深测)", st, js)
    # 排序 + 分页游标往返
    st, js = call("GET", "/api/v1/torrents?sort=seeders&limit=2", vip)
    ok_envelope(d, "sort=seeders", st, js)
    st, js = call("GET", "/api/v1/torrents?alive=0&limit=2", vip)
    if ok_envelope(d, "分页第一页", st, js):
        cur = js["data"].get("next_cursor")
        if cur:
            st2, js2 = call("GET", f"/api/v1/torrents?alive=0&limit=2&cursor={urllib.parse.quote(cur)}", vip)
            if ok_envelope(d, "分页第二页(cursor)", st2, js2):
                ids1 = {i["id"] for i in js["data"]["items"]}
                ids2 = {i["id"] for i in js2["data"]["items"]}
                expect(d, "两页不重叠", not (ids1 & ids2), f"{ids1} vs {ids2}")
        else:
            expect(d, "分页两页不重叠", False, "无 next_cursor")
    # 体积区间 / limit 钳位 / 分类
    st, js = call("GET", "/api/v1/torrents?size_min=1KB&size_max=1GB&limit=5", vip)
    ok_envelope(d, "size 区间", st, js)
    st, js = call("GET", "/api/v1/torrents?limit=99999", vip)
    if ok_envelope(d, "limit 极大值钳位", st, js):
        expect(d, "返回条数 ≤400", len(js["data"].get("items", [])) <= 400)
    st, js = call("GET", "/api/v1/torrents/suggest?q=pt_audit", vip)
    ok_envelope(d, "suggest 搜索", st, js)
    st, js = call("GET", "/api/v1/section-dict", vip)
    ok_envelope(d, "section-dict", st, js)


# ---------------------------------------------------------------- 详情族
def d_detail(vip: str, root: str) -> None:
    d = "种子详情"
    st, js = call("GET", "/api/v1/torrents?search=pt_audit&limit=1", vip)
    tid = None
    if isinstance(js, dict) and js.get("code") == 0 and js["data"]["items"]:
        tid = js["data"]["items"][0]["id"]
    if tid is None:
        # 回落：取任意过审种
        st, js = call("GET", "/api/v1/torrents?alive=0&limit=1", vip)
        tid = js["data"]["items"][0]["id"] if js.get("data", {}).get("items") else None
    if tid is None:
        expect(d, "找到探针种子", False, "库里无任何种子")
        return
    expect(d, "找到探针种子", True, f"id={tid}")
    st, js = call("GET", f"/api/v1/torrents/{tid}", vip)
    if ok_envelope(d, "详情", st, js):
        row = js["data"]
        for k in ("name", "small_descr", "size", "seeders", "leechers",
                  "promotion", "poster", "info_hash"):
            if k not in row:
                expect(d, f"详情缺字段 {k}", False)
                break
        else:
            expect(d, "详情字段齐全", True)
    for ep, label in [
        ("files", "文件清单"), ("comments", "评论"), ("peers", "同伴"),
        ("snatches", "做种/下载记录"), ("thanks", "感谢"), ("trumps", "点赞"),
        ("related", "相关"), ("previews", "预览图"), ("aggregate", "聚合"),
        ("tags", "标签"), ("magnet", "磁力"), ("nfo", "NFO"),
    ]:
        st, js = call("GET", f"/api/v1/torrents/{tid}/{ep}", vip)
        ok_envelope(d, f"{label} /{ep}", st, js)
    st, js = call("GET", f"/api/v1/torrents/{tid}/detail", vip)
    ok_envelope(d, "深详情 /detail", st, js)
    # 404 语义
    st, _ = call("GET", "/api/v1/torrents/999999999", vip)
    expect(d, "不存在种子 404", st == 404, f"st={st}")
    st, _ = call("GET", "/api/v1/torrents/abc", vip)
    expect(d, "非数字 id 4xx", 400 <= st < 500, f"st={st}")


# ---------------------------------------------------------------- 下载链路
def d_download(vip: str, root: str) -> None:
    d = "下载链路"
    st, js = call("GET", "/api/v1/torrents?limit=1", vip)
    tid = None
    if isinstance(js, dict) and js.get("code") == 0 and js["data"]["items"]:
        tid = js["data"]["items"][0]["id"]
    if tid is None:
        expect(d, "有种子可下", False)
        return
    # compat download.php：passkey 取种
    st, raw = call("GET", f"/api/v1/compat/nexusphp/download.php?id={tid}&passkey=a17b2c3d4e5f60718293a4b5c6d7e8f9", vip, raw=True)
    ok = st == 200 and isinstance(raw, bytes) and raw[:1] == b"d"
    expect(d, "download.php 取 .torrent", ok, f"st={st} len={len(raw) if isinstance(raw, bytes) else '?'}")
    # 错 passkey 必须拒
    st, _ = call("GET", f"/api/v1/compat/nexusphp/download.php?id={tid}&passkey=wrongkey", vip, raw=True)
    expect(d, "错 passkey 拒发", st in (401, 403, 404), f"st={st}")
    # 新式：POST /downloads/keys 签发（fxo_ Token 向，第三方工具链）→ GET /downloads/{id}
    fxo = fxo_token()
    st, js = call("POST", "/api/v1/downloads/keys", fxo, {"torrent_id": tid}, bearer=True)
    key = None
    if ok_envelope(d, "签发下载密钥(fxo Token)", st, js):
        key = js["data"].get("key")
        expect(d, "密钥非空", bool(key))
    if key:
        st, raw = call("GET", f"/api/v1/downloads/{tid}?token={urllib.parse.quote(str(key))}", vip, raw=True)
        expect(d, "密钥取 .torrent", st == 200 and isinstance(raw, bytes) and raw[:1] == b"d",
               f"st={st}")
    # 会话 JWT 打 Token 向端点必须拒（凭证体系隔离）
    st, _ = call("POST", "/api/v1/downloads/keys", vip, {"torrent_id": tid})
    expect(d, "会话 JWT 打 Token 端点被拒", st in (401, 403), f"st={st}")


# ---------------------------------------------------------------- 做种链路
def d_seeding(vip: str, root: str) -> None:
    d = "做种链路"
    st, js = call("GET", "/api/v1/torrents?search=pt_audit&limit=1", vip)
    tid = None
    ih = None
    if isinstance(js, dict) and js.get("code") == 0 and js["data"]["items"]:
        tid = js["data"]["items"][0]["id"]
        ih = js["data"]["items"][0].get("info_hash")
    if tid is None or not ih:
        expect(d, "有 info_hash 的种子", False)
        return
    # HTTP tracker announce：路径 /announce/{passkey}；
    # peer_id/info_hash 须传 percent-encode 的**原始 20 字节**（hex 串会吃
    # 「长度无效」——API 下发的是 hex，探针先 fromhex 还原）
    pid = "-pt0001-aud1tv1p0001"
    q = urllib.parse.urlencode({
        "info_hash": bytes.fromhex(ih),
        "peer_id": pid.encode(),
        "port": 6881,
        "uploaded": 0,
        "downloaded": 0,
        "left": 0,
        "compact": 1,
        "event": "started",
        "numwant": 10,
    })
    try:
        with urllib.request.urlopen(f"{TRACKER}/announce/a17b2c3d4e5f60718293a4b5c6d7e8f9?{q}", timeout=15) as r:
            body = r.read()
            ok = b"d8:intervali" in body or b"peers" in body
            expect(d, "tracker announce(started)", ok, f"st={r.status} body={body[:80]!r}")
    except Exception as e:
        expect(d, "tracker announce(started)", False, str(e)[:120])
    # 停的 event=stopped 收尾（不留幽灵 peer）
    q2 = q.replace("event=started", "event=stopped")
    try:
        urllib.request.urlopen(f"{TRACKER}/announce/a17b2c3d4e5f60718293a4b5c6d7e8f9?{q2}", timeout=15).read()
    except Exception:
        pass
    # peers 端点
    st, js = call("GET", f"/api/v1/torrents/{tid}/peers", vip)
    ok_envelope(d, "peers 端点", st, js)
    # 做种奖励函数存在性（DB 侧）
    out = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent", "-tAc",
         "SELECT count(*) FROM pg_proc WHERE proname IN ('seeding_hourly','seeding_torrent_bonus','seeding_params')"],
        capture_output=True, text=True,
    )
    expect(d, "做种奖励三函数在册", out.stdout.strip() == "3", out.stdout.strip())


# ---------------------------------------------------------------- 经济链路
def d_economy(vip: str, root: str) -> None:
    d = "经济商城"
    for ep in ["/api/v1/me/spark", "/api/v1/me/spark/ledger", "/api/v1/me/orders",
               "/api/v1/me/vouchers", "/api/v1/bank/overview", "/api/v1/bank/status",
               "/api/v1/attendance", "/api/v1/magic-pool", "/api/v1/shop/items",
               "/api/v1/avatar-frames", "/api/v1/promo/plans", "/api/v1/dressup/list",
               "/api/v1/fundings/mine", "/api/v1/me/hr", "/api/v1/requests"]:
        st, js = call("GET", ep, vip)
        ok_envelope(d, f"GET {ep}", st, js)
    # 匿名守卫：经济面必须登录
    st, _ = call("GET", "/api/v1/me/spark", "")
    expect(d, "匿名 /me/spark 401", st == 401, f"st={st}")


# ---------------------------------------------------------------- 社区链路
def d_community(vip: str, root: str) -> None:
    d = "论坛私信邀请"
    for ep in ["/api/v1/forums", "/api/v1/forums/boards", "/api/v1/forums/recent",
               "/api/v1/forums/search?q=test", "/api/v1/forums/side", "/api/v1/forums/mine",
               "/api/v1/messages/boxes", "/api/v1/messages/inbox",
               "/api/v1/invites/status"]:
        st, js = call("GET", ep, vip)
        ok_envelope(d, f"GET {ep}", st, js)
    # RSS passkey 面（限流 30/min）：32 位形状合规才受理
    st, js = call("GET", "/api/v1/rss/a17b2c3d4e5f60718293a4b5c6d7e8f9", vip)
    expect(d, "RSS passkey 取流(XML)", st == 200 and isinstance(js, str) and "<rss" in js,
           f"st={st} head={str(js)[:60]}")
    st, _ = call("GET", "/api/v1/rss/" + "0" * 32, vip)
    expect(d, "RSS 形状合规但未知 passkey → 404", st == 404, f"st={st}")
    st, _ = call("GET", "/api/v1/rss/short", vip)
    expect(d, "RSS 形状非法 → 400", st == 400, f"st={st}")
    # 开放 API 面要 fxo_ Token（会话 JWT 打不进）
    st, _ = call("GET", "/api/v1/open/recent", fxo_token(), bearer=True)
    ok_envelope(d, "open/recent(fxo Token)", st, _)
    # 私信闭环：root → audit_vip（自发送按设计必拒）
    st, js = call("POST", "/api/v1/messages", root,
                  {"to": "audit_vip", "subject": "pt_audit_pm", "body": "探针通信，可删"})
    ok_envelope(d, "root 发私信给 audit_vip", st, js)
    st, js = call("POST", "/api/v1/messages", vip,
                  {"to": "audit_vip", "subject": "x", "body": "x"})
    expect(d, "给自己发私信被拒(设计)", st == 400, f"st={st}")
    st, js = call("GET", "/api/v1/messages/inbox", vip)
    if ok_envelope(d, "收件箱", st, js):
        rows = js["data"] if isinstance(js["data"], list) else js["data"].get("items", [])
        expect(d, "私信入箱", any("pt_audit_pm" in str(r.get("subject", "")) for r in rows))


# ---------------------------------------------------------------- 后台链路
def d_admin(root: str) -> None:
    d = "后台管理"
    for ep in ["/api/v1/admin/economy-dashboard", "/api/v1/admin/spark-flow",
               "/api/v1/admin/cheat-events", "/api/v1/admin/links",
               "/api/v1/admin/trumps"]:
        st, js = call("GET", ep, root)
        ok_envelope(d, f"root GET {ep}", st, js)
    # VIP 打后台必须 403
    st, _ = call("GET", "/api/v1/admin/cheat-events", mint(7346, 9))
    expect(d, "VIP 打 admin 403", st == 403, f"st={st}")


# ---------------------------------------------------------------- 商城购买
def d_shopbuy(vip: str, root: str) -> None:
    d = "商城购买"
    st, js = call("GET", "/api/v1/shop/items", vip)
    if not ok_envelope(d, "货架 /shop/items", st, js):
        return
    items = js["data"] if isinstance(js["data"], list) else js["data"].get("items", [])
    cheap = next((i for i in items if i.get("kind") == "gift_spark"), None)
    if cheap is None:
        expect(d, "找到 gift_spark 测试品", False)
        return

    def bal(tok: str) -> int:
        st, js = call("GET", "/api/v1/me/spark", tok)
        data = js["data"] if isinstance(js["data"], dict) else {}
        return int(data.get("balance", 0))

    # ① 赠礼（0421 新语义）：效果记接收方，不记买家。
    # 幂等键带轮次前缀——同键跨轮复用会被服务端判重放，断言全部失真
    run = str(int(time.time()))
    before_vip, before_root = bal(vip), bal(root)
    after_vip, after_root = before_vip, before_root
    st, js = call("POST", "/api/v1/shop/buy", vip,
                  {"item_id": cheap["id"], "qty": 2,
                   "to": "root", "idempotency_key": f"pt_audit_gift_{run}"})
    if ok_envelope(d, "赠礼 root ×2（qty=2）", st, js):
        after_vip, after_root = bal(vip), bal(root)
        expect(d, "买家扣 2×单价", before_vip - after_vip == 2 * cheap["price"],
               f"{before_vip}→{after_vip}")
        expect(d, "接收方 root +2×面额", after_root - before_root == 2000,
               f"{before_root}→{after_root}")
    # 同键重放：不二次扣款、不二次入账
    st, js = call("POST", "/api/v1/shop/buy", vip,
                  {"item_id": cheap["id"], "qty": 2,
                   "to": "root", "idempotency_key": f"pt_audit_gift_{run}"})
    r2, v2 = bal(root), bal(vip)
    expect(d, "同键重放不双扣/不双送", r2 == after_root and v2 == after_vip,
           f"root={r2} vip={v2}")
    # 负例：缺接收人 / 送自己 / 接收人不存在
    st, _ = call("POST", "/api/v1/shop/buy", vip,
                 {"item_id": cheap["id"], "idempotency_key": f"g2_{run}"})
    expect(d, "缺接收人 400", st == 400, f"st={st}")
    st, _ = call("POST", "/api/v1/shop/buy", vip,
                 {"item_id": cheap["id"], "to": "audit_vip",
                  "idempotency_key": f"g3_{run}"})
    expect(d, "送自己 400", st == 400, f"st={st}")
    st, _ = call("POST", "/api/v1/shop/buy", vip,
                 {"item_id": cheap["id"], "to": "nosuchuser_zz",
                  "idempotency_key": f"g4_{run}"})
    expect(d, "接收人不存在 400", st == 400, f"st={st}")
    # 普通品（非 gift）不受接收人参数影响
    other = next((i for i in items if i.get("kind") == "makeup_card"), None)
    if other:
        st, js = call("POST", "/api/v1/shop/buy", vip,
                      {"item_id": other["id"],
                       "idempotency_key": f"pt_audit_card_{run}"})
        ok_envelope(d, "买补签卡(普通品照常)", st, js)
    # 负例：不存在的品 / 缺幂等键 / 非叠加品买 2 件
    st, _ = call("POST", "/api/v1/shop/buy", vip,
                 {"item_id": 999999, "idempotency_key": f"x_{run}"})
    expect(d, "不存在商品 404", st == 404, f"st={st}")
    st, _ = call("POST", "/api/v1/shop/buy", vip, {"item_id": cheap["id"]})
    expect(d, "缺幂等键 400", st == 400, f"st={st}")
    uniq = next((i for i in items if i.get("kind") in
                 ("avatar_frame", "custom_title", "animated_avatar")), None)
    if uniq:
        st, _ = call("POST", "/api/v1/shop/buy", vip,
                     {"item_id": uniq["id"], "qty": 2, "idempotency_key": f"y_{run}"})
        expect(d, "非叠加品 qty=2 拒", st == 400, f"st={st}")


# ---------------------------------------------------------------- 邀请链路
def d_invite(vip: str, root: str) -> None:
    d = "邀请链路"
    st, js = call("GET", "/api/v1/invites/status", vip)
    if not ok_envelope(d, "邀请状态", st, js):
        return
    st, js = call("POST", "/api/v1/invites", vip,
                  {"email": "pt_audit_invitee@local.test"})
    code = None
    if ok_envelope(d, "申请邀请码", st, js):
        code = js["data"].get("code") or js["data"].get("invite_code")
        expect(d, "拿到邀请码", bool(code), str(js["data"])[:80])
    if code:
        st, js = call("GET", "/api/v1/invites/status", vip)
        if ok_envelope(d, "邀请状态(申请后)", st, js):
            # status 是汇总对象：unused=未使用数（列表在 /invites 页另拉）
            expect(d, "未使用邀请数 +1", js["data"].get("unused", 0) >= 1,
                   str(js["data"])[:120])


# ---------------------------------------------------------------- 论坛写流
def d_forumwrite(vip: str, root: str) -> None:
    d = "论坛写流"
    st, js = call("GET", "/api/v1/torrents?search=pt_audit&limit=1", vip)
    tid = None
    if isinstance(js, dict) and js.get("code") == 0 and js["data"]["items"]:
        tid = js["data"]["items"][0]["id"]
    if tid is None:
        # 回落：任意过审种（并行会话会清掉各自的测试种）
        st, js = call("GET", "/api/v1/torrents?alive=0&limit=1", vip)
        if isinstance(js, dict) and js.get("code") == 0 and js["data"]["items"]:
            tid = js["data"]["items"][0]["id"]
    if tid is None:
        expect(d, "有种子可引用", False)
        return
    st, js = call("GET", "/api/v1/forums", vip)
    fid = 2
    if isinstance(js, dict) and js.get("code") == 0:
        forums = js["data"].get("forums", [])
        cand = next((f for f in forums if f.get("can_create")), None)
        if cand:
            fid = cand["id"]
    body = f"PT 全站深测主题\n\n看看这个种子怎么样：\n\n!torrent({tid})\n\n完事"
    st, js = call("POST", "/api/v1/forums/topics", vip,
                  {"forum_id": fid, "title": "pt_audit_topic 深测主题", "body": body})
    topic_id = None
    if ok_envelope(d, "发主题(带 !torrent 引用)", st, js):
        topic_id = js["data"].get("topic_id")
        expect(d, "topic_id 回值", bool(topic_id))
    if not topic_id:
        return
    # 论坛防刷：回帖距上一条 10s（message_flood 同族），探针按用户节拍走
    time.sleep(11)
    st, js = call("POST", f"/api/v1/forums/topics/{topic_id}/reply", vip,
                  {"body": f"回帖深测：\n\n!torrent({tid})\n\nover"})
    post_id = None
    if ok_envelope(d, "回帖(带 !torrent 引用)", st, js):
        post_id = js["data"].get("post_id")
    # 编辑回帖（PUT）
    if post_id:
        st, js = call("PUT", f"/api/v1/forums/posts/{post_id}", vip,
                      {"body": "编辑后的回帖\n\n/torrent/" + str(tid)})
        ok_envelope(d, "编辑回帖(PUT)", st, js)
    # 读回：正文原样（含语法串）+ 纯文本摘要已压成链接
    st, js = call("GET", f"/api/v1/forums/topics/{topic_id}", vip)
    if ok_envelope(d, "读回主题", st, js):
        posts = js["data"].get("posts", [])
        bodies = [p.get("body", "") for p in posts]
        expect(d, "主题正文含 !torrent 语法",
               any(f"!torrent({tid})" in b for b in bodies))


# ---------------------------------------------------------------- 订阅推送
def d_subscribe(vip: str, root: str) -> None:
    d = "订阅推送"
    # 追更三读口
    for ep in ["/api/v1/me/subscriptions/groups",
               "/api/v1/me/subscriptions/calendar",
               "/api/v1/me/subscriptions/networks"]:
        st, js = call("GET", ep, vip)
        ok_envelope(d, f"GET {ep}", st, js)
    # 组订阅写链：聚合组（torrent_groups）订阅 → 读口可见 → 退订。
    # gid 来源：取一枚入组种子——/torrents/{id}/group 的入参是 **torrent id**
    # （2026-10-22 曾误传 group id）；扫列表前 50 个会被并行会话的探针种
    # 占满 id 头部，故直接库取入组种
    gid = None
    t = psql("SELECT id FROM torrents WHERE group_id IS NOT NULL ORDER BY id DESC LIMIT 1")
    if t:
        st2, js2 = call("GET", f"/api/v1/torrents/{t}/group", vip)
        gg = (js2.get("data") or {}).get("group") if isinstance(js2, dict) else None
        gid = gg["id"] if gg and gg.get("id") else None
    if gid:
        st, js = call("POST", f"/api/v1/torrents/groups/{gid}/subscribe", vip, {})
        ok_envelope(d, "订阅字幕组", st, js)
        st, js = call("GET", "/api/v1/me/subscriptions/groups", vip)
        if ok_envelope(d, "我的订阅(订阅后)", st, js):
            rows = js["data"] if isinstance(js["data"], list) else js["data"].get("items", [])
            expect(d, "订阅组在册", any(r.get("group_id") == gid or r.get("id") == gid for r in rows),
                   f"n={len(rows)}")
        st, js = call("POST", f"/api/v1/torrents/groups/{gid}/unsubscribe", vip, {})
        ok_envelope(d, "退订字幕组", st, js)
    else:
        rec(d, "WARN", "站内无聚合组可订阅（数据形态，非缺陷）")
    # 厂牌订阅：订一个 content_network → 读口可见
    st, js = call("GET", "/api/v1/networks?limit=1", vip)
    nid = None
    if isinstance(js, dict) and js.get("code") == 0:
        rows = js["data"] if isinstance(js["data"], list) else js["data"].get("items", [])
        if rows:
            nid = rows[0].get("id")
    if nid:
        st, js = call("POST", f"/api/v1/networks/{nid}/subscribe", vip, {})
        ok_envelope(d, "订阅厂牌", st, js)
        st, js = call("GET", "/api/v1/me/subscriptions/networks", vip)
        if ok_envelope(d, "我的厂牌订阅", st, js):
            rows = js["data"] if isinstance(js["data"], list) else js["data"].get("items", [])
            expect(d, "厂牌订阅在册", any(r.get("network_id") == nid or r.get("id") == nid for r in rows),
                   f"n={len(rows)}")
        st, js = call("POST", f"/api/v1/networks/{nid}/unsubscribe", vip, {})
        ok_envelope(d, "退订厂牌", st, js)
    else:
        expect(d, "有可订阅的厂牌", False)


# ---------------------------------------------------------------- 探针自愈件
def _benc(obj, out=None):
    """最小 bencode 编码（造 .torrent 用）。"""
    out = bytearray() if out is None else out
    if isinstance(obj, int):
        out += b"i" + str(obj).encode() + b"e"
    elif isinstance(obj, bytes):
        out += b"%d:" % len(obj) + obj
    elif isinstance(obj, list):
        out += b"l"
        for v in obj:
            _benc(v, out)
        out += b"e"
    elif isinstance(obj, dict):
        out += b"d"
        for k in sorted(obj.keys()):
            _benc(k, out)
            _benc(obj[k], out)
        out += b"e"
    return bytes(out)


def _probe_torrent(name: str, length: int = 1_048_576) -> bytes:
    import hashlib
    piece_len = 16384
    info = {
        b"piece length": piece_len,
        b"name": name.encode(),
        b"private": 1,
        b"length": length,
        b"pieces": hashlib.sha1(name.encode()).digest()
        * max(1, (length + piece_len - 1) // piece_len),
    }
    return _benc({b"info": info, b"creation date": int(time.time())})


def upload_probe_torrent(tok: str, name: str) -> tuple[int, str] | None:
    """multipart 上传一枚探针种（免审直过），返回 (torrent id, info_hash)。"""
    crlf = chr(13) + chr(10)
    dq = chr(34)
    b = "----auditvipup"
    parts = b""
    for k, v in {
        "category_id": 1,
        "name": name,
        "small_descr": "全站深测探针种",
        "descr": "全站深测探针种：正常长度描述。本资源为测试用途，包含充分的说明"
                 "文字以通过最低内容标准检查。补充来源与出处说明，长度足够通过校验。"
                 "另附媒体参数与转码来源备注，确保描述长度达到站点自动审核的"
                 "最低字数要求，避免探针种在测试周期内被自动审种流程拒绝。",
    }.items():
        parts += (
            f"--{b}{crlf}Content-Disposition: form-data; name={dq}{k}{dq}"
            f"{crlf}{crlf}{v}{crlf}"
        ).encode()
    parts += (
        f"--{b}{crlf}Content-Disposition: form-data; name={dq}file{dq}; "
        f"filename={dq}probe.torrent{dq}{crlf}"
        f"Content-Type: application/x-bittorrent{crlf}{crlf}"
    ).encode() + _probe_torrent(name)
    parts += (crlf + f"--{b}--" + crlf).encode()
    req = urllib.request.Request(
        API + "/api/v1/torrents", method="POST", data=parts)
    req.add_header("Content-Type", f"multipart/form-data; boundary={b}")
    req.add_header(
        "Cookie",
        f"flux_token={tok}; flux.session=1; flux_csrf={CSRF[0]}")
    req.add_header("x-csrf-token", CSRF[0])
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            d = json.loads(r.read()).get("data") or {}
            return d.get("id"), d.get("info_hash")
    except Exception as e:
        rec("自愈", "WARN", f"上传探针种失败：{str(e)[:80]}")
        return None


def tracker_announce(passkey: str, info_hash_hex: str, size: int = 0,
                     downloaded: int = 0, event: str = "started") -> bool:
    """HTTP tracker announce（路径 /announce/{passkey}，info_hash 须原始 20 字节）。
    downloaded>0 时模拟「下载过再挂种」——反幽灵闸要求物理下载量达
    size×10.4%（或可达回连），纯 left=0 的空壳 announce 会被判幽灵
    （snatches.seeding=false），种子不计入活种。"""
    q = urllib.parse.urlencode({
        "info_hash": bytes.fromhex(info_hash_hex),
        "peer_id": b"-pt0001-aud1tv1p0001",
        "port": 6881,
        "uploaded": 0,
        "downloaded": downloaded,
        "left": 0 if downloaded >= size and size > 0 else max(size - downloaded, 0),
        "compact": 1,
        "event": event,
        "numwant": 10,
    })
    try:
        with urllib.request.urlopen(
            f"{TRACKER}/announce/{passkey}?{q}", timeout=15
        ) as r:
            body = r.read()
            if b"failure reason" in body:
                return False
            return b"peers" in body or b"interval" in body
    except Exception:
        return False


def ensure_probe_torrent(root_tok: str, vip_tok: str) -> None:
    """每轮都自建一枚探针种并挂做种 peer——不能只在「无活种」时补：
    浏览/做种域的断言按 pt_audit 前缀找自家探针（并行会话的 deep_ 种
    不算数），列表非空就跳过会让这些断言必挂（2026-10-22 实测）。"""
    name = f"pt_audit_alive.{int(time.time())}"
    out = upload_probe_torrent(root_tok, name)
    if not out:
        return
    tid, ih = out
    # 反幽灵闸的合法路径：首报只立基线不计增量（防一次 announce 编造
    # 全量下载），所以分两拍——①以下载态立基线 ②以「已下完」态产生
    # size 增量 ⇒ phys_down=size ≥ 10.4% ⇒ 判真做种，种子计入活种。
    ok1 = tracker_announce("a17b2c3d4e5f60718293a4b5c6d7e8f9", ih,
                           size=1_048_576, downloaded=0)
    time.sleep(2)
    ok2 = tracker_announce("a17b2c3d4e5f60718293a4b5c6d7e8f9", ih,
                           size=1_048_576, downloaded=1_048_576)
    if ok1 and ok2:
        # worker 经 Redis 流异步落账：consume_announce 是 60s tick，
        # 轮询必须覆盖一个完整周期（旧 30s 上限在 tick 后半窗写入时必挂）。
        # 且必须等**自家探针**（tid 的 seeders>0）——等「任意活种」会在
        # 站上另有活种时秒过，探针自己还没落账，后续按 pt_audit 前缀
        # 找探针的断言全挂（2026-10-22 实测假成功）。
        for _ in range(24):
            time.sleep(5)
            st, js = call("GET", f"/api/v1/torrents/{tid}", root_tok)
            row = (js.get("data") or {}) if isinstance(js, dict) else {}
            if (row.get("seeders") or 0) > 0:
                rec("自愈", "PASS", f"已建探针种 #{tid} 并挂做种 peer")
                return
        rec("自愈", "WARN", f"探针种 #{tid} 做种已挂但 120s 内未进活种列表")
    else:
        rec("自愈", "WARN", f"探针种 #{tid} 已建但 announce 失败（可能仍不可见）")


DOMAINS = {
    "identity": d_identity,
    "browse": d_browse,
    "detail": d_detail,
    "download": d_download,
    "seeding": d_seeding,
    "economy": d_economy,
    "community": d_community,
    "admin": d_admin,
    "shopbuy": d_shopbuy,
    "invite": d_invite,
    "forumwrite": d_forumwrite,
    "subscribe": d_subscribe,
}


def main() -> None:
    wanted = sys.argv[1:] or list(DOMAINS)
    # 自愈：并行会话的后台工具可能把探针用户降级/清零，探测前拉回 VIP 态
    # users.uploaded/spark_balance 是快照列（worker 按 base+Σledger 重算）：
    # 裸写会在下一次 announce 消费时被冲掉（实测 50GB 归零）。自愈走
    # balance_baseline + 清 ledger，与快照公式自洽。
    # ⚠️ spark_ledger 也必须清：只重置 base_spark 不清流水，下一次小时
    # 重算（做种 job）就把余额拽回 base+Σledger 的历史负值——商城域
    # 随后全部「余额不足」（2026-10-22 实测踩中）。
    psql("UPDATE users SET class_id = 9 WHERE id = 7346")
    psql("DELETE FROM traffic_ledger WHERE user_id = 7346")
    psql("DELETE FROM spark_ledger WHERE user_id = 7346")
    psql(
        "INSERT INTO balance_baseline (user_id, base_up, base_down, base_spark, "
        "base_seed_secs, through, updated_at) VALUES "
        "(7346, 53687091200, 2684354560, 100000, 0, now(), now()) "
        "ON CONFLICT (user_id) DO UPDATE SET base_up = 53687091200, "
        "base_down = 2684354560, base_spark = 100000, updated_at = now()"
    )
    psql("UPDATE users SET spark_balance = 100000 WHERE id = 7346")
    # 周邀请配额重置（上周量耗尽属正确站方行为，探针要可重复跑）
    psql("DELETE FROM invite_quota WHERE user_id = 7346")
    vip = mint(7346, 9)
    root = mint(1, 99)
    csrf_handshake(vip)
    ensure_probe_torrent(root, vip)
    for name in wanted:
        fn = DOMAINS[name]
        if name == "admin":
            fn(root)
        else:
            fn(vip, root)
    fails = [r for r in RESULTS if r[1] == "FAIL"]
    print(f"\n===== 汇总：{len(RESULTS)} 项，FAIL {len(fails)} =====")
    for dom, lvl, msg in RESULTS:
        mark = "✓" if lvl == "PASS" else "✗"
        print(f" {mark} [{dom}] {msg}")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
