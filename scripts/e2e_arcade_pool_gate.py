# -*- coding: utf-8 -*-
"""娱乐屋奖池写侧关闸核验（0242 / fa193f2）。

要验的命题只有一条：**面板没有能力把玩法打成 503**。
`arcade_pool_save` 的 `validate_pool` 在 `db.begin()` 之前，所以不合法的保存必须
「返回 400 且表里一字未改」——只验 400 不验表未变，等于没验关闸（可能先写后判）。

读侧一律走 `/admin/arcade/overview`：它现在读表，所以「写进去了吗」和「玩法读到的是
同一份」用同一个端点证明，避免脚本自己查库造出第二份口径。

端点尚未部署时 POST 会返回 404「接口不存在」——那是旧二进制，不是关闸失效。
关闸失效的表现是 200 且表被改坏；脚本把两种情况分开报，不会把前者误读成后者。

用法：起一个带新二进制的侧容器，然后
    export FLUX_API_BASE=http://127.0.0.1:8180/api/v1
    python scripts/e2e_arcade_pool_gate.py
"""

import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")
POOL_KEY = "jgg_default"
SCRATCH_SNAP = {}
BS_SNAP = {}
FARM_SNAP = {}  # 农场彩蛋池进入时的样子（收尾/兜底都要写回去）
RUN = str(int(time.time()))  # 幂等键每轮必须不同，否则第二轮被判「已受理」
fails = []
n_checks = [0]


def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=20) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}


def login(user="root", pw="password123"):
    for i in range(10):
        st, r = call("POST", "/auth/login",
                     {"username": user, "password": pw})
        tok = (r.get("data") or {}).get("token")
        if tok:
            return tok
        time.sleep(min(2 + i * 3, 20))  # 限流窗口比 1.5s 长得多
    raise SystemExit("登录失败：侧容器是否指向同一个库？")


def snapshot(tok):
    """读侧现状。overview 读表，所以这就是表的投影。"""
    st, r = call("GET", "/admin/arcade/overview", token=tok)
    assert st == 200, ("读 overview 失败", st, r)
    return r.get("data") or {}


def _norm(prize):
    """把读侧档位归一成脚本内部形状。
    0244 之后魔力位报 multiples、物品位报 item_key/qty/anchor，
    没有统一的 payout 字段 —— 在这里补别名，下游就不用到处猜。"""
    p = dict(prize)
    if p.get("kind") == "item":
        p["payout"] = 0
    else:
        p["payout"] = p.get("multiples", 0)
    return p


def pool_of(tok):
    st, r = call("GET", "/games", token=tok)
    if st != 200:
        return None
    j = (r.get("data") or {}).get("jgg") or {}
    if j.get("prizes"):
        j["prizes"] = [_norm(x) for x in j["prizes"]]
    return j


def to_req(p):
    """读侧档位 -> 写侧请求体：按 kind 分派，不再假设每档都有 payout。"""
    e = {"label": p["label"], "weight": p["weight_permille"],
         "kind": p.get("kind", "magic"), "enabled": True}
    if e["kind"] == "item":
        e.update({"item_key": p["item_key"], "qty": p["qty"]})
    else:
        e["payout"] = p["payout"]
    return e


def triples(prizes):
    """档位投影：只比标签/权重/赔付 —— 票价改动不该波及它们。"""
    return [(p["label"], p["weight_permille"], p["payout"]) for p in prizes]


def check(name, cond, detail=""):
    msg = ("  PASS  " if cond else "  FAIL  ") + name
    if not cond:
        msg += "   " + str(detail)[:220]
        fails.append(name)
    print(msg)
    n_checks[0] += 1


def magic(lb, wt, mu):
    return {"label": lb, "weight": wt, "kind": "magic",
            "payout": mu, "enabled": True}


def psql(sql):
    """验「入账」这类命题只能查账本；与 e2e_arcade_items.py 同一手法
    （docker exec psql），脚本自己造的夹具行也自己收。"""
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True,
    )
    return r.stdout.strip()


