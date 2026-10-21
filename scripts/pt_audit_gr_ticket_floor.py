# -*- coding: utf-8 -*-
"""闸门 GR：娱乐屋玩法的「票档 = 最低扣费」语义必须一致且有闸。

缘起（批二百三十七）：跑 legacy 脚本 `e2e_arcade_pool_gate.py` 有 6 条红，逐条
归因后 4 条来自脚本自己过期（它按 `win/tie/lose` 造夹具，而写侧白名单是
`any|win|triple|lose` ⇒ 400），1 条是级联假红（抬票档那步被前一条 400 带走），
1 条是站长未定性的 EV 0.951→0.9234。**产品这条链没坏**，但三处执法此前没有
任何常驻判据 ⇒ 本支钉住。

两种扣费口径必须分别验，别拿一把尺量两头：
  * 下注口（猜大小 / 刮刮乐）读客户端 `bet`，执法是 `bet < ticket ⇒ 拒`
    （`casino_bigsmall.rs:50`、`casino.rs:132`）；
  * 池抽口（九宫格 / 扭蛋 / 大转盘）**没有 bet**，直接按池配置里的 `ticket`
    扣（`pool_games.rs` 的 `spend_spark_tx(&mut tx, uid, ticket, …)`）。
    批二百三十八实测：给 jgg 发 `bet` 反而是**未知键**——按旧写法会被静默
    吞掉（见 pt_audit_gs_opt_body.py），所以 GR3 不应对 jgg 发 bet。

活体腿一律用探针成员（不用 root：root 余额被历年闸门直写 SQL 写成负数，
下注必回「余额不足」＝假红；也不许与别的活体闸门并发，见项目记忆）。

判据：
  GR1 三池票档可读且 ≥1；
  GR2 执法在册（下注口的比较式 + 池抽口的按票扣费）；
  GR3 下注口活体反例：票档 ≥2 时 `bet = ticket-1` 必须 400 且点到票档；
      票档 =1（现网就是 1）没有合法的低注值 ⇒ WARN 明说不承重，不假报绿；
  GR4 档区 side 全在写侧白名单内（legacy 脚本栽掉的正是这个漂移）；
  GR4b 正控：同一算式喂 `['win','tie','lose']` 必须抓出 `tie`；
  GR5 下注口一注一扣：`bet = ticket` 打一局 ⇒ 该键扣票流水恰 1 行、额 = bet；
  GR6 池抽口按票扣费：jgg 打一局 ⇒ 扣票流水额恰为 jgg 票档（与 bet 无关）；
  GR7 收尾：探针停用，本轮前缀的扣票流水按账面报数（流水是历史，不删）。

跑法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_gr_ticket_floor.py
"""
import inspect
import os
import re
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, psql_stdin, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUN = int(time.time())
MARK = "gr%d" % RUN
SIDES_OK = ("any", "win", "triple", "lose")
# 每个下注口的载荷形状各不同（GuessReq 要 guess，BetReq 只要 bet），而且两者
# 都挂 deny_unknown_fields ⇒ 多带邻居口的键会被当成「未识别的字段」拒掉，
# 红的是我的夹具而不是产品。逐口给最小载荷。
BET_PORTS = (("bigsmall", "/games/bigsmall", {"guess": "big"}),
             ("scratch", "/games/scratch", {}))


def rd_src(*parts):
    p = os.path.join(ROOTDIR, *parts)
    t = open(p, encoding="utf-8", errors="replace").read()
    return "\n".join(re.sub(r"(?<!:)//.*", "", ln) for ln in t.split("\n"))


def sval(q):
    return psql_stdin(q).strip()


def bal(uid):
    return int(sval("SELECT spark_balance FROM users WHERE id = %d" % uid))


