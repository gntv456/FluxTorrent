# -*- coding: utf-8 -*-
"""农场「收获彩蛋」核验（0252）：确定性收获之上的那一档来自奖池行表。

命题有五条：
  ① 彩头档读的是 arcade_pools(game='farm')，站点出厂态「什么都不加」；
  ② 定标单位是**最便宜作物的种子价**，不是站长能填的数（票价顶不掉它）；
  ③ 闸门看的是 0.90 收获 + 彩蛋 < 1，所以池子 EV 0.10 单独合法也会被拒；
     物品位按 anchor 计入同一条余量，anchor=0 的收藏件算空头承诺；
  ④ 收获真按那一档多发魔力，且与基础收获**同笔入账**（余额增量 == amount）；
  ⑤ 彩头可以是站内虚拟物品：发放账进 rand 侧，上限用尽后按回落口径报。

farm 端点受模块网关 fail-close 保护，本脚本临时开 module_farm、收尾关回 no。
夹具地块走 psql（等不起 grow_hours），slot 9 与真人田不撞，收尾删掉。

用法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/e2e_arcade_farm_egg.py
"""

import json
import sys
import time

from e2e_arcade_pool_gate import (  # 共享件不抄第二份
    BASE,
    call,
    check,
    fails,
    item,
    login,
    magic,
    n_checks,
    psql,
    to_req,
)

FARM_SNAP = {}