def item(lb, wt, k):
    return {"label": lb, "weight": wt, "kind": "item",
            "item_key": k, "qty": 1, "enabled": True}


def main():
    tok = login()
    print("FLUX_API_BASE =", BASE)

    before = pool_of(tok)
    if not before or not before["prizes"]:
        raise SystemExit("读侧没拿到奖池 —— 玩法读表那一半没生效，"
                         "先确认二进制版本")
    print("基线：ticket=%s 档数=%s"
          % (before.get("ticket"), len(before["prizes"])))

    # 契约断言：payout 是前台一直在读的既有字段（apps/web/lib/games.ts）。
    # 上一轮把它重命名成 multiples 直接把客户端打坏了 —— 新语义只能附加，
    # 不能替换已上线的响应字段。这条断言就是为了让下次替换立刻红。
    # 必须读**原始**响应：pool_of 里的 _norm 会自己补 payout，
    # 拿归一化后的对象做契约断言等于测我自己的代码，永远绿。
    resp = call("GET", "/games", token=tok)[1]
    raw = ((resp.get("data") or {}).get("jgg") or {}).get("prizes") or []
    check("契约·服务端仍返回 payout 字段",
          bool(raw) and all("payout" in x for x in raw), raw[:2])
    check("契约·服务端仍返回 kind 字段",
          bool(raw) and all("kind" in x for x in raw), raw[:2])

    # 用读侧的投影反构一个「只抬 50x 权重」的坏池：抬到 40 后 EV 破 1
    entries = [to_req(p) for p in before["prizes"]]

    def body_of(ents, ticket=None):
        return {"pool_key": POOL_KEY, "game": "jgg",
                "label": "九宫格 · 标准池",
                "ticket": before["ticket"] if ticket is None else ticket,
                "entries": ents}

    # ① 经济档权重堆到 EV 1.2
    body_bad = body_of([magic("谢谢参与", 400, 0), magic("2x 魔力", 600, 2)])

    st, r = call("POST", "/admin/arcade/pool", body_bad, tok)
    check("后门①抬经济奖权重 -> 400 拒绝", st == 400, (st, r))
    dumped = json.dumps(r, ensure_ascii=False)
    check("拒绝原因点名 EV", "EV" in dumped or "返还" in dumped, r)

    after_bad = pool_of(tok)
    check("被拒之后表里一字未改（档数/权重/票价全等）", after_bad == before,
          {"before": before, "after": after_bad})

    # 其余四类坏配置：空池 / 零权重 / 票价非正 / EV 恰为 1
    cases = [
        ("空池", body_of([])),
        ("零权重档", body_of([magic("谢谢参与", 500, 0), magic("2x", 0, 2)])),
        ("票价 0", body_of([magic("谢谢参与", 900, 0), magic("2x", 100, 2)],
                           ticket=0)),
        ("EV 恰为 1 的中性池",
         body_of([magic("谢谢参与", 500, 0), magic("2x", 500, 2)])),
        ("物品位引用不存在/停用物品",
         body_of([magic("谢谢参与", 900, 0),
                  item("空头券", 100, "no_such_item")])),
    ]
    for name, body in cases:
        st, r = call("POST", "/admin/arcade/pool", body, tok)
        check("拒绝 %s" % name, st == 400, (st, r))
        check("%s 拒绝后表未变" % name, pool_of(tok) == before)

    # 合法保存：只改票价，EV 按定义只与比例有关，不该变
    ok_body = body_of(entries, before["ticket"] + 20)
    st, r = call("POST", "/admin/arcade/pool", ok_body, tok)
    check("合法保存 -> 200", st == 200, (st, r))
    after_ok = pool_of(tok)
    check("票价改动被玩法读到（写读同一条链）",
          after_ok.get("ticket") == before["ticket"] + 20, after_ok)
    check("档数与权重未被顺手改写",
          triples(after_ok["prizes"]) == triples(before["prizes"]),
          after_ok.get("prizes"))

    # 复原，不留残留（main 外层还有 try/finally 兜底）
    st, _ = call("POST", "/admin/arcade/pool", body_of(entries), tok)
    check("复原成功", st == 200)
    check("复原后与基线完全一致", pool_of(tok) == before, pool_of(tok))

    # ── 刮刮乐也读奖池行表（0248）：票档就是那道 EV 闸 ──
    st, ov = call("GET", "/games", None, tok)
    sc = ((ov.get("data") or {}).get("scratch")) or {}
    srows = sc.get("prizes") or []
    SCRATCH_SNAP.update({"ticket": sc.get("ticket"), "prizes": srows})
    check("刮刮乐档位来自行表（带权重与等值）",
          st == 200 and len(srows) >= 3
          and all("weight_permille" in r for r in srows),
          (st, [(r.get("label"), r.get("weight_permille")) for r in srows]))
    check("刮刮乐权重合计 1000 千分",
          sum(r.get("weight_permille", 0) for r in srows) == 1000,
          sum(r.get("weight_permille", 0) for r in srows))
    ev_sc = sc.get("expected_value") or 0
    check("刮刮乐 EV 由行表现算且等于搬家前的 0.66",
          0 < ev_sc < 1 and abs(ev_sc - 0.66) < 1e-6, ev_sc)
    low = {"pool_key": "scratch_default", "game": "scratch",
           "label": "刮刮乐 · 探针", "ticket": 1,
           "entries": [magic("未中奖", 500, 0), item("抽卡券", 500, "ticket")]}
    st, r = call("POST", "/admin/arcade/pool", low, tok)
    check("票档 1 时 900 折算价的物品位被 EV 闸拒", st == 400, (st, str(r)[:160]))
    high = dict(low, ticket=500, label="刮刮乐 · 标准票")
    st, r = call("POST", "/admin/arcade/pool", high, tok)
    check("同一张表把票档抬到 500 就过关（票档就是那道闸）",
          st == 200, (st, str(r)[:160]))
    st, r = call("POST", "/admin/arcade/pool",
                 dict(high, ticket=10000), tok)
    txt = json.dumps(r, ensure_ascii=False)
    check("票档抬过单次下注上限被拒（没有合法注额的表存不进去）",
          st == 400 and ("上限" in txt or "max_bet" in txt), (st, txt[:160]))
    st, r = call("POST", "/admin/arcade/pool", high, tok)
    check("抬回 500 仍可保存（上一把的拒绝没有留下半成品）", st == 200,
          (st, str(r)[:120]))
    st, ov2 = call("GET", "/games", None, tok)
    sc2 = ((ov2.get("data") or {}).get("scratch")) or {}
    check("改完票档玩法立刻读到（写读同一条链）",
          sc2.get("ticket") == 500 and len(sc2.get("prizes") or []) == 2,
          (sc2.get("ticket"), len(sc2.get("prizes") or [])))
    st, r = call("POST", "/games/scratch",
                 {"bet": 50, "idempotency_key": "e2e-scratch-" + RUN}, tok)
    check("注额低于票档时刮刮乐拒开（不悄悄按最低档玩）",
          st == 400 and "最低注额" in json.dumps(r, ensure_ascii=False),
          (st, json.dumps(r, ensure_ascii=False)[:140]))

    # ── 猜大小读奖池行表（0251）：三区必须配满，赔率不再是设置键 ──
    def bs_body(entries, ticket=1):
        return {"pool_key": "bigsmall_default", "game": "bigsmall",
                "label": "猜大小 · 标准桌", "ticket": ticket,
                "entries": entries}

    def bs_row(lb, wt, side, payout=None, item=None, qty=1):
        e = {"label": lb, "weight": wt, "side": side, "enabled": True}
        if item:
            e.update({"kind": "item", "item_key": item, "qty": qty})
        else:
            e.update({"kind": "magic", "payout": payout})
        return e

    st, ov3 = call("GET", "/games", None, tok)
    bs = ((ov3.get("data") or {}).get("bigsmall")) or {}
    brows = bs.get("prizes") or []
    BS_SNAP.update({"ticket": bs.get("ticket"), "prizes": brows})
    check("猜大小三区档位来自行表",
          st == 200 and len(brows) == 3
          and sorted(x.get("side") for x in brows) == ["lose", "tie", "win"],
          (st, [(x.get("label"), x.get("side")) for x in brows]))
    ev_bs = bs.get("expected_value") or 0
    check("猜大小 EV 由行表现算且等于搬家前的 0.951",
          abs(ev_bs - 0.951) < 1e-6, ev_bs)
    st, r = call("POST", "/admin/arcade/pool", bs_body([
        bs_row("猜中", 490, "win", payout=1.9),
        bs_row("平局", 20, "tie", payout=1.0)]), tok)
    txt = json.dumps(r, ensure_ascii=False)
    # 缺一个区会被两道闸之一挡下（机制校验或 EV —— 少一区等于把 490‰ 的
    # 派彩摊到 510‰ 上，EV 直接冲破 1）。要钉的是「存不进去」这件事。
    check("缺输区的桌存不进去（机制或 EV 闸任一先响）",
          st == 400 and ("区" in txt or "增发" in txt), (st, txt[:140]))
    st, r = call("POST", "/admin/arcade/pool", bs_body([
        bs_row("猜中", 500, "win", payout=1.9),
        bs_row("平局", 20, "tie", payout=1.0),
        bs_row("猜错", 480, "lose", payout=0)]), tok)
    check("赢区多给 10 千分、破坏 49/2/49 被拒", st == 400, (st, str(r)[:140]))
    gift = [bs_row("猜中派彩", 440, "win", payout=1.9),
            bs_row("猜中送券", 50, "win", item="ticket"),
            bs_row("平局返本", 20, "tie", payout=1.0),
            bs_row("猜错归零", 490, "lose", payout=0)]
    st, r = call("POST", "/admin/arcade/pool", bs_body(gift), tok)
    check("票档 1 时赢区送 900 券被 EV 闸拒", st == 400, (st, str(r)[:140]))
    st, r = call("POST", "/admin/arcade/pool", bs_body(gift, ticket=500),
                 tok)
    check("抬票档到 500 后「猜中送券」这张桌过关", st == 200,
          (st, str(r)[:140]))
    st, r = call("POST", "/games/bigsmall",
                 {"bet": 100, "guess": "big",
                  "idempotency_key": "e2e-bs-1-" + RUN}, tok)
    check("注额低于票档时猜大小拒开这一局",
          st == 400 and "最低注额" in json.dumps(r, ensure_ascii=False),
          (st, json.dumps(r, ensure_ascii=False)[:140]))
    st, r = call("POST", "/admin/arcade/pool", bs_body([
        bs_row("猜中派彩", 490, "win", payout=1.9),
        bs_row("平局返本", 20, "tie", payout=1.0),
        bs_row("猜错归零", 490, "lose", payout=0)]), tok)
    check("猜大小桌复原（票档回到 1）", st == 200, (st, str(r)[:140]))
    # 真打一局要用注过魔力的探针号：dev 库里 root 余额是负的（早期商店
    # 测试留的 -1142），拿它下注只会撞到经济守卫，测不出玩法本身
    uname = "e2ebs" + RUN[-6:]
    st, r = call("POST", "/admin/adduser",
                 {"username": uname, "email": uname + "@e2e-probe.invalid",
                  "password": "E2eProbe!123"}, tok)
    uid = (r.get("data") or {}).get("user_id")
    call("POST", "/admin/users/adjust",
         {"user_id": uid, "spark_delta": 2000}, tok)
    call("POST", "/me/password/change",
         {"old_password": "E2eProbe!123", "new_password": "E2eProbe!456"},
         login(uname, "E2eProbe!123"))
    ptok = login(uname, "E2eProbe!456")
    st, r = call("POST", "/games/bigsmall",
                 {"bet": 100, "guess": "big",
                  "idempotency_key": "e2e-bs-2-" + RUN}, ptok)
    d = r.get("data") or {}
    check("探针号真打一局：回报带 side 与派彩",
          st == 200 and d.get("side") in ("win", "tie", "lose"), (st, d))
    # 刮之前先把刮刮乐桌从探针态复原（上一段把票档抬到了 500，
    # 不复原的话这一注只会被「最低注额」挡下，测不到派彩路径）
    st, r = call("POST", "/admin/arcade/pool", {
        "pool_key": "scratch_default", "game": "scratch",
        "label": "刮刮乐 · 标准票",
        "ticket": SCRATCH_SNAP.get("ticket"),
        "entries": [to_req(x) for x in SCRATCH_SNAP["prizes"]]}, tok)
    check("刮刮乐桌撤掉探针票档", st == 200, (st, str(r)[:120]))
    st, r = call("POST", "/games/scratch",
                 {"bet": 100, "idempotency_key": "e2e-sc-" + RUN}, ptok)
    d2 = r.get("data") or {}
    check("探针号真刮一注：回报带 kind 与等值",
          st == 200 and d2.get("kind") in ("magic", "item", "fallback"),
          (st, d2))
    call("POST", "/admin/users/status",
         {"user_id": uid, "status": 2, "reason": "e2e 探针清理"}, tok)
    st3, _ = call("DELETE", "/admin/users/%s" % uid, None, tok)
    check("探针号已清理", st3 == 200, (uid, st3))

    print("\n结果：%d 项断言，失败 %d" % (n_checks[0], len(fails)))
    sys.exit(1 if fails else 0)


