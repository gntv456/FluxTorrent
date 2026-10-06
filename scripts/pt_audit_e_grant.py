# -*- coding: utf-8 -*-
"""发放面闸门（资深道具发放管理员视角实测）。

覆盖：单发道具（即时/券/背包）、券作废与回收、勋章发放与改期、
批量发放（幂等/流水溯源/试运行）、列表下发字段与等级护栏、发放台账。
用法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_e_grant.py
可 FLUX_API_BASE 覆盖基址。探针号与探针数据在 finally 自清。
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, ok, summary  # noqa: E402
import pt_audit_lib  # noqa: E402

if os.environ.get("FLUX_API_BASE"):
    BASE = os.environ["FLUX_API_BASE"]
    pt_audit_lib.BASE = BASE

PW = "ProbePass123"
ITEM_SPARK = 10      # 赠送魔力 config={"spark":1000}
ITEM_UP1G = 1        # 1GB 上传量
ITEM_VOUCHER = 19    # 免费券
ITEM_RENAME = 14     # 改名卡（背包类，付费）
CREATED = []         # 探针 uid，收尾清理
QUOTA_BACKUP = {}    # item_id -> stock_quota 原值


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-q", "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip())
    return r.stdout.strip()


def login_retry(user="root", pw="password123", tries=8):
    """登录退避：dev 实例连跑多套闸门必吃 429。"""
    last = None
    for i in range(tries):
        s, r = call("POST", "/auth/login", {"username": user, "password": pw})
        if s == 200 and r.get("code") == 0:
            return r["data"]["token"]
        last = (s, r)
        time.sleep(min(2 + i * 3, 20))
    raise SystemExit("login failed for %s: %s" % (user, last))


def new_user(tag, cls=None):
    """建探针号：adduser -> 改密（临时密码禁消费）-> 可选改等级。"""
    name = "gnt_%s_%d" % (tag, int(time.time() * 1000) % 100000)
    s, r = call("POST", "/admin/adduser",
                {"username": name, "email": "%s@example.com" % name,
                 "password": PW}, TOKEN)
    if s != 200 or r.get("code") != 0:
        raise SystemExit("adduser failed %s %s" % (s, r))
    uid = r["data"]["user_id"]
    CREATED.append(uid)
    t2 = login_retry(name, PW)
    s, r = call("POST", "/me/password/change",
                {"old_password": PW, "new_password": "ProbePass456"}, t2)
    if s != 200:
        raise SystemExit("password change failed %s %s" % (s, r))
    if cls is not None:
        psql("UPDATE users SET class_id=%d WHERE id=%d" % (cls, uid))
    return uid, login_retry(name, "ProbePass456")


def bal(uid):
    return int(psql("SELECT spark_balance FROM users WHERE id=%d" % uid))


def uploaded(uid):
    return int(psql("SELECT uploaded FROM users WHERE id=%d" % uid))


def audit_row(action, after_id=0):
    """回读该 action 的最新审计（ref 是 JSONB 现场）。

    after_id 必填基线：不限定「本次新写的行」时，历史遗留行会让断言恒绿
    ——那是不会红的门禁，等于没测。
    """
    row = psql("SELECT ref::text FROM audit_log WHERE action='%s' AND id>%d"
               " ORDER BY id DESC LIMIT 1" % (action, after_id))
    return json.loads(row) if row else None


def audit_max():
    return int(psql("SELECT coalesce(max(id),0) FROM audit_log"))


def sec_a_single_item():
    print("\n--- A 单发道具 ---")
    uid = PROBE["target"][0]
    b0, u0 = bal(uid), uploaded(uid)
    s, r = call("POST", "/admin/users/%d/grant-item/%d" % (uid, ITEM_SPARK),
                {}, TOKEN)
    got = bal(uid) - b0
    ok("A1 发魔力道具必须真入账（+1000）", s == 200 and got == 1000,
       "HTTP %s 实收 %d（SKU 配的是 spark，代码读 amount）" % (s, got))
    d = r.get("data") or {}
    ok("A2 单发响应要报实际入账量", d.get("granted") is not None,
       "响应字段：%s" % sorted(d.keys()))

    s, r = call("POST", "/admin/users/%d/grant-item/%d" % (uid, ITEM_UP1G),
               {}, TOKEN)
    gained = uploaded(uid) - u0
    row = psql("SELECT count(*) FROM traffic_ledger WHERE user_id=%d"
               " AND operator_id IS NOT NULL AND delta_up=%d"
               % (uid, 1024 ** 3))
    ok("A3 上传量道具落账且流水带操作者",
       s == 200 and gained == 1024 ** 3 and int(row) >= 1,
       "HTTP %s 增量 %d 带主流水 %s 条" % (s, gained, row))

    # 即时类 config 缺关键字段：不得「200 但什么都没发」
    psql("UPDATE shop_items SET config='{\"stackable\":true}'::jsonb"
         " WHERE id=%d" % ITEM_SPARK)
    b1 = bal(uid)
    s, r = call("POST", "/admin/users/%d/grant-item/%d" % (uid, ITEM_SPARK),
                {}, TOKEN)
    ok("A4 道具配置缺字段要拒发并说明，不得静默空转",
       s == 400 and bal(uid) == b1,
       "HTTP %s %s" % (s, (r.get("message") or "")[:60]))
    psql("UPDATE shop_items SET config='{\"spark\":1000,\"stackable\":true}'"
         "::jsonb WHERE id=%d" % ITEM_SPARK)

    # 权限：发魔力/发上传量属调账级，只给 staff.panel 的 90~92 不该能发。
    # 注意必须「92 发给 1 级」——92 发给自己是 ensure_outranks 挡的，
    # 那样测不到权限位，会假绿。
    mod, modtok = PROBE["mod"]
    tgt = PROBE["target"][0]
    b2 = bal(tgt)
    s, r = call("POST", "/admin/users/%d/grant-item/%d" % (tgt, ITEM_SPARK),
                {}, modtok)
    ok("A5 版主(92) 无 user.adjust 不得单发魔力道具",
       s in (401, 403) and bal(tgt) == b2,
       "HTTP %s %s 目标余额 +%d" % (s, (r.get("message") or "")[:40],
                                    bal(tgt) - b2))


def sec_b_voucher():
    print("\n--- B 券发放 / 作废 ---")
    uid = PROBE["target"][0]
    psql("DELETE FROM user_vouchers WHERE user_id=%d" % uid)
    QUOTA_BACKUP[ITEM_VOUCHER] = psql(
        "SELECT coalesce(stock_quota::text,'') FROM shop_items WHERE id=%d"
        % ITEM_VOUCHER)
    psql("UPDATE shop_items SET stock_quota=10, stock_used=0 WHERE id=%d"
         % ITEM_VOUCHER)
    for _ in range(2):
        call("POST", "/admin/users/%d/grant-item/%d" % (uid, ITEM_VOUCHER),
             {}, TOKEN)
    used = int(psql("SELECT stock_used FROM shop_items WHERE id=%d"
                    % ITEM_VOUCHER))
    ok("B1 发券占用库存配额", used == 2, "stock_used=%d" % used)

    # 用户自购券（source<>'admin'，花钱买的）必须动不得
    psql("INSERT INTO user_vouchers (user_id, kind, source) VALUES"
         " (%d, 'free', 'shop')" % uid)
    bought = psql("SELECT id FROM user_vouchers WHERE user_id=%d"
                  " AND source='shop'" % uid)
    aud_base = audit_max()
    s, r = call("POST", "/admin/user-vouchers/void",
                {"user_id": uid, "limit": 2}, TOKEN)
    left_admin = int(psql("SELECT count(*) FROM user_vouchers WHERE user_id=%d"
                          " AND source='admin' AND used_at IS NULL" % uid))
    left_shop = int(psql("SELECT count(*) FROM user_vouchers WHERE id=%s"
                         % bought))
    ok("B2 券作废接口可用（当前 prepare 期即失败）", s == 200,
       "HTTP %s %s" % (s, (r.get("message") or "")[:60]))
    ok("B3 作废只收管理员发放券，自购券不得被吞",
       s == 200 and left_shop == 1 and left_admin == 0,
       "自购剩 %d 未核销 admin 券剩 %d" % (left_shop, left_admin))
    used2 = int(psql("SELECT stock_used FROM shop_items WHERE id=%d"
                     % ITEM_VOUCHER))
    ok("B4 作废要退回库存配额", s == 200 and used2 == 0,
       "stock_used=%d（作废 2 张应回 0）" % used2)
    ok("B5 作废必须留审计（限本次新写的行，历史行不算）",
       audit_row("voucher.void", aud_base) is not None,
       "audit_log 无本次 voucher.void")
    psql("DELETE FROM user_vouchers WHERE user_id=%d" % uid)


def sec_c_medal():
    print("\n--- C 勋章发放 / 改期 ---")
    uid = PROBE["target"][0]
    psql("DELETE FROM user_medals WHERE user_id=%d" % uid)
    s, r = call("POST", "/admin/users/%d/medal/2" % uid, {"days": 99999},
                TOKEN)
    n = int(psql("SELECT count(*) FROM user_medals WHERE user_id=%d" % uid))
    ok("C1 days 越界须先拒再写（不得「已按默认期发出 + 回 400」）",
       s == 400 and n == 0, "HTTP %s 已落 %d 行" % (s, n))
    # C1 若漏发成功会污染 C2（ON CONFLICT DO NOTHING 让 days 永远写不进去），
    # 逐条断言之间清行，保证 C2 测的是「首次发放」这条真实路径。
    psql("DELETE FROM user_medals WHERE user_id=%d" % uid)

    s, r = call("POST", "/admin/users/%d/medal/2" % uid, {"days": 30}, TOKEN)
    exp = psql("SELECT (expires_at > now() + interval '25 days' AND"
               " expires_at < now() + interval '35 days')::int"
               " FROM user_medals WHERE user_id=%d AND medal_id=2" % uid)
    ok("C2 指定 days 时有效期按 days", s == 200 and exp == "1",
       "HTTP %s 命中窗口 %s" % (s, exp))

    s, r = call("POST", "/admin/users/%d/medal/2" % uid, {"days": 30}, TOKEN)
    ok("C3 已持有者重复发放须给出可读结果（不能假成功）",
       s == 200 and (r.get("data") or {}).get("already_held") is not None,
       "响应 %s" % json.dumps(r.get("data"), ensure_ascii=False)[:80])

    s, r = call("GET", "/admin/user-medals?uid=%d" % uid, None, TOKEN)
    rows = (r.get("data") or {}).get("rows") or []
    ok("C4 持有列表下发有效期（有改期按钮却看不到到期）",
       bool(rows) and "expires_at" in rows[0],
       "字段 %s" % sorted(rows[0].keys()) if rows else "无行")

    s, r = call("POST", "/admin/users/%d/medal/2/redate" % uid,
                {"days": 36500}, TOKEN)
    ok("C5 改期 days 越界要拒", s == 400, "HTTP %s" % s)


def sec_d_bulk():
    print("\n--- D 批量发放 ---")
    uid = PROBE["target"][0]
    body = {"kind": "spark", "amount": 100, "user_ids": [uid],
            "idempotency_key": "gate-e-bulk-replay-0001"}
    b0 = bal(uid)
    s1, r1 = call("POST", "/admin/increment-bulk", body, TOKEN)
    b1 = bal(uid)
    s2, r2 = call("POST", "/admin/increment-bulk", body, TOKEN)
    b2 = bal(uid)
    ok("D1 批量发放须支持幂等键，同键重放不双发",
       s1 == 200 and b1 - b0 == 100 and b2 - b1 == 0,
       "第一次 %s(+%d) 第二次 %s(+%d)" % (s1, b1 - b0, s2, b2 - b1))

    u0 = uploaded(uid)
    s, r = call("POST", "/admin/increment-bulk",
                {"kind": "uploaded", "amount": 5, "user_ids": [uid]}, TOKEN)
    row = psql("SELECT count(*) FROM traffic_ledger WHERE user_id=%d"
               " AND delta_up=%d AND operator_id IS NOT NULL"
               " AND coalesce(reason,'')<>''" % (uid, 5 * 1024 ** 3))
    ok("D2 批量流量发放流水须带来源与操作者",
       s == 200 and uploaded(uid) - u0 == 5 * 1024 ** 3 and int(row) >= 1,
       "HTTP %s 增量 %d 溯源流水 %s 条"
       % (s, uploaded(uid) - u0, row))

    psql("UPDATE shop_items SET stock_quota=1, stock_used=0 WHERE id=%d"
         % ITEM_RENAME)
    s, r = call("POST", "/admin/increment-bulk",
                {"kind": "item", "amount": 1, "item_id": ITEM_RENAME,
                 "user_ids": [uid, PROBE["mod"][0]]}, TOKEN)
    left = int(psql("SELECT stock_used FROM shop_items WHERE id=%d"
                    % ITEM_RENAME))
    ok("D3 发放失败不得吃掉库存配额（配额泄漏=后续少卖）",
       s == 400 and left == 0, "HTTP %s stock_used=%d" % (s, left))

    s, r = call("POST", "/admin/increment-bulk",
                {"kind": "item", "amount": 1, "item_id": ITEM_RENAME,
                 "user_ids": [uid, PROBE["mod"][0]], "dry_run": True}, TOKEN)
    d = r.get("data") or {}
    ok("D4 试运行须给库存体检（否则「试运行通过」是假绿）",
       s == 200 and d.get("stock_ok") is not None,
       "字段 %s" % sorted(d.keys()))

    s, r = call("POST", "/admin/users/adjust",
                {"user_id": uid, "invite_grant": 100}, TOKEN)
    d = r.get("data") or {}
    codes = int(psql("SELECT count(*) FROM invites WHERE inviter_id=%d" % uid))
    ok("D5 邀请增发不得静默截断后谎报请求值",
       s == 400 or (codes == 50 and d.get("invite_granted") == 50),
       "HTTP %s 回显 %s 实发 %d" % (s, d.get("invite_granted"), codes))
    psql("DELETE FROM invites WHERE inviter_id=%d" % uid)

    # 名单里点了两次同一个人（常见手抖）：受众去重后只能发一份
    b0 = bal(uid)
    s, r = call("POST", "/admin/increment-bulk",
                {"kind": "spark", "amount": 10, "user_ids": [uid, uid]},
                TOKEN)
    ok("D6 受众名单重复用户不双发（targets 去重）",
       s == 200 and bal(uid) - b0 == 10,
       "HTTP %s +%d" % (s, bal(uid) - b0))

    n0 = int(psql("SELECT count(*) FROM grant_batches"))
    call("POST", "/admin/increment-bulk",
         {"kind": "spark", "amount": 10, "user_ids": [uid],
          "dry_run": True}, TOKEN)
    n1 = int(psql("SELECT count(*) FROM grant_batches"))
    ok("D7 试运行不留台账批次（不占位也就不占幂等键）", n1 == n0,
       "grant_batches %d -> %d" % (n0, n1))


def sec_e_reclaim():
    print("\n--- E 回收路径 ---")
    uid = PROBE["target"][0]
    hi = PROBE["super"][0]
    order = psql("INSERT INTO shop_orders (user_id, item_id, price,"
                 " idempotency_key, effect_applied) VALUES (%d, %d, 100000,"
                 " 'gate-e-paid-%d', false) RETURNING id" % (hi, ITEM_RENAME,
                                                             int(time.time())))
    s, r = call("DELETE", "/admin/user-props/%s" % order, None,
                PROBE["hr"][1])
    still = int(psql("SELECT count(*) FROM shop_orders WHERE id=%s" % order))
    ok("E1 付费购买的背包单不得被直接删（那是退款范畴）",
       s in (400, 403) and still == 1, "HTTP %s 行仍在=%d" % (s, still))

    s, r = call("POST", "/admin/users/%d/grant-item/%d" % (uid, ITEM_RENAME),
                {}, TOKEN)
    used1 = int(psql("SELECT stock_used FROM shop_items WHERE id=%d"
                     % ITEM_RENAME))
    s, r = call("GET", "/admin/user-props?uid=%d&per_page=5" % uid, None,
                TOKEN)
    prow = ((r.get("data") or {}).get("rows") or [{}])[0]
    ok("E2a 背包列表须下发 effect_applied（兼本批部署的阳性对照）",
       "effect_applied" in prow, "字段 %s" % sorted(prow.keys()))
    free = psql("SELECT id FROM shop_orders WHERE user_id=%d AND price=0"
                " ORDER BY id DESC LIMIT 1" % uid)
    s, r = call("DELETE", "/admin/user-props/%s" % free, None, TOKEN)
    back = int(psql("SELECT count(*) FROM shop_orders WHERE id=%s" % free))
    used2 = int(psql("SELECT stock_used FROM shop_items WHERE id=%d"
                     % ITEM_RENAME))
    ok("E2 界面回收须与 revoke-item 同校验，并退回库存配额",
       back == 0 and used2 == used1 - 1,
       "HTTP %s 行仍在=%d 配额 %d->%d（应回落到 %d）"
       % (s, back, used1, used2, used1 - 1))
    s, r = call("POST", "/admin/users/%d/revoke-item/999999999" % uid,
                None, TOKEN)
    ok("E3 回收不存在的单要 404 而非静默成功", s == 404, "HTTP %s" % s)

    psql("INSERT INTO user_medals (user_id, medal_id, source) VALUES"
         " (%d, 3, 'admin') ON CONFLICT DO NOTHING" % hi)
    s, r = call("POST", "/admin/user-medals/delete",
                {"user_id": hi, "medal_id": 3}, PROBE["hr"][1])
    held = int(psql("SELECT count(*) FROM user_medals WHERE user_id=%d"
                    " AND medal_id=3" % hi))
    ok("E4 回收勋章须过等级护栏（93 不能动 99 的持有）",
       s in (400, 403) and held == 1, "HTTP %s 仍持有=%d" % (s, held))
    psql("DELETE FROM user_medals WHERE user_id=%d" % hi)


def sec_f_ledger():
    print("\n--- F 发放台账 ---")
    uid = PROBE["target"][0]
    s, r = call("POST", "/admin/increment-bulk",
                {"kind": "spark", "amount": 7, "user_ids": [uid]}, TOKEN)
    batch = (r.get("data") or {}).get("batch_id")
    s, r = call("GET", "/admin/grants?per_page=20", None, TOKEN)
    rows = (r.get("data") or {}).get("rows") or []
    ok("F1 要有可按批次回放的发放台账（现只有 audit 抽样 20 个目标）",
       s == 200 and len(rows) > 0, "HTTP %s 行数 %d" % (s, len(rows)))
    # F2 无条件断言：把空列表也算「缺字段」，不让 if rows 把它变成不会红的门禁
    row = rows[0] if rows else {}
    ok("F2 台账行须含批次/受众/数量/操作者",
       all(k in row for k in ("batch_id", "kind", "amount", "actor_id")),
       "字段 %s" % sorted(row.keys()))
    s, r = call("GET", "/admin/grants?batch=%s" % batch, None, TOKEN)
    d = r.get("data") or {}
    brows = d.get("rows") or []
    tgt = brows[0].get("target_ids") if brows else None
    ok("F3 按 batch_id 能回放出完整受众（漏发核对靠它）",
       s == 200 and tgt is not None, "HTTP %s 受众字段 %s" % (s, tgt))


def cleanup():
    ids = ",".join(str(u) for u in CREATED)
    for uid in CREATED:
        try:
            psql("DELETE FROM shop_orders WHERE user_id=%d" % uid)
            psql("DELETE FROM user_vouchers WHERE user_id=%d" % uid)
            psql("DELETE FROM user_medals WHERE user_id=%d" % uid)
            psql("DELETE FROM invites WHERE inviter_id=%d" % uid)
            psql("DELETE FROM spark_ledger WHERE user_id=%d" % uid)
            psql("DELETE FROM traffic_ledger WHERE user_id=%d" % uid)
            psql("UPDATE users SET spark_balance=0, uploaded=0 WHERE id=%d"
                 % uid)
            # 等级先降回平民：封禁与删除都过 ensure_outranks（操作者须严格
            # 高于目标），root=99 删不掉 99 级的「上级探针」——实测漏下过号。
            psql("UPDATE users SET class_id=1 WHERE id=%d" % uid)
            call("POST", "/admin/users/status", {"user_id": uid, "status": 2},
                 TOKEN)
            call("DELETE", "/admin/users/%d" % uid, None, TOKEN)
        except Exception as e:  # noqa: BLE001
            print("[CLEAN-WARN] uid %s: %s" % (uid, e))
    if ids:
        # 自清范围 = 脚本写过的每一张表（0291 台账也在内）。
        # 注意台账的 actor 是 root（操作者），删探针号的行会漏；
        # 真正的锚点是「受众里含探针号」。
        psql("DELETE FROM grant_batches WHERE target_ids && ARRAY[%s]::bigint[]"
             % ids)
    for iid, q in QUOTA_BACKUP.items():
        val = q if q else "NULL"
        psql("UPDATE shop_items SET stock_quota=%s WHERE id=%d" % (val, iid))
    psql("UPDATE shop_items SET stock_used=0 WHERE id IN (%d,%d,%d)"
         % (ITEM_VOUCHER, ITEM_RENAME, ITEM_SPARK))
    psql("UPDATE shop_items SET config='{\"spark\":1000,\"stackable\":true}'"
         "::jsonb WHERE id=%d" % ITEM_SPARK)
    print("[CLEAN] 探针号 %s 已封禁删除，配额与配置已复原" % CREATED)


def main():
    global TOKEN, PROBE
    TOKEN = login_retry()
    s, r = call("GET", "/me", None, TOKEN)
    if s != 200:
        raise SystemExit("token self-check failed: %s" % s)
    PROBE = {}
    # 建号也放在保护段里：限流让某个号登录失败时，前面已建的号必须被收尾
    # （实测有一次 abort 在 hr 建号上，留下 4 个未删探针号）。
    try:
        PROBE = {
            "target": new_user("target"),
            "mod": new_user("mod", cls=92),
            "hr": new_user("hr", cls=93),
            "super": new_user("super", cls=99),
        }
        print("探针 uid: target=%d mod=92:%d hr=93:%d super=99:%d"
              % (PROBE["target"][0], PROBE["mod"][0], PROBE["hr"][0],
                 PROBE["super"][0]))
        sec_a_single_item()
        sec_b_voucher()
        sec_c_medal()
        sec_d_bulk()
        sec_e_reclaim()
        sec_f_ledger()
    finally:
        cleanup()
    summary()


if __name__ == "__main__":
    main()
