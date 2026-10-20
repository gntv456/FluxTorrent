# -*- coding: utf-8 -*-
"""常驻闸门：签名有效但查无此单的支付回调，不许 ack success（P-463）。

形状：`/donate/notify` 只做「验签 → CAS 入账」，CAS 打不中时一律回
`SettleOutcome::Duplicate`，处理器 `Ok(_)` ⇒ 给网关回 `success`。于是
「订单号在 `payment_orders` 里根本不存在」和「这张单已经入账过」被压成同
一个结果：钱确实在网关那侧收到了，但我们无处入账，而 **ack 把网关的重投掐
断**——这笔钱从此在系统里蒸发，站长与用户都不会知道。管理员手工补单那条口
早就要求「单号不存在必须 404」（`pt_audit_bh_donate_cheat.py` H1g），回调这
条口一直没人管。

修法（P-463）：CAS 打不中时再查一次订单是否存在——确有此单 ⇒ Duplicate（ack
success，幂等正确）；查无此单 ⇒ `UnknownOrder`，处理器回 `fail: unknown order`
并 `warn` 记下 order_no / trade_no / money，让网关按自己的策略继续重投。
「部分支付要不要入账」是另一件事，`check_paid_amount` 的注释里已写明是刻意
不比对面额（批四十四 P-173，另列待拍板），本批不推翻那个口径。

判据分层：
  N0 静态：settle.rs 有 UnknownOrder 分支且 handler 不再一把抓 `Ok(_)`
  N1 现网活体（默认只报读数；`--live` 升硬）：用同一套签名对一个**从未建过**
     的订单号发回调 ⇒ 回执必须以 fail 开头、`donation_ledger` 不得多出一行
  N2 正控（同密钥、真实存在的 pending 订单）：必须照常 success，证明 N1 的
     fail 不是「所有回调都被拒」——两支一起看才叫分岔
     （N2 会真入账，收尾按 (id > 起点) AND user_id=探针 精确清账并回滚钱包）
跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_notify_unknown_order.py
      带 --live 才把 N1/N2 升成硬断言（现网二进制没带本批修复时会红，
      那条红就是缺陷本体的证据，存进提交说明）
"""
import io
import json
import os
import re
import subprocess
import sys
import time
import urllib.parse
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import BASE, ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LIVE = "--live" in sys.argv
SETTLE = "apps/api/src/payment/settle.rs"
HANDLER = "apps/api/src/staff_http/donate_notify.rs"
KEY = "0123456789abcdef0123456789abcdef"   # 只用于签名，跑完即还原
PROBE_TAG = "P463"


def epay_sign(params, key):
    """与 `payment/epay.rs` 同口径：升序拼 k=v（跳过 sign/sign_type/空值）
    + 密钥，取 MD5。这里自己实现一遍，是为了让闸门能独立复算签名。"""
    import hashlib
    items = sorted((k, v) for k, v in params.items()
                   if k not in ("sign", "sign_type") and v != "")
    raw = "&".join("%s=%s" % kv for kv in items)
    return hashlib.md5((raw + key).encode()).hexdigest()


def notify(order_no, money, trade_no):
    p = {"pid": "p463gate", "out_trade_no": order_no, "trade_no": trade_no,
         "money": money, "trade_status": "TRADE_SUCCESS", "sign_type": "MD5"}
    p["sign"] = epay_sign(p, KEY)
    url = BASE + "/donate/notify?" + urllib.parse.urlencode(p)
    try:
        with urllib.request.urlopen(url, timeout=25) as r:
            return r.status, r.read().decode("utf-8", "replace").strip()
    except Exception as ex:  # noqa: BLE001
        return 0, str(ex)[:120]


