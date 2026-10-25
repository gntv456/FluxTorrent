# -*- coding: utf-8 -*-
"""全站深测探针：会员侧写链路（私信全链 + 商城消耗链）。

从 pt_audit_deep_walk.py 拆出（500 行门禁）。这两支都是「跨端点对账」型：
未读计数 vs 列表条数、扣费 vs 到账 vs 流水——单口能过、两口对不上才是缺陷。
"""
from __future__ import annotations

from pt_audit_deep_walk_base import *  # noqa: F401,F403
import sys

# ---------------------------------------------------------------- 域：pm

def d_pm(vip: str, root: str) -> None:
    D = "pm"
    # 收件箱基线 + /me 未读计数对账（真交叉验证）。limit=100 是上限：
    # 用 unread=true 只取未读，55 条未读不会被 limit=50 截成一页 50。
    st, js = call("GET",
                  "/api/v1/messages/inbox?unread=true&limit=100", vip)
    if not ok(D, "收件箱可读", st, js):
        return
    before_unread = sum(1 for m in rows_of(js) if m.get("unread"))
    st, js_me = call("GET", "/api/v1/me", vip)
    me_unread = data_of(js_me).get("unread_messages")
    expect(D, "/me 未读数 == 收件箱实际未读", me_unread == before_unread,
           f"me={me_unread} inbox={before_unread}")

    # flood 闸先测（vip 普通用户 60s 一条）：连发第二条应被 400 拦
    st1, _ = call("POST", "/api/v1/messages", vip,
                  body={"to": "root", "subject": "deep_flood_a",
                      "body": "flood"})
    st2, js2 = call("POST", "/api/v1/messages", vip,
                    body={"to": "root", "subject": "deep_flood_b",
                        "body": "flood"})
    expect(D, "私信 flood 闸拦第二条", st1 == 200 and st2 == 400 and
           "频繁" in str(js2.get("message",
               "")), f"st1={st1} st2={st2} {str(js2)[:60]}")
    time.sleep(62)  # 让开 flood 窗口，后续主流程用 vip 发信

    # 主流程：vip→root（发件箱可见性由用户设置 save_pm 决定——NP
    # 「保存短讯至发件箱」口径，P-448 起写值改由本人这一列决定）
    st, js = call("GET", "/api/v1/me/settings", vip)
    save_pm_before = data_of(js).get("save_pm")
    if save_pm_before is not True:
        st, js = call("PUT", "/api/v1/me/settings", vip,
                      body={"save_pm": True})
        ok(D, "开启「保存到发件箱」设置", st, js)
    subj = f"deep_pm_{int(time.time())}"
    st, js = call("POST", "/api/v1/messages", vip,
                  body={"to": "root", "subject": subj,
                        "body": "深测私信：收发链路。"})
    if not ok(D, "发信(vip→root)", st, js):
        return
    st, js = call("GET", "/api/v1/messages/sent?limit=10", vip)
    sent = rows_of(js)
    expect(D, "发件箱可见(开 save_pm)", any(m.get("subject") == subj for m in sent),
           f"n={len(sent)}")
    # 关回设置后发信，发件箱不留副本
    if save_pm_before is not True:
        call("PUT", "/api/v1/me/settings", vip, body={"save_pm": False})

    # root 收件箱收到 + 未读态 + 搜索 + 未读筛选
    st, js = call("GET", "/api/v1/messages/inbox?limit=50", root)
    inbox_root = rows_of(js)
    target = next((m for m in inbox_root if m.get("subject") == subj), None)
    expect(D, "root 收件箱收到", target is not None)
    if not target:
        return
    mid = target["id"]
    expect(D, "新信未读态", target.get("unread") is True)
    st, js = call("GET", "/api/v1/messages/inbox?search="
                  + urllib.parse.quote(subj), root)
    expect(D, "收件箱关键词搜索", st == 200 and
           any(m.get("subject") == subj for m in rows_of(js)))
    st, js = call("GET", "/api/v1/messages/inbox?unread=true&limit=50", root)
    expect(D, "unread=true 筛选", st == 200 and
           any(m.get("subject") == subj for m in rows_of(js)))
    # 已读流转
    st, js = call("POST", "/api/v1/messages/markread", root,
                  body={"ids": [mid]})
    ok(D, "标记已读", st, js)
    st, js = call("GET", "/api/v1/messages/inbox?limit=50", root)
    again = next((m for m in rows_of(js) if m.get("subject") == subj), None)
    expect(D, "已读后 unread=false", again is not None and
           again.get("unread") is False)
    # root 回复（staff 不受 flood 限）
    st, js = call("POST", "/api/v1/messages", root,
                  body={"to": "audit_vip", "subject": f"Re: {subj}",
                        "body": "深测回复。"})
    ok(D, "回复(root→vip)", st, js)
    st, js = call("GET", "/api/v1/messages/inbox?limit=50", vip)
    expect(D, "vip 收到回复", any(
        m.get("subject") == f"Re: {subj}" for m in rows_of(js)))

    # 自定义文件夹：vip 建盒 → root 发信给 vip → vip move 进盒 → 盒内可见 → 删
    # （先清理历次残留 deep盒——一人最多 3 个，不清理会被「最多 3 个」挡下）
    st, js = call("GET", "/api/v1/messages/boxes", vip)
    for bx in rows_of(js):
        if "deep盒" in str(bx.get("name") or ""):
            call("POST", "/api/v1/messages/boxes", vip,
                 body={"id": bx.get("id"), "name": ""})
    st, js = call("POST", "/api/v1/messages/boxes", vip,
                  body={"name": f"deep盒{int(time.time()) % 10000}"})
    if ok(D, "建自定义文件夹", st, js):
        box_id = data_of(js).get("id")
        subj2 = f"deep_pm_box_{int(time.time())}"
        st, js = call("POST", "/api/v1/messages", root,
                      body={"to": "audit_vip", "subject": subj2,
                            "body": "进盒测试"})
        st, js = call("GET", "/api/v1/messages/inbox?limit=50", vip)
        m2 = next((m for m in rows_of(js) if m.get("subject") == subj2), None)
        if m2:
            st, js = call("POST", "/api/v1/messages/move", vip,
                          body={"ids": [m2["id"]], "folder": box_id})
            ok(D, "移动进盒", st, js)
            st, js = call("GET",
                f"/api/v1/messages/inbox?box_id={box_id}&limit=10", vip)
            expect(D, "盒内可见", st == 200 and
                   any(m.get("subject") == subj2 for m in rows_of(js)))
            st, js = call("POST", "/api/v1/messages/delete", vip,
                          body={"ids": [m2["id"]]})
            ok(D, "删除私信", st, js)
            st, js = call("GET",
                f"/api/v1/messages/inbox?box_id={box_id}&limit=10", vip)
            expect(D, "删除后盒内不可见", st == 200 and not any(
                m.get("subject") == subj2 for m in rows_of(js)))
        else:
            rec(D, "WARN", "未收到进盒测试信，move/delete 链跳过")

    # 边界：给自己发 / 不存在收件人
    st, js = call("POST", "/api/v1/messages", vip,
                  body={"to": "audit_vip", "subject": "deep_self", "body": "x"})
    expect(D, "给自己发被拒", st == 400 and "自己" in str(js.get("message", "")),
           f"st={st} {str(js)[:60]}")
    st, js = call("POST", "/api/v1/messages", vip,
                  body={"to": "no_such_user_zz", "subject": "x", "body": "y"})
    expect(D, "不存在收件人拒发", st in (400, 404), f"st={st}")

    # 管理信：root→vip + staff 信箱可见
    st, js = call("POST", "/api/v1/admin/staffmess", root,
                  body={"receiver": "audit_vip",
                        "subject": f"deep_staff_{int(time.time())}",
                        "body": "管理组通知深测。"})
    ok(D, "管理信下发(staffmess)", st, js)
    st, js = call("GET", "/api/v1/messages/staff?limit=10", vip)
    expect(D, "管理信信箱可见", st == 200)


