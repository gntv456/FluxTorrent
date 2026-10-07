# -*- coding: utf-8 -*-
"""权益与到期闸门（资深站点运营视角实测，0295）。

覆盖：捐赠者待遇判定（donor / vip_until / donor_until 三态与回落）、
魔力购买 VIP 不再顺带写永久 donor、头衔解锁券的购买拦截与过期核销、
「15 天去广告」下架、任务面板的手触真实性。
用法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_f_entitlement.py
可 FLUX_API_BASE 覆盖基址。探针号与探针数据在 finally 自清。
"""
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, ok, summary  # noqa: E402
import pt_audit_lib  # noqa: E402

if os.environ.get("FLUX_API_BASE"):
    BASE = os.environ["FLUX_API_BASE"]
    pt_audit_lib.BASE = BASE

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PW = "ProbePass123"
VIP_ITEM = 8         # 贵宾待遇 30 天（站内魔力档）
TITLE_ITEM = 7       # 自定义头衔（发 title_unlock 券）
ADFREE_ITEM = 11     # 15 天去广告（0295 起下架）
CREATED = []
TOKEN = ""


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-q", "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip())
    return r.stdout.strip()


def login_retry(user="root", pw="password123", tries=8):
    last = None
    for i in range(tries):
        s, r = call("POST", "/auth/login", {"username": user, "password": pw})
        if s == 200 and r.get("code") == 0:
            return r["data"]["token"]
        last = (s, r)
        time.sleep(min(2 + i * 3, 20))
    raise SystemExit("login failed for %s: %s" % (user, last))


def new_user(tag):
    """建探针号：adduser -> 改密（临时密码禁消费）。返回 (uid, token)。"""
    name = "ent_%s_%d" % (tag, int(time.time() * 1000) % 100000)
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


def bal(uid):
    return int(psql("SELECT spark_balance FROM users WHERE id=%d" % uid))


def fund(uid, amount):
    """走 /admin/users/adjust 注资：余额权威在 spark_ledger，直接 UPDATE
    会破坏「spark_balance == SUM(spark_ledger)」的台账不变式。"""
    key = "ent-fund-%d-%d" % (uid, int(time.time() * 1000))
    s, r = call("POST", "/admin/users/adjust",
                {"user_id": uid, "spark_delta": amount,
                 "idempotency_key": key}, TOKEN)
    return s, r


def priv(uid):
    v = psql("SELECT donor_privileged(%d)" % uid)
    return v == "t"


def setcols(uid, donor=None, vip=None, duntil=None):
    """vip/duntil: None=不动, ''=置 NULL, 其它=SQL 时间表达式"""
    sets = []
    if donor is not None:
        sets.append("donor=%s" % ("true" if donor else "false"))
    for col, val in (("vip_until", vip), ("donor_until", duntil)):
        if val is None:
            continue
        sets.append("%s=%s" % (col, "NULL" if val == "" else val))
    if not sets:
        return
    psql("UPDATE users SET %s WHERE id=%d" % (", ".join(sets), uid))


def buy(tok, item_id, key):
    return call("POST", "/shop/buy",
                {"item_id": item_id, "qty": 1, "idempotency_key": key}, tok)


def vouchers(uid, kind="title_unlock"):
    """返回 (可用张数, 总张数)——可用 = 未核销且未过期，与后端同判据。"""
    row = psql("SELECT count(*) FILTER (WHERE used_at IS NULL "
               "AND expires_at > now()), count(*) FROM user_vouchers "
               "WHERE user_id=%d AND kind='%s'" % (uid, kind))
    a, b = row.split("|")
    return int(a or 0), int(b or 0)


