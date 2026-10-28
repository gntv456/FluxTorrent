# -*- coding: utf-8 -*-
"""闸门 ME：库存计数器只在「限量」时才走 ⇒ 无配额 SKU 的阶梯价永不涨（P-454）。

机制（读码 + 现网取数）：
  · 售价 = `domain::shop::price_with_step(单价, sold, step, max, anchor)`，
    其中 `sold` **取的是 `shop_items.stock_used`**（`economy_http/shop_list.rs`
    的 `COALESCE(si.stock_used,0) AS sold`；扣款侧同源于此，目录价=扣款价）。
  · 而 `stock_used` 的每一条增减语句都写着
    `WHERE id = $1 AND stock_quota IS NOT NULL`——**不限量（quota 为空）的 SKU
    永远不累加**。现网实测：6 只常见 SKU（补签卡/1GB 上传量/赠送魔力/免费券/
    抽卡券/慈善捐赠）各有 3~50 笔订单，`stock_used` 仍是 0 或 3。
  · 于是「给不限量商品配递增价」这个完全合理的配置组合，结果是**价格一条都不涨**：
    界面没有已售数（前端只渲染 stock_left/售罄，不显示 sold），后台看到的
    「已售」也是 0，站长只会以为阶梯价功能坏了。

现网账（2026-10-28 只读取数）：带 `price_step` 的 SKU = 0 只 ⇒ **洞被数据压住，
今天没有在错定价**。因此本闸门不擅自改钱路：增减计数器要同时改
`Option<bool>` 解码（拆掉守卫后 RETURNING 会回 NULL，按非 Option 读必 500）、
补齐回填迁移（历史订单的 sold 要一次算准，否则阶梯从 0 起步）、并把 3 处
撤销/退款侧的守卫一起拆——那是站长的产品口径决定，不是可半途换的写法。

判据：
  ME1 行为不变量：现网不得存在「price_step 已配 且 stock_quota 为空」的 SKU
      （这条就是缺陷本体的现场；一旦这么配，阶梯价永不推进）
  ME2 钉桩：`stock_used` 的增减语句处数 = 台账，且**每一处都带 quota 守卫**
      ⇒ 有人开始修就要同步销号（防止只拆一处＝计数器半走半不走）
  ME3 同源：目录价与扣款价都从 `stock_used` 取（拆成两处就是第二个洞）
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "apps", "api", "src")

# 台账两栏，都按**判据自己的输出**登记（不凭记忆）：
#   GUARDED＝增减都带 quota 守卫（不限量 SKU 根本不碰计数器）
#   PARTNER＝语句本身没写守卫、但被上游 token 挡住的（拆了上游守卫才会露出来）
GUARDED = {"economy_http/shop.rs:+", "admin_http/user_grant_item.rs:+",
           "admin_http/user_revoke.rs:-", "admin_http/user_vouchers.rs:-",
           "admin_p3_http/increment_stock.rs:+"}
PARTNER = {"economy_http/shop.rs:-":
           "三处减都写在 `if let Some((item_id, q)) = stock_rollback` 里，"
           "而这个 token 只有增量真发生（quota 非空）才置位 ⇒ 负数不可达"
           "（现网实测 min(stock_used)=0、<0 的 0 只）"}
CNT_RX = re.compile(r"(?:\w+\.)?stock_used\s*=\s*(?:GREATEST\(0,\s*)?"
                    r"(?:\w+\.)?stock_used\s*([+-])")


def sql_literal(txt, i):
    """取包含位置 i 的那条 Rust 字符串字面量整段（**不按固定窗口切**——
    守卫条款离 `stock_used` 那句可能有 7 行，窗口窄了会把「其实带守卫」的
    语句读成无守卫；窗口宽了又会吸进隔壁语句的守卫。按引号边界切才准）。"""
    a = txt.rfind('"', 0, i)
    b = txt.find('"', i + 1)
    while b != -1 and txt[b - 1] == "\\":      # 跳过 `\"` 与 `\<换行>` 续行
        b = txt.find('"', b + 1)
    return txt[a:b + 1] if a != -1 and b != -1 else ""


def stock_sites():
    out, ung = set(), set()
    for dirpath, _d, names in os.walk(SRC):
        for n in names:
            if not n.endswith(".rs"):
                continue
            p = os.path.join(dirpath, n)
            txt = open(p, encoding="utf-8", errors="replace").read()
            rel = os.path.relpath(p, SRC).replace(os.sep, "/")
            for m in CNT_RX.finditer(txt):
                key = "%s:%s" % (rel, m.group(1))
                if "stock_quota IS NOT NULL" in sql_literal(txt, m.start()):
                    out.add(key)
                else:
                    ung.add(key)
    return out, ung


guarded, unguarded = stock_sites()
print("[读数] 带 quota 守卫=%s" % sorted(guarded))
print("[读数] 无守卫（含被上游 token 挡住的）=%s" % sorted(unguarded))
newg = unguarded - set(PARTNER)
goneg = set(PARTNER) - unguarded
ok("ME2 计数器增减点两栏对账：带守卫集合与「被上游 token 挡住」集合都不许飘",
   guarded == GUARDED and not newg and not goneg,
   "带守卫=%s 台账=%s 新露出的无守卫=%s 已消失=%s"
   % (sorted(guarded), sorted(GUARDED), sorted(newg), sorted(goneg)))

mixed = psql_stdin(
    "SELECT count(*) FROM shop_items WHERE price_step IS NOT NULL "
    "AND price_step <> 0 AND stock_quota IS NULL;")
ok("ME1 活体：不得有「配了递增价却不限量」的 SKU（这么配＝价格永不推进）",
   mixed.splitlines()[0].strip() == "0",
   "命中=%s（现网带 price_step 的 SKU 共 %s 只；不限量那几只的 stock_used "
   "根本不累加）" % (mixed.splitlines()[0].strip(), psql_stdin(
       "SELECT count(*) FROM shop_items WHERE price_step IS NOT NULL "
       "AND price_step <> 0;").splitlines()[0].strip()))

behind = psql_stdin(
    "SELECT count(*) FROM shop_items si WHERE si.stock_quota IS NULL "
    "AND si.stock_used <> (SELECT count(*) FROM shop_orders o "
    "WHERE o.item_id = si.id);")
print("[读数] 不限量 SKU 里计数器落后于真实订单数的：%s 只"
      % behind.splitlines()[0].strip())

list_src = open(os.path.join(SRC, "economy_http", "shop_list.rs"),
                encoding="utf-8", errors="replace").read()
buy_src = open(os.path.join(SRC, "economy_http", "shop.rs"),
               encoding="utf-8", errors="replace").read()
ok("ME3 目录价与扣款价同源 stock_used（改成两处取数=第二个洞，别单边改）",
   "si.stock_used, 0) AS sold" in re.sub(r"\s+", " ", list_src)
   and "stock_used" in buy_src,
   "目录侧取 sold=%s 扣款侧读 counter=%s"
   % ("si.stock_used, 0) AS sold" in re.sub(r"\s+", " ", list_src),
      "stock_used" in buy_src))

summary()
