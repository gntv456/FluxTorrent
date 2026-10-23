# -*- coding: utf-8 -*-
"""VIP 深测探针的写侧域（商城购买 / 邀请 / 论坛发帖 / 订阅）。

从 pt_audit_vip_walk.py 拆出（500 行门禁）。这四段都要真写库，跑完由各自的
`pt_audit_` 前缀清理；只读域（浏览/详情/下载/做种/经济/社区/后台）仍留在
入口文件 pt_audit_vip_walk.py，两支共用 pt_audit_vip_walk_base 的工具与
RESULTS 列表——入口文件跑全量时把这里的四个函数并进 DOMAINS。
"""
from pt_audit_vip_walk_base import *  # noqa: F401,F403
import json  # noqa: F401
import os  # noqa: F401
import subprocess  # noqa: F401
import sys
import time  # noqa: F401
import urllib.error  # noqa: F401
import urllib.parse  # noqa: F401
import urllib.request  # noqa: F401

def d_shopbuy(vip: str, root: str) -> None:
    d = "商城购买"
    st, js = call("GET", "/api/v1/shop/items", vip)
    if not ok_envelope(d, "货架 /shop/items", st, js):
        return
    items = js["data"] if isinstance(js["data"],
        list) else js["data"].get("items", [])
    cheap = next((i for i in items if i.get("kind") == "gift_spark"), None)
    if cheap is None:
        expect(d, "找到 gift_spark 测试品", False)
        return

    def bal(tok: str) -> int:
        st, js = call("GET", "/api/v1/me/spark", tok)
        data = js["data"] if isinstance(js["data"], dict) else {}
        return int(data.get("balance", 0))

    # ① 赠礼（0421 新语义）：效果记接收方，不记买家。
    # 幂等键带轮次前缀——同键跨轮复用会被服务端判重放，断言全部失真
    run = str(int(time.time()))
    before_vip, before_root = bal(vip), bal(root)
    after_vip, after_root = before_vip, before_root
    st, js = call("POST", "/api/v1/shop/buy", vip,
                  {"item_id": cheap["id"], "qty": 2,
                   "to": "root", "idempotency_key": f"pt_audit_gift_{run}"})
    if ok_envelope(d, "赠礼 root ×2（qty=2）", st, js):
        after_vip, after_root = bal(vip), bal(root)
        expect(d, "买家扣 2×单价", before_vip - after_vip == 2 * cheap["price"],
               f"{before_vip}→{after_vip}")
        expect(d, "接收方 root +2×面额", after_root - before_root == 2000,
               f"{before_root}→{after_root}")
    # 同键重放：不二次扣款、不二次入账
    st, js = call("POST", "/api/v1/shop/buy", vip,
                  {"item_id": cheap["id"], "qty": 2,
                   "to": "root", "idempotency_key": f"pt_audit_gift_{run}"})
    r2, v2 = bal(root), bal(vip)
    expect(d, "同键重放不双扣/不双送", r2 == after_root and v2 == after_vip,
           f"root={r2} vip={v2}")
    # 负例：缺接收人 / 送自己 / 接收人不存在
    st, _ = call("POST", "/api/v1/shop/buy", vip,
                 {"item_id": cheap["id"], "idempotency_key": f"g2_{run}"})
    expect(d, "缺接收人 400", st == 400, f"st={st}")
    st, _ = call("POST", "/api/v1/shop/buy", vip,
                 {"item_id": cheap["id"], "to": "audit_vip",
                  "idempotency_key": f"g3_{run}"})
    expect(d, "送自己 400", st == 400, f"st={st}")
    st, _ = call("POST", "/api/v1/shop/buy", vip,
                 {"item_id": cheap["id"], "to": "nosuchuser_zz",
                  "idempotency_key": f"g4_{run}"})
    expect(d, "接收人不存在 400", st == 400, f"st={st}")
    # 普通品（非 gift）不受接收人参数影响
    other = next((i for i in items if i.get("kind") == "makeup_card"), None)
    if other:
        st, js = call("POST", "/api/v1/shop/buy", vip,
                      {"item_id": other["id"],
                       "idempotency_key": f"pt_audit_card_{run}"})
        ok_envelope(d, "买补签卡(普通品照常)", st, js)
    # 负例：不存在的品 / 缺幂等键 / 非叠加品买 2 件
    st, _ = call("POST", "/api/v1/shop/buy", vip,
                 {"item_id": 999999, "idempotency_key": f"x_{run}"})
    expect(d, "不存在商品 404", st == 404, f"st={st}")
    st, _ = call("POST", "/api/v1/shop/buy", vip, {"item_id": cheap["id"]})
    expect(d, "缺幂等键 400", st == 400, f"st={st}")
    uniq = next((i for i in items if i.get("kind") in
                 ("avatar_frame", "custom_title", "animated_avatar")), None)
    if uniq:
        st, _ = call("POST", "/api/v1/shop/buy", vip,
                     {"item_id": uniq["id"], "qty": 2,
                         "idempotency_key": f"y_{run}"})
        expect(d, "非叠加品 qty=2 拒", st == 400, f"st={st}")


