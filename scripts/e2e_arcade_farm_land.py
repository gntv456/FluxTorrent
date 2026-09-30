# -*- coding: utf-8 -*-
"""农场土地阶梯核验（0260）：买地/升级只沉没魔力，等级只买周转。

命题有六条：
  ① 报价、档数、上限、满级判定全部由服务端给（前端与脚本都不抄公式）；
  ② 地块只能按阶梯连续往外买：跳号、买免费地、买满都点名拒绝，
     同一档的重放不双扣（幂等键绑槽位）；
  ③ 站长参数配坏时**读路径就报**（比率 ≤1000‰、上限低于免费地块数），
     不带着坏配置继续卖地；
  ④ 升级买到的是**时长**：同一株作物在 Lv.2 与 Lv.1 上的成熟分钟数不同，
     而同一窗口内的市场价（价值侧）一字不变 —— 这条是整刀的护栏；
  ⑤ 等级上限由周转地板推出：Lv.12 之后拒收钱，且 4 小时的作物最低也要
     120 分钟才熟（不会被压成「种下即熟」）；
  ⑥ 买来的地才落持有行：免费地不升级就没有行（阶梯价因此不会被打折）。

farm 端点受模块网关 fail-close 保护，脚本临时开 module_farm、收尾关回 no。
探针号花钱买地（dev 库 root 余额是负的），用完删除 —— farm_land 有
ON DELETE CASCADE，行随账号一起走。

用法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/e2e_arcade_farm_land.py
"""

import json
import sys
import time

from e2e_arcade_pool_gate import (  # 共享件不抄第二份
    BASE,
    RUN,
    call,
    check,
    fails,
    login,
    n_checks,
    psql,
)

KEYS = [
    "farm_max_plots",
    "farm_land_base",
    "farm_land_ratio_permille",
    "farm_up_base",
    "farm_up_ratio_permille",
]
SNAP = {}


def land_of(tok):
    """GET /farm 的 land 块；读路径自己被参数闸拦下时返回错误文本"""
    st, r = call("GET", "/farm", None, tok)
    if st != 200:
        return None, r
    return (r.get("data") or {}).get("land"), r


def set_key(name, value):
    psql("UPDATE site_settings SET value = %s WHERE name = '%s'"
         % ("'" + str(value) + "'", name))


