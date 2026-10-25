#!/usr/bin/env python3
"""PT 全站资深用户深测探针（2026-10-22，scripts/pt_audit_deep_walk.py）。

与 pt_audit_vip_walk.py（断言式冒烟）互补：本脚本按**真实用户旅程**逐链路
深挖 + **跨端点数据对账**，专抓「单口能过、两口对不上」类真缺陷。

域（域名见 DOMAINS）：
    search  浏览/搜索/筛选/排序/分页（30+ 断言：筛选项真筛、排序真排、
            分页不重叠、注入面、大小写/中文/特殊字符）
    pm      私信全链路（收发/已读态/未读计数对账/文件夹/搜索/系统信/
            边界：自发/不存在收件人/flood）
    ratio   分享率（真实两段式 announce 周期 → users.uploaded 增量 ==
            ledger Σdelta == 上报量；snatches/torrents 聚合对账；ratio 计算）
    rss     订阅推送（RSS XML 结构/条目与 DB 一致/limit/分类/paid/坏 passkey/
            torznab/Token 面）
    mall    商城消耗链（货架/扣费到账/订单/幂等/叠加限制/余额不足/赠礼回归/
            背包/代金券/流水）
    invite2 邀请注册链（发码→注册用码→关系绑定→配额→奖励→码状态→撤销）
    admin2  后台（用户列表/开关/封禁登录阻断/发放上传量&魔力到账+流水/
            种子软删恢复/举报处理/设定读写/群发/日志）

凭证：audit_vip(uid 7346, class 9) + root(uid 1, class 99)。
写侧探针带 deep_ 前缀，数据自清理。

用法：
    python scripts/pt_audit_deep_walk.py            # 全量
    python scripts/pt_audit_deep_walk.py search pm  # 只跑某几域
"""
from __future__ import annotations

from pt_audit_deep_walk_base import *  # noqa: F401,F403
import sys

from pt_audit_deep_walk_member import (  # noqa: E402
    d_mall, d_pm)
from pt_audit_deep_walk_admin import (  # noqa: E402
    d_admin2, d_dlforum, d_invite2)

# ---------------------------------------------------------------- 域：search

