# -*- coding: utf-8 -*-
"""闸门 BK：牧场栏位（买/喂/收）与钓鱼（抛竿/起竿/鱼竿/图鉴）的钱与状态闭环。

批四十七的洞是「注释与实现不一致」的两张近照：

  P-183（P1）`ranch_collect` 里写着「行锁：并收只放一发」，但 `SELECT … FOR
        UPDATE OF p` 跑在**连接池的自动提交**上（没有事务）⇒ 锁在 SELECT 返回
        那一刻就释放。并发两发读到同一个 ready_at：`collected_spark` 记两份，
        而魔力受幂等键只入一份 ⇒ 终身额度（价 ×95%）被白吃、台账与实收不等。
  P-184（P2）`ranch_buy` 先扣费再落栏位，两次写不共事务；只有唯一键冲突那一支
        有退款兜底，其余失败（外键、DB 抖）= 钱扣了没牲畜。同域 `fishing.rs`
        的注释早已写死这条纪律（「扣鱼饵与落下这一竿必须同一事务」）。
  P-185（只取证）`ranch_feed` 不扣费，且 `LEAST(ready_at, now()+cycle)` 在真实
        数据下恒等于原值（ready_at 只会 ≤ now+cycle）⇒ 注释说的「喂了就把
        ready_at 往前挪」与界面文案「投喂完成，长得更壮了」都不成立：那颗按钮
        是空操作。修它要先定机制（扣多少、缩短多久），不在本批发明，见文档。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bk_ranch_fishing.py
"""
import json
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
INV = []


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], capture_output=True, text=True,
        encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:56], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


def sval(k):
    return psql("SELECT value FROM site_settings WHERE name = '%s'" % k)


def setv(k, v):
    psql("UPDATE site_settings SET value = '%s' WHERE name = '%s'" % (v, k))


def cnt(t, w=None):
    return psql("SELECT count(*) FROM %s%s"
                % (t, "" if w is None else " WHERE " + w))


def blob(r):
    return json.dumps(r, ensure_ascii=False) if r is not None else ""


def data(r):
    if isinstance(r, tuple):
        r = r[1]
    return (r or {}).get("data") or {}


def msg(r):
    return str((r[1] or {}).get("message") or "")


def post(p, b=None, tok=None):
    return call("POST", p, b, token=tok or ROOT)


