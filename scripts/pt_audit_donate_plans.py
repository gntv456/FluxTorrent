# -*- coding: utf-8 -*-
"""法币套餐发放面闸门（资深捐赠运营视角实测）。

0341 起因：捐赠页 quota 档原名「10 个片单额度」，而 plan_type='quota' 落的是
users.quota_extra——全站唯一消费者是领邀请码时优先扣的额外名额，本站没有任何
「片单」实体。这条闸门同时钉住三件事：文案与消费点对得上、发放量随标题数字走
（此前硬编码 +10，改标题无效）、数量非法时在扣款之前拒单（此前静默收钱不给货）。
用法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_donate_plans.py
可 FLUX_API_BASE 覆盖基址。探针号与 9xx 段探针套餐在 finally 自清。
"""
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, ok, summary  # noqa: E402
import pt_audit_lib  # noqa: E402

if os.environ.get("FLUX_API_BASE"):
    pt_audit_lib.BASE = os.environ["FLUX_API_BASE"]

BASE = pt_audit_lib.BASE
PW = "ProbePass123"
PROBE_PLAN_IDS = list(range(901, 907))
CREATED = []
TOKEN = ""
PROBE = None


def login_retry(user="root", pw="password123", tries=10):
    last = None
    for i in range(tries):
        s, r = call("POST", "/auth/login", {"username": user, "password": pw})
        if s == 200 and isinstance(r, dict) and r.get("code") == 0:
            return r["data"]["token"]
        last = (s, r)
        time.sleep(min(2 + i * 3, 20))
    raise SystemExit("login failed %s: %s" % (user, last))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-q", "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip())
    return r.stdout.strip()


def new_user(tag):
    name = "dp_%s_%d" % (tag, int(time.time() * 1000) % 100000)
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
    return uid, login_retry(name, "ProbePass456")


def add_plan(pid, ptype, title, price):
    psql("INSERT INTO donation_plans (id, plan_type, title, price_usd, sort)"
         " VALUES (%d, '%s', '%s', %s, %d) ON CONFLICT (id) DO UPDATE SET"
         " title = EXCLUDED.title, price_usd = EXCLUDED.price_usd,"
         " plan_type = EXCLUDED.plan_type" % (pid, ptype, title, price, pid))


def state(tok):
    s, r = call("GET", "/donate/state", None, tok)
    return r["data"]


def order(tok, pid):
    return call("POST", "/donate/order", {"plan_id": pid}, tok)


def wallet(uid):
    return psql("SELECT wallet_usd::float8 FROM users WHERE id=%d" % uid)


def quota(uid):
    return int(psql("SELECT quota_extra FROM users WHERE id=%d" % uid))


def uploaded(uid):
    return int(psql("SELECT uploaded FROM users WHERE id=%d" % uid))


def sec_a_surface():
    print("\n-- A 文案与消费点对得上 --")
    d = state(PROBE[1])
    p1 = [x for x in d["plans"] if x["id"] == 1]
    ok("A1 plan 1 在册", len(p1) == 1, p1)
    ok("A2 plan 1 标题=10 枚邀请名额",
       p1 and p1[0]["title"] == "10 枚邀请名额", p1 and p1[0]["title"])
    bad = [x["title"] for x in d["plans"] if "片单" in x["title"]]
    ok("A3 站点不再出现「片单」文案", not bad, bad)
    got = psql("SELECT title FROM donation_plans WHERE id = 1")
    ok("A4 0341 迁移已落库", got == "10 枚邀请名额", got)
    n = int(psql("SELECT count(*) FROM _sqlx_migrations WHERE version=341"))
    ok("A5 迁移 341 已记账", n == 1, n)


