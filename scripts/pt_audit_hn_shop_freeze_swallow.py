# -*- coding: utf-8 -*-
"""闸门 HN：商店「扣款成功、订单行被 ON CONFLICT 静默吞掉」在册钉桩（批一百八十八）。

洞（尚未修，本闸门负责让它别被忘掉、并把现网账面钉住）：
  `POST /shop/buy` 是**两段写**——
    ① `spend_spark()` 自带事务并已 commit（钱已扣、流水已落）；
    ② `repo::shop::freeze_order()` 另起事务插 `shop_orders`，写的是
       `INSERT ... ON CONFLICT (idempotency_key) DO NOTHING RETURNING id`，
       拿到 `oid = None` 时**照样 `Ok(())` 返回**（repo/shop.rs:49-62）。
  ⇒ 键一旦撞上已有订单行：接口回 200「ok」、钱扣了、没有订单行、没有履约、
  没有任何一条 warn。这是「失败演成成功」，比回 500 更坏（用户以为买到了）。
  可达路径（不是假想）：`spark_ledger` 按月分区、`maintain.rs` 每月把旧分区
  冻结进 `balance_baseline` 后**归档**，而 `shop_orders` 不分区、永久留行。
  于是「流水侧看不见这把键 ⇒ spend 判 Spent 真扣钱」与「订单侧仍占着这把键
  ⇒ 插不进去」可以在同一次请求里同时成立 ⇒ 月界之后重放同一把键就是二次扣款
  且零发货。现网已能观测到该形状的后果：2 笔 `kind='shop'` 且键形如
  `shop:<uid>:` 的流水**没有任何订单行**（uid 69 的 item 12、root 的 item 10）。

修法（待站方拍板，二选一，都要配套第 3 条）：
  A. 收钱与落单同事务：`spend_spark_tx` + `freeze_order_tx` 放进同一个 tx，
     插不到订单行就整体回滚（本仓 P-411 一族的既有先例）；
  B. 保持两段但在 `oid.is_none()` 时**出声并退钱**（复用 clawback/退款口），
     绝不 `Ok(())` 蒙过去。
  3. 键的存活期必须与流水对齐：`shop_orders` 要么随分区归档、要么改用
     「永不再用」的键（服务端生成 uuid 存进 ledger ref_id），别让月界成为
     二次扣款的窗口。

判据：
  HN1 在册钉桩：`freeze_order` 里 `ON CONFLICT (idempotency_key) DO NOTHING`
      与「oid 为空仍返回 Ok」两处必须同时存在——只修其一（例如只加 warn）
      判据变红，逼来者读上面这段并把另一条也做完。
  HN2 同事务检查：`shop_buy` 内 `spend_spark(` 与 `freeze_order(` 之间若出现
      `begin()`，说明已改成方案 A ⇒ 本条转绿并把 HN1 一起撤（撤桩见 HN4）。
  HN3 活体台账：现网「shop 流水无订单行」的违例集合必须等于基线台账
      （只许减不许增，多一个少一个都红）。
  HN4 承重自证：把 `DO NOTHING` 摘掉的合成码必须被 HN1 抓住（旧形状/新形状
      各测一次，防判据恒真）。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hn_shop_freeze_swallow.py`
      HN_LIVE=0 跳过 HN3 的只读查询。
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import call, login, ok, summary  # noqa: E402

LIVE = os.environ.get("HN_LIVE", "1") != "0"
REPO_SHOP = "apps/api/src/repo/shop.rs"
BUY = "apps/api/src/economy_http/shop.rs"
# 违例台账（2026-10-19 实测，集合级钉：多一个少一个都红）。逐笔来源已核：
#   shop:69:e2e-buy-mk-1 / shop:1:probe-1791332041488 —— 本闸门要抓的正主：
#     钱扣了（kind='shop' 流水在）、订单行一条没有，账号至今在册；
#   shop:100:srcbuy1790757465 —— uid 100 已被墓碑化，`user_del` 级联
#     `DELETE FROM shop_orders WHERE user_id`，而账本按口径不删 ⇒ 属注销
#     设计的必然结果，不是本洞的证据（改法 3 落地后这笔会自己消失）。
BASELINE_KEYS = ["shop:100:srcbuy1790757465", "shop:1:probe-1791332041488",
                 "shop:69:e2e-buy-mk-1"]


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def freeze_body(src):
    i = src.find("pub async fn freeze_order")
    j = src.find("pub async fn settle_freeze", i)
    return src[i:j if j > i else len(src)]


def swallow_shapes(block):
    """静默吞插的两处形状是否**同时**在场。

    洞 = 「插单 DO NOTHING（键撞上就静默跳过）」+「提交后无条件 Ok(())」
    同时成立。**两处必须同时在场才算洞**：
      * 只留 DO NOTHING ＝ 正常幂等语义（重放不重复下单），不是洞；
      * 只留无条件 Ok(()) ＝ 函数根本不返回订单号，调用方无从判落空。
    2026-10-20 实测：产品已按「裁 B」修好（freeze_order 返回 Option<i64>，
    落空由调用方退款+回滚库存+出声），所以旧洞的两处形状不再同时在场；
    本闸门从「钉桩在册」改成「洞不许回来」的回归护栏。
    """
    has_do_nothing = "ON CONFLICT (idempotency_key) DO NOTHING" in block
    # 「无条件 Ok(())」＝ 函数返回的不是订单号（洞的另一半：
    # 调用方根本无从判「落空」）。修好的形状是 Ok(oid)（Option<i64>）。
    has_silent_ok = re.search(r"Ok\(\(\)\)", block) is not None
    return has_do_nothing, has_silent_ok


def hole_present(block):
    """洞是否成立（两处形状同时在场）。"""
    do_nothing, silent_ok = swallow_shapes(block)
    return do_nothing and silent_ok


def buy_span(src):
    i = src.find("async fn shop_buy")
    j = src.find("spend_spark(", i)
    k = src.find("freeze_order(", i)
    if min(i, j, k) < 0:
        return ""
    return src[j:k if k > j else j + 1]


frz = freeze_body(rd(REPO_SHOP))
bsp = buy_span(rd(BUY))
buy_src = rd(BUY)
# ① 洞不许回来：两处形状不再同时在场（产品已按「裁 B」修好）
ok("HN1 静默吞插的洞不再成立（DO NOTHING 与无条件 Ok(()) 不同时候在场）",
   not hole_present(frz),
   "两处形状=%s" % (swallow_shapes(frz),))
# ② 裁 B 的三条腿：落空 ⇒ 退款 + 库存回滚 + 出声（缺一条就又变成
#    「失败演成成功」）
refund = (":refund" in buy_src and "earn_spark(" in buy_src
          and "tracing::error!(" in buy_src)
ok("HN2 落空必须退款 + 回滚库存 + 出声（裁 B 三条腿）", refund,
   "退款键=%s 退款调用=%s 出声=%s"
   % (":refund" in buy_src, "earn_spark(" in buy_src,
      "tracing::error!(" in buy_src))
# ③ 承重：把旧洞形状（DO NOTHING + 无条件 Ok(())）合成回来，
#    判据必须判它仍是洞；修好的形状必须判它不是洞。
syn_hole = frz.replace("Ok(oid)", "Ok(())")
ok("HN4 承重：旧洞形状（DO NOTHING + 无条件 Ok(())）必须被判为洞",
   hole_present(freeze_body(syn_hole)),
   "合成洞=%s" % (swallow_shapes(freeze_body(syn_hole)),))
ok("HN4b 承重：修好的形状（返回 Option）必须判为不是洞",
   not hole_present(freeze_body(frz)),
   "现网=%s" % (swallow_shapes(freeze_body(frz)),))

if LIVE:
    def psql(q):
        r = subprocess.run(
            ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
             "-d", "fluxtorrent", "-q", "-tA", "-F", "|"], input=q,
            capture_output=True, text=True, encoding="utf-8", errors="replace")
        if r.returncode != 0 or "ERROR" in (r.stderr or ""):
            raise RuntimeError("psql: " + (r.stderr or "")[:300])
        return (r.stdout or "").strip()

    q = ("SELECT l.idempotency_key FROM spark_ledger l WHERE l.kind = 'shop' "
         "AND l.idempotency_key LIKE 'shop:%' AND NOT EXISTS "
         "(SELECT 1 FROM shop_orders o WHERE o.idempotency_key = "
         "l.idempotency_key OR o.idempotency_key LIKE "
         "l.idempotency_key || ':%')")
    got = sorted(x for x in psql(q).split("\n") if x)
    ok("HN3 活体台账：扣了钱没有订单行的键集合 == 基线（增减都红）",
       got == sorted(BASELINE_KEYS),
       "现网=%s 基线=%s" % (got, sorted(BASELINE_KEYS)))
    n_shop = psql("SELECT count(*) FROM spark_ledger WHERE kind = 'shop' "
                  "AND idempotency_key LIKE 'shop:%'")
    ok("HN3b 违例基数确有对照（shop 类流水 > 违例数）",
       int(n_shop) > len(got), "shop 键流水=%s 违例=%s" % (n_shop, len(got)))
else:
    print("[HN3/HN3b] HN_LIVE=0 跳过活体两条")


print("== HN5/HN6 面板 Top 消耗只认钱货两清的订单（P-541）==")
DSH = rd("apps/api/src/economy_http/dashboard.rs")
seg = DSH[DSH.find("top_sku"):][:900]
ok("HN5 静态：top_sku 查询带 fulfill_state='done'",
   "AND o.fulfill_state = 'done'" in seg, "该段 fulfill_state 命中=%d"
   % seg.count("fulfill_state"))
if LIVE:
    TOK = login("root")
    ITEM = int(psql("INSERT INTO shop_items (name, kind, price, config,"
                    " active) VALUES ('HN探针面板卡', 'gift_spark', 500, "
                    "'{\"spark\": 10}'::jsonb, true) RETURNING id"
                    ).split("\n")[0])

    def ex(sql):
        psql(sql)

    ex("INSERT INTO shop_orders (user_id, item_id, price, idempotency_key,"
       " fulfill_state) VALUES (1, %d, 500, 'hn6-done', 'done'), (1, %d, "
       "999999, 'hn6-ref', 'refunded'), (1, %d, 888888, 'hn6-frz', 'frozen')"
       % (ITEM, ITEM, ITEM))
    st, body = call("GET", "/admin/economy-dashboard", token=TOK)
    rows = ((body or {}).get("data") or {}).get("top_sku_30d") or []
    mine = [x for x in rows if x.get("name") == "HN探针面板卡"]
    ok("HN6 面板不把 refunded/frozen 两笔（1,888,887）算进近 30 天消耗"
       "（旧行为＝orders=3/spent=1889387）",
       st == 200 and len(mine) == 1 and mine[0].get("orders") == 1
       and mine[0].get("spent") == 500,
       "http=%s 命中=%s 全表=%s" % (st, mine,
                                    sum(x.get("orders", 0) for x in rows)))
    ex("DELETE FROM shop_orders WHERE idempotency_key LIKE 'hn6-%'")
    ex("DELETE FROM shop_items WHERE id = %d" % ITEM)
    resid = psql("SELECT count(*) FROM shop_orders WHERE idempotency_key"
                 " LIKE 'hn6-%'")
    print("   [cleanup] hn6 残料 订单=%s SKU=%s" % (
        resid.split("\n")[0],
        psql("SELECT count(*) FROM shop_items WHERE name = 'HN探针面板卡'"
             ).split("\n")[0]))

summary()