# ---------------------------------------------------------------- 邀请链路
def d_invite(vip: str, root: str) -> None:
    d = "邀请链路"
    st, js = call("GET", "/api/v1/invites/status", vip)
    if not ok_envelope(d, "邀请状态", st, js):
        return
    st, js = call("POST", "/api/v1/invites", vip,
                  {"email": "pt_audit_invitee@local.test"})
    code = None
    if ok_envelope(d, "申请邀请码", st, js):
        code = js["data"].get("code") or js["data"].get("invite_code")
        expect(d, "拿到邀请码", bool(code), str(js["data"])[:80])
    if code:
        st, js = call("GET", "/api/v1/invites/status", vip)
        if ok_envelope(d, "邀请状态(申请后)", st, js):
            # status 是汇总对象：unused=未使用数（列表在 /invites 页另拉）
            expect(d, "未使用邀请数 +1", js["data"].get("unused", 0) >= 1,
                   str(js["data"])[:120])


# ---------------------------------------------------------------- 论坛写流
def d_forumwrite(vip: str, root: str) -> None:
    d = "论坛写流"
    st, js = call("GET", "/api/v1/torrents?search=pt_audit&limit=1", vip)
    tid = None
    if isinstance(js, dict) and js.get("code") == 0 and js["data"]["items"]:
        tid = js["data"]["items"][0]["id"]
    if tid is None:
        # 回落：任意过审种（并行会话会清掉各自的测试种）
        st, js = call("GET", "/api/v1/torrents?alive=0&limit=1", vip)
        if isinstance(js, dict) and js.get("code") == 0 and js["data"]["items"]:
            tid = js["data"]["items"][0]["id"]
    if tid is None:
        expect(d, "有种子可引用", False)
        return
    st, js = call("GET", "/api/v1/forums", vip)
    fid = 2
    if isinstance(js, dict) and js.get("code") == 0:
        forums = js["data"].get("forums", [])
        cand = next((f for f in forums if f.get("can_create")), None)
        if cand:
            fid = cand["id"]
    body = f"PT 全站深测主题\n\n看看这个种子怎么样：\n\n!torrent({tid})\n\n完事"
    st, js = call("POST", "/api/v1/forums/topics", vip,
                  {"forum_id": fid, "title": "pt_audit_topic 深测主题",
                      "body": body})
    topic_id = None
    if ok_envelope(d, "发主题(带 !torrent 引用)", st, js):
        topic_id = js["data"].get("topic_id")
        expect(d, "topic_id 回值", bool(topic_id))
    if not topic_id:
        return
    # 论坛防刷：回帖距上一条 10s（message_flood 同族），探针按用户节拍走
    time.sleep(11)
    st, js = call("POST", f"/api/v1/forums/topics/{topic_id}/reply", vip,
                  {"body": f"回帖深测：\n\n!torrent({tid})\n\nover"})
    post_id = None
    if ok_envelope(d, "回帖(带 !torrent 引用)", st, js):
        post_id = js["data"].get("post_id")
    # 编辑回帖（PUT）
    if post_id:
        st, js = call("PUT", f"/api/v1/forums/posts/{post_id}", vip,
                      {"body": "编辑后的回帖\n\n/torrent/" + str(tid)})
        ok_envelope(d, "编辑回帖(PUT)", st, js)
    # 读回：正文原样（含语法串）+ 纯文本摘要已压成链接
    st, js = call("GET", f"/api/v1/forums/topics/{topic_id}", vip)
    if ok_envelope(d, "读回主题", st, js):
        posts = js["data"].get("posts", [])
        bodies = [p.get("body", "") for p in posts]
        expect(d, "主题正文含 !torrent 语法",
               any(f"!torrent({tid})" in b for b in bodies))