def sec_b_grant():
    print("\n-- B 发放量 = 标题首段数字 --")
    uid, tok = PROBE
    psql("UPDATE users SET wallet_usd = 200, quota_extra = 0 WHERE id=%d" % uid)
    s, r = order(tok, 1)
    ok("B1 买 plan 1 成交", s == 200 and r.get("code") == 0, r)
    ok("B2 发 10 枚（与标题一致）", quota(uid) == 10, quota(uid))
    ok("B3 扣 10 USD", abs(float(wallet(uid)) - 190.0) < 1e-6, wallet(uid))
    note = psql("SELECT note FROM donation_ledger WHERE user_id=%d"
                " AND kind='order' ORDER BY id DESC LIMIT 1" % uid)
    ok("B4 流水留痕套餐名", note == "10 枚邀请名额", note)

    add_plan(901, "quota", "3 枚邀请名额", 1)
    q0 = quota(uid)
    s, r = order(tok, 901)
    ok("B5 标题改 3 枚就发 3 枚", s == 200 and quota(uid) == q0 + 3,
       "%s -> %s %s" % (q0, quota(uid), r))

    st = state(tok)
    t901 = [x["title"] for x in st["plans"] if x["id"] == 901]
    ok("B6 探针套餐读得到", t901 == ["3 枚邀请名额"], t901)


def refused(r):
    """只认我们这条闸门的拒单——500/权限错也会 code!=0，不能当通过。"""
    msg = (r or {}).get("message") or ""
    return (r or {}).get("code") != 0 and "数量无效" in msg


def order_lang(tok, pid, lang):
    """带 Accept-Language 打一次订购：验拒单文案已进 validation_details.tsv。"""
    req = urllib.request.Request(BASE + "/donate/order", method="POST")
    req.add_header("Content-Type", "application/json")
    req.add_header("Accept-Language", lang)
    req.add_header("Authorization", "Bearer " + tok)
    body = json.dumps({"plan_id": pid}).encode()
    try:
        return json.loads(urllib.request.urlopen(req, body, 20).read())
    except urllib.error.HTTPError as e:  # 拒单是 400，信封在 body 里
        return json.loads(e.read())


def sec_c_refuse():
    print("\n-- C 数量非法先拒单，钱不动 --")
    uid, tok = PROBE
    w0, q0, u0 = float(wallet(uid)), quota(uid), uploaded(uid)

    add_plan(902, "quota", "枚邀请名额", 5)
    s, r = order(tok, 902)
    ok("C1 无数字的 quota 标题被拒", refused(r), (s, r))
    en = order_lang(tok, 902, "en")
    ok("C1b 拒单句已进译文表（Accept-Language: en）",
       "Invalid quantity in the plan title" in (en.get("message") or ""),
       en)
    ok("C2 拒单未扣款", abs(float(wallet(uid)) - w0) < 1e-6, wallet(uid))
    ok("C3 拒单未发货", quota(uid) == q0, quota(uid))

    add_plan(903, "quota", "9999 枚邀请名额", 5)
    s, r = order(tok, 903)
    ok("C4 超上限（50 枚）被拒", refused(r), (s, r))

    add_plan(904, "upload", "GB 上传量", 5)
    s, r = order(tok, 904)
    ok("C5 无数字的 upload 标题同样被拒", refused(r), (s, r))
    ok("C6 两笔拒单后余额仍未动", abs(float(wallet(uid)) - w0) < 1e-6,
       wallet(uid))
    ok("C7 upload 坏标题也没发货", uploaded(uid) == u0,
       "%s -> %s" % (u0, uploaded(uid)))

    # 正控：同一个号紧接着买成功套餐，证明 C 段不是号被锁/余额花光
    s, r = order(tok, 901)
    ok("C8 正控——坏标题拒单后仍能正常成交",
       s == 200 and r.get("code") == 0 and quota(uid) == q0 + 3,
       (s, r, quota(uid)))


