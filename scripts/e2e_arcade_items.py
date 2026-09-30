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
import subprocess
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
    for i in range(10):
        st, r = call("POST", "/auth/login",
                     {"username": user, "password": pw})
        tok = (r.get("data") or {}).get("token")
        if tok:
            return tok
        time.sleep(min(2 + i * 3, 20))  # 限流窗口比 1.5s 长得多
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


BASE_POOL = None


def pool_body(prizes, ticket):
    """读侧投影 -> 写侧请求体：收尾复原与中途兜底共用这一份。"""
    return {"pool_key": "jgg_default", "game": "jgg",
            "label": "九宫格 · 标准池", "ticket": ticket,
            "entries": [
                dict(
                    [("label", p["label"]),
                     ("weight", p["weight_permille"]),
                     ("kind", p.get("kind", "magic")),
                     ("enabled", True)]
                    + ([("payout", p["payout"])]
                       if p.get("kind") != "item"
                       else [("item_key", p["item_key"]),
                             ("qty", p["qty"])]),
                )
                for p in prizes]}


def _restore_pool():
    """崩在半路也要把奖池写回进入时的样子（与 pool_gate 同一纪律）。"""
    if not BASE_POOL:
        print("兜底复原：没抓到基线，跳过")
        return
    tok = login()
    st, _ = call("POST", "/admin/arcade/pool", BASE_POOL, tok)
    left = ((call("GET", "/games", None, tok)[1].get("data") or {})
            .get("jgg") or {}).get("prizes") or []
    print("兜底复原奖池 -> HTTP %s，表里 %s 档" % (st, len(left)))


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

    # 基线体检：上次崩在半路可能把探针池留在表里，那样「复原」会把脏态
    # 当基线并断言成功 —— 本轮真发生过（跑完 33/33，表里其实只剩 2 档）。
    check("基线奖池是正常表（不是上次留下的探针态）",
          len(prizes) >= 3 and not any(
              (x.get("item_key") or "").endswith("_probe")
              for x in prizes),
          [x.get('label') for x in prizes])
    global BASE_POOL
    BASE_POOL = pool_body(prizes, j.get("ticket"))
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
    # 删除端点的三道闸：① 还被奖池引用 → 拒（删了会让 load_pool 当场炸，
    # 因为 item_key 是 SET NULL 而 CHECK 要求 item 位非空）
    st, r = call("DELETE", "/admin/arcade/items/ticket", None, tok)
    txt = json.dumps(r, ensure_ascii=False)
    check("被奖池引用的物品删不掉且点名奖池",
          st == 400 and ("奖池" in txt or "引用" in txt), (st, txt[:120]))
    # ② 没被引用、也没发过账 → 真删掉（顺带自清 gate_probe，不再留残渣）
    st, r = call("DELETE", "/admin/arcade/items/gate_probe", None, tok)
    check("无引用无发放的物品可删", st == 200, (st, r))
    keys = [x["key"] for x in ((call("GET", "/admin/arcade/overview",
                                None, tok)[1].get("data") or {})
                                .get("items") or [])]
    check("删完目录里确实没有 gate_probe",
          "gate_probe" not in keys, keys)

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
    # 此刻 cap_probe 还在探针池里，先测到的其实是「引用闸」：
    # 服务端点名是哪个池的哪一档挡着，不让人盲删。
    st, r = call("DELETE", "/admin/arcade/items/cap_probe", None, tok)
    txt2 = json.dumps(r, ensure_ascii=False)
    check("在池里的物品删不掉（引用闸先响）",
          st == 400 and "引用" in txt2, (st, txt2[:120]))

    # 闭环：发出去的东西必须回到玩家眼前。背包读的就是发放账本身，
    # 所以「中奖」与「看得到」之间不该再有第二份清单。
    st, bk = call("GET", "/games/arcade-meta", None, ptok)
    data = bk.get("data") or {}
    pack = data.get("backpack") or {}
    owned = pack.get("items") or []
    check("背包端点随大厅返回物品列表",
          st == 200 and isinstance(owned, list), st)
    check("抽中的物品出现在中奖者背包里",
          any(x.get("key") in ("cap_probe", "ticket") and x.get("qty", 0) >= 1
              for x in owned), owned[:3])
    check("背包件数由发放账反推（合计=total）",
          pack.get("total") == sum(x.get("qty", 0) for x in owned), pack)
    gate = [c for c in (data.get("checks") or [])
            if "经济类物品" in str(c.get("name", ""))]
    check("确定侧经济物品门禁出现在自检里且为绿",
          len(gate) == 1 and gate[0].get("pass") is True, gate)

    # 用途侧闭环：奖品是站内虚拟物品，「抽到」之后还必须「用得上」。
    # 目录里一件可用途的都没有 = 这半个功能是零，不能靠「端点存在」过关。
    usable = psql("SELECT count(*)::text FROM arcade_items"
                  " WHERE use_kind <> 'collect' AND enabled")
    check("目录里确实有带用途的奖品（不是只有收藏件）",
          int(usable) >= 1, usable)
    bad_bind = psql("SELECT count(*)::text FROM arcade_items i"
                    " WHERE i.use_kind = 'sku' AND NOT EXISTS ("
                    "  SELECT 1 FROM shop_items s WHERE s.id::text = i.use_ref"
                    "    AND s.active AND i.anchor >= s.price)")
    check("已绑 SKU 的奖品都能真兑出等价东西", int(bad_bind) == 0, bad_bind)
    # 三条都要钉住：① 没指定用途的物品点使用必须当场说清（静默等于骗人）；
    # ② 兑现按 anchor 到帐，且持有数由「发放 − 消耗」两张账反推；
    # ③ 绑定关系写坏（SKU 不存在 / anchor 低于售价）在保存时就拒，
    #    不能等玩家扣了物品才发现无处生效——那是审计里「花钱买空气」的老坑。
    st, u0 = call("POST", "/games/arcade/backpack/use",
                  {"item_key": "cap_probe", "qty": 1,
                   "idempotency_key": "e2e-use-0"}, ptok)
    txtu = json.dumps(u0, ensure_ascii=False)
    check("未指定用途的物品用不了且说明是收藏件",
          st == 400 and "收藏" in txtu, (st, txtu[:120]))

    st, _ = call("POST", "/admin/arcade/items",
                 dict(probe_item, use_kind="spark"), tok)
    check("探针物品指定「兑现魔力」用途可存", st == 200, (st, _))
    bal0 = int(psql("SELECT spark_balance FROM users WHERE id=%d" % uid))
    st, u1 = call("POST", "/games/arcade/backpack/use",
                  {"item_key": "cap_probe", "qty": 1,
                   "idempotency_key": "e2e-use-1"}, ptok)
    d1 = u1.get("data") or {}
    bal1 = int(psql("SELECT spark_balance FROM users WHERE id=%d" % uid))
    anchor_cap = int(probe_item["anchor"])
    check("兑现入账等于 anchor 且真进了余额",
          st == 200 and d1.get("spark") == anchor_cap
          and bal1 == bal0 + anchor_cap,
          {"st": st, "spark": d1.get("spark"), "delta": bal1 - bal0})
    st, u2 = call("POST", "/games/arcade/backpack/use",
                  {"item_key": "cap_probe", "qty": 1,
                   "idempotency_key": "e2e-use-2"}, ptok)
    bal2 = int(psql("SELECT spark_balance FROM users WHERE id=%d" % uid))
    check("用过的那件不能再凭空用第二次", st == 400, (st, u2))
    check("重试没有二次入账", bal2 == bal1, {"bal1": bal1, "bal2": bal2})
    st, bk3 = call("GET", "/games/arcade-meta", None, ptok)
    p3 = [x for x in (((bk3.get("data") or {}).get("backpack") or {})
                      .get("items") or []) if x.get("key") == "cap_probe"]
    check("背包按「发放-消耗」显示：qty 归零、used 记 1",
          bool(p3) and p3[0].get("qty") == 0 and p3[0].get("used") == 1, p3)

    st, _ = call("POST", "/admin/arcade/items",
                 dict(probe_item, use_kind="sku", use_ref="999999"), tok)
    check("绑定不存在的商店 SKU 时保存被拒", st == 400, (st, _))
    row = psql("SELECT id::text || ':' || price::text FROM shop_items"
               " WHERE kind='avatar_frame' AND active"
               " ORDER BY price DESC, id LIMIT 1")
    sku_id, sku_price = row.split(":")
    st, r4 = call("POST", "/admin/arcade/items",
                  dict(probe_item, use_kind="sku", use_ref=sku_id,
                       anchor=1), tok)
    check("anchor 低于所绑 SKU 售价时保存被拒（少计负债）",
          st == 400, (st, json.dumps(r4, ensure_ascii=False)[:120]))
    check("两次被拒之后目录里用途未改写",
          psql("SELECT use_kind FROM arcade_items WHERE key='cap_probe'")
          == "spark",
          psql("SELECT use_kind || '/' || use_ref FROM arcade_items"
               " WHERE key='cap_probe'"))
    # 只改一个字段的一把保存，不能把站长没打算改的东西一起改掉。
    # 这里踩过：use_kind / unlimited / per_user 走 serde 缺省时，
    # 一次「只改名」的保存会把限量打成不限量、把奖品用途打成收藏件，
    # 而界面回读到的正是被改后的值——没人会去追是哪次保存干的。
    st, _ = call("POST", "/admin/arcade/items",
                 {"key": "cap_probe", "name": "上限探针券（改名）",
                  "kind": "voucher", "anchor": j["ticket"],
                  "anchor_src": "derived"}, tok)
    keep = psql("SELECT use_kind || '/' || unlimited::text || '/'"
                " || per_user::text FROM arcade_items"
                " WHERE key='cap_probe'")
    check("只改名字的保存不把用途与限购打回缺省",
          st == 200 and keep == "spark/true/1", keep)
    st, r5 = call("POST", "/admin/arcade/items",
                  dict(probe_item, use_kind="sparkk"), tok)
    check("use_kind 写错拼法直接拒，不静默当成收藏件",
          st == 400 and "use_kind" in json.dumps(r5, ensure_ascii=False),
          (st, json.dumps(r5, ensure_ascii=False)[:110]))

    # 探针账号自清（删除接口设计为仅封禁态可删，故先封再删）
    st, _ = call("POST", "/admin/users/status",
                 {"user_id": uid, "status": 2, "reason": "e2e 探针清理"}, tok)
    st2, _ = call("DELETE", "/admin/users/%s" % uid, None, tok)
    check("探针账号已清理（封禁→删除）", st == 200 and st2 == 200,
          {"uid": uid, "ban": st, "del": st2})

    # 墓碑回归闸：删除是「用户名改写成 deleted-<id>-<hash> + status=3、账本留证」，
    # 所以探针号的 40 局流水还在 spark_ledger 里 —— 榜与公示一旦漏掉
    # u.status < 2，这里立刻会看到一个 deleted- 名字挂在榜首（真发生过）。
    st, bk2 = call("GET", "/games/arcade-meta", None, tok)
    d2 = bk2.get("data") or {}
    names = [str(x.get("who")) for x in
             ((d2.get("board") or {}).get("stubs") or [])
             + ((d2.get("board") or {}).get("plays") or [])
             + (d2.get("feed") or [])]
    check("周榜与公示里没有墓碑账号",
          st == 200 and not any(n.startswith("deleted-") for n in names), names)
    check("周榜两列都真的返回了",
          isinstance((d2.get("board") or {}).get("stubs"), list)
          and isinstance((d2.get("board") or {}).get("plays"), list),
          sorted((d2.get("board") or {}).keys()))

    # 公示口径：/games 的 expected_value 必须与闸门同源且 < 1；
    # 物品位要带目录图标（灯阵与公示都读它，不再一律 🎁）。
    st, gv = call("GET", "/games", None, tok)
    j2 = ((gv.get("data") or {}).get("jgg") or {})
    ev = j2.get("expected_value")
    check("公示 EV 随 /games 下发且 < 1",
          st == 200 and isinstance(ev, float) and 0 < ev < 1, ev)
    its = [p for p in (j2.get("prizes") or []) if p.get("kind") == "item"]
    check("物品位带目录图标", bool(its) and all(p.get("icon") for p in its),
          [(p.get("label"), p.get("icon")) for p in its])
    # 公示要能说出「抽到之后拿到什么」：只有概率和价值、没有用途，
    # 玩家读到的是半句话（这件东西究竟能不能用、用成什么，全凭猜）。
    check("公示把物品位的用途一并下发",
          bool(its) and all(p.get("use_kind") for p in its),
          [(p.get("label"), p.get("use_kind")) for p in its])

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
    # ③ 奖池已不引用它，但它发过 7 件 ——
    # 发放账是余量与每人上限的唯一真相，
    # 删物品会 CASCADE 清账，所以只许停用。
    # 真相，删物品会 CASCADE 清账，所以只许停用。
    st, r = call("DELETE", "/admin/arcade/items/cap_probe", None, tok)
    txt3 = json.dumps(r, ensure_ascii=False)
    check("发过账的物品只能停用不能删",
          st == 400 and ("停用" in txt3 or "发放" in txt3),
          (st, txt3[:140]))
    back = dict(cap1)
    back.update({"unlimited": False, "stock": 40, "per_user": 5})
    st, r = call("POST", "/admin/arcade/items", back, tok)
    check("ticket 上限复原", st == 200, (st, r))
    check("复原后池子与核验前逐项一致", pool(tok).get("prizes") == prizes,
          [x.get("label") for x in pool(tok).get("prizes", [])])

    ov2 = (call("GET", "/admin/arcade/overview", None, tok)[1]
             .get("data") or {})
    probe_left = [x["key"] for x in (ov2.get("items") or [])
                    if x["key"].endswith("_probe")]
    check("探针物品确实建出来了（待收尾清理）",
          bool(probe_left), probe_left)

    tail = "" if not fails else " -> " + "; ".join(fails[:5])
    print("\n结果：%d 项断言，失败 %d%s" % (n[0], len(fails), tail))
    sys.exit(1 if fails else 0)



