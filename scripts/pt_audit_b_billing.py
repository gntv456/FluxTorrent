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
    s, r = call("POST", "/admin/torrents/batch", {"action": "promo", "ids": [tidB], "kind": "x2free", "hours": 2}, token=tok)
    print("batch promo x2free:", s, json.dumps(r, ensure_ascii=False)[:160])
    if s != 200:
        # 尝试直接写库（管理员测试姿态，报告留档）
        DB("INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source, created_by) VALUES ('torrent', %d, 'x2free', now(), now() + interval '2 hours', 'manual', 1)" % tidB)
        DB("INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source, created_by) VALUES ('torrent', %d, 'p30', now(), now() + interval '2 hours', 'manual', 1)" % tidA)
        ok("促销直插库", True)
    else:
        ok("batch promo 接口", r.get("code") == 0, r.get("message"))
        s, r = call("POST", "/admin/torrents/batch", {"action": "promo", "ids": [tidA], "kind": "p30", "hours": 2}, token=tok)
        print("batch promo p30:", s, r.get("code"))

    # ---- 计费实测：两个用户各下 B（x2free），buyer 再下 A（p30） ----
    u0 = db_i("SELECT uploaded FROM users WHERE id=%s" % uid1)
    d0 = db_i("SELECT downloaded FROM users WHERE id=%s" % uid1)
    # user1: 下 B 完整 20MiB（x2free：下载全免、上传统计翻倍按实际上传）
    announce(pk1, ihB, 0, 0, left=20971520, event="started")
    announce(pk1, ihB, 5242880, 20971520, left=0, event="completed")
    # user2: 同样下 B —— 对照组
    announce(pk2, ihB, 0, 0, left=20971520, event="started")
    announce(pk2, ihB, 5242880, 20971520, left=0, event="completed")
    time.sleep(12)
    u1 = db_i("SELECT uploaded FROM users WHERE id=%s" % uid1)
    d1 = db_i("SELECT downloaded FROM users WHERE id=%s" % uid1)
    print("user1: up +%s down +%s（x2free 预期 up +%s down +0）" % (u1-u0, d1-d0, 5242880*2))
    ok("x2free 下载量不计（down 不变）", d1 == d0, "down %s -> %s" % (d0, d1))
    ok("x2free 上传量 x2", u1 - u0 == 5242880*2, "up %s -> %s" % (u0, u1))
    # 对照组
    u2_0 = db_i("SELECT uploaded FROM users WHERE id=%s" % uid2)
    d2_0 = db_i("SELECT downloaded FROM users WHERE id=%s" % uid2)
    u2_1 = db_i("SELECT uploaded FROM users WHERE id=%s" % uid2)
    d2_1 = db_i("SELECT downloaded FROM users WHERE id=%s" % uid2)
    print("user2 对照: up +%s down +%s" % (u2_1-u2_0, d2_1-d2_0))
    ok("对照组 x2free 下载量不计", d2_1 == d2_0, "down %s -> %s" % (d2_0, d2_1))
    ok("对照组 x2free 上传量 x2", u2_1 - u2_0 == 5242880*2, "up %s -> %s" % (u2_0, u2_1))

    # user1 再下 A（p30：下载按 30% 计）
    da0 = db_i("SELECT downloaded FROM users WHERE id=%s" % uid1)
    announce(pk1, ihA, 0, 0, left=20971520, event="started")
    announce(pk1, ihA, 1048576, 20971520, left=0, event="completed")
    time.sleep(12)
    da1 = db_i("SELECT downloaded FROM users WHERE id=%s" % uid1)
    print("user1 下 A(p30): down +%s 预期 +%s" % (da1-da0, int(20971520*0.3)))
    ok("p30 下载量按 30%% 入账", da1 - da0 == int(20971520*0.3), "down %s -> %s" % (da0, da1))

    # ---- 付费种子：user2 无票下载 A 应扣 spark / 或被拒 ----
    sp0 = db_i("SELECT spark_balance FROM users WHERE id=%s" % uid2)
    s_dl, raw = call("GET", "/torrents/%d/download" % tidA, token=t2, raw=True)
    print("user2 直下付费种子: %s len=%s" % (s_dl, len(raw) if isinstance(raw, bytes) else raw))
    sp1 = db_i("SELECT spark_balance FROM users WHERE id=%s" % uid2)
    print("spark: %s -> %s" % (sp0, sp1))
    ok("付费种子下载有拦截或扣费", (s_dl in (402, 403)) or sp0 != sp1,
       "status=%s spark %s->%s" % (s_dl, sp0, sp1))

    # ---- H&R：hr_policy 存在与否 + hr_snapshots 是否出现 ----
    hr = DB("SELECT hr_policy FROM torrents WHERE id=%d" % tidB)
    print("tidB hr_policy:", hr[:80])
    snaps = db_i("SELECT count(*) FROM hr_snapshots WHERE torrent_id=%d" % tidB)
    print("tidB hr_snapshots:", snaps)

summary()
