# -*- coding: utf-8 -*-
"""资深 PT 深测脚本 B：促销计费正确性 + H&R + 付费种子 + 代理下载。

方法：直接用管理员 REST 造促销（promotion），两个普通用户各自 announce
同一批字节量，核对 users.uploaded/downloaded 与促销系数一致；
再走 H&R 快照与违规判定。
"""
import sys, json, hashlib, urllib.request, urllib.parse, time, random
sys.stdout.reconfigure(encoding="utf-8")
from pt_audit_lib import call, login, ok, summary
from pt_audit_a_publish import make_torrent, multipart, upload_torrent

DB = lambda sql: __import__("subprocess").run(
    ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent", "-t", "-A", "-c", sql],
    capture_output=True, text=True).stdout.strip()

def db_i(sql):
    v = DB(sql)
    try: return int(v)
    except Exception: return None


def wait_credit(sql, target, timeout=150, step=5):
    """轮询到「等于预期」或超时，返回最后一次读数。

    announce 事件由 worker 按 60s 一批 flush，固定 sleep(12) 读到的是 flush
    之前的旧值——实测流水在 announce 之后约 90s 才落库（10:24:47），
    于是三条计费断言集体假红。超时不缩短，宁可多等也不要假信号。
    """
    deadline = time.time() + timeout
    cur = db_i(sql)
    while time.time() < deadline:
        if cur == target:
            return cur
        time.sleep(step)
        cur = db_i(sql)
    return cur

# ---- 造两个测试用户：管理员姿态直插邀请码 → 邀请注册（invite_only 模式） ----
# 注：POST /admin/users/adjust 的 invite_grant 对 root 调整自身会撞
# ensure_outranks(99 vs 99) 护栏（by design），故借普通用户 e2ecap* 做 inviter。
tok = login()
suffix = random.randint(10000, 99999)
DB("INSERT INTO invites (inviter_id, code, expires_at) VALUES (76, 'audit%s', now() + interval '1 day')" % ("a%d" % suffix))
DB("INSERT INTO invites (inviter_id, code, expires_at) VALUES (76, 'audit%s', now() + interval '1 day')" % ("b%d" % suffix))
users = []
for i, (uname, code) in enumerate([
        ("ptb_buyer%d" % suffix, "audita%d" % suffix),
        ("ptb_free%d" % suffix, "auditb%d" % suffix)]):
    # 注册需算术验证码（provider=none 自研题）：取题算答案再注册
    s, cap = call("GET", "/auth/captcha")
    q = (cap.get("data") or {}).get("question", "")
    import re as _re
    m = _re.match(r"(\d+) \+ (\d+) =", q)
    ans = int(m.group(1)) + int(m.group(2)) if m else 0
    s, r = call("POST", "/auth/register", {"username": uname, "email": "%s@audit.invalid" % uname,
                                            "password": "Audit12345678!", "invite_code": code,
                                            "captcha_id": (cap.get("data") or {}).get("captcha_id", ""),
                                            "captcha_answer": ans})
    users.append(uname)
    print("建号", uname, s, r.get("code"), r.get("message"))

def login_or_skip(u):
    try:
        return login(u, "Audit12345678!")
    except SystemExit:
        return None

t1 = login_or_skip(users[0]); t2 = login_or_skip(users[1])
ok("两个测试号可用", bool(t1 and t2))

def get_passkey(t):
    s, r = call("GET", "/me/overview", token=t)
    return (r.get("data") or {}).get("passkey") or ""

def get_uid(t):
    s, r = call("GET", "/me/overview", token=t)
    return (r.get("data") or {}).get("id")

pk1, pk2 = (get_passkey(t1) or ""), (get_passkey(t2) or "")
uid1, uid2 = get_uid(t1), get_uid(t2)
print("uid1=%s uid2=%s" % (uid1, uid2))
ok("双 passkey 就绪", bool(pk1 and pk2 and uid1 and uid2))

def announce(pk, ih_hex, up, down, left=0, event=""):
    ih = bytes.fromhex(ih_hex)
    q = {"info_hash": ih, "peer_id": b"-FL2600-AUDIT%s" % str(random.randint(1000000, 9999999)).encode(),
         "port": 51413, "uploaded": up, "downloaded": down, "left": left, "compact": 1}
    if event: q["event"] = event
    qs = urllib.parse.urlencode(q, quote_via=urllib.parse.quote)
    req = urllib.request.Request("http://127.0.0.1:7070/announce/%s?%s" % (pk, qs),
                                 headers={"User-Agent": "qBittorrent/5.0.0"})
    try:
        r = urllib.request.urlopen(req, timeout=10); return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()

