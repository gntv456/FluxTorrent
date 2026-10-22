# -*- coding: utf-8 -*-
"""闸门 GV：后台「赠予道具」口不许 200 + 空单（批二百五十九，P-539）。

缺陷本体（现网实测）：`POST /admin/users/{id}/grant-item/{item}` 的
`parse_effect` 只写了 gift_spark / upload_credit / invite·temp_invite /
voucher_* 四类腿，其余 kind 一律 `_ => Effect::Backpack` ⇒ 只往 shop_orders
插一条 0 价行。而 `crates/flux-shared` 的生效链真会执行 15 个 kind，
商城购买口（POST /shop/buy）走的就是那条链。于是同一支 VIP 卡：
  会员自购 → users.vip_until 真的 +7 天；
  后台赠予 → 接口 200/code=0，vip_until 不动，只多一条 fulfill_state='done'
             （列默认值）的订单行 = 账面说这单完成了、效果列也没人报警。
站长在「用户详情 → 赠予道具」里点一次就中招，而现网 active SKU 里这类
有 11 支（7 装扮 + 2 动效头像 + 霓虹名/霓虹ID + vip/app_vip/custom_title/
ad_free/charity/rename_card 等）。

本闸门钉三件事（一支支 SKU 现场造、现场删）：
  GV1 赠 VIP 卡：要么真加天数，要么 400 拒发且不留空单——不许「200 + 空单」
  GV2 正控：同一支 SKU 走商城购买口确实加天数（证明洞只在后台口，
       也证明生效链本身可用，不是我把两边都判死）
  GV3 「订单行即凭证」的类（makeup_card，补签口真在读）照旧能赠：
       200 + 恰好一条订单行 —— 防止把修复做成「一概拒发」的过度收紧
  GV4 静态：admin 口的兜底臂不许再把生效链认识的 kind 咽下去

跑法：
    PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_gv_grant_effect.py
    FLUX_API_BASE=http://127.0.0.1:8099/api/v1 python -u scripts/\\
pt_audit_gv_grant_effect.py
"""
import io
import os
import re
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, login, ok, psql_stdin, summary)

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PW = "ProbePass456"
TOKEN = None
CREATED = []


def psql(sql):
    return psql_stdin(sql.rstrip(";") + ";").strip()


def q1(sql):
    """psql 走 stdin 还会打命令标签 ⇒ 只取第一行第一段。"""
    return psql(sql).split("\n")[0].strip()


def msg(r):
    return (r.get("message") or "")[:44]


def new_probe(tag):
    name = "gv_%s_%d" % (tag, int(time.time() * 1000) % 100000)
    s, r = call("POST", "/admin/adduser",
                {"username": name, "email": "%s@example.com" % name,
                 "password": "TempPass123"}, TOKEN)
    if s != 200 or r.get("code") != 0:
        raise SystemExit("adduser failed %s %s" % (s, msg(r)))
    uid = r["data"]["user_id"]
    CREATED.append(uid)
    t0 = login(name, "TempPass123")
    s, r = call("POST", "/me/password/change",
                {"old_password": "TempPass123", "new_password": PW}, t0)
    if s != 200:
        raise SystemExit("password change failed %s %s" % (s, msg(r)))
    return uid, login(name, PW)


def snap(uid):
    """回 (vip_until 文本, 该会员订单行数)。"""
    v = q1("SELECT coalesce(vip_until::text,'N') FROM users WHERE id=%d" % uid)
    n = q1("SELECT count(*) FROM shop_orders WHERE user_id=%d" % uid)
    return v, int(n)


def cleanup():
    for uid in CREATED:
        try:
            psql("DELETE FROM shop_orders WHERE user_id=%d" % uid)
            psql("DELETE FROM user_vouchers WHERE user_id=%d" % uid)
            psql("DELETE FROM spark_ledger WHERE user_id=%d" % uid)
            psql("DELETE FROM traffic_ledger WHERE user_id=%d" % uid)
            call("POST", "/admin/users/status", {"user_id": uid, "status": 2},
                 TOKEN)
            call("DELETE", "/admin/users/%d" % uid, None, TOKEN)
        except Exception as e:  # noqa: BLE001
            print("[CLEAN-WARN] uid %s: %s" % (uid, e))
    psql("DELETE FROM shop_items WHERE name LIKE 'GV探针%'")
    print("[CLEAN] 探针 %s 已删；临时 SKU 残留=%s"
          % (CREATED, q1("SELECT count(*) FROM shop_items"
                         " WHERE name LIKE 'GV探针%'")))


def mkitem(kind):
    return int(q1("INSERT INTO shop_items (name, kind, price, config, active)"
                  " VALUES ('GV探针%s', '%s', 100, "
                  "'{\"days\": 7}'::jsonb, true) RETURNING id" % (kind, kind)))


def main():
    global TOKEN
    print("== GV 后台赠予的生效链 == 基线 %s" % BASE)
    TOKEN = login("root")
    try:
        uid, utok = new_probe("vip")
        vip_item, mkp_item = mkitem("vip"), mkitem("makeup_card")
        psql("UPDATE users SET spark_balance=5000 WHERE id=%d" % uid)
        v0, o0 = snap(uid)
        s, r = call("POST", "/admin/users/%d/grant-item/%d" % (uid, vip_item),
                    {}, TOKEN)
        v1, o1 = snap(uid)
        ok("GV1 赠 VIP 卡：真加天数或拒发且不留空单（200+空单＝假成功）",
           (s == 400 and o1 == o0) or (s == 200 and v1 != v0),
           "HTTP %s code=%s vip %s->%s 订单 +%d %s"
           % (s, r.get("code"), v0, v1, o1 - o0, msg(r)))
        s, r = call("POST", "/shop/buy", {"item_id": vip_item, "qty": 1,
                                          "idempotency_key": "gv-buy-vip"},
                    utok)
        v2, _ = snap(uid)
        ok("GV2 正控：同支 SKU 会员自购确实加天数（洞只在后台口）",
           s == 200 and v2 != v0, "HTTP %s vip %s->%s %s" % (s, v0, v2, msg(r)))
        s, r = call("POST", "/admin/users/%d/grant-item/%d" % (uid, mkp_item),
                    {}, TOKEN)
        _, o3 = snap(uid)
        ok("GV3 订单行即凭证的类（makeup_card）照旧能赠：200 + 一条订单行",
           s == 200 and o3 == o1 + 2,
           "HTTP %s 订单 %d->%d %s" % (s, o1, o3, msg(r)))
        src = io.open(os.path.join(
            ROOT, "apps", "api", "src", "admin_http",
            "user_grant_item.rs"), encoding="utf-8", errors="replace").read()
        src = re.sub(r"//[^\n]*", "", src)
        arm = re.search(r"Effect::parse|_ =>\s*Effect::Backpack|_ =>", src)
        swallows = "_ => Effect::Backpack" in src
        ok("GV4 静态：兜底臂不再把生效链认识的 kind 咽成背包类", not swallows,
           "兜底命中=%s（%s）" % (swallows, arm.group(0) if arm else "-"))
    finally:
        cleanup()
    print()
    summary()


if __name__ == "__main__":
    main()