def charge(uid, key):
    """这一注的扣票流水：(行数, 扣掉的火花)。
    只数 `amount < 0` 那一行——派彩行的键是 `<前缀>-win:` 把同一个客户端键包在
    里面（实测 `game-bs-win:game-bs:7259:gr…-fair`），混算就把一注数成两注。
    前缀各口不同名（bigsmall 是 `game-bs`、jgg 是 `game-jgg`），所以按
    「键以 `:<uid>:<客户端键>` 结尾」这条谓词取，行数与金额必须共用它。"""
    pred = "idempotency_key LIKE '%%:%d:%s' AND amount < 0" % (uid, key)
    return (int(sval("SELECT count(*) FROM spark_ledger WHERE " + pred)),
            int(sval("SELECT coalesce(sum(-amount), 0) FROM spark_ledger "
                     "WHERE " + pred)))


tok = login()
print("== GR1 三池票档 ==")
s, r = call("GET", "/games", token=tok)
data = (r or {}).get("data") or {}
pools = {}
for key in ("bigsmall", "scratch", "jgg", "capsule", "wheel"):
    blk = data.get(key) or {}
    pools[key] = blk.get("ticket")
ok("GR1 五玩法票档可读且都是正整数（实测 %s）" % pools,
   s == 200 and all(isinstance(v, int) and v >= 1 for v in pools.values()),
   "http=%s keys=%s" % (s, sorted(data.keys())[:8]))

print("== GR2 执法在册 ==")
bs_src = rd_src("apps", "api", "src", "games_http", "casino_bigsmall.rs")
sc_src = rd_src("apps", "api", "src", "games_http", "casino.rs")
jg_src = rd_src("apps", "api", "src", "games_http", "pool_games.rs")
ok("GR2 猜大小与刮刮乐都有「bet < 票档 ⇒ 拒」的比较",
   re.search(r"body\.bet\s*<\s*\w+\.ticket", bs_src) is not None
   and re.search(r"body\.bet\s*<\s*\w+\.ticket", sc_src) is not None,
   "bs=%s sc=%s" % (bool(re.search(r"bet\s*<\s*\w+\.ticket", bs_src)),
                    bool(re.search(r"bet\s*<\s*\w+\.ticket", sc_src))))
ok("GR2 池抽口按 ticket 扣费（结构上不存在「低于票档」那条缝）",
   "spend_spark_tx(&mut tx, uid, ticket," in jg_src,
   "pool_round 的扣费表达式=%d 处" % jg_src.count("ticket,"))

print("== GR3 下注口活体反例（池抽口不吃 bet，见 GR6）==")
PW = inspect.signature(login).parameters["pw"].default
ROOT = login()
user = "%su" % MARK
call("POST", "/admin/adduser", {"username": user, "password": PW,
                                "email": "%s@gr.local" % MARK}, token=ROOT)
uid = int(sval("SELECT id FROM users WHERE username = '%s'" % user))
t0 = login(user)
call("POST", "/me/password/change", {"old_password": PW,
                                     "new_password": PW + "x"}, token=t0)
mtok = login(user, PW + "x")
# 注资走发放口，不落直写 SQL（见 GS 同处理与 GT5 对账）
sf, rf = call("POST", "/admin/amountbonus",
              {"amount": 9000, "user_id": uid,
               "idempotency_key": "%s-fund" % MARK}, token=ROOT)
ok("GR3a 探针成员可用且注资走发放口（uid=%d 余额=%d http=%s）"
   % (uid, bal(uid), sf), mtok is not None and bal(uid) >= 9000,
   "不用 root：它的余额被历年闸门写成负数，下注必回「余额不足」＝假红；"
   "code=%s msg=%s" % ((rf or {}).get("code"),
                       str((rf or {}).get("message"))[:26]))
