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
from pt_audit_vip_walk_base import *  # noqa: F401,F403
import json  # noqa: F401
import os  # noqa: F401
import subprocess  # noqa: F401
import sys
import time  # noqa: F401
import urllib.error  # noqa: F401
import urllib.parse  # noqa: F401
import urllib.request  # noqa: F401

from pt_audit_vip_walk_write import (  # noqa: E402
    d_forumwrite, d_invite, d_shopbuy, d_subscribe)


# ---------------------------------------------------------------- 身份与总览
def d_identity(vip: str, root: str) -> None:
    d = "身份总览"
    st, js = call("GET", "/api/v1/me", vip)
    ok_envelope(d, "GET /me", st, js)
    if isinstance(js, dict) and js.get("code") == 0:
        me = js["data"]
        expect(d, "class=9 Nexus Master", me.get("class_id") == 9,
            str(me.get("class_name")))
        expect(d, "上传量 50G 到位", me.get("uploaded") == 53687091200,
            str(me.get("uploaded")))
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
    st, js = call("GET", "/api/v1/torrents?search=zzqqxx_not_exist&limit=5",
        vip)
    if ok_envelope(d, "无结果搜索", st, js):
        expect(d, "无结果 items 空", js["data"].get("items") == [])
    st, js = call("GET",
        "/api/v1/torrents?search_area=1&search=%E6%B7%B1%E6%B5%8B&limit=5", vip)
    ok_envelope(d, "副标题搜索(深测)", st, js)
    # 排序 + 分页游标往返
    st, js = call("GET", "/api/v1/torrents?sort=seeders&limit=2", vip)
    ok_envelope(d, "sort=seeders", st, js)
    st, js = call("GET", "/api/v1/torrents?alive=0&limit=2", vip)
    if ok_envelope(d, "分页第一页", st, js):
        cur = js["data"].get("next_cursor")
        if cur:
            st2, js2 = call("GET", f"/api/v1/torrents?alive=0&limit=2&"
                f"cursor={urllib.parse.quote(cur)}", vip)
            if ok_envelope(d, "分页第二页(cursor)", st2, js2):
                ids1 = {i["id"] for i in js["data"]["items"]}
                ids2 = {i["id"] for i in js2["data"]["items"]}
                expect(d, "两页不重叠", not (ids1 & ids2), f"{ids1} vs {ids2}")
        else:
            expect(d, "分页两页不重叠", False, "无 next_cursor")
    # 体积区间 / limit 钳位 / 分类
    st, js = call("GET", "/api/v1/torrents?size_min=1KB&size_max=1GB&limit=5",
        vip)
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
        tid = js["data"]["items"][0]["id"] if js.get("data",
            {}).get("items") else None
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
    st, raw = call("GET", f"/api/v1/compat/nexusphp/download.php?id={tid}&"
        f"passkey=a17b2c3d4e5f60718293a4b5c6d7e8f9", vip, raw=True)
    ok = st == 200 and isinstance(raw, bytes) and raw[:1] == b"d"
    expect(d, "download.php 取 .torrent", ok,
        f"st={st} len={len(raw) if isinstance(raw, bytes) else '?'}")
    # 错 passkey 必须拒
    bad_dl = (f"/api/v1/compat/nexusphp/download.php?id={tid}"
              "&passkey=wrongkey")
    st, _ = call("GET", bad_dl, vip, raw=True)
    expect(d, "错 passkey 拒发", st in (401, 403, 404), f"st={st}")
    # 新式：POST /downloads/keys 签发（fxo_ Token 向，第三方工具链）→ GET /downloads/{id}
    fxo = fxo_token()
    st, js = call("POST", "/api/v1/downloads/keys", fxo, {"torrent_id": tid},
        bearer=True)
    key = None
    if ok_envelope(d, "签发下载密钥(fxo Token)", st, js):
        key = js["data"].get("key")
        expect(d, "密钥非空", bool(key))
    if key:
        keyed = (f"/api/v1/downloads/{tid}"
                 f"?token={urllib.parse.quote(str(key))}")
        st, raw = call("GET", keyed, vip, raw=True)
        expect(d, "密钥取 .torrent", st == 200 and isinstance(raw,
            bytes) and raw[:1] == b"d",
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
        ann = (f"{TRACKER}/announce/"
               "a17b2c3d4e5f60718293a4b5c6d7e8f9"
               f"?{q}")
        with urllib.request.urlopen(ann, timeout=15) as r:
            body = r.read()
            ok = b"d8:intervali" in body or b"peers" in body
            expect(d, "tracker announce(started)", ok,
                f"st={r.status} body={body[:80]!r}")
    except Exception as e:
        expect(d, "tracker announce(started)", False, str(e)[:120])
    # 停的 event=stopped 收尾（不留幽灵 peer）
    q2 = q.replace("event=started", "event=stopped")
    try:
        ann2 = (f"{TRACKER}/announce/"
                "a17b2c3d4e5f60718293a4b5c6d7e8f9"
                f"?{q2}")
        urllib.request.urlopen(ann2, timeout=15).read()
    except Exception:
        pass
    # peers 端点
    st, js = call("GET", f"/api/v1/torrents/{tid}/peers", vip)
    ok_envelope(d, "peers 端点", st, js)
    # 做种奖励函数存在性（DB 侧）
    out = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
            "fluxtorrent", "-tAc",
         "SELECT count(*) FROM pg_proc WHERE proname IN "
             "('seeding_hourly','seeding_torrent_bonus','seeding_params')"],
        capture_output=True, text=True,
    )
    expect(d, "做种奖励三函数在册", out.stdout.strip() == "3", out.stdout.strip())