def sec_a_privilege_predicate(uid):
    print("\n--- A 待遇判定：三态与回落 ---")
    setcols(uid, donor=False, vip="", duntil="")
    ok("A1 无 donor 无时长 ⇒ 无待遇", priv(uid) is False)
    setcols(uid, vip="now() + interval '30 days'")
    ok("A2 vip_until 在期内 ⇒ 有待遇", priv(uid) is True)
    setcols(uid, vip="", duntil="now() + interval '15 days'")
    ok("A3 donor_until（特权档）在期内 ⇒ 有待遇（此前全库无读者）",
       priv(uid) is True)
    setcols(uid, duntil="now() - interval '1 second'")
    ok("A4 到期即失效，不依赖清理任务", priv(uid) is False)
    setcols(uid, donor=True)
    ok("A5 真捐赠 donor ⇒ 永久待遇", priv(uid) is True)
    setcols(uid, donor=False, vip="", duntil="")


def sec_b_vip_purchase(uid, tok):
    print("\n--- B 魔力购买 VIP：只写时长，不写永久位 ---")
    s, r = fund(uid, 1000000)
    ok("B0 注资 100 万魔力", s == 200, r)
    b0 = bal(uid)
    s, r = buy(tok, VIP_ITEM, "ent-vip-%d-1" % uid)
    ok("B1 购买贵宾待遇成功", s == 200 and r.get("code") == 0, r)
    donor = psql("SELECT donor FROM users WHERE id=%d" % uid)
    ok("B2 关键回归：购买 VIP 不再顺带写 donor=TRUE（0295 前必为 t）",
       donor == "f", donor)
    has = psql("SELECT COALESCE(vip_until > now(), false) FROM users "
               "WHERE id=%d" % uid)
    ok("B3 待遇改由 vip_until 驱动", has == "t", has)
    ok("B4 魔力按标价扣除", bal(uid) == b0 - int(
        psql("SELECT price FROM shop_items WHERE id=%d" % VIP_ITEM)),
        bal(uid))
    v1 = psql("SELECT vip_until FROM users WHERE id=%d" % uid)
    call("POST", "/shop/buy", {"item_id": VIP_ITEM, "qty": 1,
                               "idempotency_key": "ent-vip-%d-2" % uid}, tok)
    v2 = psql("SELECT vip_until FROM users WHERE id=%d" % uid)
    ok("B5 重复购买在现有点上续期（不是从今天重算，也不叠加永久位）",
       v2 != v1 and psql("SELECT donor FROM users WHERE id=%d" % uid) == "f")


def sec_c_title_voucher(uid, tok):
    print("\n--- C 头衔解锁券：购买拦截 / 核销 / 过期 ---")
    s, r = buy(tok, TITLE_ITEM, "ent-title-%d-1" % uid)
    ok("C1 购券成功", s == 200 and r.get("code") == 0, r)
    usable, total = vouchers(uid)
    ok("C2 手上有一张可用券", usable == 1, (usable, total))
    b0 = bal(uid)
    s, r = buy(tok, TITLE_ITEM, "ent-title-%d-2" % uid)
    ok("C3 已有可用券时再买：拦在扣款之前（0295 前是「扣魔力不发券」）",
       s == 400, (s, r))
    ok("C4 拦下时不扣魔力", bal(uid) == b0, (b0, bal(uid)))
    ok("C5 券数没有增加", vouchers(uid)[1] == total)

    s, r = call("GET", "/me/title", None, tok)
    d = r.get("data") or {}
    ok("C6 自读接口给出券状态（兑换入口的前提）",
       s == 200 and d.get("usable") == 1, (s, d))

    s, r = call("POST", "/me/title", {"title": "实测头衔"}, tok)
    ok("C7 核销券设置头衔", s == 200 and r.get("code") == 0, r)
    ok("C8 头衔已落库",
       psql("SELECT title FROM users WHERE id=%d" % uid) == "实测头衔")
    ok("C9 券已核销", vouchers(uid) == (0, 1))

    s, r = call("POST", "/me/title", {"title": "再来一次"}, tok)
    ok("C10 无可用券时拒绝再改", s == 400, (s, r))

    # 造一张「未用但已过期」的券：0295 前它能被用来改头衔
    psql("INSERT INTO user_vouchers (user_id, kind, source, granted_at, "
         "expires_at) VALUES (%d, 'title_unlock', 'admin', now(), "
         "now() - interval '1 day')" % uid)
    s, r = call("GET", "/me/title", None, tok)
    ok("C11 过期券不计入可用",
       s == 200 and (r.get("data") or {}).get("usable") == 0, (s, r))
    s, r = call("POST", "/me/title", {"title": "过期券试试"}, tok)
    ok("C12 过期券不能核销（与下载免流券同口径）", s == 400, (s, r))
    ok("C13 过期券仍在原地（未被静默消耗）",
       vouchers(uid)[1] == 2)

    b1 = bal(uid)
    s, r = buy(tok, TITLE_ITEM, "ent-title-%d-3" % uid)
    ok("C14 只有过期券时允许再买并发新券",
       s == 200 and vouchers(uid)[0] == 1, (s, r))
    ok("C15 这次真的扣了魔力", bal(uid) == b1 - int(
        psql("SELECT price FROM shop_items WHERE id=%d" % TITLE_ITEM)))


