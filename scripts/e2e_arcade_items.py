# -*- coding: utf-8 -*-
"""娱乐屋物品侧核验（0244 目录 / 0245 物品位 / arcade_item_save）。

三条命题：
1. 物品位真的在池子里，且价值按 arcade_items.anchor 折算（不是按登记价、不是按倍数）；
2. 改物品 anchor 会连带改掉所有引用它的奖池 EV —— 所以写侧必须回查并整体回滚，
   否则面板本身就是一条绕过关闸的后门；
3. 玩法能真的发出物品，且每人上限用尽后走「回落折魔力」而不是继续白发。

用法：FLUX_API_BASE=http://127.0.0.1:8080/api/v1 python scripts/e2e_arcade_items.py
"""
import json
import os
import sys
import time
import urllib.error
import urllib.request

BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")
fails = []
n = [0]


def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=25) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}


def login(user="root", pw="password123"):
    for _ in range(5):
        st, r = call("POST", "/auth/login",
                     {"username": user, "password": pw})
        tok = (r.get("data") or {}).get("token")
        if tok:
            return tok
        time.sleep(1.5)
    raise SystemExit("登录失败")


def check(name, cond, detail=""):
    n[0] += 1
    msg = ("  PASS  " if cond else "  FAIL  ") + name
    if not cond:
        msg += "   " + str(detail)[:260]
        fails.append(name)
    print(msg)


def pool(tok):
    st, r = call("GET", "/games", None, tok)
    assert st == 200, st
    return (r.get("data") or {}).get("jgg") or {}


def anchor_of_item(tok, key):
    """读侧投影：某个物品位当前生效的目录 anchor（不另查库造第二口径）。"""
    rows = [p for p in pool(tok).get("prizes", [])
            if p.get("item_key") == key]
    return rows[0]["anchor"]


def item_body(key, kind="cosmetic", anchor=0, src="n/a", per_user=1,
              unlimited=True, stock=0):
    return {"key": key, "name": "核验用物品 " + key, "kind": kind,
            "anchor": anchor, "anchor_src": src, "unlimited": unlimited,
            "stock": stock, "per_user": per_user, "icon": "🧪",
            "enabled": True}


def magic_row(label, weight):
    return {"label": label, "weight": weight, "payout": 0,
            "kind": "magic", "enabled": True}


def item_row(label, weight, key):
    return {"label": label, "weight": weight, "kind": "item",
            "item_key": key, "qty": 1, "enabled": True}