def main():
    tok = login()
    print("FLUX_API_BASE =", BASE)

    for k in KEYS:
        SNAP[k] = psql("SELECT value FROM site_settings WHERE name = '%s'" % k)
    check("五个阶梯参数都在 site_settings 里（配置面的缺省不是代码私有）",
          all(SNAP[k] not in ("", "NULL") for k in KEYS), SNAP)

    psql("UPDATE site_settings SET value = 'yes' WHERE name = 'module_farm'")
    time.sleep(31)  # 模块开关 30s TTL，等它自己过期（不猜、不重启容器）

    uname = "e2eld" + RUN[-6:]
    st, r = call("POST", "/admin/adduser",
                 {"username": uname, "email": uname + "@e2e-probe.invalid",
                  "password": "E2eProbe!123"}, tok)
    uid = (r.get("data") or {}).get("user_id")
    call("POST", "/admin/users/adjust",
         {"user_id": uid, "spark_delta": 300000}, tok)
    call("POST", "/me/password/change",
         {"old_password": "E2eProbe!123", "new_password": "E2eProbe!456"},
         login(uname, "E2eProbe!123"))
    pt = login(uname, "E2eProbe!456")

    # ── ① 出厂态：免费 6 块，下一块按底数报价 ──
    land, _ = land_of(pt)
    check("总览带出土地阶梯（free/cap/owned/next_slot/报价）",
          land and all(k in land for k in ("free", "cap", "owned",
                                           "next_slot", "next_land_price")),
          land and sorted(land)[:9])
    check("免费地块就是代码里的 FARM_PLOTS（不是第二个数）",
          land and land["free"] == 6 and land["owned"] == 6, land)
    check("第一块买地价 = farm_land_base 底数",
          land and land["next_slot"] == 7
          and land["next_land_price"] == int(SNAP["farm_land_base"]), land)
    check("满级 12 由周转地板推出，投影里带着它", land and land["max_level"]
          == 12, land and land.get("max_level"))

    # ── ② 只能按阶梯连续买 ──
    st, r = call("POST", "/farm/land/buy", {"slot": 9}, pt)
    check("跳号买地（下一块是 7 却买 9）被拒并点名",
          st == 400 and "第 7 号" in json.dumps(r, ensure_ascii=False),
          (st, str(r)[:150]))
    st, r = call("POST", "/farm/land/buy", {"slot": 3}, pt)
    check("买免费送的地块被拒（不用重复买）",
          st == 400 and "已经有了" in json.dumps(r, ensure_ascii=False),
          (st, str(r)[:150]))
    land, _ = land_of(pt)
    check("两次被拒之后一档没多买", land and land["purchased"] == 0, land)

    bal = int(psql("SELECT spark_balance FROM users WHERE id = %d" % uid))
    st, r = call("POST", "/farm/land/buy", {"slot": 7}, pt)
    d = (r.get("data") or {})
    spent = bal - int(psql("SELECT spark_balance FROM users WHERE id = %d"
                           % uid))
    check("买第 7 号地：200 且真扣到底数那么多",
          st == 200 and d.get("cost") == int(SNAP["farm_land_base"])
          and spent == d.get("cost"), (st, d, spent))
    check("动账走统一管线（spark_ledger 里有 farm_land 这一笔）",
          psql("SELECT count(*) FROM spark_ledger WHERE user_id = %d "
               "AND ref_type = 'farm_land'" % uid) == "1", uid)
    land, _ = land_of(pt)
    step = int(SNAP["farm_land_ratio_permille"])
    want = int(SNAP["farm_land_base"]) * step // 1000
    check("下一块报价按阶梯 ×比率（2000 -> %d）" % want,
          land and land["next_slot"] == 8
          and land["next_land_price"] == want, land)
    check("买来的地在地块投影里标 bought",
          land and land["plots"][6]["bought"] is True
          and land["plots"][5]["bought"] is False,
          land and land["plots"][5:8])

    st, r = call("POST", "/farm/land/buy", {"slot": 8}, pt)
    d2 = (r.get("data") or {})
    bal2 = int(psql("SELECT spark_balance FROM users WHERE id = %d" % uid))
    st2, r2 = call("POST", "/farm/land/buy", {"slot": 8}, pt)
    check("同一档重放被拒且不双扣（幂等键绑槽位）",
          st == 200 and st2 == 400
          and int(psql("SELECT spark_balance FROM users WHERE id = %d"
                       % uid)) == bal2, (st, st2, str(r2)[:120]))
    check("档数只对成功那一次加一", land_of(pt)[0]["purchased"] == 2,
          land_of(pt)[0])

    # ── ③ 参数配坏时读路径就报 ──
    set_key("farm_land_ratio_permille", 1000)
    land, r = land_of(pt)
    check("比率配成 1000‰（阶梯不涨）-> 总览直接拒",
          land is None and "1000" in json.dumps(r, ensure_ascii=False),
          (land, str(r)[:150]))
    set_key("farm_land_ratio_permille", SNAP["farm_land_ratio_permille"])
    set_key("farm_max_plots", 3)
    land, r = land_of(pt)
    check("上限低于免费地块数 -> 配置自己说不通，拒",
          land is None and "低于" in json.dumps(r, ensure_ascii=False),
          (land, str(r)[:150]))
    set_key("farm_max_plots", 8)
    land, _ = land_of(pt)
    check("站长把上限收到 8：买满就报买满，next_slot 交回 null",
          land and land["cap"] == 8 and land["owned"] == 8
          and land["next_slot"] is None, land)
    st, r = call("POST", "/farm/land/buy", {"slot": 9}, pt)
    check("买满之后再买被拒（阶梯到头，且报的是站长设的那个上限）",
          st == 400 and "8/8" in json.dumps(r, ensure_ascii=False),
          (st, str(r)[:140]))
    set_key("farm_max_plots", SNAP["farm_max_plots"])

    # ── ④⑤ 升级买到的是时长，不是价值 ──
    crop = psql("SELECT id || '/' || seed_price || '/' || grow_hours "
                "FROM farm_crops ORDER BY seed_price LIMIT 1").split("/")
    cid, grow_h = int(crop[0]), int(crop[2])
    base_min = grow_h * 60
    # 基线用 slot 4（Lv.1 免费地）：同一槽位一分钟内只许种一次（反刷保护），
    # 后面还要在 slot 3 升级后复测，这里换槽避免撞上那一分钟窗口。
    st, r = call("POST", "/farm/plant", {"slot": 4, "crop_id": cid}, pt)
    d3 = (r.get("data") or {})
    check("免费地块未升级时按作物表原速成熟",
          st == 200 and d3.get("minutes") == base_min, (st, d3))
    psql("DELETE FROM farm_plots WHERE user_id = %d AND slot = 4" % uid)

    up_k = "e2e-land-up-%d" % int(time.time())
    st, r = call("POST", "/farm/land/upgrade",
                 {"slot": 3, "idempotency_key": up_k + "-a"}, pt)
    d4 = (r.get("data") or {})
    check("升级免费地块（Lv.1->2）按底数收钱",
          st == 200 and d4.get("level") == 2
          and d4.get("cost") == int(SNAP["farm_up_base"]), (st, d4))
    check("免费地一旦升级就落持有行（不升级就没有行）",
          psql("SELECT level FROM farm_land WHERE user_id = %d AND slot = 3"
               % uid) == "2", uid)
    # 重放要**带同一个客户端幂等键**才成立：两个字节完全相同的请求
    # （都不带键）服务端无从区分「上一次的重试」和「新的一次升级」，
    # 那种断言天然不可满足 —— 是测试不自洽，不是产品坏。
    st, r = call("POST", "/farm/land/upgrade",
                 {"slot": 3, "idempotency_key": up_k + "-a"}, pt)
    check("同一级的重放不双扣（客户端幂等键去重）",
          st == 400, (st, str(r)[:120]))
    st, r = call("POST", "/farm/land/upgrade",
                 {"slot": 3, "idempotency_key": up_k + "-b"}, pt)
    d5 = (r.get("data") or {})
    check("Lv.2->3 收更贵的下一级价（×比率）",
          st == 200 and d5.get("level") == 3
          and d5.get("cost") == int(SNAP["farm_up_base"])
          * int(SNAP["farm_up_ratio_permille"]) // 1000, (st, d5))

    st, r = call("POST", "/farm/plant", {"slot": 3, "crop_id": cid}, pt)
    d6 = (r.get("data") or {})
    want_min = base_min * 883 // 1000
    check("Lv.3 的地成熟更快：分钟数按 940‰ 两步走（%d）" % want_min,
          st == 200 and d6.get("minutes") == want_min, (st, base_min, d6))

    # 价值侧复算：同一窗口内升级地与未升级地收同一株，市场价必须一致
    win0 = land_of(pt)[1].get("data", {}).get("window_start")
    psql("DELETE FROM farm_plots WHERE user_id = %d AND slot = 3" % uid)
    paid = {}
    for lvl, slot in ((3, 3), (1, 4)):
        psql("UPDATE farm_land SET level = %d WHERE user_id = %d AND slot = 3"
             % (lvl, uid))
        psql("INSERT INTO farm_plots (user_id, slot, crop_id, planted_at, "
             "ready_at, watered) VALUES (%d, %d, %d, now() - interval '2 "
             "hour', now() - interval '5 minute', FALSE) ON CONFLICT "
             "(user_id, slot) DO UPDATE SET crop_id = %d"
             % (uid, slot, cid, cid))
        st, r = call("POST", "/farm/harvest", {"slot": slot}, pt)
        paid[lvl] = (r.get("data") or {}).get("market_price")
    win1 = land_of(pt)[1].get("data", {}).get("window_start")
    check("两次收获落在同一个价格窗口（否则价值比较不成立）",
          win0 == win1, (win0, win1))
    check("等级只买周转：Lv.3 与 Lv.1 的价值侧一字不差",
          paid.get(3) is not None and paid.get(3) == paid.get(1), paid)

    psql("UPDATE farm_land SET level = 12 WHERE user_id = %d AND slot = 3"
         % uid)
    st, r = call("POST", "/farm/land/upgrade", {"slot": 3}, pt)
    check("满级之后拒收钱（那一注买不到任何东西）",
          st == 400 and "12" in json.dumps(r, ensure_ascii=False),
          (st, str(r)[:140]))
    # 满级地板用 slot 5（换槽，避开同一分钟内的种植限次）。
    # ⚠️ 免费地**没有持有行**（不升级就没有行），UPDATE 影响 0 行等于没改 ——
    # 必须真的 INSERT 一行 Lv.12（0260 的 CHECK 允许 level>1 的免费槽落行）。
    psql("INSERT INTO farm_land (user_id, slot, level) VALUES (%d, 5, 12) "
         "ON CONFLICT (user_id, slot) DO UPDATE SET level = 12" % uid)
    st, r = call("POST", "/farm/plant", {"slot": 5, "crop_id": cid}, pt)
    d7 = (r.get("data") or {})
    check("满级也只压到地板：4 小时的作物最低 120 分钟，不会种下即熟",
          st == 200 and d7.get("minutes") == base_min * 503 // 1000 == 120,
          (st, base_min, d7))
    psql("DELETE FROM farm_land WHERE user_id = %d AND slot NOT IN (7, 8)"
         % uid)
    call("POST", "/admin/users/status",
         {"user_id": uid, "status": 2, "reason": "e2e 土地探针清理"}, tok)
    st3, _ = call("DELETE", "/admin/users/%s" % uid, None, tok)
    check("探针号已清理（持有的地随账号级联删掉）", st3 == 200,
          (uid, psql("SELECT count(*) FROM farm_land WHERE user_id = %d"
                     % uid)))

    print("\n结果：%d 项断言，失败 %d" % (n_checks[0], len(fails)))
    sys.exit(1 if fails else 0)


def _restore():
    """崩在半路也要把五个参数与模块开关写回进入时的样子。"""
    for k, v in SNAP.items():
        if v:
            set_key(k, v)
    psql("UPDATE site_settings SET value = 'no' WHERE name = 'module_farm'")
    psql("DELETE FROM farm_plots WHERE slot IN (3, 4, 9) AND user_id IN "
         "(SELECT id FROM users WHERE username LIKE 'e2eld%')")
    psql("DELETE FROM farm_land WHERE user_id IN "
         "(SELECT id FROM users WHERE username LIKE 'e2eld%')")
    print("兜底复原：阶梯参数已写回 %s，module_farm 关回 no" % SNAP)


if __name__ == "__main__":
    try:
        main()
    finally:
        try:
            _restore()
        except Exception as e:  # 兜底本身失败要喊出来，不能静默
            print("兜底复原失败，请手工核对 site_settings：%r" % e)
    if fails:
        print("未通过：%s" % fails)