# ---------------------------------------------------------------- 经济链路
def d_economy(vip: str, root: str) -> None:
    d = "经济商城"
    for ep in ["/api/v1/me/spark", "/api/v1/me/spark/ledger",
               "/api/v1/me/orders", "/api/v1/me/vouchers",
               "/api/v1/bank/overview", "/api/v1/bank/status",
               "/api/v1/attendance", "/api/v1/magic-pool",
               "/api/v1/shop/items", "/api/v1/avatar-frames",
               "/api/v1/promo/plans", "/api/v1/dressup/list",
               "/api/v1/fundings/mine", "/api/v1/me/hr",
               "/api/v1/requests"]:
        st, js = call("GET", ep, vip)
        ok_envelope(d, f"GET {ep}", st, js)
    # 匿名守卫：经济面必须登录
    st, _ = call("GET", "/api/v1/me/spark", "")
    expect(d, "匿名 /me/spark 401", st == 401, f"st={st}")


# ---------------------------------------------------------------- 社区链路
def d_community(vip: str, root: str) -> None:
    d = "论坛私信邀请"
    for ep in ["/api/v1/forums", "/api/v1/forums/boards",
               "/api/v1/forums/recent",
               "/api/v1/forums/search?q=test", "/api/v1/forums/side",
               "/api/v1/forums/mine", "/api/v1/messages/boxes",
               "/api/v1/messages/inbox", "/api/v1/invites/status"]:
        st, js = call("GET", ep, vip)
        ok_envelope(d, f"GET {ep}", st, js)
    # RSS passkey 面（限流 30/min）：32 位形状合规才受理
    st, js = call("GET", "/api/v1/rss/a17b2c3d4e5f60718293a4b5c6d7e8f9", vip)
    expect(d, "RSS passkey 取流(XML)", st == 200 and isinstance(js,
        str) and "<rss" in js,
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
                  {"to": "audit_vip", "subject": "pt_audit_pm",
                      "body": "探针通信，可删"})
    ok_envelope(d, "root 发私信给 audit_vip", st, js)
    st, js = call("POST", "/api/v1/messages", vip,
                  {"to": "audit_vip", "subject": "x", "body": "x"})
    expect(d, "给自己发私信被拒(设计)", st == 400, f"st={st}")
    st, js = call("GET", "/api/v1/messages/inbox", vip)
    if ok_envelope(d, "收件箱", st, js):
        rows = js["data"] if isinstance(js["data"],
            list) else js["data"].get("items", [])
        expect(d, "私信入箱", any("pt_audit_pm" in str(r.get("subject",
            "")) for r in rows))


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
        "INSERT INTO balance_baseline (user_id, base_up, base_down, "
            "base_spark, "
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
        # 每域之前重钉档位：开头那一次自愈扛不住长跑——worker 的自动降级
        # job 会把闲置探针号打回 class 1（P-325 那条链是对的），于是后半程
        # 「class=9」「LV3 才能生成邀请码」类判据假红（2026-10-23 实测：
        # 全量跑到身份域时 class 已经回到 1，邀请域少记 3 项）。
        psql("UPDATE users SET class_id = 9 WHERE id = 7346")
        fn = DOMAINS[name]
        if name == "admin":
            fn(root)
        else:
            fn(vip, root)
    # 本轮探针 Token 收走：名字是本探针专属，撤销线（token_revocations.nbf）
    # 之后重跑会读不到旧行，留着只会攒悬行
    psql("DELETE FROM api_tokens WHERE user_id = 7346 AND name = 'pt_audit'")
    FXO[0] = ""
    fails = [r for r in RESULTS if r[1] == "FAIL"]
    print(f"\n===== 汇总：{len(RESULTS)} 项，FAIL {len(fails)} =====")
    for dom, lvl, msg in RESULTS:
        mark = "✓" if lvl == "PASS" else "✗"
        print(f" {mark} [{dom}] {msg}")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
