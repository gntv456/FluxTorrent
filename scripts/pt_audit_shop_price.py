# -*- coding: utf-8 -*-
"""常驻闸门：商城目录上的价必须等于下单扣的价（同一个递增公式）。

会员视角：商店页写「慈善捐赠 1000 魔力」，点下去扣 4000。
0383 起单价可按已售量递增（price_step / price_max / price_anchor_qty），
本体只有一处：`domain::shop::price_with_step`，购买口 shop.rs:81 用的就是它。
但目录口 shop_list.rs 当时只 SELECT 了 `shop_items.price` 这一列死值——
档位四列建了、后台能配、购买会算，**目录不显示**。
站方一旦真的配上递增档，会员先在价格上看错，再在余额上看到。

现网读数（2026-10-21）：26 件在售商品里配了递增档的是 **0 件**，
所以今天两条路径同价，本闸门是**上线前的引信**而不是现存事故。
交付时的实测证据（活体 A/B，18 号商品 已售 3 / anchor 0 / step 1000）：
  SQL 侧独立算出的真价 4000；现网(:8080) 目录回 1000；修复后目录回 4000。

判据：
  SP1 静态·目录必须读档位四列并调 price_with_step，且不许把裸 si.price 直接输出
  SP2 活体·每件在售商品的目录价 == SQL 侧独立算出的递增价（配了档位才咬得住）
  SP3 前提·四列与 domain 函数本体都还在（列被删/函数改名 ⇒ 判据要红，不许静默跳过）
  SP4 读数·报「现网配了递增档的商品数」，为 0 时 SP2 只能算休眠引信（打印说明）
自测：--selftest 把目录的调用换成直接输出 si.price ⇒ SP1 必须红，还原后必须绿。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_shop_price.py
      FLUX_API_BASE 默认 http://127.0.0.1:8080/api/v1
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import BASE, call, ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LIST_SRC = os.path.join(ROOT, "apps", "api", "src", "economy_http",
                        "shop_list.rs")
DOMAIN = os.path.join(ROOT, "apps", "api", "src", "domain", "shop", "mod.rs")
# 目录 SQL 必须带上的档位列
COLS = ("price_step", "price_max", "price_anchor_qty", "stock_used")


def code_only(path):
    """按行剥 // 注释：修复说明里就写着旧写法，不剥会自造违例。"""
    out = []
    for line in io.open(path, encoding="utf-8", errors="replace"):
        out.append(re.sub(r"//.*$", "", line.rstrip("\n")))
    return "\n".join(out)


def main():
    src = code_only(LIST_SRC)
    # ---------- SP1 目录走的是同一个公式 ----------
    has_cols = all(c in src for c in COLS)
    calls_fn = "price_with_step" in src
    # 接线点：映射里必须经公式算，而不是把行里的原始 price 直接塞进响应
    maps_via_helper = re.search(r"price:\s*eff_price\(", src) is not None
    leaks_raw = re.search(r"price:\s*(?:r\.price|base)\b", src) is not None
    ok("SP1 目录读档位四列(%s)、调 price_with_step(%s)、"
       "映射经公式(%s)、不直出裸 price(%s)"
       % (has_cols, calls_fn, maps_via_helper, not leaks_raw),
       has_cols and calls_fn and maps_via_helper and not leaks_raw,
       "缺一处就意味着目录与扣款不是同一个式子")

    # ---------- SP3 前提还在 ----------
    cols_live = psql_stdin(
        "SELECT count(*) FROM information_schema.columns "
        "WHERE table_name = 'shop_items' AND column_name IN "
        "('price_step','price_max','price_anchor_qty','stock_used')")
    fn_alive = "pub fn price_with_step(" in code_only(DOMAIN)
    ok("SP3 前提：shop_items 档位列 %s/4 在、domain 公式本体存在=%s"
       % (cols_live.strip(), fn_alive),
       cols_live.strip() == "4" and fn_alive,
       "列或公式没了，SP1/SP2 就都失去意义，先修前提")

    # ---------- SP2 活体逐件对账 ----------
    st, resp = call("GET", "/shop/items")
    ok("SP2a GET /shop/items 200（BASE=%s）" % BASE, st == 200,
       "http=%s %s" % (st, str(resp)[:150]))
    data = (resp or {}).get("data")
    rows = data if isinstance(data, list) else ((data or {}).get("items") or [])
    # SQL 侧独立算一遍（与 domain 同式，故意不复用 Rust 代码，才叫对照）
    oracle = {}
    for line in psql_stdin(
            "SELECT id, price + COALESCE(price_step, 0) * GREATEST("
            "COALESCE(stock_used,0) - price_anchor_qty, 0) "
            "FROM shop_items WHERE active = true").splitlines():
        if "|" in line:
            a, b = line.split("|", 1)
            oracle[int(a.strip())] = int(b.strip())
    mismatch = [(r["id"], r["name"], r["price"], oracle.get(r["id"]))
                for r in rows if oracle.get(r["id"]) != int(r["price"])]
    ok("SP2b 目录 %d 件与 SQL 对照逐件相等（不等 %s）"
       % (len(rows), mismatch or "无"), not mismatch,
       "目录价与扣价款分叉：%s" % mismatch[:3])

    # ---------- SP4 读数：递增档今天有没有人配 ----------
    stepped = psql_stdin("SELECT count(*) FROM shop_items WHERE active "
                         "AND price_step IS NOT NULL AND price_step > 0")
    print("   [读数] 现网在售 %d 件，其中配了递增档 %s 件 ⇒ %s"
          % (len(rows), stepped.strip(),
             "SP2 目前是休眠引信（配上就咬）" if stepped.strip() == "0"
             else "SP2 正在承重"))
    summary()


def selftest():
    src = io.open(LIST_SRC, encoding="utf-8").read()
    anchor = "price: eff_price("
    if src.count(anchor) != 1:
        raise SystemExit("自测锚点不唯一：%d" % src.count(anchor))
    i = src.index(anchor)
    j = src.index(")?,", i)
    # 翻成旧写法：直接把行里的原始 price 塞进响应（保留逗号，形状仍像个赋值）
    flipped = src[:i] + "price: r.price," + src[j + 3:]
    with io.open(LIST_SRC, "w", encoding="utf-8", newline="") as f:
        f.write(flipped)
    cmd = [sys.executable, os.path.abspath(__file__)]
    rc_flip = None
    try:
        r = subprocess.run(cmd, capture_output=True, text=True,
                           encoding="utf-8", errors="replace")
        rc_flip = r.returncode
        print("[自测] 目录改回直出裸 price 后 rc=%s（必须非 0）" % rc_flip)
        for l in [x for x in (r.stdout or "").splitlines() if "SP1" in x][:2]:
            print("   ", l.strip()[:200])
    finally:
        with io.open(LIST_SRC, "w", encoding="utf-8", newline="") as f:
            f.write(src)
    back = subprocess.run(cmd, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")
    print("[自测] 还原后 rc=%s（必须 0）" % back.returncode)
    return 0 if (rc_flip not in (None, 0) and back.returncode == 0) else 2


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
