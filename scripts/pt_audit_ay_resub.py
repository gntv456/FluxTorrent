# -*- coding: utf-8 -*-
"""闸门 AY：补签链路与并发形状（批三十三）。

钉三件事（每件先说清「为什么这样断言」）：
  S3 P-93 补签并发双花：resub_uses 此前只有 idempotency_key UNIQUE，而
     「该日已有签到记录」的检查在事务外——同用户同一天两张并发：两个请求都
     通过检查、各锁一单、幂等键各不同 → 一天耗两卡（红基线实测 3 轮全部
     succ=2 / resub_uses=2）。断言：4 并发 × 3 轮，每轮成功恰 1、该日
     resub_uses 恰 1、败者一律 400 业务文案（不许 500）。
  S4/S5 P-95 挑卡 SQL 锁宽了：`FOR UPDATE SKIP LOCKED` 没写 `OF o`，把
     JOIN 进来的 shop_items 参照行一起锁住；并发的第二个人把所有候选订单
     全跳光而拿到「无卡」，于是错走娱乐屋分支，凭空写进 arcade_item_uses
     （红基线 PG 日志：Key (idem)=(resub-arc:4830:20261006) already
     exists，而该成员娱乐屋发放为 0；同日期第二笔直接 500）。账一负，此后
     双源口径（发放−消耗）把新买的卡抵成 0 —— 卡被吞。
     断言：异日 2 并发（有 2 单）两笔都走商店单；只剩 1 单时并发 2 个日期
     只成 1 笔、败者 400『没有可用补签卡』；两种形状下娱乐屋消耗行恒 0，
     且持有数回读 = 发卡数 − 成功补签数。
     机制另在两会话 psql 实证：锁住行后无 OF 的 SELECT 返回空、加 `OF o`
     后返回下一单（见审计文档批三十三）。
  S2 P-94 拒单文案顺序：用完最后一张卡后再点同一天，旧顺序先判持有数，报
     「没有可用补签卡」，用户以为卡被吞；真因是日期已占。断言：0 卡 +
     已补过的日期 → 「该日已有签到记录」。

S1 为链路正控：7 天窗口边界（d-8、d-0 都拒）、无卡拒单且不留痕。
签到主链与首页口径在闸门 AX（pt_audit_ax_attendance.py）。

共享设置：本闸不写任何站点设置（module_attendance 由 AX 负责实测与复原）。
探针产物（3 个成员号 + 出勤/补签/订单/发放消耗/流水）自己收尾。

跑法（仓库根目录）：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_ay_resub.py
"""
import atexit
import os
import re
import subprocess
import sys
import threading
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
CLEAN = []
UIDS = []
SPARK0 = {}
SIGNED_KW = "该日已有签到记录"
NOCARD_KW = "没有可用补签卡"


def q(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip()[:200])
    return r.stdout.strip()


