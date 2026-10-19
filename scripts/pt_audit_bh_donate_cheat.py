# -*- coding: utf-8 -*-
"""闸门 BH：钱进钱包的两把门、作弊处置的护栏、后台订单台账的检索。

批四十四的选题来自「同一条判据在第二个入口失踪」这个病形：捐赠入账有两个入口
（网关回调、站长手工补单），金额形状只在补单口注释里写着「以 max(实收, 0) 为准」，
代码里没有那行 max；处置类动作同族 —— 赦免三口在批四十三补了等级/本人护栏，
`/admin/cheat-events/resolve`（恢复做种收益）这一口还只判「是不是职员」。

  P-173（P0）入账金额无形状校验。补单口 `amount_paid` 由站长**手填** f64，
        负数＝凭空扣款、1e9＝凭空铸币，`donation_ledger` 与累计捐赠跟着虚增；
        回调口同样把网关声明的 `money` 解析完直接加进 wallet_usd。两处共用
        一把新尺 `payment::check_paid_amount`（有限 / >0 / ≤100 万）。
  P-175（P1）作弊事件处置无等级/本人护栏，也没有处置说明：class ≥ 90 的职员可
        清掉**自己**名下被 tracker 判定的 ghost/speed/reset 事件（即恢复做种
        收益），也能清上级；审计只有 action 名，没有 reason、没有「处置了谁」。
  P-176（P1）后台商城订单检索手拼 LIKE：搜索框里一个百分号＝全表通配、一个
        下划线＝单字符通配；`page` 只 max(1) 不封顶，OFFSET 可拼到 50 亿。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bh_donate_cheat.py
"""
import hashlib
import json
import os
import re
import subprocess
import sys
import time
import urllib.parse, urllib.request, urllib.error

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
INV, CEID = [], []   # 本轮发的邀请码 id / 合成的 cheat_events id

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

def rq(method, path, body=None, tok=None):
    """原始 body / 纯文本回执用：1e400 这类字面量 json.dumps 会变 inf。"""
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if tok:
        req.add_header("Authorization", "Bearer " + tok)
    if isinstance(body, str):
        data_b = body.encode("utf-8")
    else:
        data_b = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data_b, timeout=30) as rp:
            return rp.status, rp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")

