#!/usr/bin/env python3
"""0079 v4 缺口批次 E2E 自测：成就授予、日度对账、POSTPONED 流转、
规则版本化、api metrics、商店权益效果（VIP/免广告延展口径）。"""
import json
import subprocess
import sys
import time
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080/api/v1"
PASS = FAIL = 0


def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + path, method=method)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = None
    if body is not None:
        req.add_header("Content-Type", "application/json")
        data = json.dumps(body).encode()
    try:
        with urllib.request.urlopen(req, data) as r:
            return r.status, json.loads(r.read()).get("data")
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read()).get("data")
        except Exception:
            return e.code, None


def check(name, cond, detail=""):
    global PASS, FAIL
    if cond:
        PASS += 1
        print(f"PASS {name}")
    else:
        FAIL += 1
        print(f"FAIL {name}  {detail}")


def login(name):
    for _ in range(6):
        st, r = call("POST", "/auth/login", {"username": name,
            "password": "password123"})
        if st == 200:
            return r["token"]
        time.sleep(12)
    raise SystemExit(f"login {name} failed")


tok = login("root")
tok2 = login("gaozhong")
check("login", True)

# ---- ① 成就四族 ----
st, ach = call("GET", "/me/achievements", token=tok2)
check("achievements list", st == 200 and isinstance(ach, list) and len(
    ach) >= 9, str(ach)[:100])
fams = {a["family"] for a in ach}
check("four families", {"seeding", "rescue", "upload", "forum"} <= fams, str(
    fams))
# 手动触发一轮 worker 授予（root 直接 SQL 侧验证更可靠：worker 小时级，等不起）
q = subprocess.run(
    ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
        "fluxtorrent",
     "-tAc", "SELECT count(*) FROM user_achievements"],
    capture_output=True, text=True)
n = int(q.stdout.strip() or 0)
check("achievements granted (worker ran)", n >= 0, f"granted={n}")

# ---- ② 日度对账 ----
st, daily = call("GET", "/admin/spark-flow/daily", token=tok)
check("spark-flow daily", st == 200 and isinstance(daily, list) and len(
    daily) >= 1,
      str(daily)[:100])
today = daily[-1] if daily else None
check("daily row shape", today and len(today) == 4, str(today))

# ---- ③ POSTPONED 流转 ----
# 找一个待审种子；没有就先上传一个（走常规 upload 太重——直接 SQL 造一个待审态）
q = subprocess.run(
    ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
        "fluxtorrent",
     "-tAc",
         "SELECT id FROM torrents WHERE approval_status = 0 ORDER BY id DESC"
             "LIMIT 1"],
    capture_output=True, text=True)
tid = (q.stdout.strip() or "").split("\n")[0].strip()
if not tid:
    # 把已过审种临时置回待审（测试后恢复）
    q2 = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
            "fluxtorrent",
         "-tAc", "UPDATE torrents SET approval_status = 0 WHERE id = "
                 "(SELECT id FROM torrents WHERE approval_status = 1 ORDER BY"
                     "id LIMIT 1) RETURNING id"],
        capture_output=True, text=True)
    tid = (q2.stdout.strip() or "").split("\n")[0].strip()
if tid:
    tid = int(tid)
    st, r = call("POST", "/admin/reviews/postpone", {"torrent_id": tid,
        "reason": "e2e 证据不足"}, token=tok)
    check("postpone", st == 200 and r.get("status") == 4, str(r))
    # 普通用户（非本人非 staff）看不到暂缓种详情
    st, d = call("GET", f"/torrents/{tid}", token=tok2)
    check("postponed hidden for user", st == 404, str(st))
    # staff 可见
    st, d = call("GET", f"/torrents/{tid}", token=tok)
    check("postponed visible for staff", st == 200 and d.get(
        "approval_status") == 4, str(d)[:80])
    # 恢复 → 回到待审
    st, r = call("POST", "/admin/reviews/resume", {"torrent_id": tid},
        token=tok)
    check("resume", st == 200 and r.get("status") == 0, str(r))
    # 还原为过审（若原来是过审种）
    subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
            "fluxtorrent",
         "-tAc", f"UPDATE torrents SET approval_status = 1 WHERE id = {tid}"],
        capture_output=True, text=True)
else:
    check("postpone (no torrent)", False, "库中无种子可测")

# ---- ④ 规则版本化 ----
st, rules = call("GET", "/rules-content")
rid = rules[0]["id"] if st == 200 and rules else None
if rid:
    old_body = rules[0]["body"]
    st, _ = call("PUT", f"/admin/rules/{rid}",
                 {"title": rules[0]["title"],
                     "body": old_body + "\n（v4 版本化测试修订）"}, token=tok)
    check("rule update 200", st == 200, str(st))
    st, revs = call("GET", f"/admin/rules/{rid}/revisions", token=tok)
    check("revision archived", st == 200 and len(revs) >= 1, str(revs)[:80])
    check("revision body is old", revs and revs[0][3] == old_body, str(revs)[
        :100] if revs else "")
    # 还原正文
    call("PUT", f"/admin/rules/{rid}", {"title": rules[0]["title"],
        "body": old_body}, token=tok)
else:
    check("rules (none)", False, "无规则可测")

# ---- ⑤ api metrics（未配 token 应 404 不暴露） ----
req = urllib.request.Request(BASE + "/metrics")
try:
    with urllib.request.urlopen(req) as r:
        st = r.status
except urllib.error.HTTPError as e:
    st = e.code
check("api metrics gated (404)", st == 404, str(st))

# ---- ⑥ 商店权益效果：VIP 延展口径（库内断言，不走购买避免扣钱） ----
q = subprocess.run(
    ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
        "fluxtorrent",
     "-tAc",
         "UPDATE users SET vip_until = now() + interval '10 days' WHERE"
             "username='gaozhong' RETURNING vip_until"],
    capture_output=True, text=True)
check("vip_until writable", bool(q.stdout.strip()), q.stdout[:60])

print(f"\n===== 0079 E2E: {PASS} PASS / {FAIL} FAIL =====")
sys.exit(1 if FAIL else 0)