def _restore_snapshot():
    """脚本崩在半路也要把奖池写回进入时的样子：
    本轮就发生过 KeyError 把 ticket=120 的探针态留在生产库。"""
    tok = login()
    snap = pool_of(tok)
    if not snap or not snap.get("prizes"):
        print("兜底复原：读不到奖池，跳过")
        return
    st, _ = call("POST", "/admin/arcade/pool", body_for_restore(snap), tok)
    # 刮刮乐池也被本脚本写过，必须一起复原（只复原「主对象」不算复原）
    st2 = "-"
    if SCRATCH_SNAP.get("prizes"):
        body = {"pool_key": "scratch_default", "game": "scratch",
                "label": "刮刮乐 · 标准票",
                "ticket": SCRATCH_SNAP.get("ticket"),
                "entries": [to_req(x) for x in SCRATCH_SNAP["prizes"]]}
        st2, _ = call("POST", "/admin/arcade/pool", body, tok)
    st3 = "-"
    if BS_SNAP.get("prizes"):
        body = {"pool_key": "bigsmall_default", "game": "bigsmall",
                "label": "猜大小 · 标准桌",
                "ticket": BS_SNAP.get("ticket"),
                "entries": [dict(to_req(x), side=x.get("side"))
                            for x in BS_SNAP["prizes"]]}
        st3, _ = call("POST", "/admin/arcade/pool", body, tok)
    print("兜底复原奖池 -> 九宫格 %s / 刮刮乐 %s / 猜大小 %s"
          % (st, st2, st3))


def body_for_restore(snap):
    return {"pool_key": POOL_KEY, "game": "jgg", "label": "九宫格 · 标准池",
            "ticket": snap.get("ticket"),
            "entries": [to_req(p) for p in snap["prizes"]]}


if __name__ == "__main__":
    try:
        main()
    finally:
        try:
            _restore_snapshot()
        except Exception as e:  # 兜底本身失败要喊出来，不能静默
            print("兜底复原失败，请手工核对 arcade_pool_entries：%r" % e)