def sec_d_upload_vip():
    print("\n-- D upload 双写 / vip 加期 未被闸门打断 --")
    uid, tok = PROBE
    add_plan(905, "upload", "7 GB 上传量", 1)
    u0 = uploaded(uid)
    s, r = order(tok, 905)
    ok("D1 7 GB 上传量到账", s == 200 and uploaded(uid) == u0 + 7 * 2**30,
       "%s -> %s %s" % (u0, uploaded(uid), r))
    led = int(psql("SELECT count(*) FROM traffic_ledger WHERE user_id=%d"
                   " AND torrent_id=0 AND delta_up=%d" % (uid, 7 * 2**30)))
    ok("D2 差额流水同落（6h 快照重算不吃掉付费量）", led >= 1, led)

    add_plan(906, "vip", "30 天", 1)
    s, r = order(tok, 906)
    days = psql("SELECT EXTRACT(day FROM (vip_until - now()))::int"
                " FROM users WHERE id=%d" % uid)
    ok("D3 vip 套餐加 30 天", 29 <= int(days or 0) <= 31, (s, r, days))


def sec_e_consume():
    print("\n-- E 名额真的花在领码上（文案指向的消费点）--")
    uid, tok = PROBE
    q0 = quota(uid)
    if q0 <= 0:
        psql("UPDATE users SET quota_extra = 2 WHERE id=%d" % uid)
        q0 = 2
    s, r = call("POST", "/invites", None, tok)
    ok("E1 有额外名额即可领码", s == 200 and r.get("code") == 0, (s, r))
    ok("E2 领码优先扣 quota_extra", quota(uid) == q0 - 1,
       "%s -> %s" % (q0, quota(uid)))
    unused = int(psql("SELECT count(*) FROM invites WHERE inviter_id=%d"
                      " AND status=0" % uid))
    ok("E3 码已下发", unused >= 1, unused)


def purge_user(uid):
    psql("DELETE FROM invites WHERE inviter_id=%d" % uid)
    psql("DELETE FROM traffic_ledger WHERE user_id=%d AND torrent_id=0" % uid)
    # 删号是墓碑化（users 行不删），donation_ledger 不会被级联带走，
    # 而它外键引用 donation_plans——探针套餐只有先清流水才删得掉。
    # audit_log 里的 donate_order 留痕按设计保留（链断不得）。
    psql("DELETE FROM donation_ledger WHERE user_id=%d" % uid)
    call("POST", "/admin/users/status", {"user_id": uid, "status": 2}, TOKEN)
    call("DELETE", "/admin/users/%d" % uid, None, TOKEN)


def cleanup():
    ids = list(CREATED)
    # 中途崩掉的轮次会留下 dp_ 探针号（本轮就断在收尾第一条语句上），
    # 按自己的前缀扫一遍再走人——前缀 dp_ 只属于这条闸门。
    stray = [int(x) for x in psql(
        "SELECT id FROM users WHERE username LIKE 'dp\\_%'"
        " AND status < 2").split() if x.isdigit()]
    for uid in dict.fromkeys(ids + stray):
        try:
            purge_user(uid)
        except Exception as e:  # noqa: BLE001
            print("[CLEAN-WARN] uid=%s %s" % (uid, e))
    psql("DELETE FROM donation_plans WHERE id IN (%s)"
         % ",".join(str(i) for i in PROBE_PLAN_IDS))
    left = psql("SELECT count(*) FROM donation_plans WHERE id >= 900")
    alive = psql("SELECT count(*) FROM users WHERE username LIKE 'dp\\_%'"
                 " AND status < 2")
    print("[CLEAN] 探针号 %s 已墓碑化（dp_ 在册 %s），9xx 段套餐残留 %s 行"
          % (CREATED, alive, left))


def main():
    global TOKEN, PROBE
    TOKEN = login_retry()
    s, r = call("GET", "/me", None, TOKEN)
    if s != 200:
        raise SystemExit("token self-check failed: %s" % s)
    try:
        PROBE = new_user("buyer")
        print("探针 uid=%d" % PROBE[0])
        sec_a_surface()
        sec_b_grant()
        sec_c_refuse()
        sec_d_upload_vip()
        sec_e_consume()
    finally:
        cleanup()
    summary()


if __name__ == "__main__":
    main()