def mkprobe(prefix):
    codes = data(post("/admin/invites", {"user_id": 1, "count": 1})
                 ).get("codes") or []
    cap = data(call("GET", "/auth/captcha", token=ROOT))
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(nums) < 2:
        return 0, None, "发码/验证码没准备好"
    INV.append(codes[0]["id"])
    name = "%s%d" % (prefix, TAG * 100 + int(time.time()) % 100)
    rg = post("/auth/register", {
        "username": name, "email": "%s@bk.local" % name,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    tok = data(rg).get("token") or data(
        post("/auth/login", {"username": name,
                             "password": "password123"})).get("token")
    if not tok:
        return 0, None, "注册/登录 http=%s %s" % (rg[0], blob(rg[1])[:90])
    uid = int(psql("SELECT id FROM users WHERE username = '%s'" % name))
    return uid, tok, ""


clear_register_rl()
P1, T1, E1 = mkprobe("bkp1")
P2, T2, E2 = mkprobe("bkp2")
ok("K0a 两个成员探针就绪（P1=%s P2=%s）%s" % (P1, P2, E1 + E2),
   P1 > 0 and P2 > 0 and T1 and T2, E1 or E2)
for u in (P1, P2):
    post("/admin/users/adjust", {"user_id": u, "spark_delta": 4000})
SP0 = dict((u, int(psql("SELECT spark_balance FROM users WHERE id = %d" % u)))
           for u in (P1, P2))
LED0 = int(psql("SELECT coalesce(max(id), 0) FROM spark_ledger"))
PRICE = int(psql("SELECT price FROM arcade_ranch_animals WHERE key = 'chick'"))
YIELD = int(psql("SELECT yield_spark FROM arcade_ranch_animals "
                 "WHERE key = 'chick'"))
ok("K0b 样本牲畜就绪（chick 价 %s、单次产出 %s）" % (PRICE, YIELD),
   PRICE > 0 and YIELD > 0, "price=%s yield=%s" % (PRICE, YIELD))


FARM0 = sval("module_farm")


def pen(uid, col="collected_spark"):
    return psql("SELECT %s FROM arcade_ranch_pens WHERE user_id = %d AND "
                 "animal = 'chick'" % (col, uid))


def ledrows(uid, ref):
    return cnt("spark_ledger",
               "id > %d AND user_id = %d AND ref_type = '%s'"
               % (LED0, uid, ref))


# 牧场的三个写口都在 `module_farm` 闸后（本站现值 %s）：取证窗口内临时
# 开闸，finally 复原并复读 —— 与 BH 开支付通道同一套纪律。
try:
    setv("module_farm", "yes")
    ok("K0c 造现场：module_farm 已临时开成 yes（开局=%s）" % FARM0,
       sval("module_farm") == "yes", "现值=%s" % sval("module_farm"))
    # P-386（仪器账）：模块开关是进程内 30s TTL 缓存（apps/api/src/
    # modules.rs 的 enabled/invalidate），只有走后台保存才会主动失效。
    # 本闸门直接 UPDATE 库后立刻打接口 ⇒ 在「模块关着」的站上会整批被
    # 旧缓存吃成 4101/404，活体判据恒红——红的是判据不是站。等到接口
    # 真的认这个开关为止（等不到位就是产品问题，照红）。
    opened = False
    for _ in range(45):
        sc, rc = post("/farm/ranch/buy", {"animal": "nope"})
        if sc != 404 and "未开放" not in msg((sc, rc)):
            opened = True
            break
        time.sleep(1)
    ok("K0e 改库后接口在 TTL 内认账（等到位才跑活体）", opened,
       "最后一格 http=%s msg=%s" % (sc, msg((sc, rc))[:40]))
    # ============ K1 买入：扣费与落栏必须同事务（P-184） ============
    s0, r0 = post("/farm/ranch/buy", {"animal": "nope"}, T1)
    ok("K1a 买不存在的物种必须 400（不得 500）", s0 == 400 and "没有这种牲畜"
       in msg((s0, r0)), "http=%s msg=%s" % (s0, msg((s0, r0))[:70]))
    res = []
    tk = [threading.Thread(target=lambda: res.append(
        post("/farm/ranch/buy", {"animal": "chick"}, T1)[0])) for _ in range(3)]
    for t in tk:
        t.start()
    for t in tk:
        t.join()
    ok("K1b 并发三发买入（状态码 %s）：只落一只栏位、只扣一次钱（净扣 %s，"
       "扣费流水 %s 笔）" % (res, SP0[P1] - int(psql(
           "SELECT spark_balance FROM users WHERE id = %d" % P1)),
           ledrows(P1, "ranch_buy")),
       cnt("arcade_ranch_pens", "user_id = %d AND animal = 'chick'" % P1) == "1"
       and SP0[P1] - int(psql("SELECT spark_balance FROM users WHERE id = %d"
                               % P1)) == PRICE
       and ledrows(P1, "ranch_buy") == "1",
       "pens=%s delta=%s led=%s" % (
           cnt("arcade_ranch_pens", "user_id = %d AND animal = 'chick'" % P1),
           SP0[P1] - int(psql("SELECT spark_balance FROM users WHERE id = %d"
                               % P1)), ledrows(P1, "ranch_buy")))
    s1, r1 = post("/farm/ranch/buy", {"animal": "chick"}, T1)
    ok("K1c 重复买同一物种必须 400（余额不再动）",
       s1 == 400 and cnt("arcade_ranch_pens",
                         "user_id = %d AND animal = 'chick'" % P1) == "1",
       "http=%s msg=%s" % (s1, msg((s1, r1))[:60]))

    # ============ K2 收集：行锁必须真的在事务里（P-183 核心） ============
    before = int(psql("SELECT spark_balance FROM users WHERE id = %d" % P1))
    res2 = []
    tk2 = [threading.Thread(target=lambda: res2.append(
        post("/farm/ranch/collect", {"animal": "chick"}, T1)))
    for _ in range(2)]
    for t in tk2:
        t.start()
    for t in tk2:
        t.join()
    codes2 = [c for c, _ in res2]
    got = int(psql("SELECT spark_balance FROM users WHERE id = %d"
                  % P1)) - before
    COL = int(pen(P1) or 0)
    ok("K2a 并发两发收集：`collected_spark` 必须等于魔力实收（台账 %s、实收 %s，"
       "状态码 %s）—— 不等就是终身额度被白吃" % (COL, got, codes2),
       COL == got and got in (0, YIELD) and 200 in codes2,
       "collected=%s got=%s codes=%s" % (COL, got, codes2))
    s2, r2 = post("/farm/ranch/collect", {"animal": "chick"}, T1)
    ok("K2b 未到周期再收必须 400（ready_at 已被推后；同一周期不重复付）",
       s2 == 400 and "还没到产出时间" in msg((s2, r2)),
       "http=%s msg=%s" % (s2, msg((s2, r2))[:60]))
    s3, r3 = post("/farm/ranch/collect", {"animal": "chick"}, T2)
    ok("K3c 别人栏里的牲畜收不到（P2 收 P1 的鸡必须 400）",
       s3 == 400 and cnt("arcade_ranch_pens", "user_id = %d" % P2) == "0",
       "http=%s msg=%s" % (s3, msg((s3, r3))[:60]))

        # P-183 的机制证据（不依赖撞上时间窗）：`FOR UPDATE` 只在同一事务里挡人。
    # 另一会话 BEGIN + SELECT … FOR UPDATE + pg_sleep(3) 持锁时，带 FOR UPDATE
    # 的读必须等待；而修前 `ranch_collect` 没有事务 ⇒ 它的 SELECT 一返回锁就没了，
    # 两发都不用等（那正是 K2a 的红：台账双计 80 / 实收 40）。
    LOCKQ = ("BEGIN; SELECT p.user_id FROM arcade_ranch_pens p WHERE "
             "p.user_id = %d AND p.animal = 'chick' FOR UPDATE OF p; "
             "SELECT pg_sleep(3); COMMIT" % P1)
    r2 = subprocess.Popen(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-c", LOCKQ], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True)
    time.sleep(1)
    t0 = time.time()
    blocked = psql("SELECT p.user_id FROM arcade_ranch_pens p WHERE p.user_id"
                   " = %d AND p.animal = 'chick' FOR UPDATE OF p" % P1)
    waited = round(time.time() - t0, 1)
    r2.wait(timeout=40)
    ok("K2c 机制证据：另一会话在事务里持锁时，这条 FOR UPDATE 读必须等"
       "（实测等 %.1fs ≥ 2s，读到=%s）⇒ 锁只在事务内有效，而修前的 collect "
       "没在事务里" % (waited, blocked),
       blocked == str(P1) and waited >= 2.0,
       "waited=%s blocked=%s" % (waited, blocked))

    # ============ K3 投喂是空操作（P-185，只取证不断言） ============
    rd0 = psql("SELECT floor(extract(epoch from ready_at))::text FROM "
               "arcade_ranch_pens WHERE user_id = %d AND animal = 'chick'" % P1)
    w0 = psql("SELECT spark_balance::text FROM users WHERE id = %d" % P1)
    sf, rf = post("/farm/ranch/feed", {"animal": "chick"}, T1)
    rd1 = psql("SELECT floor(extract(epoch from ready_at))::text FROM "
               "arcade_ranch_pens WHERE user_id = %d AND animal = 'chick'" % P1)
    w1 = psql("SELECT spark_balance::text FROM users WHERE id = %d" % P1)
    print("   INFO P-185 现网取证：投喂 http=%s，ready_at %s -> %s，余额 %s -> %s"
          "（都不变 ⇒ 界面那句「投喂完成，长得更壮了」是空反馈；修它要先定"
          "扣费与缩短时长，本批不发明机制）" % (sf, rd0, rd1, w0, w1))
    ok("K3d 投喂不产生任何流水（不扣费、不结算；这条钉的是「现状确实无副作用」，"
       "实装投喂时必须一并更新判据）",
       ledrows(P1, "ranch_feed") == "0" and w0 == w1,
       "led=%s w0=%s w1=%s" % (ledrows(P1, "ranch_feed"), w0, w1))

    # ============ K4 退休上限（造样：直接把 collected 逼近 cap） ============
    cap = PRICE * 95 // 100
    psql("UPDATE arcade_ranch_pens SET collected_spark = %d, ready_at = now() "
         "WHERE user_id = %d AND animal = 'chick'" % (cap, P1))
    s4, r4 = post("/farm/ranch/collect", {"animal": "chick"}, T1)
    ok("K4a 产出满额（回本 95%%）后必须退休并报出来（造的样本：collected=%s、"
       "cap=%s）" % (cap, cap),
       s4 == 400 and "退休" in msg((s4, r4))
       and cnt("arcade_ranch_pens", "user_id = %d AND animal = 'chick'" % P1)
       == "0", "http=%s msg=%s pens=%s" % (s4, msg((s4, r4))[:60],
                                           cnt("arcade_ranch_pens",
                                               "user_id = %d" % P1)))

    # K4b/K4c（P-334）：退休之后必须能「重新领养」——退休那句提示就是这么写的。
    # 买入的幂等键原先是 `ranch-buy:{uid}:{animal}`：退休只删栏位，删不掉
    # spark_ledger 里那笔扣费 ⇒ 同种动物的第二次领养永远被当成重复提交拒掉，
    # 而且报的是误导性的「请勿重复提交」。重复提交本身另有两道更硬的防护
    # （`exists` 预检 + (user_id, animal) 唯一键在事务内冲突即回滚扣费）。
    b1 = int(psql("SELECT spark_balance FROM users WHERE id = %d" % P1))
    s5, r5 = post("/farm/ranch/buy", {"animal": "chick"}, T1)
    # 自诊断：最近一条 ranch-buy 幂等键的形状能直接指出跑的是哪版代码
    lastkey = psql("SELECT coalesce(max(idempotency_key),'-') FROM "
                   "spark_ledger WHERE user_id = %d AND idempotency_key "
                   "LIKE 'ranch-buy%%'" % P1)
    ok("K4b 退休后可以重新领养同一只（http=%s msg=%s 最近键=%s）"
       % (s5, msg((s5, r5))[:36], lastkey[:40]), s5 == 200,
       "http=%s msg=%s 键=%s" % (s5, msg((s5, r5))[:70], lastkey[:56]))
    pens_after = cnt("arcade_ranch_pens",
                     "user_id = %d AND animal = 'chick'" % P1)
    bal_after = int(psql("SELECT spark_balance FROM users WHERE id = %d" % P1))
    ok("K4c 重新领养是一笔真实消费：余额减 %s、栏位回来（%s→%s pens=%s）"
       % (PRICE, b1, bal_after, pens_after),
       s5 == 200 and b1 - bal_after == PRICE and pens_after == "1",
       "http=%s 余额 %s→%s pens=%s" % (s5, b1, bal_after, pens_after))

finally:
    setv("module_farm", FARM0)
    ok("K0d module_farm 已复原并复读一致（%s）" % sval("module_farm"),
       sval("module_farm") == FARM0, sval("module_farm"))

# ============ K5 钓鱼链：抛竿扣费、起竿幂等、图鉴读得通 ============
s5, r5 = post("/games/fishing/cast", {"bet": 1}, T2)
ok("K5a 鱼饵低于票档必须拒（http=%s msg=%s）" % (s5, msg((s5, r5))[:50]),
   s5 == 400, "http=%s msg=%s" % (s5, msg((s5, r5))[:70]))
w2 = int(psql("SELECT spark_balance FROM users WHERE id = %d" % P2))
s6, r6 = post("/games/fishing/cast", {"bet": 100}, T2)
RID = data((s6, r6)).get("round_id")
ok("K6b 抛竿成功并扣 100（round_id=%s，余额 %s->%s，扣费流水 %s 笔）"
   % (RID, w2, psql("SELECT spark_balance::text FROM users WHERE id = %d"
                    % P2), ledrows(P2, "fishing")),
   s6 == 200 and RID and w2 - int(psql(
       "SELECT spark_balance FROM users WHERE id = %d" % P2)) == 100
   and ledrows(P2, "fishing") == "1",
   "http=%s %s" % (s6, blob(r6)[:90]))
if RID:
    bite = int(data((s6, r6)).get("bite_after_ms") or 2000)
    time.sleep(max(0.2, bite / 1000.0 + 0.05))
    s7, r7 = post("/games/fishing/reel", {"round_id": RID}, T2)
    s8, r8 = post("/games/fishing/reel", {"round_id": RID}, T2)
    ok("K7c 起竿后同一竿再收必须 400（注已扣只结一次；首发 http=%s，"
       "won=%s）" % (s7, data((s7, r7)).get("won")),
       s7 == 200 and s8 == 400 and "已收线" in msg((s8, r8)),
       "first=%s second=%s %s" % (s7, s8, blob(r8)[:70]))
    ok("K8d 钓鱼的扣费与派彩各留台账（ref_type=fishing 扣 1 笔；起竿命中时"
       "派彩至多 1 笔）",
       ledrows(P2, "fishing") == "1" and int(cnt(
           "spark_ledger", "id > %d AND user_id = %d AND ref_type = 'fishing'"
           " AND amount > 0" % (LED0, P2))) <= 1,
       "扣=%s 收=%s" % (ledrows(P2, "fishing"), cnt(
           "spark_ledger", "id > %d AND user_id = %d AND ref_type = "
           "'fishing' AND amount > 0" % (LED0, P2))))
sc, rc = call("GET", "/games/fishing/collection", token=T2)
coll = data((sc, rc)) or {}
ok("K9e 渔获图鉴读得通且与奖池档位同源（total=%s got=%s）"
   % (coll.get("total"), coll.get("got")),
   sc == 200 and int(coll.get("total") or 0) > 0
   and int(coll.get("got") or 0) >= 0, blob(rc)[:90])
srod, rrod = post("/games/fishing/rod/upgrade", None, T2)
ok("K10f 鱼竿升级是纯 sink：成功则扣费 + 等级 +1，且同一目标等级不重复扣"
   "（http=%s level=%s spent=%s）"
   % (srod, data((srod, rrod)).get("level"), data((srod, rrod)).get("spent")),
   srod == 200 and int(data((srod, rrod)).get("level") or 0) == 2,
   "http=%s %s" % (srod, blob(rrod)[:90]))

# ============ K9 收尾 ============
psql("DELETE FROM arcade_ranch_pens WHERE user_id IN (%d, %d)" % (P1, P2))
psql("DELETE FROM arcade_fishing_rounds WHERE user_id IN (%d, %d)" % (P1, P2))
psql("DELETE FROM arcade_fishing_rods WHERE user_id IN (%d, %d)" % (P1, P2))
psql("DELETE FROM arcade_pool_rounds WHERE user_id IN (%d, %d)" % (P1, P2))
psql("DELETE FROM spark_ledger WHERE id > %d AND user_id IN (%d, %d)"
     % (LED0, P1, P2))
for u, v in SP0.items():
    psql("UPDATE users SET spark_balance = %d WHERE id = %d" % (v, u))
END = dict((u, psql("SELECT spark_balance::text FROM users WHERE id = %d"
                     % u)) for u in SP0)
ok("K11b 两个探针余额逐字回基线（%s vs %s）" % (SP0, END),
   all(int(END[u]) == SP0[u] for u in SP0), blob(END)[:100])
for u in (P1, P2):
    post("/admin/users/status", {"user_id": u, "status": 2})
    call("DELETE", "/admin/users/%d" % u, None, token=ROOT)
    psql("DELETE FROM spark_ledger WHERE user_id = %d" % u)
    psql("DELETE FROM traffic_ledger WHERE user_id = %d" % u)
if INV:
    psql("UPDATE invites SET status = 2 WHERE used_by IS NULL AND id IN (%s)"
         % ",".join(str(int(i)) for i in INV))
ok("K11a 本轮栏位/钓局/流水已清空（pens/rounds/led 全为 0）",
   cnt("arcade_ranch_pens", "user_id IN (%d, %d)" % (P1, P2)) == "0"
   and cnt("arcade_fishing_rounds", "user_id IN (%d, %d)"
           % (P1, P2)) == "0",
   "pens=%s rounds=%s" % (cnt("arcade_ranch_pens",
                             "user_id IN (%d, %d)" % (P1, P2)),
       cnt("arcade_fishing_rounds",
           "user_id IN (%d, %d)" % (P1, P2))))
TB = psql("SELECT count(*) FROM users WHERE id IN (%d, %d) AND status >= 2 "
          "AND passkey LIKE 'deleted-%%'" % (P1, P2))
ok("K11c 两个探针都走产品自己的墓碑（%s/2；余额被 user_purge 冲平属收尾留痕，"
   "不是我的账）" % TB, TB == "2", "tomb=%s" % TB)
summary()