def main():
    tok = login()
    print("FLUX_API_BASE =", BASE)
    # ── 农场收获彩蛋读奖池行表（0252）：定标单位不是站长能填的数 ──
    def farm_body(entries, ticket=7):
        return {"pool_key": "farm_default", "game": "farm",
                "label": "农场 · 收获彩蛋", "ticket": ticket,
                "entries": entries}

    def farm_proj():
        return (call("GET", "/games", None, tok)[1].get("data")
                or {}).get("farm") or {}

    def plant_ready(slot):
        """夹具地块：直接落一行「已成熟、未枯萎」的plot（slot 9 与真人田不撞）。
        等不起 grow_hours，也不该为测试往业务里塞时间旅行端点。"""
        psql("INSERT INTO farm_plots (user_id, slot, crop_id, planted_at, "
             "ready_at, watered) VALUES (%d, %d, %s, "
             "now() - interval '2 hour', now() - interval '5 minute', FALSE) "
             "ON CONFLICT (user_id, slot) DO UPDATE SET "
             "crop_id = EXCLUDED.crop_id, planted_at = EXCLUDED.planted_at, "
             "ready_at = EXCLUDED.ready_at, watered = FALSE"
             % (rid, slot, crop_id))

    fp = farm_proj()
    seed_unit = int(psql("SELECT min(seed_price)::bigint FROM farm_crops"))
    FARM_SNAP.update({"ticket": fp.get("unit"), "prizes": fp.get("prizes")
                      or []})
    check("农场投影带出彩蛋池（档位 / 定标单位 / 总回收）",
          bool(fp.get("prizes")) and "unit" in fp
          and "expected_value" in fp, sorted(fp)[:8])
    check("定标单位 = 作物表现值最便宜种子价", fp.get("unit") == seed_unit,
          (fp.get("unit"), seed_unit))
    check("出厂表只有一档「什么都不加」，总回收就是作物表的 0.90",
          len(fp.get("prizes") or []) == 1
          and abs((fp.get("expected_value") or 0) - 0.90) < 1e-6,
          (len(fp.get("prizes") or []), fp.get("expected_value")))
    st, r = call("POST", "/admin/arcade/pool",
                 farm_body([magic("彩头", 1000, 0.1)]), tok)
    check("池子 EV 0.10 单独没超，加上收获 0.90 就穿线 -> 农场闸自己拒",
          st == 400, (st, str(r)[:140]))
    st, r = call("POST", "/admin/arcade/pool",
                 farm_body([magic("空档", 990, 0),
                            item("免考核卡", 10, "pass3")]), tok)
    check("农场池放一件贵物品（0.01×12000/100 = 1.2）被拒", st == 400,
          (st, str(r)[:140]))
    st, r = call("POST", "/admin/arcade/pool",
                 farm_body([magic("空档", 900, 0),
                            item("收藏件", 100, "board")]), tok)
    check("anchor=0 的收藏件在农场池同样是空头承诺 -> 拒", st == 400,
          (st, str(r)[:140]))
    check("三次被拒之后表里一字未改", farm_proj()["prizes"]
          == FARM_SNAP["prizes"], farm_proj()["prizes"])
    st, r = call("POST", "/admin/arcade/pool",
                 farm_body([magic("空档", 1000, 0)], ticket=999999), tok)
    check("合法保存农场池 -> 200", st == 200, (st, str(r)[:140]))
    check("表单里的票价顶不掉定标单位（作物表说了算）",
          farm_proj().get("unit") == seed_unit, farm_proj().get("unit"))

    # 收获路径要真收一株：farm 模块网关是 fail-close，先临时开、收尾关回 no
    rid = int(psql("SELECT id FROM users WHERE username = 'root'"))
    crop_row = psql("SELECT id || '/' || seed_price FROM farm_crops "
                    "ORDER BY seed_price LIMIT 1").split("/")
    crop_id, seed = int(crop_row[0]), int(crop_row[1])
    egg = seed * 50 // 1000
    psql("UPDATE site_settings SET value = 'yes' WHERE name = 'module_farm'")
    time.sleep(31)  # 模块开关有 30s TTL，等它自己过期（不猜、不重启容器）
    st, r = call("POST", "/admin/arcade/pool",
                 farm_body([magic("五厘彩头", 1000, 0.05)]), tok)
    d0 = (r.get("data") or {})
    check("5% 种子价的彩头池合法：ev 报池子（0.05），total_ev 报总回收（0.95）",
          st == 200 and abs((d0.get("ev") or 0) - 0.05) < 1e-9
          and abs((d0.get("total_ev") or 0) - 0.95) < 1e-9,
          (st, d0, str(r)[:140]))
    plant_ready(9)
    bal0 = int(psql("SELECT spark_balance FROM users WHERE id = %d" % rid))
    st, r = call("POST", "/farm/harvest", {"slot": 9}, tok)
    d = (r.get("data") or {})
    check("收获按这一株的种子价多发彩蛋",
          st == 200 and not d.get("withered") and d.get("extra") == egg,
          (st, seed, egg, json.dumps(r, ensure_ascii=False)[:180]))
    check("回报分开报 base / extra，amount 就是两者之和",
          d.get("amount") == (d.get("base") or 0) + egg, d)
    check("彩蛋与收获同笔入账（余额增量 == amount）",
          int(psql("SELECT spark_balance FROM users WHERE id = %d" % rid))
          - bal0 == d.get("amount"), (bal0, d.get("amount")))
    check("收获流水记的是含彩蛋的总额",
          int(psql("SELECT amount FROM farm_harvests WHERE user_id = %d "
                   "ORDER BY id DESC LIMIT 1" % rid)) == d.get("amount"),
          d.get("amount"))

    # 物品彩蛋：造一件便宜探针物品（anchor 5），满权重进池 -> 必中
    st, r = call("POST", "/admin/arcade/items",
                 {"key": "e2e_egg", "name": "彩蛋探针", "kind": "cosmetic",
                  "anchor": 5, "anchor_src": "declared", "unlimited": True,
                  "stock": 0, "per_user": 1, "icon": "🥚",
                  "use_kind": "collect", "enabled": True}, tok)
    check("探针物品建出来了（每人上限 1，好验回落）", st == 200,
          (st, str(r)[:140]))
    st, r = call("POST", "/admin/arcade/pool",
                 farm_body([item("一枚彩蛋", 1000, "e2e_egg")]), tok)
    check("满权重放一件 5 折算价的物品合法（EV 0.05）",
          st == 200 and abs(((r.get("data") or {}).get("total_ev") or 0)
                            - 0.95) < 1e-9,
          (st, str(r)[:140]))
    plant_ready(9)
    st, r = call("POST", "/farm/harvest", {"slot": 9}, tok)
    d = (r.get("data") or {})
    p = d.get("prize") or {}
    check("收获发的是物品而不是魔力",
          st == 200 and p.get("kind") == "item"
          and p.get("item_key") == "e2e_egg",
          (st, json.dumps(r, ensure_ascii=False)[:180]))
    check("发放账上有一条 rand 侧的农场物品发放",
          psql("SELECT count(*) FROM arcade_item_grants WHERE item_key = "
               "'e2e_egg' AND side = 'rand' AND game = 'farm_harvest'")
          == "1", psql("SELECT idem FROM arcade_item_grants WHERE item_key "
                       "= 'e2e_egg'"))
    plant_ready(9)
    st, r = call("POST", "/farm/harvest", {"slot": 9}, tok)
    p2 = ((r.get("data") or {}).get("prize") or {})
    check("每人上限用尽后按回落口径报，不谎称发了物品",
          st == 200 and p2.get("kind") == "fallback"
          and p2.get("fell_back"), (st, p2))
    # 先复原池子，探针物品才解除了引用；再删账、删目录项（不留残渣）
    st, r = call("POST", "/admin/arcade/pool",
                 farm_body([to_req(x) for x in FARM_SNAP["prizes"]],
                           ticket=seed_unit), tok)
    check("农场池已复原为出厂那一档", st == 200, (st, str(r)[:140]))
    psql("DELETE FROM arcade_item_grants WHERE item_key = 'e2e_egg'")
    st, r = call("DELETE", "/admin/arcade/items/e2e_egg", None, tok)
    check("彩蛋探针物品已清理", st == 200, (st, str(r)[:140]))
    psql("DELETE FROM farm_plots WHERE user_id = %d AND slot = 9" % rid)

    # ── 作物表本身也是配置面（0253）：一行改动同时决定回收率与池子定标 ──
    def crop_body(**kw):
        base = {"name": "e2e 探针草", "seed_price": 20, "base_yield": 15,
                "grow_hours": 4, "active": True}
        base.update(kw)
        return base

    st, r = call("POST", "/admin/arcade/crops", None, tok)
    check("作物表端点只有 GET/POST 单行，列表不会被误写成 POST",
          st in (404, 405), st)
    st, r = call("GET", "/admin/arcade/crops", None, tok)
    ct = r.get("data") or {}
    rows0 = ct.get("crops") or []
    cap, unit0 = ct.get("cap"), ct.get("unit")
    check("作物表读得到现值（每行带收获期望、表带定标与上限）",
          st == 200 and len(rows0) >= 5 and unit0 and cap,
          (st, len(rows0), unit0, cap))
    check("播种的每一株都压在校准上（0.90 是天花板，不是假设）",
          all(x["expected_value"] <= cap + 1e-9 for x in rows0),
          [(x["name"], x["expected_value"]) for x in rows0])
    st, r = call("POST", "/admin/arcade/crop",
                 crop_body(name="e2e 越校准草", base_yield=16), tok)
    txt = json.dumps(r, ensure_ascii=False)
    check("产量越过「种子价 × 0.75」的作物存不进去", st == 400,
          (st, txt[:140]))
    check("拒绝原因点名「增发」这条根因",
          "增发" in txt or "校准" in txt, txt[:140])

    # 跨表回查：把便宜一株塞进作物表 = 把定标单位拉下来 = 顶穿整池余量
    st, r = call("POST", "/admin/arcade/pool",
                 farm_body([magic("空档", 981, 0),
                            item("银行券", 19, "bank500")]), tok)
    check("池子放 1.9% 概率的 500 折算价物品，当前定标下合法", st == 200,
          (st, str(r)[:140]))
    st, r = call("POST", "/admin/arcade/crop", crop_body(), tok)
    txt = json.dumps(r, ensure_ascii=False)
    check("加一株更便宜的作物会顶穿彩蛋池 -> 跨表回查拒", st == 400,
          (st, txt[:160]))
    check("拒绝原因点名定标单位", "定标" in txt, txt[:160])
    st, back = call("GET", "/admin/arcade/crops", None, tok)
    bt = back.get("data") or {}
    check("被拒之后作物表没多出一行、定标未变",
          len(bt.get("crops") or []) == len(rows0) and bt.get("unit") == unit0,
          (len(bt.get("crops") or []), bt.get("unit")))

    # 池子先收回「什么都不加」，同一株便宜作物就装得下了
    st, r = call("POST", "/admin/arcade/pool",
                 farm_body([to_req(x) for x in FARM_SNAP["prizes"]],
                           ticket=unit0), tok)
    check("彩蛋池收回出厂那一档", st == 200, (st, str(r)[:120]))
    st, r = call("POST", "/admin/arcade/crop", crop_body(), tok)
    new_id = (r.get("data") or {}).get("id")
    check("池子留出余量后，同一株便宜作物存得进去", st == 200 and new_id,
          (st, str(r)[:140]))
    st, back = call("GET", "/admin/arcade/crops", None, tok)
    bt = back.get("data") or {}
    check("定标单位跟着变成新最便宜种子价", bt.get("unit") == 20,
          bt.get("unit"))
    check("农场投影读到同一个新单位",
          farm_proj().get("unit") == 20, farm_proj().get("unit"))
    st, mk = call("GET", "/farm", None, tok)
    names = [c.get("name") for c in ((mk.get("data") or {}).get("crops") or [])]
    check("新作物进了玩家行情", "e2e 探针草" in names, names)
    st, r = call("POST", "/admin/arcade/crop",
                 crop_body(id=new_id, active=False), tok)
    check("下架一行作物合法", st == 200, (st, str(r)[:120]))
    st, mk = call("GET", "/farm", None, tok)
    names = [c.get("name") for c in ((mk.get("data") or {}).get("crops") or [])]
    check("下架后不再出现在行情里", "e2e 探针草" not in names, names)
    check("下架的作物不参与定标（单位回到 100）",
          farm_proj().get("unit") == unit0, farm_proj().get("unit"))
    # 播种要过 slot 1-6 的参数校验，夹具用的 9 号位是给收获用的
    st, r = call("POST", "/farm/plant", {"slot": 1, "crop_id": new_id}, tok)
    check("拿下架作物播种被点名拒绝（不是「作物不存在」）",
          st == 400 and "下架" in json.dumps(r, ensure_ascii=False),
          (st, str(r)[:140]))
    st, r = call("DELETE", "/admin/arcade/crop/%d" % new_id, None, tok)
    check("没收过种的作物可以直接删掉", st == 200, (st, str(r)[:140]))
    st, r = call("DELETE", "/admin/arcade/crop/%d" % crop_id, None, tok)
    txt = json.dumps(r, ensure_ascii=False)
    check("收过的作物删不掉，并指路「下架」",
          st == 400 and ("收获史" in txt or "下架" in txt), (st, txt[:160]))
    check("收尾：作物表回到播种的那几株",
          len((call("GET", "/admin/arcade/crops", None, tok)[1]
               .get("data") or {}).get("crops") or []) == len(rows0))

    print("\n结果：农场彩蛋 %d 项断言，失败 %d"
          % (n_checks[0], len(fails)))
    sys.exit(1 if fails else 0)