# ---- 探针：92 职员（处置者样本）、95 上级（92 处置不得）、1 成员 ----
def mkprobe(prefix, cls):
    # #75（批一百五十三）：24h 同 IP 上限是 2，数的是 reg_ip_signups 表。
    # 本闸门要连建多个探针号，所以**每个号建之前**清一次额度——
    # 只在开头清一次只够前两个，后两个必撞上限，红在「探针就绪」。
    clear_register_rl()
    codes = data(post("/admin/invites", {"user_id": 1, "count": 1})
                 ).get("codes") or []
    cap = data(call("GET", "/auth/captcha", token=ROOT))
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(nums) < 2:
        return 0, None, "发码/验证码没准备好"
    INV.append(codes[0]["id"])
    name = "%s%d" % (prefix, TAG * 100 + int(time.time()) % 100)
    rg = post("/auth/register", {
        "username": name, "email": "%s@bh.local" % name,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    tok = data(rg).get("token") or data(
        post("/auth/login", {"username": name,
                             "password": "password123"})).get("token")
    if not tok:
        return 0, None, "注册/登录 http=%s %s" % (rg[0], blob(rg[1])[:100])
    uid = int(psql("SELECT id FROM users WHERE username = '%s'" % name))
    if cls:
        s, r = post("/admin/users/class", {"user_id": uid, "class_id": cls})
        if s != 200:
            return uid, tok, "提档 %s 失败 http=%s" % (cls, s)
        tok = data(post("/auth/login", {"username": name,
                                       "password": "password123"})
                   ).get("token") or tok
    return uid, tok, ""

clear_register_rl()
S92, T92, E1 = mkprobe("bh92", 92)
S95, T95, E2 = mkprobe("bh95", 95)
MV, MTV, E3 = mkprobe("bhmv", None)
ok("H0a 职员探针就绪（uid=%s class=%s）%s"
   % (S92, psql("SELECT class_id::text FROM users WHERE id = %d" % S92), E1),
   S92 > 0 and T92, E1 or "职员探针没就绪 ⇒ H1/H3 不承重")
ok("H0b 上级探针就绪（uid=%s class=%s）%s"
   % (S95, psql("SELECT class_id::text FROM users WHERE id = %d" % S95), E2),
   S95 > 0 and T95, E2 or "上级样本没就绪 ⇒ H3b 不承重")
ok("H0c 成员探针就绪（uid=%s class=%s）%s"
   % (MV, psql("SELECT class_id::text FROM users WHERE id = %d" % MV), E3),
   MV > 0 and MTV, E3 or "成员样本没就绪 ⇒ 越权负控不承重")

# 本轮流水起点：只按 (id > 起点) AND user_id = 探针 清自己造的账
L0 = int(psql("SELECT coalesce(max(id), 0) FROM donation_ledger"))
M0 = int(psql("SELECT coalesce(max(id), 0) FROM user_modify_logs"))
A0 = int(psql("SELECT count(*) FROM audit_log"))

def mkorder(uid, usd, status="pending", tag=""):
    ono = "BH%d%s" % (TAG, tag)
    psql("INSERT INTO payment_orders (order_no, user_id, amount_usd, channel, "
         "status) VALUES ('%s', %d, %s, 'epay', '%s')"
         % (ono, uid, usd, status))
    return ono

def wof(uid):
    return psql("SELECT wallet_usd::text FROM users WHERE id = %d" % uid)

def stof(ono):
    return psql("SELECT status FROM payment_orders WHERE order_no = '%s'" % ono)

def complete(ono, amt=None, tok=None, raw=None):
    if raw is not None:
        return rq("POST", "/admin/payment-orders/complete", raw, tok or ROOT)
    b = {"order_no": ono}
    if amt is not None:
        b["amount_paid"] = amt
    return post("/admin/payment-orders/complete", b, tok)

FACE = "5.00"          # 合成订单面额（美元）
W0 = wof(MV)


def led(o):
    """该补单号写了几条流水（用 ASCII 尾巴匹配，不把中文放进 SQL 条件里）。"""
    return cnt("donation_ledger", "note LIKE '%%order_no=%s'" % o)


# 每个金额形状用例各用一张新的 pending 订单。上一版四个用例共用一张单，第一笔
# （负数）在现网真把单置了 paid，后面三条全读成 duplicate —— 红账里混进一片
# 「被上一条带崩的级联假红」，分不清哪条是洞。
for tg, amt, kw in (("A", -12.5, "正数"), ("B", 0.0, "正数"),
                    ("C", 1000001.0, "上限"), ("D", 20.0, "不得高于订单面额")):
    o = mkorder(MV, FACE, tag=tg)
    s, r = complete(o, amt)
    ok("H1%s 面额 %s 的订单填实付 %s：必须 400 并报「%s」，订单留在 pending、"
       "钱包不动、流水不写" % (tg, FACE, amt, kw),
       s == 400 and kw in msg((s, r)) and stof(o) == "pending"
       and wof(MV) == W0 and led(o) == "0",
       "http=%s msg=%s status=%s wallet=%s led=%s"
       % (s, msg((s, r))[:48], stof(o), wof(MV), led(o)))

OQ = mkorder(MV, FACE, tag="E")
s, r = complete(OQ, None, raw='{"order_no":"%s","amount_paid":1e400}' % OQ)
ok("H1e 溢出成无穷的字面量金额只能被拒在门口：不得 5xx，订单留 pending、"
   "账不动（这一支在 JSON 反序列化层就挡住了；回调侧 money 是字符串，见 H2i）",
   400 <= s < 500 and stof(OQ) == "pending" and wof(MV) == W0,
   "http=%s body=%s status=%s" % (s, blob(r)[:60], stof(OQ)))
s, r = complete(OQ, None, tok=MTV)
ok("H1f 成员打不开补单口（403，perm user.adjust 的门槛还在）", s == 403,
   "http=%s %s" % (s, blob(r)[:60]))
s, r = complete("BH_NO_SUCH_ORDER")
ok("H1g 订单号不存在必须 404（不得静默建账）", s == 404,
   "http=%s %s" % (s, blob(r)[:60]))
ORX = mkorder(MV, FACE, tag="X", status="cancelled")
s, r = complete(ORX)
ok("H1h 非 pending 的订单补单必须拒并把状态报出来",
   s == 400 and "仅 pending" in msg((s, r)),
   "http=%s msg=%s" % (s, msg((s, r))[:70]))
OF = mkorder(MV, FACE, tag="F")
s, r = complete(OF)
ok("H1i 正例：不填金额按面额入账（http=%s amount=%s 钱包 %s->%s）"
   % (s, data((s, r)).get("amount"), W0, wof(MV)),
   s == 200 and abs(float(data((s, r)).get("amount") or 0) - 5.0) < 1e-9
   and stof(OF) == "paid"
   and abs(float(wof(MV)) - float(W0) - 5.0) < 1e-9,
   "http=%s %s wallet=%s" % (s, blob(r)[:90], wof(MV)))
W1 = wof(MV)
ok("H1j 入账必留台账：该单流水恰 1 条、金额 5.00、balance_after 与钱包一致",
   led(OF) == "1"
   and psql("SELECT amount_usd::text FROM donation_ledger WHERE note LIKE "
             "'%%order_no=%s'" % OF) == "5.00"
   and psql("SELECT balance_after::text FROM donation_ledger WHERE note LIKE "
             "'%%order_no=%s'" % OF) == W1, "led=%s W1=%s" % (led(OF), W1))
s, r = complete(OF)
ok("H1k 重复补单幂等（result=duplicate，钱包不再动）",
   s == 200 and data((s, r)).get("result") == "duplicate" and wof(MV) == W1,
   "http=%s %s wallet=%s" % (s, blob(r)[:70], wof(MV)))

# ============ H2 网关回调的同一把尺（P-173 第二入口） ============
SNAP = dict((k, sval(k)) for k in ("payment_provider", "payment_pid",
                                   "payment_key", "payment_gateway_url"))
LONGKEY = "bhgate%s0123456789" % TAG

def epay_sign(params, key):
    items = sorted((k, v) for k, v in params.items()
                   if k not in ("sign", "sign_type") and v != "")
    raw = "&".join("%s=%s" % (k, v) for k, v in items)
    return hashlib.md5((raw + key).encode("utf-8")).hexdigest()

def notify(ono, money, key, trade_no, bad=False, status="TRADE_SUCCESS"):
    p = {"pid": "bhgate", "out_trade_no": ono, "trade_no": trade_no,
         "money": money, "trade_status": status, "sign_type": "MD5"}
    p["sign"] = "deadbeef" if bad else epay_sign(p, key)
    return rq("GET", "/donate/notify?" + urllib.parse.urlencode(p))

def nled(tn):
    return cnt("donation_ledger", "note LIKE '%%trade_no=%s'" % tn)

try:
    # 只改 provider/pid/key 三项：gateway_url 现值 'none' 已经非空，够开闸
    setv("payment_provider", "epay")
    setv("payment_pid", "bhgate")
    ORD2 = mkorder(MV, FACE, tag="N1")
    w = wof(MV)
    sa, ba = notify(ORD2, "-1.00", SNAP["payment_key"], "BHTNa")
    ok("H2a 弱密钥闸在线：本站 payment_key 现值 %d 位（< 16），通道按设计算"
       "未配置，合法签名的负额回调也一律拒 —— 这条同时证明「通道平时是关"
       "着的」" % len(SNAP["payment_key"]),
       ba == "fail" and stof(ORD2) == "pending" and wof(MV) == w
       and nled("BHTNa") == "0",
       "回执=%s status=%s wallet=%s" % (ba[:10], stof(ORD2), wof(MV)))
    setv("payment_key", LONGKEY)
    ok("H2b 造现场：把通道开成可用（provider/pid/key 三项复读一致，密钥 %d 位）"
       % len(LONGKEY),
       sval("payment_provider") == "epay" and sval("payment_pid") == "bhgate"
       and sval("payment_key") == LONGKEY and len(LONGKEY) >= 16,
       "provider=%s pid=%s keylen=%s" % (sval("payment_provider"),
                                         sval("payment_pid"),
                                         len(sval("payment_key"))))
    # 每个金额形状一支新单：上一版共用一张，第一笔在现网真入了账，后面全读成
    # Duplicate，红账分不清哪条是洞
    for tg, money, tn in (("c", "-1.00", "BHTNneg"), ("d", "0.00", "BHTNzero"),
                          ("e", "1000001.00", "BHTNbig"),
                          ("i", "1e400", "BHTNinf")):
        o = mkorder(MV, FACE, tag="N" + tg)
        w = wof(MV)
        s, b = notify(o, money, LONGKEY, tn)
        ok("H2%s 合法签名 + money=%s 不得入账（回执 fail、订单留 pending、"
           "钱包不动、不写流水）" % (tg, money),
           b == "fail" and stof(o) == "pending" and wof(MV) == w
           and nled(tn) == "0",
           "回执=%s status=%s %s->%s led=%s"
           % (b[:10], stof(o), w, wof(MV), nled(tn)))
    OG2 = mkorder(MV, FACE, tag="Ng")
    w = wof(MV)
    s, b = notify(OG2, "5.00", LONGKEY, "BHTNok")
    ok("H2f 正例：合法签名 + 正常金额照常入账（回执 success、订单 paid、"
       "钱包 +5、流水恰 1 条）",
       b == "success" and stof(OG2) == "paid"
       and abs(float(wof(MV)) - float(w) - 5.0) < 1e-9
       and nled("BHTNok") == "1",
       "回执=%s status=%s %s->%s led=%s"
       % (b[:10], stof(OG2), w, wof(MV), nled("BHTNok")))
    W3 = wof(MV)
    s, b = notify(OG2, "5.00", LONGKEY, "BHTNok2")
    ok("H2g 重复回调幂等（回执仍是 success，钱只入这一次：钱包 %s 不动）" % W3,
       b == "success" and wof(MV) == W3 and nled("BHTNok2") == "0",
       "回执=%s wallet=%s led=%s" % (b[:10], wof(MV), nled("BHTNok2")))
    OB2 = mkorder(MV, FACE, tag="Nb")
    w = wof(MV)
    s, b = notify(OB2, "5.00", LONGKEY + "x", "BHTNbad")
    ok("H2h 负控：验签不过的回调照样不入账（加了形尺没把签名闸带松）",
       b == "fail" and stof(OB2) == "pending" and wof(MV) == w,
       "回执=%s status=%s wallet=%s" % (b[:10], stof(OB2), wof(MV)))
finally:
    for k, v in SNAP.items():
        setv(k, v)
    back = dict((k, sval(k)) for k in SNAP)
    ok("H2j 支付设置四项已复原并逐项复读一致（不留半开通道）",
       back == SNAP, blob(back)[:120])

# ============ H3 作弊事件处置（P-175） ============
AG = "BHagent%d" % TAG

def mkev(uid, seq=None):
    # cheat_events 有 (user_id, agent, reason) 唯一键：agent 必须逐条带序号，
    # 否则同一探针的第二条事件会被唯一键吃掉（上一版正是在这里崩掉，连带
    # 收尾整段没跑，把铸出来的钱包留在了现网）
    ag = "%s_%s" % (AG, seq if seq is not None else len(CEID))
    i = psql("INSERT INTO cheat_events (user_id, agent, peer_ip, reason, "
             "hits) VALUES (%d, '%s', '203.0.113.7', 'ghost', 1) "
             "RETURNING id" % (uid, ag))
    if not str(i).isdigit():
        raise SystemExit("合成 cheat_events 失败：%s" % str(i)[:80])
    CEID.append(int(i))
    return i

def resolve(eid, res=True, note=None, tok=None):
    b = {"id": int(eid), "resolved": res}
    if note is not None:
        b["note"] = note
    return post("/admin/cheat-events/resolve", b, tok)

def resof(eid):
    return psql("SELECT coalesce(resolved_at::text, 'NULL') FROM cheat_events"
                " WHERE id = %s" % eid)

E92, E95 = mkev(S92), mkev(S95)
sa, ra = resolve(E92, tok=T92)
ok("H3a 职员处置**自己**名下的作弊事件必须 403（恢复做种收益不能自审自批）",
   sa == 403 and resof(E92) == "NULL",
   "http=%s resolved_at=%s %s" % (sa, resof(E92), blob(ra)[:60]))
sb, rb = resolve(E95, tok=T92)
ok("H3b 职员处置上级（class 95）的事件必须 403",
   sb == 403 and resof(E95) == "NULL",
   "http=%s resolved_at=%s" % (sb, resof(E95)))
sc, rc = resolve(E92, res=False, tok=T92)
ok("H3c 撤销处置（resolved=false）同受护栏：职员不得给自己翻案",
   sc == 403, "http=%s resolved_at=%s" % (sc, resof(E92)))
sd, rd = resolve(E92, tok=MTV)
ok("H3d 成员根本进不了这个口（403，require_staff 门槛还在）",
   sd == 403, "http=%s %s" % (sd, blob(rd)[:60]))
A1 = int(psql("SELECT count(*) FROM audit_log WHERE action = "
              "'cheat_event.resolve'"))
L1 = int(psql("SELECT count(*) FROM user_modify_logs WHERE uid = %d" % S92))
s, r = resolve(E92, note="复核 announce 日志，判定为误报")
AR = psql("SELECT ref FROM audit_log WHERE action = 'cheat_event.resolve' "
          "ORDER BY id DESC LIMIT 1")
ok("H3e 正例：站长处置下级成功，且审计里同时有「为什么」和「处置了谁」",
   s == 200 and resof(E92) != "NULL"
   and json.loads(AR).get("reason") == "复核 announce 日志，判定为误报"
   and str(json.loads(AR).get("target_user")) == str(S92),
   "http=%s ref=%s" % (s, AR[:120]))
MLQ = "SELECT content FROM user_modify_logs WHERE uid=%d" \
      " ORDER BY id DESC LIMIT 1"
ML = psql(MLQ % S92)
ok("H3f 被处置的人的台账里多一条可读摘要（含事件号与说明）",
   ("#%s" % E92) in ML and "复核 announce 日志" in ML, ML[:100])
A2 = int(psql("SELECT count(*) FROM audit_log WHERE action = "
              "'cheat_event.resolve'"))
L2 = int(psql("SELECT count(*) FROM user_modify_logs WHERE uid = %d" % S92))
ok("H3g 处置一次 = 审计一条 + 该用户台账一条，既不双写也不漏写"
   "（audit %s->%s / modify %s->%s）" % (A1, A2, L1, L2),
   A2 == A1 + 1 and L2 == L1 + 1, "audit=%s modify=%s" % (A2, L2))
E95B = mkev(S95)      # H3h 用新单：不复用 H3b 那张（未修版已把它置成已处置）
s, r = resolve(E95B, note="上级违规复核通过")
ok("H3h 正控：站长按同样口径处置下级（95 < 99）应成功 —— 护栏没把整条线关掉",
   s == 200 and resof(E95B) != "NULL", "http=%s resolved_at=%s"
   % (s, resof(E95B)))
s, r = resolve(999999999)
ok("H3i 不存在的事件号必须 404（原先与「状态未变化」共用一句 400）",
   s == 404, "http=%s %s" % (s, blob(r)[:80]))
s, r = resolve(E92, res=False)
MJ = psql(MLQ % S92)
ok("H3j 不填说明仍可处置（本轮没把 note 做成必填，界面字段另列待拍板），"
   "但台账不得留白", s == 200 and "未附说明" in MJ,
   "http=%s 最新台账=%s" % (s, MJ[:80]))
# ============ H4 后台商城订单检索（P-176） ============
def tot(qs):
    """取台账接口的 total。上一版写 `get("total") or -1`，把合法的 0 读成 -1
    （数字判据不许用 `x or 默认值`），三条 H4 因此在修复后假红。"""
    d = data(call("GET", "/admin/shop-orders" + qs, token=ROOT)) or {}
    v = d.get("total")
    return -999 if v is None else int(v)


J = "shop_orders o JOIN users u ON u.id = o.user_id"
TOT = cnt("shop_orders")
BASE_ROW = data(call("GET", "/admin/shop-orders", token=ROOT)) or {}
ok("H4a 基数就绪：库中商城订单 %s 条、接口不带筛选也回 %s 条"
   % (TOT, BASE_ROW.get("total")),
   int(TOT) > 0 and str(BASE_ROW.get("total")) == str(TOT), blob(BASE_ROW)[:90])
DBP = cnt(J, "u.username ILIKE '%\\%%'")
t_pct = tot("?q=%25")
ok("H4b 搜索框里的百分号是字面量不是通配（接口命中 " + str(t_pct) +
   "，库里用户名真含百分号的订单 " + str(DBP) + "，全表基数 " + str(TOT) +
   "）", t_pct == int(DBP), "api=%s db=%s tot=%s" % (t_pct, DBP, TOT))
DBU = cnt(J, "u.username ILIKE '%\\_%'")
t_us = tot("?q=%5F")
ok("H4c 下划线同理不得当单字符通配（接口命中 " + str(t_us) +
   "，库里真含下划线者 " + str(DBU) + "）", t_us == int(DBU),
   "api=%s db=%s tot=%s" % (t_us, DBU, TOT))
pg = data(call("GET", "/admin/shop-orders?page=99999999", token=ROOT)) or {}
ok("H4d 页码必须有上界（回显 page=%s，OFFSET 由它拼出）" % pg.get("page"),
   1 <= int(pg.get("page") or 0) <= 100000, "page=%s" % pg.get("page"))
pp = data(call("GET", "/admin/shop-orders?per_page=9999", token=ROOT)) or {}
ok("H4e 每页条数封顶 50（回显 per_page=%s）" % pp.get("per_page"),
   1 <= int(pp.get("per_page") or 0) <= 50, "per_page=%s" % pp.get("per_page"))
UN = psql("SELECT u.username FROM shop_orders o JOIN users u ON u.id = "
          "o.user_id ORDER BY o.id DESC LIMIT 1")
DBH = cnt(J, "u.username = '%s'" % UN)
hit = data(call("GET", "/admin/shop-orders?q=%s" % urllib.parse.quote(UN),
                token=ROOT)) or {}
own = set(str(r.get("username")) for r in (hit.get("rows") or []))
ok("H4f 正例：按真实用户名（%s）精确搜，命中数与库里同名订单数一致（接口 %s "
   "/ 库 %s）" % (UN, hit.get("total"), DBH),
   str(hit.get("total")) == DBH and int(DBH) >= 1
   and (not own or own == set([UN])), blob(hit)[:110])
wid = tot("?q=%s%%25" % urllib.parse.quote(UN))
DBW = cnt(J, "u.username = '%s%%'" % UN)
ok("H4g 真用户名后跟一个百分号：转义后是字面量匹配（接口命中 " + str(wid) +
   "，库里「用户名正好是 " + UN + "%」的订单 " + str(DBW) + " 条，而单搜 "
   + UN + " 有 " + str(DBH) + " 条）—— 通配不再能放大结果集",
   wid == int(DBW), "api=%s db=%s plain=%s" % (wid, DBW, DBH))
sh = call("GET", "/admin/shop-orders", token=MTV)
ok("H4h 成员打不开后台订单台账（403）", sh[0] == 403,
   "http=%s %s" % (sh[0], blob(sh[1])[:60]))
# ============ H5 审计链自检：只钉不承重的形状（P-174 在制品） ============
s, r = call("GET", "/admin/audit/chain-verify", token=ROOT)
cv = data((s, r)) or {}
AT = int(cnt("audit_log"))
ok("H5a 自检接口可用且 total 与库中 audit_log 行数一致（%s vs %s）"
   % (cv.get("total"), AT), s == 200 and int(cv.get("total") or -1) == AT,
   "http=%s %s" % (s, blob(r)[:90]))
ok("H5b 自检把 first_broken_id 如实报出（有断点时必须是数字，不得为 0 或"
   "空串糊过去）", cv.get("first_broken_id") is None
   or int(cv.get("first_broken_id") or 0) > 0, blob(cv)[:90])
DUP = psql("SELECT count(*) FROM (SELECT prev_hash FROM audit_log GROUP BY "
           "prev_hash HAVING count(*) > 1) x")
print("   INFO 已知未修（待站长，P-174）：first_broken_id=%s，被多行共用的 "
      "prev_hash 有 %s 组 —— 触发器取链尾没加锁，并发写必分叉，自检因此"
      "在把正常并发判成断链。本轮不改 audit.rs（站长在制品），只报不判。"
      % (cv.get("first_broken_id"), DUP))
# ============ H6 收尾：把本轮动过的账逐条复原 ============
psql("DELETE FROM payment_orders WHERE order_no LIKE 'BH%d%%'" % TAG)
psql("DELETE FROM donation_ledger WHERE id > %d AND user_id IN (%d, %d, %d)"
     % (L0, S92, S95, MV))
# 只删本轮作弊处置写的台账（按事件号匹配）。状态变更/墓碑这两步产品自己会
# 往 user_modify_logs 里留行，那是收尾的正常产物，不当我造的残留
CEPAT = " OR ".join("content LIKE '%%#%s %%'" % int(i) for i in CEID)
psql("DELETE FROM user_modify_logs WHERE id > %d AND uid IN (%d, %d, %d) "
     "AND (%s)" % (M0, S92, S95, MV, CEPAT))
psql("DELETE FROM cheat_events WHERE id IN (%s)"
     % ",".join(str(int(i)) for i in CEID))
psql("UPDATE users SET wallet_usd = 0, donor = FALSE WHERE id IN (%d, %d, %d)"
     % (S92, S95, MV))
if INV:
    psql("UPDATE invites SET status = 2 WHERE used_by IS NULL AND id IN (%s)"
         % ",".join(str(int(i)) for i in INV))
for uid in (S92, S95, MV):
    psql("DELETE FROM messages WHERE receiver_id = %d OR sender_id = %d"
         % (uid, uid))
    post("/admin/users/status", {"user_id": uid, "status": 2})
    call("DELETE", "/admin/users/%d" % uid, None, token=ROOT)
    # 墓碑那一步产品自己会写一条 user_purge 流水，必须**在墓碑之后**删，
    # 否则数到的残留是产品的收尾动作，不是我的账
    psql("DELETE FROM spark_ledger WHERE user_id = %d" % uid)
    psql("DELETE FROM traffic_ledger WHERE user_id = %d" % uid)
    psql("DELETE FROM snatches WHERE user_id = %d" % uid)

WZ = psql("SELECT wallet_usd::float8 FROM users WHERE id = %d" % MV)
ok("H6a 探针钱包归零、本轮 donation_ledger 残留 %s 条"
   % cnt("donation_ledger", "id > %d AND user_id IN (%d, %d, %d)"
         % (L0, S92, S95, MV)),
   abs(float(WZ or 0)) < 1e-9
   and cnt("donation_ledger", "id > %d AND user_id IN (%d, %d, %d)"
           % (L0, S92, S95, MV)) == "0", "wallet=%s" % WZ)
ok("H6b 合成订单与 cheat_events 已清空（订单残留 %s 条、事件残留 %s 条）"
   % (cnt("payment_orders", "order_no LIKE 'BH%d%%'" % TAG),
      cnt("cheat_events", "agent LIKE '%s%%'" % AG)),
   cnt("payment_orders", "order_no LIKE 'BH%d%%'" % TAG) == "0"
   and cnt("cheat_events", "agent LIKE '%s%%'" % AG) == "0")
TOMB = psql("SELECT count(*) FROM users WHERE id IN (%d, %d, %d) AND "
            "status >= 2 AND passkey LIKE 'deleted-%%'" % (S92, S95, MV))
CELEAK = psql("SELECT count(*) FROM user_modify_logs WHERE id > %d AND "
              "uid IN (%d, %d, %d) AND (%s)" % (M0, S92, S95, MV, CEPAT))
TNL = psql("SELECT count(*) FROM user_modify_logs WHERE id > %d AND uid IN "
           "(%d, %d, %d)" % (M0, S92, S95, MV))
print("   INFO 探针收尾时产品自己写的台账 %s 条（改状态与墓碑各一条），属合法"
      "留痕，不计入残留。" % TNL)
ok("H6c 三个探针都走产品自己的墓碑（%s/3），本轮作弊处置台账残留 0 条" % TOMB,
   TOMB == "3" and CELEAK == "0",
   "tomb=%s cheat_logs=%s total=%s" % (TOMB, CELEAK, TNL))
ok("H6d 支付设置四项与本轮开始前逐字一致（复读比，不靠人记）",
   dict((k, sval(k)) for k in SNAP) == SNAP, blob(SNAP)[:120])
ok("H6e 审计只增不减（本轮新增 %s 条，一条都没删：总数 %s >= 起点 %s）"
   % (int(cnt("audit_log")) - A0, cnt("audit_log"), A0),
   int(cnt("audit_log")) >= A0, "A0=%s now=%s" % (A0, cnt("audit_log")))

summary()