tested = 0
for key, path, extra in BET_PORTS:
    tk = pools.get(key)
    if not isinstance(tk, int) or tk < 2:
        print("   WARN GR3 %s 票档=%s，注额没有低于 1 的合法取值 ⇒ 这一腿"
              "此刻不承重（抬档后自动变硬）" % (key, tk))
        continue
    b0 = bal(uid)
    idem = "%s-low-%s" % (MARK, key)
    body = {"bet": tk - 1, "idempotency_key": idem}
    body.update(extra)
    s3, r3 = call("POST", path, body, token=mtok)
    txt = str((r3 or {}).get("message") or "")
    ok("GR3 %s 下注 %d（票档 %d）⇒ 400 且点到票档，余额不变"
       "（实测 %s %s；余额 %d→%d）"
       % (key, tk - 1, tk, s3, txt[:34], b0, bal(uid)),
       s3 == 400 and ("票" in txt or "注额" in txt) and bal(uid) == b0,
       str(r3)[:110])
    tested += 1
if not tested:
    print("   GR3 汇总：两个下注口票档都是 1 ⇒ 本轮只有静态腿承重")

print("== GR4 档区 side 必须在写侧白名单内 ==")
bs_rows = ((data.get("bigsmall") or {}).get("prizes")) or []
bad = [x.get("side") for x in bs_rows if x.get("side") not in SIDES_OK]
ok("GR4 猜大小 %d 档的 side 全在 %s 内（越界=%s）"
   % (len(bs_rows), "|".join(SIDES_OK), bad),
   len(bs_rows) >= 1 and not bad,
   "实测=%s" % [x.get("side") for x in bs_rows])
ctl_bad = [x for x in ["win", "tie", "lose"] if x not in SIDES_OK]
ok("GR4b 正控：算式喂 ['win','tie','lose'] 必须抓出 tie（抓到=%s）" % ctl_bad,
   ctl_bad == ["tie"], "抓不住＝GR4 是恒真判据")

print("== GR5 下注口一注一扣 ==")
tk = pools.get("bigsmall") if isinstance(pools.get("bigsmall"), int) else 1
idem = "%s-fair" % MARK
b0 = bal(uid)
s5, r5 = call("POST", "/games/bigsmall",
              {"bet": tk, "guess": "big", "idempotency_key": idem},
              token=mtok)
cnt, led = charge(uid, idem)
ok("GR5 打一局 code=0、扣票流水恰 1 行、额 = 注额 %d（实扣 %d 行=%d）"
   % (tk, led, cnt),
   (r5 or {}).get("code") == 0 and cnt == 1 and led == tk,
   "余额 %d→%d http=%s" % (b0, bal(uid), s5))

print("== GR6 池抽口按票档扣费（与 bet 无关）==")
ptk = pools.get("jgg") if isinstance(pools.get("jgg"), int) else 0
idem6 = "%s-jgg" % MARK
b6 = bal(uid)
s6, r6 = call("POST", "/games/jgg", {"idempotency_key": idem6}, token=mtok)
cnt6, led6 = charge(uid, idem6)
ok("GR6 jgg 打一局 code=0、扣票流水恰 1 行、额 = jgg 票档 %d（实扣 %d 行=%d）"
   % (ptk, led6, cnt6),
   (r6 or {}).get("code") == 0 and cnt6 == 1 and led6 == ptk,
   "余额 %d→%d http=%s；若额≠票档＝池抽口被改成读客户端注额（低注白嫖那条缝）"
   % (b6, bal(uid), s6))

print("== GR7 收尾 ==")
sval("UPDATE users SET status = 2 WHERE id = %d" % uid)
left = sval("SELECT string_agg(id::text, ',') FROM spark_ledger WHERE "
            "idempotency_key LIKE '%%%s%%'" % MARK)
ok("GR7 探针已停用（status=%s）；本轮前缀 %s 的流水=%s（历史账，不删）"
   % (sval("SELECT status FROM users WHERE id = %d" % uid), MARK, left),
   sval("SELECT status FROM users WHERE id = %d" % uid) == "2",
   "探针余额 %d 随号退役；若日后做清账，按本轮前缀点名" % bal(uid))

summary()