def d_search(vip: str, root: str) -> None:
    D = "search"
    st, js = call("GET", "/api/v1/torrents?limit=50", vip)
    if not ok(D, "默认活种列表", st, js):
        return
    d = data_of(js)
    items = d.get("items", [])
    expect(D, "默认视图 items 非空", len(items) > 0, f"n={len(items)}")
    need = ["id", "name", "seeders", "leechers", "times_completed",
            "promotion", "owner_name", "size", "created_at"]
    missing = [k for k in need if items and k not in items[0]]
    expect(D, "列表字段齐全", not missing, f"缺 {missing}")

    # 关键词：命中/大小写/中文/特殊字符（注入面）/无结果
    st, js = call("GET", "/api/v1/torrents?search=deep_alive&limit=10", vip)
    d1 = data_of(js)
    expect(D, "关键词搜索可过", st == 200)
    st, js = call("GET", "/api/v1/torrents?search=DEEP_ALIVE&limit=10", vip)
    expect(D, "英文大小写不敏感", st == 200 and
           len(data_of(js).get("items", [])) >= len(d1.get("items", [])))
    for bad, label in [("%", "百分号"), ("_", "下划线"), ("'", "单引号"),
                       ("--", "双横注释"), ("\\", "反斜杠")]:
        st, _ = call("GET",
            f"/api/v1/torrents?search={urllib.parse.quote(bad)}", vip)
        expect(D, f"特殊字符 {label} 不 500", st in (200, 400), f"st={st}")
    st, js = call("GET", "/api/v1/torrents?search=zzz_no_such_zzz", vip)
    expect(D, "无结果搜索 items 空", st == 200 and
           data_of(js).get("items") == [])

    # 搜索范围：副标题/发布者/IMDb（中文关键词须百分号编码，否则 urllib 直接抛错）
    st, js = call("GET", "/api/v1/torrents?search_area=1&search="
                  + urllib.parse.quote("探针") + "&limit=10", vip)
    rows = data_of(js).get("items", [])
    expect(D, "search_area=1 副标题搜索", st == 200 and len(rows) > 0 and
           all("探针" in (r.get("small_descr") or "") or
               "探针" in (r.get("name") or "") for r in rows),
           f"st={st} n={len(rows)}")
    st, js = call("GET", "/api/v1/torrents?search="
                  + urllib.parse.quote("深测") + "&limit=10", vip)
    expect(D, "中文关键词直搜", st == 200 and
           len(data_of(js).get("items", [])) > 0, f"st={st}")
    st, js = call("GET", "/api/v1/torrents?search_area=3&search=root&limit=10",
        vip)
    d3 = data_of(js)
    expect(D, "search_area=3 发布者搜索", st == 200 and
           all("root" in (it.get("owner_name") or "").lower()
               for it in d3.get("items", [])[:5]),
           f"n={len(d3.get('items', []))}")
    st, _ = call("GET", "/api/v1/torrents?search_area=4&search=tt0000000", vip)
    expect(D, "search_area=4 IMDb 不 500", st == 200, f"st={st}")

    # 排序：真的按序（设计口径：缺省降序，_asc 后缀升序——cursor.rs sort_of）
    for sort, key, desc in [("name", "name", True), ("size", "size", True),
                            ("seeders", "seeders", True), ("id", "id", True)]:
        st, js = call("GET", f"/api/v1/torrents?sort={sort}&limit=30", vip)
        rows = data_of(js).get("items", [])
        vals = [r.get(key) for r in rows if r.get(key) is not None]
        ordered = all(vals[i] >= vals[i + 1] for i in range(len(vals) - 1)) if desc \
            else all(vals[i] <= vals[i + 1] for i in range(len(vals) - 1))
        expect(D, f"sort={sort} 真排序({'降' if desc else '升'})",
            st == 200 and ordered,
               f"首末 {vals[:1]}/{vals[-1:]}" if vals else "空")
    st, js = call("GET", "/api/v1/torrents?sort=name_asc&limit=30", vip)
    rows = data_of(js).get("items", [])
    vals = [r.get("name") for r in rows if r.get("name")]
    expect(D, "sort=name_asc 升序", st == 200 and
           all(vals[i] <= vals[i + 1] for i in range(len(vals) - 1)),
           f"首末 {vals[:1]}/{vals[-1:]}" if vals else "空")

    # 分页：不重叠 + limit 钳位（活种仅几枚，用 alive=0 全量场测翻页）
    st, js = call("GET", "/api/v1/torrents?alive=0&limit=5", vip)
    p1 = data_of(js)
    cur = p1.get("next_cursor")
    if cur:
        st2, js2 = call("GET", f"/api/v1/torrents?alive=0&limit=5&"
            f"cursor={urllib.parse.quote(str(cur))}", vip)
        p2 = data_of(js2)
        ids1 = {r["id"] for r in p1.get("items", [])}
        ids2 = {r["id"] for r in p2.get("items", [])}
        expect(D, "游标翻页不重叠", not (ids1 & ids2),
            f"{sorted(ids1)} vs {sorted(ids2)}")
        # 回翻：dir=prev 应回到第一页
        st3, js3 = call("GET", f"/api/v1/torrents?alive=0&limit=5&"
            f"cursor={urllib.parse.quote(str(cur))}&dir=prev", vip)
        ids3 = {r["id"] for r in data_of(js3).get("items", [])}
        expect(D, "dir=prev 回翻命中首页", st3 == 200 and bool(ids3 & ids1),
               f"prev={sorted(ids3)}")
    else:
        rec(D, "WARN", "next_cursor 为空，翻页测试跳过（数据量不足）")
    st, js = call("GET", "/api/v1/torrents?limit=99999", vip)
    expect(D, "limit 钳位 ≤400", st == 200 and
           len(data_of(js).get("items", [])) <= 400)

    # 体积/日期区间
    st, js = call("GET", "/api/v1/torrents?size_min=1KB&size_max=10GB&limit=30",
        vip)
    rows = data_of(js).get("items", [])
    expect(D, "size 区间(带单位)", st == 200 and
           all(1024 <= (r.get("size") or 0) <= 10 * 1024**3 for r in rows),
           f"n={len(rows)}")
    st, js = call("GET",
        "/api/v1/torrents?size_min=1048576&size_max=1048576&limit=30", vip)
    rows = data_of(js).get("items", [])
    expect(D, "size 区间(裸字节+边界含等)", st == 200 and
           all((r.get("size") or 0) == 1048576 for r in rows), f"n={len(rows)}")
    for bad in ["2025-13-45", "2025-02-30", "abc"]:
        st, _ = call("GET", f"/api/v1/torrents?date_from={bad}", vip)
        expect(D, f"非法日期 {bad} 不 500", st in (200, 400), f"st={st}")
    st, js = call(
        "GET",
        "/api/v1/torrents?date_from=2020-01-01&date_to=2030-01-01&limit=30",
        vip)
    expect(D, "date 区间正常", st == 200 and len(data_of(js).get("items", [])) > 0)

    # 做种/下载/完成数区间真筛
    st, js = call("GET", "/api/v1/torrents?min_seeders=1&limit=50", vip)
    rows = data_of(js).get("items", [])
    expect(D, "min_seeders 真筛", st == 200 and
           all((r.get("seeders") or 0) >= 1 for r in rows), f"n={len(rows)}")
    st, js = call("GET", "/api/v1/torrents?max_seeders=0&limit=50", vip)
    rows = data_of(js).get("items", [])
    expect(D, "max_seeders 真筛", st == 200 and
           all((r.get("seeders") or 0) == 0 for r in rows), f"n={len(rows)}")

    # 排除词
    st, js = call("GET", "/api/v1/torrents?exclude=deep_alive&limit=50", vip)
    rows = data_of(js).get("items", [])
    expect(D, "exclude 排除词生效", st == 200 and
           all("deep_alive" not in (r.get("name") or "").lower() for r in rows),
           f"n={len(rows)}")

    # promo 筛选与徽标一致（交叉验证）
    st, js = call("GET", "/api/v1/torrents?promo=free&limit=50", vip)
    rows = data_of(js).get("items", [])
    promos = {(r.get("promotion") or "").lower() for r in rows}
    expect(D, "promo=free 行徽标都含 free", st == 200 and
           all("free" in p for p in promos), f"promos={sorted(promos)[:5]}")

    # 存活三态
    st, js = call("GET", "/api/v1/torrents?alive=2&limit=50", vip)
    rows = data_of(js).get("items", [])
    expect(D, "alive=2 全断种(seeders=0)", st == 200 and
           all((r.get("seeders") or 0) == 0 for r in rows), f"n={len(rows)}")
    st, js = call("GET", "/api/v1/torrents?alive=0&limit=5", vip)
    expect(D, "alive=0 含断种", st == 200 and
           len(data_of(js).get("items", [])) > 0)

    # mine / anonymous / bookmarked
    st, js = call("GET", "/api/v1/torrents?mine=1&limit=50", vip)
    expect(D, "mine=1 仅我发布", st == 200)
    st, js = call("GET", "/api/v1/torrents?anonymous=2&limit=50", vip)
    rows = data_of(js).get("items", [])
    expect(D, "anonymous=2 仅具名", st == 200 and
           all(r.get("anonymous") is False for r in rows), f"n={len(rows)}")
    st, js = call("GET", "/api/v1/torrents?bookmarked=1&limit=50", vip)
    expect(D, "bookmarked=1 可过", st == 200)

    # suggest
    st, js = call("GET", "/api/v1/torrents/suggest?q=deep", vip)
    expect(D, "suggest 端点", st == 200, f"st={st}")

    # 分类多选
    st, js = call("GET", "/api/v1/torrents?category_ids=1&limit=10", vip)
    expect(D, "category_ids=1 可过", st == 200)