def main():
    tok = login()
    print("FLUX_API_BASE =", BASE)

    # ── 1. 物品位在池子里，且按 anchor 折算 ──
    j = pool(tok)
    prizes = j.get("prizes") or []
    items = [p for p in prizes if p.get("kind") == "item"]
    check("奖池含物品位（0245 生效）", len(items) >= 1,
          [p.get("label") for p in prizes])
    check("物品位带目录 anchor 与件数",
          all(p.get("anchor", 0) > 0 and p.get("qty", 0) >= 1 for p in items),
          items[:2])
    check("物品位的 value 等于 anchor×qty（不是倍数）",
          all(p["value"] == p["anchor"] * p["qty"] for p in items), items[:2])
    st, adm = call("GET", "/admin/arcade/overview", None, tok)
    ev = json.dumps(adm)
    check("后台面板读到含物品的 EV 且 < 1", st == 200 and '"ev"' in ev, st)

    # ── 2. 物品写侧关闸 ──
    st, r = call("POST", "/admin/arcade/items",
                 item_body("gate_probe", kind="economic"), tok)
    check("economic 物品 anchor=0 -> 400", st == 400, (st, r))
    st, r = call("POST", "/admin/arcade/items",
                 item_body("gate_probe", kind="gold_bag"), tok)
    check("未知 kind -> 400（kind 是自由 TEXT 但目录口径要收口）",
          st == 400, (st, r))
    st, r = call("POST", "/admin/arcade/items",
                 {**item_body("gate_probe"), "anchor_src": "guess"}, tok)
    check("未知 anchor_src -> 400", st == 400, (st, r))
    st, r = call("POST", "/admin/arcade/items", item_body("gate_probe"), tok)
    check("合法 cosmetic -> 200", st == 200, (st, r))

    # ── 3. 跨池回查：抬 anchor 到会让引用池 EV>=1 的水位，必须整体拒绝 ──
    ref = items[0]["item_key"]
    anchor_before = anchor_of_item(tok, ref)
    # 抬到一个必然把 EV 顶破 1 的价值（票价 100，权重占比小，所以给个天文数字）
    st, r = call("POST", "/admin/arcade/items",
                 item_body(ref, kind="economic", anchor=10 ** 9,
                           src="declared"), tok)
    check("抬 anchor 会让引用池 EV>=1 -> 400 且回滚", st == 400, (st, r))
    anchor_after = anchor_of_item(tok, ref)
    check("被拒之后目录 anchor 未被改写", anchor_before == anchor_after,
          (anchor_before, anchor_after))

    # ── 4. 玩法真发物品 + 每人上限用尽后回落（做成确定性，不靠概率撞）──
    # 临时把池子改成「11% 抽中抽卡券」：EV = 110×900/1000/100 = 0.99 < 1，
    # 仍在闸内；再把 ticket 的每人上限压到 1。
    # 于是「第一次发出、之后每次必回落」是确定的。
    base = [(x["label"], x.get("weight_permille"), x.get("multiples"),
             x.get("kind"), x.get("item_key"), x.get("qty")) for x in prizes]
    probe_pool = {
        "pool_key": "jgg_default", "game": "jgg", "label": "九宫格 · 标准池",
        "ticket": j["ticket"],
        "entries": [magic_row("谢谢参与", 890),
                    item_row("抽卡券 ×1", 110, "ticket")]}
    st, r = call("POST", "/admin/arcade/pool", probe_pool, tok)
    check("临时探针池（EV 0.99）可存", st == 200, (st, r))
    t_item = [x for x in prizes if x.get("item_key") == "ticket"][0]
    cap1 = {"key": "ticket", "name": "抽卡券 ×1", "kind": "voucher",
            "anchor": t_item["anchor"], "anchor_src": "derived",
            "unlimited": True, "stock": 0, "per_user": 1, "icon": "🎟",
            "enabled": True}
    st, r = call("POST", "/admin/arcade/items", cap1, tok)
    check("把 ticket 每人上限压到 1 可存", st == 200, (st, r))

    # 抽数必须用一次性探针账号，不能用 root：
    #  a) games_max_plays_per_hour 的限次会把大批量抽压成前 60 抽有效，
    #     而循环里的 `if st != 200: continue` 会把 429 全吞掉，
    #     报成「0 次发放」——看着像功能坏了，其实是测试自己把证据吃掉了；
    #  b) 所以「成功抽了多少次」本身要做成断言，被限次不能伪装成没抽中。
    uname = "e2ecap%s" % int(time.time() % 100000)
    st, r = call("POST", "/admin/adduser",
                 {"username": uname, "email": uname + "@e2e-probe.invalid",
                  "password": "E2eProbe!123"}, tok)
    uid = (r.get("data") or {}).get("user_id")
    check("前置·探针账号已建出 uid", uid is not None, (st, r))
    st, r = call("POST", "/admin/users/adjust",
                 {"user_id": uid, "spark_delta": 20000}, tok)
    check("前置·探针账号已注魔力（余额 0 付不起票价）", st == 200, (st, r))
    # 后台建的账号用的是临时密码，**不改密码不许消费**。漏这一步
    # 会让所有抽数吃 400「账号正在使用临时密码」。
    st, r = call("POST", "/me/password/change",
                 {"old_password": "E2eProbe!123",
                  "new_password": "E2eProbe!456"},
                 login(uname, "E2eProbe!123"))
    check("前置·探针账号已改掉临时密码", st == 200, (st, r))
    ptok = login(uname, "E2eProbe!456")

    # 物品档权重受 EV<1 限制（anchor 900 / 票价 100 → 最多约 11%），命中不确定。
    # 造一个 anchor == 票价 的探针物品，它等价于 1 倍魔力档，
    # 于是可以在 EV 远小于 1 的前提下占到 50% 权重，命中成为确定事件。
    probe_item = {"key": "cap_probe", "name": "上限探针券", "kind": "voucher",
                  "anchor": j["ticket"], "anchor_src": "derived",
                  "unlimited": True, "stock": 0, "per_user": 1,
                  "icon": "🧪", "enabled": True}
    st, r = call("POST", "/admin/arcade/items", probe_item, tok)
    check("探针物品（anchor=票价）可存", st == 200, (st, r))
    st, r = call(
        "POST", "/admin/arcade/pool",
        {"pool_key": "jgg_default", "game": "jgg",
         "label": "九宫格 · 上限探针", "ticket": j["ticket"],
         "entries": [magic_row("谢谢参与", 500),
                     item_row("上限探针券", 500, "cap_probe")]}, tok)
    check("50% 物品档且 EV 0.5 的探针池可存", st == 200, (st, r))

    ok_draws = granted = fell = 0
    fell_reason = None
    for i in range(40):
        st, r = call("POST", "/games/jgg",
                     {"idempotency_key": "e2e-cap-%d" % i}, ptok)
        if st != 200:
            continue
        d = r.get("data") or {}
        ok_draws += 1
        if d.get("kind") == "item":
            granted += 1
        elif d.get("kind") == "fallback":
            fell += 1
            fell_reason = d.get("fell_back")
    check("成功抽数足够（限次没有把断言掏空）", ok_draws >= 30, ok_draws)
    check("上限内确实发出了物品", granted >= 1,
          {"draws": ok_draws, "granted": granted, "fell": fell})
    check("上限用尽后一律走回落并给出原因",
          fell >= 1 and bool(fell_reason),
          {"granted": granted, "fell": fell, "reason": fell_reason})

    # 探针账号自清（删除接口设计为仅封禁态可删，故先封再删）
    st, _ = call("POST", "/admin/users/status",
                 {"user_id": uid, "status": 2, "reason": "e2e 探针清理"}, tok)
    st2, _ = call("DELETE", "/admin/users/%s" % uid, None, tok)
    check("探针账号已清理（封禁→删除）", st == 200 and st2 == 200,
          {"uid": uid, "ban": st, "del": st2})

    # 复原：池子与上限都回到核验前，不给线上留残留
    restore = {
        "pool_key": "jgg_default", "game": "jgg", "label": "九宫格 · 标准池",
        "ticket": j["ticket"],
        "entries": [
            dict([("label", lb), ("weight", wt), ("kind", kd),
                  ("enabled", True)]
                 + ([("payout", mu)] if kd == "magic"
                    else [("item_key", ik), ("qty", q)]))
            for lb, wt, mu, kd, ik, q in base]}
    st, r = call("POST", "/admin/arcade/pool", restore, tok)
    check("池子复原成功", st == 200, (st, r))
    back = dict(cap1)
    back.update({"unlimited": False, "stock": 40, "per_user": 5})
    st, r = call("POST", "/admin/arcade/items", back, tok)
    check("ticket 上限复原", st == 200, (st, r))
    check("复原后池子与核验前逐项一致", pool(tok).get("prizes") == prizes,
          [x.get("label") for x in pool(tok).get("prizes", [])])

    tail = "" if not fails else " -> " + "; ".join(fails[:5])
    print("\n结果：%d 项断言，失败 %d%s" % (n[0], len(fails), tail))
    sys.exit(1 if fails else 0)


main()