def sec_d_ad_free():
    print("\n--- D 「15 天去广告」下架 ---")
    active = psql("SELECT active::text FROM shop_items WHERE id=%d"
                  % ADFREE_ITEM)
    ok("D1 已下架（站内没有任何广告渲染，商品描述的是不存在的能力）",
       active == "false", active)
    s, r = buy(PROBE_TOK, ADFREE_ITEM, "ent-adfree-1")
    ok("D2 下架后买不到", s != 200 or r.get("code") != 0, (s, r))


def sec_e_jobs():
    print("\n--- E 任务面板的手触真实性 ---")
    s, r = call("POST", "/admin/jobs/run", {"job": "usage_stats"}, TOKEN)
    ok("E1 触发入队", s == 200 and r.get("code") == 0, (s, r))
    deadline = time.time() + 150
    row = ""
    while time.time() < deadline:
        row = psql("SELECT status || '|' || coalesce(result, '') "
                   "FROM job_triggers WHERE job='usage_stats' "
                   "ORDER BY id DESC LIMIT 1")
        if row.startswith("done|") or row.startswith("failed|"):
            break
        time.sleep(5)
    ok("E2 usage_stats 手触真的会跑（0295 前恒为「未知任务」失败）",
       row.startswith("done|"), row)
    listed = psql("SELECT count(*) FROM job_status WHERE "
                  "job IN ('usage_stats','request_expire')")
    ok("E3 两条任务都在目录里（面板可见可触）", listed == "2", listed)


def sec_f_source_wired():
    print("\n--- F 判定点接线（防「写面有、读面零」复发）---")
    hits = 0
    for rel in ["apps/worker/src/jobs/seeding.rs",
                "apps/api/src/economy_http/voucher_use.rs",
                "apps/api/src/content_http/misc.rs",
                "apps/api/src/economy_http/shop.rs",
                "apps/api/src/economy_http/shop_effects.rs"]:
        p = os.path.join(ROOT, rel)
        try:
            with open(p, encoding="utf-8") as f:
                text = f.read()
            hits += 1 if ("donor_privileged(" in text
                          or "has_usable_voucher(" in text) else 0
        except OSError:
            pass
    ok("F1 五个消费点至少 5 处引用判定函数", hits >= 5, hits)
    # 做种结算必须走判定函数，不能再裸读 u.donor。
    # （用 Python 扫而不起 grep 子进程：Windows 宿主上没有 grep，
    # 上一版这里直接抛异常，闸门红得毫无意义。）
    # 判据要跳过 SQL 注释（注释里正是在解释「此前直接读 u.donor」），
    # 且 `pu.donor` 是 CTE 别名不是裸列读取——上一版把这两类都算成违规。
    seed_src = os.path.join(ROOT, "apps/worker/src/jobs/seeding.rs")
    with open(seed_src, encoding="utf-8") as f:
        lines = [ln for ln in f.read().splitlines()
                 if not ln.strip().startswith("--")]
    bare = [ln.strip() for ln in lines
            if re.search(r"\bu\.donor\b", ln)
            and "donor_privileged" not in ln]
    ok("F2 做种结算不再裸读 u.donor", bare == [], bare[:2])