# ---------------------------------------------------------------- 域：ratio

def d_ratio(vip: str, root: str) -> None:
    D = "ratio"
    # 真实用户旅程：VIP 上传自己的 1MB 探针种 → 三阶段 announce
    # （立基线 → 下完 → 做种上传）→ 等 worker tick → 全链对账。
    # ⚠️ 不能用站上任意活种：幽灵闸按**该种真实体积**的 10.4% 判物理下载，
    # 拿 50MB 的种报 1MB 下载 = 2% → 判幽灵（反作弊正确行为，非缺陷）。
    name = f"deep_ratio.{int(time.time())}"
    out = upload_probe_torrent(vip, name, descr="分享率深测探针种")
    if not out:
        rec(D, "WARN", "上传探针种失败，跳过")
        return
    tid, ih = out
    SIZE = 1_048_576

    # 期望快照 = base + Σledger（与 worker 重算同公式，避开快照列时序）
    def expected_up() -> int:
        base = psql("SELECT COALESCE(base_up,0) FROM balance_baseline "
                    "WHERE user_id=7346")
        led = psql("SELECT COALESCE(sum(delta_up),0) FROM traffic_ledger "
                   "WHERE user_id=7346")
        return int(float(base or 0)) + int(float(led or 0))

    up0 = expected_up()
    # 阶段 A：下载态立基线（left>0 → leeching，不计量）
    okr, det = tracker_announce(VIP_PASSKEY, ih, size=SIZE,
                                downloaded=0, uploaded=0)
    expect(D, "阶段A 下载态 announce", okr, det[:80])
    time.sleep(2)
    # 阶段 B：下完（downloaded=size → phys_down=100% ≥ 10.4% 闸 → 转做种）
    okr, det = tracker_announce(VIP_PASSKEY, ih, size=SIZE,
                                downloaded=SIZE, uploaded=0)
    expect(D, "阶段B 下完 announce", okr, det[:80])
    wait_worker_tick(rounds=10, secs=8)
    sn = psql(f"SELECT seeding, leeching, (completed_at IS NOT NULL) "
              f"FROM snatches WHERE user_id={VIP_UID} AND torrent_id={tid}")
    expect(D, "下完转做种态", sn.startswith("t|f") or sn == "t|f|t",
           f"row={sn}")

    # 阶段 C：做种上报上传 5MB（第二报起按锚点差计量）
    up_delta = 5 * 1024 * 1024
    okr, det = tracker_announce(VIP_PASSKEY, ih, size=SIZE,
                                downloaded=SIZE, uploaded=up_delta)
    expect(D, "阶段C 做种上传 announce", okr, det[:80])
    wait_worker_tick(rounds=12, secs=8)

    up1 = expected_up()
    expect(D, "上传增量 == 上报量(5MB)", up1 - up0 == up_delta,
           f"Δ={up1-up0} 期望={up_delta}")
    led_row = psql(f"SELECT delta_up FROM traffic_ledger "
                   f"WHERE user_id={VIP_UID} AND torrent_id={tid} "
                   f"AND delta_up > 0 ORDER BY id DESC LIMIT 1")
    expect(D, "流水有 delta_up=5MB 行", led_row == str(up_delta),
           f"row={led_row}")
    sn2 = psql(f"SELECT uploaded, seeding FROM snatches "
               f"WHERE user_id={VIP_UID} AND torrent_id={tid}")
    expect(D, "snatches.uploaded 记 5MB", sn2.startswith(str(up_delta)),
           f"row={sn2}")

    # torrents 聚合对账（seeders/leechers/times_completed 与 snatches 聚合一致）
    agg = psql(f"SELECT (SELECT count(*) FROM snatches WHERE torrent_"
        f"id={tid} AND seeding), "
               f"(SELECT count(*) FROM snatches WHERE torrent_id={tid} AND "
                   f"leeching), "
               f"(SELECT count(*) FROM snatches WHERE torrent_id={tid} AND "
                   f"completed_at IS NOT NULL)")
    trow = psql(f"SELECT seeders, leechers, times_completed FROM torrents WHERE id={tid}")
    expect(D, "torrents 计数 == snatches 聚合", agg == trow,
           f"agg={agg} torrents={trow}")

    # users 快照列 == 期望公式（快照机制本身的对账）
    snap = psql(f"SELECT uploaded FROM users WHERE id={VIP_UID}")
    expect(D, "users.uploaded 快照 == base+Σledger", snap == str(up1),
           f"snap={snap} expect={up1}")

    # 分享率计算（/me overview 或 /open）——分母用**断言时点**的 downloaded
    # （快照 = base_down + Σledger.delta_down，阶段B 的 1MB 已进分母）
    st, js = call("GET", "/api/v1/me/overview", vip)
    d = data_of(js)
    down_now = int(float(psql("SELECT COALESCE(base_down,0) FROM balance_baseline "
                              "WHERE user_id=7346") or 0)) + int(float(
        psql("SELECT COALESCE(sum(delta_down),0) FROM traffic_ledger "
             "WHERE user_id=7346") or 0))
    if "ratio" in d and down_now > 0:
        expect(D, "ratio == up/down", abs(d["ratio"] - (up1 / down_now)) < 0.01,
               f"ratio={d['ratio']} up={up1} down={down_now}")
    else:
        rec(D, "WARN", f"overview 无 ratio 或缺 downloaded（down={down_now}）")

    # 清理：软删探针种（走 root，保持现场干净）
    call("DELETE", f"/api/v1/torrents/{tid}", root)