# ---------------------------------------------------------------- 订阅推送
def d_subscribe(vip: str, root: str) -> None:
    d = "订阅推送"
    # 追更三读口
    for ep in ["/api/v1/me/subscriptions/groups",
               "/api/v1/me/subscriptions/calendar",
               "/api/v1/me/subscriptions/networks"]:
        st, js = call("GET", ep, vip)
        ok_envelope(d, f"GET {ep}", st, js)
    # 组订阅写链：聚合组（torrent_groups）订阅 → 读口可见 → 退订。
    # gid 来源：取一枚入组种子——/torrents/{id}/group 的入参是 **torrent id**
    # （2026-10-22 曾误传 group id）；扫列表前 50 个会被并行会话的探针种
    # 占满 id 头部，故直接库取入组种
    gid = None
    t = psql("SELECT id FROM torrents WHERE group_"
        "id IS NOT NULL ORDER BY id DESC LIMIT 1")
    if t:
        st2, js2 = call("GET", f"/api/v1/torrents/{t}/group", vip)
        gg = (js2.get("data") or {}).get("group") if isinstance(js2,
            dict) else None
        gid = gg["id"] if gg and gg.get("id") else None
    if gid:
        st, js = call("POST", f"/api/v1/torrents/groups/{gid}/subscribe", vip,
            {})
        ok_envelope(d, "订阅字幕组", st, js)
        st, js = call("GET", "/api/v1/me/subscriptions/groups", vip)
        if ok_envelope(d, "我的订阅(订阅后)", st, js):
            rows = js["data"] if isinstance(js["data"],
                list) else js["data"].get("items", [])
            hit = any(r.get("group_id") == gid or r.get("id") == gid
                      for r in rows)
            expect(d, "订阅组在册", hit, f"n={len(rows)}")
        st, js = call("POST", f"/api/v1/torrents/groups/{gid}/unsubscribe", vip,
            {})
        ok_envelope(d, "退订字幕组", st, js)
    else:
        rec(d, "WARN", "站内无聚合组可订阅（数据形态，非缺陷）")
    # 厂牌订阅：订一个 content_network → 读口可见
    st, js = call("GET", "/api/v1/networks?limit=1", vip)
    nid = None
    if isinstance(js, dict) and js.get("code") == 0:
        rows = js["data"] if isinstance(js["data"],
            list) else js["data"].get("items", [])
        if rows:
            nid = rows[0].get("id")
    if nid:
        st, js = call("POST", f"/api/v1/networks/{nid}/subscribe", vip, {})
        ok_envelope(d, "订阅厂牌", st, js)
        st, js = call("GET", "/api/v1/me/subscriptions/networks", vip)
        if ok_envelope(d, "我的厂牌订阅", st, js):
            rows = js["data"] if isinstance(js["data"],
                list) else js["data"].get("items", [])
            hit = any(r.get("network_id") == nid or r.get("id") == nid
                      for r in rows)
            expect(d, "厂牌订阅在册", hit, f"n={len(rows)}")
        st, js = call("POST", f"/api/v1/networks/{nid}/unsubscribe", vip, {})
        ok_envelope(d, "退订厂牌", st, js)
    else:
        expect(d, "有可订阅的厂牌", False)


# ---------------------------------------------------------------- 探针自愈件