STOCK_BACKUP = {}


def raw_write(headers):
    """直接对 /auth/login 发一个写请求，只改头部形态，看闸门放不放行。
    返回 (status, code)。用 root 的错误口令也行——我们要判的是 403 与否。"""
    import urllib.request
    import json as _json
    url = BASE + "/auth/login"
    data = _json.dumps({"username": "nobody_probe", "password": "x"}).encode()
    req = urllib.request.Request(url, data=data, method="POST")
    req.add_header("Content-Type", "application/json")
    for k, v in headers.items():
        req.add_header(k, v)
    try:
        r = urllib.request.urlopen(req, data, timeout=20)
        body = _json.loads(r.read())
    except urllib.error.HTTPError as e:  # noqa: BLE001
        try:
            body = _json.loads(e.read())
        except Exception:
            body = {"code": None}
        return e.code, body.get("code")
    return r.status, body.get("code")


def sec_g_write_origin():
    print("\n--- G 写请求来源闸：同源代理形态必须放行 ---")
    # 浏览器经 web 容器同源代理打进来时：Origin 是用户地址栏的 host，
    # Host 是上游容器名，代理把用户 host 放进 x-forwarded-host。
    # 0295 前闸门只比 Host ⇒ 这类请求全判跨站，实测连登录都 403/2003。
    s, code = raw_write({
        "Origin": "http://127.0.0.1:3000",
        "X-Forwarded-Host": "127.0.0.1:3000",
    })
    ok("G1 同源代理形态（Origin == x-forwarded-host）不再被误拦",
       code != 2003, (s, code))
    s, code = raw_write({
        "Origin": "http://evil.example",
        "X-Forwarded-Host": "127.0.0.1:3000",
    })
    ok("G2 真跨站仍然 403（修的是误拦，不是把闸拆了）",
       code == 2003, (s, code))
    s, code = raw_write({"Origin": "http://evil.example"})
    ok("G3 无转发头且 Origin 非白名单 ⇒ 仍然 403", code == 2003, (s, code))


def cleanup():
    for uid in CREATED:
        try:
            psql("DELETE FROM shop_orders WHERE user_id=%d" % uid)
            psql("DELETE FROM user_vouchers WHERE user_id=%d" % uid)
            psql("DELETE FROM spark_ledger WHERE user_id=%d" % uid)
            psql("UPDATE users SET spark_balance=0, title=NULL, donor=false,"
                 " vip_until=NULL, donor_until=NULL, class_id=1 WHERE id=%d"
                 % uid)
            call("POST", "/admin/users/status",
                 {"user_id": uid, "status": 2}, TOKEN)
            call("DELETE", "/admin/users/%d" % uid, None, TOKEN)
        except Exception as e:  # noqa: BLE001
            print("[CLEAN-WARN] uid %s: %s" % (uid, e))
    for iid, used in STOCK_BACKUP.items():
        psql("UPDATE shop_items SET stock_used=%d WHERE id=%d"
             % (used, iid))
    print("[CLEAN] 探针号 %s 已封禁删除，商品计数已复原" % CREATED)


PROBE_TOK = ""


def main():
    global TOKEN, PROBE_TOK
    TOKEN = login_retry()
    try:
        for iid in (VIP_ITEM, TITLE_ITEM, ADFREE_ITEM):
            STOCK_BACKUP[iid] = int(
                psql("SELECT stock_used FROM shop_items WHERE id=%d" % iid))
        uid, tok = new_user("payer")
        PROBE_TOK = tok
        sec_a_privilege_predicate(uid)
        sec_b_vip_purchase(uid, tok)
        sec_c_title_voucher(uid, tok)
        sec_d_ad_free()
        sec_e_jobs()
        sec_f_source_wired()
        sec_g_write_origin()
    finally:
        cleanup()
    summary()


if __name__ == "__main__":
    main()