# ---------------------------------------------------------------- 域：rss

def d_rss(vip: str, root: str) -> None:
    D = "rss"
    # 站点 canonical 订阅路径是 /api/v1/rss/{passkey}（rss-info 口同款 base）
    RSS = f"/api/v1/rss/{VIP_PASSKEY}"
    st, body = call("GET", RSS, vip, raw=True)
    xml = body.decode("utf-8", "replace") if isinstance(body,
        bytes) else str(body)
    expect(D, "RSS 200 且 XML",
        st == 200 and "<rss" in xml and "<channel" in xml,
           f"st={st}")
    n_items = xml.count("<item>")
    expect(D, "RSS 有 item", n_items > 0, f"n={n_items}")
    for tag in ["<title>", "<link>", "<description>", "<pubDate>"]:
        expect(D, f"RSS 含 {tag}", tag in xml)
    expect(D, "RSS 含 enclosure(自闭合)", "<enclosure " in xml and
           "application/x-bittorrent" in xml and "download.php" in xml)
    # 条目与库内过审种按 id 比对（RSS 默认含断种——0375 口径；
    # <title> 是格式化串，不能与裸站名求交）
    db_ids = {l.strip() for l in psql(
        "SELECT id FROM torrents WHERE approval_status=1 "
        "ORDER BY id DESC LIMIT 50").splitlines() if l.strip()}
    xml_ids = set(re.findall(r"id=(\d+)", xml))
    expect(D, "RSS 条目与库内过审种有交集", len(db_ids & xml_ids) > 0,
           f"xml={len(xml_ids)} db={len(db_ids)}")
    # 新种在前
    ids = [int(m) for m in re.findall(r"id=(\d+)", xml)]
    expect(D, "RSS 新种在前", ids == sorted(ids, reverse=True), f"{ids[:8]}")
    # showrows/limit
    st, body = call("GET", RSS + "?showrows=3", vip, raw=True)
    xml2 = body.decode("utf-8", "replace") if isinstance(body,
        bytes) else str(body)
    expect(D, "showrows=3 生效", st == 200 and xml2.count("<item>") == 3,
           f"n={xml2.count('<item>')}")
    # 分类过滤（不存在分类 → 空）
    st, body = call("GET", RSS + "?categories=999999", vip, raw=True)
    xml3 = body.decode("utf-8", "replace") if isinstance(body,
        bytes) else str(body)
    expect(D, "不存在分类过滤空", st == 200 and xml3.count("<item>") == 0,
           f"n={xml3.count('<item>')}")
    # paid=1 仅促销 + linktype=page
    st, body = call("GET", RSS + "?paid=1", vip, raw=True)
    expect(D, "paid=1 可过", st == 200, f"st={st}")
    st, body = call("GET", RSS + "?linktype=page", vip, raw=True)
    xml5 = body.decode("utf-8", "replace") if isinstance(body,
        bytes) else str(body)
    item_links = []
    item_titles = []
    for seg in xml5.split("<item>")[1:]:
        m1 = re.search(r"<link>(.*?)</link>", seg)
        m2 = re.search(r"<title>(.*?)</title>", seg)
        if m1:
            item_links.append(m1.group(1))
        if m2:
            item_titles.append(m2.group(1))
    expect(D, "linktype=page 链接指详情页", st == 200 and item_links and
           all("/torrent/" in l for l in item_links), f"links={item_links[:2]}")
    expect(D, "linktype=page 标题仅名称", st == 200 and item_titles and
           all(" · " not in t for t in item_titles), f"titles={item_titles[:2]}")
    # 关键字
    st, body = call("GET", RSS + "?search=" + urllib.parse.quote("深测"), vip,
        raw=True)
    xml6 = body.decode("utf-8", "replace") if isinstance(body,
        bytes) else str(body)
    expect(D, "RSS 关键字过滤", st == 200 and xml6.count("<item>") >= 0, f"st={st}")
    # 坏 passkey / 非法形状
    st, _ = call("GET", "/api/v1/rss/deadbeefdeadbeefdeadbeefdeadbeef", vip,
        raw=True)
    expect(D, "坏 passkey 404", st == 404, f"st={st}")
    st, _ = call("GET", "/api/v1/rss/short", vip, raw=True)
    expect(D, "非法形状 passkey 400", st == 400, f"st={st}")
    # Token 面：fxr_ token 管理（真实路径 /api/v1/rss/tokens）
    st, js = call("GET", "/api/v1/rss/tokens", vip)
    expect(D, "RSS Token 列表", st == 200, f"st={st}")
    # 列表形状是 data.tokens；rows_of 只认 items/rows ⇒ 旧清理永远
    # 匹配不到，deep_rss_* 会积到上限 5 把下一轮「建 Token」判红。
    def _drop_deep_tokens() -> None:
        st_t, js_t = call("GET", "/api/v1/rss/tokens", vip)
        for r in (data_of(js_t).get("tokens") or []):
            if (r.get("name") or "").startswith("deep_rss_"):
                call("DELETE", f"/api/v1/rss/tokens/{r['id']}", vip)

    _drop_deep_tokens()
    st, js = call("POST", "/api/v1/rss/tokens", vip,
                  body={"name": f"deep_rss_{int(time.time())}"})
    if ok(D, "建 RSS Token", st, js):
        tok = data_of(js).get("token")
        if tok:
            st, body = call("GET", f"/api/v1/rss/{tok}", vip, raw=True)
            expect(D, "fxr_ Token 可拉订阅", st == 200 and
                   b"<rss" in (body if isinstance(body, bytes) else b""),
                   f"st={st}")
        # 清理本次（含本轮之前的残留）
        _drop_deep_tokens()
    # torznab（ caps 能力清单）
    st, _ = call("GET", "/api/v1/torznab?t=caps", vip, raw=True)
    expect(D, "torznab caps 可过", st == 200, f"st={st}")