PROBES = ("gate_probe", "cap_probe")


def psql(sql):
    """物品目录没有 DELETE 端点，脚本自己造的测试行只能自己删。
    与 install_e2e.py 同一手法（docker exec psql）。"""
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True,
    )
    return r.stdout.strip()


def _drop_probe_items():
    """删之前先确认没有池还引用：arcade_pool_entries.item_key 是
    ON DELETE SET NULL，而 kind='item' 且 item_key 为 NULL 会撞 CHECK，
    直接删会让玩法读表当场炸。"""
    keys = ",".join("'%s'" % k for k in PROBES)
    refs = psql("SELECT count(*) FROM arcade_pool_entries"
                " WHERE item_key IN (%s)" % keys)
    if refs != "0":
        print("探针物品仍被 %s 个奖池位引用，跳过清理（人工核对）" % refs)
        return
    psql("DELETE FROM arcade_items WHERE key IN (%s)" % keys)
    left = psql("SELECT count(*) FROM arcade_items"
            " WHERE key LIKE '%_probe'")
    print("收尾：目录里 *_probe 残留 %s 件" % left)


if __name__ == "__main__":
    try:
        main()
    finally:
        try:
            _restore_pool()
        except BaseException as e:  # 兜底失败要喊出来，不能静默
            print("奖池兜底复原失败，请手工核对 arcade_pool_entries：%r" % e)
        try:
            _drop_probe_items()
        except BaseException as e:
            print("探针物品清理失败，请手工核对 arcade_items：%r" % e)