def main():
    st = io.open(os.path.join(ROOT, SETTLE), encoding="utf-8",
                 errors="replace").read()
    hd = io.open(os.path.join(ROOT, HANDLER), encoding="utf-8",
                 errors="replace").read()
    ok("N0 静态：settle 有 UnknownOrder 分支并先查订单是否存在；handler 分岔"
       "回执（不再一把抓 Ok(_) ⇒ success）",
       "SettleOutcome::UnknownOrder" in st
       and "SELECT id FROM payment_orders WHERE order_no = $1" in st
       and "Ok(SettleOutcome::UnknownOrder)" in hd
       and "Ok(_) =>" not in hd, "分支被抹平＝静默吞掉一笔付了钱的订单")
    # 现网开闸三步（跑完原样还原；这三把键本身不在面板里，只能这样设）
    snap = {}
    for k in ("payment_provider", "payment_pid", "payment_key"):
        snap[k] = psql_stdin("SELECT value FROM site_settings WHERE name="
                             "'%s';" % k)
    for k, v in (("payment_provider", "epay"), ("payment_pid", "p463gate"),
                 ("payment_key", KEY)):
        # upsert：这三把键正常由迁移播种，缺行时（新库）也要能开闸
        psql_stdin("INSERT INTO site_settings (name, value) VALUES ('%s','%s') "
                   "ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value;"
                   % (k, v))
    try:
        led0 = int(psql_stdin("SELECT count(*) FROM donation_ledger WHERE "
                              "note LIKE '%%%s%%';" % PROBE_TAG))
        # N1：从未建过的订单号
        o1 = "%sNONE%d" % (PROBE_TAG, int(time.time()))
        s1, b1 = notify(o1, "5.00", "%sT1" % PROBE_TAG)
        led_after = int(psql_stdin("SELECT count(*) FROM donation_ledger "
                                   "WHERE note LIKE '%%%s%%';" % PROBE_TAG))
        cond = s1 == 200 and b1.startswith("fail") and led_after == led0
        if LIVE:
            ok("N1 --live 硬断言：查无此单的签名回调必须回 fail 且不写流水"
               "（回执=%r 流水 +%d）" % (b1[:24], led_after - led0), cond,
               "回 success＝网关停止重投，这笔钱在系统里蒸发")
        else:
            print("   [N1] 现网回执=%r（应为 fail 开头）；本批修复未进镜像时"
                  "这条预期回 success——加 --live 升硬" % b1[:24])
        # N2 正控：真实存在的 pending 订单（探针用户），必须照常入账
        # 探针用户三条挑选条件：不是 root（别的活体闸门也在花 root 的账，
        # 撞在一起会把红账读成级联假红）、本来就 donor=false 且 wallet=0
        # ——这样"入账→回滚"能被逐字段复原，不留 donor 痕迹
        uid = psql_stdin("SELECT min(id) FROM users WHERE status < 2 "
                         "AND id <> 1 AND NOT donor AND wallet_usd = 0;")
        before = psql_stdin("SELECT donor::text||'|'||wallet_usd::text FROM "
                            "users WHERE id=%s;" % uid)
        psql_stdin("INSERT INTO payment_orders (order_no, user_id, amount_usd,"
                   " channel, status) VALUES ('%sOK', %s, 5.00, 'epay', "
                   "'pending');" % (PROBE_TAG, uid))
        w0 = psql_stdin("SELECT wallet_usd::text FROM users WHERE id=%s;"
                        % uid)
        s2, b2 = notify("%sOK" % PROBE_TAG, "5.00", "%sT2" % PROBE_TAG)
        st2 = psql_stdin("SELECT status FROM payment_orders WHERE "
                         "order_no='%sOK';" % PROBE_TAG)
        cond2 = s2 == 200 and b2 == "success" and st2 == "paid"
        if LIVE:
            ok("N2 正控：同密钥对真实订单的回调照常 success + paid（回执=%r "
               "状态=%s）——N1 的 fail 不是把所有回调都拒了"
               % (b2[:24], st2), cond2, "正控红＝分岔做成了全拒")
        else:
            print("   [N2] 正控回执=%r 订单状态=%s" % (b2[:24], st2))
        # 清账：只删本轮专属前缀造出来的行，并把探针用户钱包回滚原值
        psql_stdin("DELETE FROM donation_ledger WHERE note LIKE '%%%s%%';"
                   % PROBE_TAG)
        psql_stdin("UPDATE users SET wallet_usd = (%s)::numeric, "
                   "donor = false WHERE id=%s;" % (w0, uid))
        psql_stdin("DELETE FROM payment_orders WHERE order_no LIKE '%s%%';"
                   % PROBE_TAG)
        left = psql_stdin("SELECT count(*) FROM payment_orders WHERE "
                          "order_no LIKE '%s%%';" % PROBE_TAG)
        after = psql_stdin("SELECT donor::text||'|'||wallet_usd::text FROM "
                           "users WHERE id=%s;" % uid)
        ok("N3 探针清账无残留（orders/ledger 残留=%s/%s）且探针账号身份已复原"
           "（%s → %s）"
           % (left, psql_stdin("SELECT count(*) FROM donation_ledger WHERE "
                              "note LIKE '%%%s%%';" % PROBE_TAG), before,
              after),
           left == "0" and after == before,
           "残留会把一笔模拟捐赠（含 donor 身份）留在演示站账上")
    finally:
        for k, v in snap.items():
            if v == "":
                psql_stdin("UPDATE site_settings SET value='' WHERE name='%s';"
                           % k)
            else:
                psql_stdin("UPDATE site_settings SET value='%s' WHERE "
                           "name='%s';" % (v.replace("'", "''"), k))
        got = json.dumps({k: psql_stdin("SELECT value FROM site_settings "
                                        "WHERE name='%s';" % k) for k in snap})
        print("   [N4] 支付三项已还原为 %s" % got)
    # 收尾必须调 summary()：漏掉它，闸门不管多少条红都 exit 0（本闸门第一版
    # 就是这样——现网 --live 明明有一条 [FAIL]，rc 仍是 0，一条永远不会红的
    # 门禁比没有门禁更坏）。
    summary()


if __name__ == "__main__":
    main()