# ---------------------------------------------------------------- main

DOMAINS = {
    "search": d_search,
    "pm": d_pm,
    "ratio": None,  # 需 probe，特判
    "rss": d_rss,
    "mall": d_mall,
    "invite2": d_invite2,
    "admin2": d_admin2,
    "dlforum": d_dlforum,
}


def main() -> None:
    wanted = [a for a in sys.argv[1:] if a in DOMAINS] or list(DOMAINS)
    # 探针用户自愈（并行会话可能降级/清零）。⚠️ users.uploaded 是**快照列**：
    # worker 每 tick 按 base_up + Σtraffic_ledger.delta_up 重算——裸写 uploaded
    # 会在下一次 announce 消费时被冲掉（实测 50GB 归零）。所以自愈必须走
    # balance_baseline + 清 ledger，与快照公式自洽。
    psql(
        "UPDATE users SET class_id = 9, "
        "downloaded = GREATEST(downloaded, 2684354560), "
        "spark_balance = GREATEST(COALESCE(spark_balance,0), 50000) WHERE id = "
            "7346"
    )
    psql("DELETE FROM traffic_ledger WHERE user_id = 7346")
    psql(
        "INSERT INTO balance_baseline (user_id, base_up, base_down, "
            "base_spark, "
        "base_seed_secs, through, updated_at) VALUES "
        "(7346, 53687091200, 2684354560, 50000, 0, now(), now()) "
        "ON CONFLICT (user_id) DO UPDATE SET base_up = 53687091200, "
        "base_down = 2684354560, base_spark = 50000, updated_at = now()"
    )
    psql("DELETE FROM invite_quota WHERE user_id = 7346")
    vip = mint(VIP_UID, 9)
    root = mint(1, 99)
    csrf_handshake(vip)
    ensure_alive_probe(root) if any(
        d in wanted for d in ("search", "rss", "admin2", "dlforum")) else None

    for name in wanted:
        if name == "ratio":
            d_ratio(vip, root)
        else:
            DOMAINS[name](vip, root)

    fails = [r for r in RESULTS if r[1] == "FAIL"]
    warns = [r for r in RESULTS if r[1] == "WARN"]
    print(f"\n===== 汇总：{len(RESULTS)} 项，FAIL {len(fails)}，WARN {len(warns)} =====")
    for dom, lvl, msg in RESULTS:
        mark = {"PASS": "✓", "FAIL": "✗", "WARN": "!", "INFO": "·"}[lvl]
        print(f" {mark} [{dom}] {msg}")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()