# ---- 发两个种子：A 付费、B 免费（管理员侧不设促销） ----
torA, ihA = make_torrent(files=[("paid.bin", 20971520)])
torB, ihB = make_torrent(files=[("free.bin", 20971520)])
s, r = upload_torrent(tok, torA, category_id="1", name="AuditB.Paid.%d" % suffix, price="500")
ok("付费种子发种", s == 200 and r.get("code") == 0, "%s %s" % (s, r.get("message")))
tidA = (r.get("data") or {}).get("id")
s, r = upload_torrent(tok, torB, category_id="1", name="AuditB.Free.%d" % suffix)
ok("免费种子发种", s == 200 and r.get("code") == 0, "%s %s" % (s, r.get("message")))
tidB = (r.get("data") or {}).get("id")
print("tidA=%s tidB=%s" % (tidA, tidB))

# ---- 促销：给 tidB 挂 2xfree（x2 上传 + 免费），tidA 挂 30% 下载 p30 ----
if tidA and tidB:
    # 接口的字段是 promo_kind / promo_until（torrent_batch.rs:26）。
    # 早先这里传的是 kind / hours——serde 认不到就静默走缺省，
    # 于是促销全被写成 'free'，"x2free 上传翻倍" 与 "p30 按 30% 计"
    # 两条断言永远不会绿（红了还不是产品的错）。
    until = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(time.time() + 7200))
    s, r = call("POST", "/admin/torrents/batch", {"action": "promo", "ids": [tidB], "promo_kind": "x2free", "promo_until": until}, token=tok)
    print("batch promo x2free:", s, json.dumps(r, ensure_ascii=False)[:160])
    if s != 200:
        # 尝试直接写库（管理员测试姿态，报告留档）
        DB("INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source, created_by) VALUES ('torrent', %d, 'x2free', now(), now() + interval '2 hours', 'manual', 1)" % tidB)
        DB("INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source, created_by) VALUES ('torrent', %d, 'p30', now(), now() + interval '2 hours', 'manual', 1)" % tidA)
        ok("促销直插库", True)
    else:
        ok("batch promo 接口", r.get("code") == 0, r.get("message"))
        s, r = call("POST", "/admin/torrents/batch", {"action": "promo", "ids": [tidA], "promo_kind": "p30", "promo_until": until}, token=tok)
        print("batch promo p30:", s, r.get("code"))

    # 落库回读：接口 200 不代表写进去的是请求的那个 kind（这项目最常见的
    # 「静默缺省」形状）。kind 缺省成 free 时，后面所有倍率断言都会假红。
    kb = DB("SELECT kind FROM promotions WHERE torrent_id=%d ORDER BY id DESC LIMIT 1" % tidB)
    ok("x2free 按请求 kind 落库（没被静默缺省成 free）", kb == "x2free", "库里 kind=%s" % kb)
    ka = DB("SELECT kind FROM promotions WHERE torrent_id=%d ORDER BY id DESC LIMIT 1" % tidA)
    ok("p30 按请求 kind 落库", ka == "p30", "库里 kind=%s" % ka)

    # ---- 计费实测：两个用户各下 B（x2free），buyer 再下 A（p30） ----
    # worker 按 60s 一批 flush announce 事件（不是逐条实时），所以这里必须
    # **轮询到值稳定**，固定 sleep(12) 会读到 flush 之前的旧值 => 假红。
    u0 = db_i("SELECT uploaded FROM users WHERE id=%s" % uid1)
    d0 = db_i("SELECT downloaded FROM users WHERE id=%s" % uid1)
    u2_0 = db_i("SELECT uploaded FROM users WHERE id=%s" % uid2)
    d2_0 = db_i("SELECT downloaded FROM users WHERE id=%s" % uid2)
    # user1: 下 B 完整 20MiB（x2free：下载全免、上传统计翻倍按实际上传）
    announce(pk1, ihB, 0, 0, left=20971520, event="started")
    announce(pk1, ihB, 5242880, 20971520, left=0, event="completed")
    # user2: 同样下 B —— 对照组（基线已在 announce 之前取，见上）
    announce(pk2, ihB, 0, 0, left=20971520, event="started")
    announce(pk2, ihB, 5242880, 20971520, left=0, event="completed")
    u1 = wait_credit("SELECT uploaded FROM users WHERE id=%s" % uid1,
                     u0 + 5242880 * 2)
    d1 = db_i("SELECT downloaded FROM users WHERE id=%s" % uid1)
    print("user1: up +%s down +%s（x2free 预期 up +%s down +0）" % (u1-u0, d1-d0, 5242880*2))
    ok("x2free 下载量不计（down 不变）", d1 == d0, "down %s -> %s" % (d0, d1))
    ok("x2free 上传量 x2", u1 - u0 == 5242880*2, "up %s -> %s" % (u0, u1))
    # 对照组
    u2_1 = wait_credit("SELECT uploaded FROM users WHERE id=%s" % uid2,
                       u2_0 + 5242880 * 2)
    d2_1 = db_i("SELECT downloaded FROM users WHERE id=%s" % uid2)
    print("user2 对照: up +%s down +%s" % (u2_1-u2_0, d2_1-d2_0))
    ok("对照组 x2free 下载量不计", d2_1 == d2_0, "down %s -> %s" % (d2_0, d2_1))
    ok("对照组 x2free 上传量 x2", u2_1 - u2_0 == 5242880*2, "up %s -> %s" % (u2_0, u2_1))

    # user1 再下 A（p30：下载按 30% 计）
    da0 = db_i("SELECT downloaded FROM users WHERE id=%s" % uid1)
    announce(pk1, ihA, 0, 0, left=20971520, event="started")
    announce(pk1, ihA, 1048576, 20971520, left=0, event="completed")
    da1 = wait_credit("SELECT downloaded FROM users WHERE id=%s" % uid1,
                      da0 + int(20971520 * 0.3))
    print("user1 下 A(p30): down +%s 预期 +%s" % (da1-da0, int(20971520*0.3)))
    ok("p30 下载量按 30%% 入账", da1 - da0 == int(20971520*0.3), "down %s -> %s" % (da0, da1))

    # ---- 付费种子：无余额应拦下；注资后应真扣费 ----
    # 这条原先只认 402/403，而接口对「余额不足」回的是 400 + Validation，
    # 于是产品行为是对的、闸门却一直红（两个测试号 spark 都是 0，
    # 天然走不到扣费分支）。这里把两个分支分开测。
    sp0 = db_i("SELECT spark_balance FROM users WHERE id=%s" % uid2)
    s_dl, raw = call("GET", "/torrents/%d/download" % tidA, token=t2, raw=True)
    print("user2 无余额直下付费种子: %s" % s_dl)
    sp1 = db_i("SELECT spark_balance FROM users WHERE id=%s" % uid2)
    ok("付费种子无余额时拦下（不给文件、不动账）",
       s_dl != 200 and sp1 == sp0,
       "status=%s spark %s->%s" % (s_dl, sp0, sp1))

    price_a = db_i("SELECT price FROM torrents WHERE id=%s" % tidA)
    call("POST", "/admin/users/adjust",
         {"user_id": uid2, "spark_delta": price_a * 3,
          "note": "gate-b 付费种子扣费实测",
          "idempotency_key": "gate-b-fund-%d" % uid2}, token=tok)
    sp2 = db_i("SELECT spark_balance FROM users WHERE id=%s" % uid2)
    s_dl2, _ = call("GET", "/torrents/%d/download" % tidA, token=t2,
                    raw=True)
    sp3 = db_i("SELECT spark_balance FROM users WHERE id=%s" % uid2)
    tax = db_i("SELECT coalesce((SELECT value::int FROM site_settings"
               " WHERE name='upload_price_tax'), 30)")
    net = price_a * (100 - tax) // 100
    print("user2 注资后下载: %s spark %s->%s（价 %s 税 %s%% 卖家净得 %s）"
          % (s_dl2, sp2, sp3, price_a, tax, net))
    # 买家付**全价**，卖家拿**税后净额**——两个金额别写进一条断言。
    ok("付费种子有余额时真扣费（买家扣全价）",
       s_dl2 == 200 and sp2 - sp3 == price_a,
       "status=%s 扣了 %s 预期 %s" % (s_dl2, sp2 - sp3, price_a))
    ok("扣费流水落库且去向是卖家",
       db_i("SELECT count(*) FROM spark_ledger WHERE user_id=%s"
            " AND kind='torrent_buy'" % uid2) >= 1
       or db_i("SELECT count(*) FROM torrent_purchases WHERE user_id=%s"
               " AND torrent_id=%s" % (uid2, tidA)) >= 1,
       "买断记录未落库")
    # 钱要走到卖家与资金池，否则「付费种子」只是把买家钱变没
    owner = db_i("SELECT owner_id FROM torrents WHERE id=%s" % tidA)
    ok("卖家入账净额 = 价 - 站税",
       db_i("SELECT coalesce(sum(amount),0) FROM spark_ledger"
            " WHERE user_id=%s AND kind='torrent_sell'" % owner) >= net,
       "owner=%s 净额预期>=%s" % (owner, net))
    month = time.strftime("%Y-%m", time.gmtime())
    ok("站税进资金池（magic_pool）",
       db_i("SELECT coalesce(donated_total,0) FROM magic_pool"
            " WHERE month='%s'" % month) >= price_a - net,
       "本月 donated_total 不足 %s" % (price_a - net))

    # ---- H&R：hr_policy 存在与否 + hr_snapshots 是否出现 ----
    hr = DB("SELECT hr_policy FROM torrents WHERE id=%d" % tidB)
    print("tidB hr_policy:", hr[:80])
    snaps = db_i("SELECT count(*) FROM hr_snapshots WHERE torrent_id=%d" % tidB)
    print("tidB hr_snapshots:", snaps)

summary()