# ---------------------------------------------------------------- 域：mall

def d_mall(vip: str, root: str) -> None:
    D = "mall"
    st, js = call("GET", "/api/v1/shop/items", vip)
    if not ok(D, "商城货架", st, js):
        return
    items = rows_of(js)
    expect(D, "货架有商品", len(items) > 0, f"n={len(items)}")
    if not items:
        return
    by_kind: dict[str, dict] = {}
    for it in items:
        by_kind.setdefault(it.get("kind") or "", it)
    stackable = next((i for i in items
                      if (i.get("config") or {}).get("stackable")), None)
    nonstack = next((i for i in items
                     if not (i.get("config") or {}).get("stackable")), None)
    gift = by_kind.get("gift_spark")
    credit = by_kind.get("upload_credit")
    voucher = by_kind.get("voucher_free")

    def bal(uid: int) -> int:
        return int(psql(f"SELECT COALESCE(spark_balance,0) FROM users WHERE id={uid}") or 0)

    # 不存在商品 / 缺幂等键 / 非叠加品 qty=2（三条拒绝路径）
    st, _ = call("POST", "/api/v1/shop/buy", vip,
                 body={"item_id": 999999999, "qty": 1,
                       "idempotency_key": f"deep_mx_{int(time.time())}"})
    expect(D, "不存在商品拒购", st in (400, 404), f"st={st}")
    if stackable:
        st, _ = call("POST", "/api/v1/shop/buy", vip,
                     body={"item_id": stackable["id"], "qty": 1})
        expect(D, "缺幂等键 400", st == 400, f"st={st}")
    if nonstack:
        st, _ = call("POST", "/api/v1/shop/buy", vip,
                     body={"item_id": nonstack["id"], "qty": 2,
                           "idempotency_key": f"deep_ns_{int(time.time())}"})
        expect(D, "非叠加品 qty=2 拒", st in (400, 422), f"st={st}")

    # 主购买：最便宜叠加品，扣费到账 + 幂等不双扣 + 订单在册
    if stackable:
        price = stackable.get("price") or 0
        idem = f"deep_mall_{int(time.time())}"
        b0 = bal(VIP_UID)
        if price <= b0:
            st, js = call("POST", "/api/v1/shop/buy", vip,
                          body={"item_id": stackable["id"], "qty": 1,
                                "idempotency_key": idem})
            if ok(D, f"购买({stackable['name']})", st, js):
                b1 = bal(VIP_UID)
                expect(D, "扣费到账", b0 - b1 == price,
                       f"Δ={b0-b1} price={price}")
                call("POST", "/api/v1/shop/buy", vip,
                     body={"item_id": stackable["id"], "qty": 1,
                           "idempotency_key": idem})
                b2 = bal(VIP_UID)
                expect(D, "幂等键不双扣", b2 == b1, f"b1={b1} b2={b2}")
                st, js = call("GET", "/api/v1/me/orders?limit=20", vip)
                expect(D, "订单记录可查", st == 200 and len(rows_of(js)) > 0)
        else:
            rec(D, "WARN", f"余额不足({b0}<{price})跳过购买链")

    # 上传量商品：经济↔流量桥（扣魔力 + 上传量入账 + 流水留痕）
    if credit:
        price = credit.get("price") or 0
        b0 = bal(VIP_UID)
        up0 = int(psql("SELECT COALESCE(base_up,0) FROM balance_baseline "
                       "WHERE user_id=7346") or 0)
        led0 = int(float(psql("SELECT COALESCE(sum(delta_up),0) FROM traffic_ledger "
                              "WHERE user_id=7346") or 0))
        if price <= b0:
            st, js = call("POST", "/api/v1/shop/buy", vip,
                          body={"item_id": credit["id"], "qty": 1,
                                "idempotency_key": f"deep_uc_{int(time.time())}"})
            if ok(D, f"购买上传量({credit['name']})", st, js):
                expect(D, "上传量购买扣魔力", b0 - bal(VIP_UID) == price)
                up1 = int(psql("SELECT COALESCE(base_up,0) FROM balance_baseline "
                               "WHERE user_id=7346") or 0)
                led1 = int(float(psql("SELECT COALESCE(sum(delta_up),0) FROM traffic_ledger "
                                      "WHERE user_id=7346") or 0))
                gb = (credit.get("config") or {}).get("gb") or 0
                expect(D, "上传量增量 == 商品 GB",
                       (up1 + led1) - (up0 + led0) == gb * 1024**3,
                       f"Δ={(up1+led1)-(up0+led0)} 期望={gb}GB")
        else:
            rec(D, "WARN", f"余额不足({b0}<{price})跳过上传量购买")

    # 赠礼回归（0421 修复）：gift_spark → root 到账
    if gift:
        price = gift.get("price") or 0
        b0 = bal(VIP_UID)
        r0 = bal(1)
        if price <= b0:
            st, js = call("POST", "/api/v1/shop/buy", vip,
                          body={"item_id": gift["id"], "qty": 1, "to": "root",
                                "idempotency_key": f"deep_gift_{int(time.time())}"})
            if ok(D, "赠礼下单(root 接收)", st, js):
                expect(D, "赠礼扣买家", b0 - bal(VIP_UID) == price)
                got = (gift.get("config") or {}).get("spark") or 0
                r1 = bal(1)
                expect(D, "赠礼效果到接收方", r1 - r0 == got,
                       f"Δroot={r1-r0} 期望={got}")

    # 免费券进背包
    if voucher:
        price = voucher.get("price") or 0
        b0 = bal(VIP_UID)
        if price <= b0:
            st, js = call("POST", "/api/v1/shop/buy", vip,
                          body={"item_id": voucher["id"], "qty": 1,
                                "idempotency_key": f"deep_vc_{int(time.time())}"})
            if ok(D, "购买免费券", st, js):
                st, js = call("GET", "/api/v1/me/vouchers", vip)
                expect(D, "背包有券", st == 200 and len(rows_of(js)) > 0,
                       f"n={len(rows_of(js))}")

    # 流水/订单/背包三视图
    st, js = call("GET", "/api/v1/me/spark", vip)
    ok(D, "魔力流水", st, js)
    st, js = call("GET", "/api/v1/me/orders", vip)
    ok(D, "我的订单", st, js)