def ex(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode:
        print("   ! SQL 失败: %s" % (r.stderr or "").strip()[:150])
    return r.returncode


def qi(sql, default=0):
    try:
        v = q(sql)
        return int(v) if v else default
    except Exception:  # noqa: BLE001
        return default


def setting(name):
    return q("SELECT COALESCE((SELECT value FROM site_settings WHERE name = "
             "'%s'), '')" % name)


def cleanup():
    """可重复执行：出勤/补签/订单/发放消耗/流水按探针成员号收干净。"""
    for uid in UIDS:
        for t in ("attendance", "resub_uses", "shop_orders",
                  "arcade_item_grants", "arcade_item_uses",
                  "gacha_ticket_ledger", "gacha_ticket_balance",
                  "spark_ledger"):
            ex("DELETE FROM %s WHERE user_id = %d" % (t, uid))
        ex("DELETE FROM messages WHERE receiver_id = %d" % uid)
        if uid in SPARK0:
            ex("UPDATE users SET spark_balance = %d WHERE id = %d"
               % (SPARK0[uid], uid))
        ex("UPDATE users SET status = 2 WHERE id = %d" % uid)
    for stmt in reversed(CLEAN):
        ex(stmt)
    del CLEAN[:]
    print("   [cleanup] 探针成员已墓碑：%s" % (UIDS,))


atexit.register(cleanup)


def brief(x):
    return str(x)[:200]


def site_day(off):
    """站点时区（UTC+8）today 减 off 天，返回 YYYY-MM-DD。"""
    return q("SELECT ((CURRENT_TIMESTAMP + interval '8 hours')::date - %d)"
             "::text" % off)


def rej(name, s, r, code, kw):
    msg = str(r.get("message", "")) if isinstance(r, dict) else str(r)
    return ok(name, s == code and kw in msg, "%s %s %s" % (s, msg, brief(r)))


def new_member(role):
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return None
    code, cid = codes[0]["code"], codes[0]["id"]
    CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None
    user = "ay%s%d" % (role, TAG)
    s, rr = call("POST", "/auth/register", {
        "username": user, "email": "%s@ay.local" % user,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    uid = (rr.get("data") or {}).get("user_id")
    if not uid:
        return None
    UIDS.append(uid)
    tok = None
    for _ in range(8):
        tok = (call("POST", "/auth/login",
                    {"username": user, "password": "password123"})[1].get(
                        "data") or {}).get("token")
        if tok:
            break
        time.sleep(6)
    return (tok, uid) if tok else None


def get_att(tok):
    s, r = call("GET", "/attendance", token=tok)
    return s, (r.get("data") or {})


def give_card(uid, key):
    """发一张商店补签卡（直接落 shop_orders，绕过支付口）。"""
    ex("INSERT INTO shop_orders (user_id, item_id, price, idempotency_key) "
       "VALUES (%d, %d, 1000, '%s')" % (uid, SHOP_ITEM, key))
    return qi("SELECT id FROM shop_orders WHERE idempotency_key = '%s'" % key)


def resub(tok, day, key):
    return call("POST", "/attendance/resub",
                {"target_date": day, "idempotency_key": key}, token=tok)


def race(tok, shots):
    """同一用户并发打出多笔补签：shots=[(target_date, idem), ...]。

    用 Barrier 齐发把请求摁进同一个事务窗口——P-93/P-95 的洞只在并发时打开，
    串行永远复现不了（实测串行重放会被外层检查干净拒掉）。
    """
    out = []
    bar = threading.Barrier(len(shots))

    def one(day, key):
        bar.wait()
        out.append(resub(tok, day, key))

    th = [threading.Thread(target=one, args=(d, k)) for d, k in shots]
    for t in th:
        t.start()
    for t in th:
        t.join()
    return out


print("== S0 地基：探针成员 + 补签商品 ==")
SHOP_ITEM = qi("SELECT id FROM shop_items WHERE kind IN ('makeup_card',"
               "'resub_card') ORDER BY id LIMIT 1")
ok("S0 补签商品存在且 module_attendance=yes",
   SHOP_ITEM > 0 and setting("module_attendance") == "yes",
   "item=%s module=%s" % (SHOP_ITEM, setting("module_attendance")))
C = new_member("c")   # 拒单 + 正路径 + P-94 文案
F = new_member("f")   # 同日并发（P-93）
X = new_member("x")   # 异日并发（P-95）
ok("三个探针成员可准备（注册/登录限流下）", all((C, F, X)))
if not all((C, F, X)):
    sys.exit(1)
CT, CID = C
FT, FID = F
XT, XID = X
for uid in (CID, FID, XID):
    SPARK0[uid] = qi("SELECT spark_balance FROM users WHERE id = %d" % uid)
T0 = site_day(0)
D1 = site_day(1)
D8 = site_day(8)

print("== S1 窗口与持有：越界拒 / 当天拒 / 无卡拒 ==")
rej("S1 补签超 7 天窗口（d-8）拒单", *resub(CT, D8, "ay-j8"),
    400, "只能补过去 7 天内")
rej("S1 补签当天（d-0）拒单", *resub(CT, T0, "ay-j0"),
    400, "只能补过去 7 天内")
rej("S1 无卡补签拒单", *resub(CT, D1, "ay-j1"), 400, NOCARD_KW)
ok("S1 三次拒单均未留痕（补签记录/出勤行为 0）",
   qi("SELECT count(*) FROM resub_uses WHERE user_id = %d" % CID) == 0
   and qi("SELECT count(*) FROM attendance WHERE user_id = %d" % CID) == 0)

print("== S2 补签正路径：耗卡 / 回执 / 重复拒（P-94 文案）==")
ORDER1 = give_card(CID, "ay-c-ord1")
s, r = resub(CT, D1, "ay-ok1")
d = r.get("data") or {}
ok("S2 有卡补签 d-1 成功、回执 resubbed/cards_left=0",
   s == 200 and d.get("resubbed") == D1 and d.get("cards_left") == 0,
   "%s %s" % (s, brief(r)))
ok("S2 消耗锚定商店订单（resub:{order_id}）且出勤行 makeup/0 火花",
   qi("SELECT count(*) FROM resub_uses WHERE user_id = %d AND "
      "target_date = '%s' AND idempotency_key = 'resub:%d'"
      % (CID, D1, ORDER1)) == 1
   and qi("SELECT count(*) FROM attendance WHERE user_id = %d AND date = "
          "'%s' AND makeup AND streak = 0 AND reward = 0" % (CID, D1)) == 1)
rej("S2 P-94 无卡 + 已占日期重放：报『该日已有签到记录』",
    *resub(CT, D1, "ay-ok2"), 400, SIGNED_KW)

print("== S3 P-93 同日并发双花（4 并发 × 3 轮）==")
for rd in range(3):
    DD = site_day(4 + rd)
    for k in range(4):
        give_card(FID, "ay-f-%d-%d" % (rd, k))
    results = race(FT, [(DD, "ay-race-%d-%d" % (rd, i)) for i in range(4)])
    succ = [x for x in results if x[0] == 200]
    losers = [x for x in results if x[0] != 200]
    rus = qi("SELECT count(*) FROM resub_uses WHERE user_id = %d AND "
             "target_date = '%s'" % (FID, DD))
    mk = qi("SELECT count(*) FROM attendance WHERE user_id = %d AND date = "
            "'%s'" % (FID, DD))
    ok("S3 P-93 第%d轮：成功恰 1 / 当日补签记录恰 1 行（%s）" % (rd, DD),
       len(succ) == 1 and rus == 1 and mk == 1,
       "succ=%d rus=%d mk=%d" % (len(succ), rus, mk))
    ok("S3 P-93 第%d轮：败者 3 个全是 400 业务文案（不许 500）" % rd,
       len(losers) == 3 and all(
           st == 400 and (SIGNED_KW in str(bb.get("message"))
                          or NOCARD_KW in str(bb.get("message")))
           for st, bb in losers),
       "losers=%s" % [(st, brief(bb)) for st, bb in losers])
ok("S3 P-93 三轮共耗 3 卡（每轮恰 1）",
   qi("SELECT count(*) FROM resub_uses WHERE user_id = %d" % FID)
   + qi("SELECT count(*) FROM attendance WHERE user_id = %d AND makeup"
        % FID) == 6)
s, fa = get_att(FT)
ok("S3 P-95 F 持有数回读=9（发 12 单耗 3，无凭空扣）",
   fa.get("makeup_cards") == 9, "mc=%s" % fa.get("makeup_cards"))

print("== S4 P-95 异日并发：两卡两日期都应走商店单 ==")
for k in range(2):
    give_card(XID, "ay-x-b%d" % k)
res = race(XT, [(site_day(4), "ay-xb0"), (site_day(5), "ay-xb1")])
codes = [c for c, _ in res]
ok("S4 两张卡补两个不同日期：两笔都成 %s" % codes,
   codes.count(200) == 2, brief(res))
ok("S4 P-95 未凭空扣娱乐屋卡（uses=0 且无 resub:arc: 键）",
   qi("SELECT count(*) FROM arcade_item_uses WHERE user_id = %d" % XID) == 0
   and qi("SELECT count(*) FROM resub_uses WHERE user_id = %d AND "
          "idempotency_key LIKE 'resub:arc:%%'" % XID) == 0, brief(res))

print("== S5 P-95 只剩一张卡时并发两个日期 ==")
give_card(XID, "ay-x-c1")
res = race(XT, [(site_day(6), "ay-xc0"), (site_day(7), "ay-xc1")])
codes = [c for c, _ in res]
losers = [b for c, b in res if c != 200]
ok("S5 一张卡并发两日期：只成 1 笔 %s" % codes,
   codes.count(200) == 1, brief(res))
ok("S5 败者 400『没有可用补签卡』（旧版此处会成功并留下负账）",
   len(losers) == 1 and NOCARD_KW in str(losers[0].get("message")),
   brief(res))
ok("S5 P-95 娱乐屋账未被凭空扣（uses=0）",
   qi("SELECT count(*) FROM arcade_item_uses WHERE user_id = %d" % XID) == 0)
s, xa = get_att(XT)
ok("S5 X 持有数回读=0（发 3 单耗 3、无负账）",
   xa.get("makeup_cards") == 0, "mc=%s" % xa.get("makeup_cards"))

print()
summary()