def _restore():
    """崩在半路也要把彩头池与模块开关写回进入时的样子。"""
    tok = login()
    if not FARM_SNAP.get("prizes"):
        print("兜底复原：读不到农场池，跳过")
        return
    body = {
        "pool_key": "farm_default", "game": "farm",
        "label": "农场 · 收获彩蛋", "ticket": FARM_SNAP.get("ticket"),
        "entries": [dict(to_req(x), side="any") for x in FARM_SNAP["prizes"]],
    }
    st, _ = call("POST", "/admin/arcade/pool", body, tok)
    psql("UPDATE site_settings SET value = 'no' WHERE name = 'module_farm'")
    # 崩在半路时探针作物也会留在表里（它没被种过，直接删安全）
    psql("DELETE FROM farm_crops WHERE name LIKE 'e2e %'")
    # 探针物品：确认没有池子还引用它才删（item_key 被 SET NULL 会撞 CHECK）
    refs = psql("SELECT count(*) FROM arcade_pool_entries"
                " WHERE item_key = 'e2e_egg'")
    if refs == "0":
        psql("DELETE FROM arcade_item_grants WHERE item_key = 'e2e_egg'")
        psql("DELETE FROM arcade_items WHERE key = 'e2e_egg'")
        psql("DELETE FROM farm_plots WHERE slot = 9")
    else:
        print("彩蛋探针仍被 %s 个奖池位引用，跳过清理（人工核对）" % refs)
    print("兜底复原农场彩头池 -> %s，module_farm 已关回 no" % st)


if __name__ == "__main__":
    try:
        main()
    finally:
        try:
            _restore()
        except Exception as e:  # 兜底本身失败要喊出来，不能静默
            print("兜底复原失败，请手工核对 arcade_pools(farm)：%r" % e)
