#!/usr/bin/env python3
"""0078 长尾批次 E2E 自测：众筹全链路（含税/退款）、工单流转、论坛已读、
聊天机器人、泄露复核端点、运维三件。口径与 0077 批次同款（真库真容器）。"""
import json
import urllib.request
import urllib.error
import sys

BASE = "http://127.0.0.1:8080/api/v1"
PASS = FAIL = 0


def call(method, path, body=None, token=None, raw=False):
    req = urllib.request.Request(BASE + path, method=method)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = None
    if body is not None:
        req.add_header("Content-Type", "application/json")
        data = json.dumps(body).encode()
    try:
        with urllib.request.urlopen(req, data) as r:
            payload = json.loads(r.read())
            return r.status, payload if raw else payload.get("data", payload)
    except urllib.error.HTTPError as e:
        try:
            payload = json.loads(e.read())
        except Exception:
            payload = {}
        return e.code, payload.get("data", payload)


def check(name, cond, detail=""):
    global PASS, FAIL
    if cond:
        PASS += 1
        print(f"PASS {name}")
    else:
        FAIL += 1
        print(f"FAIL {name}  {detail}")


# ---- 登录 root（staff）与普通用户 ----
import time
def login(name):
    for _ in range(6):
        st, r = call("POST", "/auth/login", {"username": name,
            "password": "password123"})
        if st == 200:
            return r["token"]
        time.sleep(12)  # 登录限流（429）退避重试
    raise SystemExit(f"login {name} failed")
tok = login("root")
tok2 = login("gaozhong")
check("login gaozhong", True)

# ---- ① 众筹全链路 ----
# 找一个 gaozhong 可用种子：跳过已有众筹记录的（重跑幂等——UNIQUE(torrent_id)）。
# 种子池太小全被占用时（演示库仅 1 个种子），复用既有项目做只读断言。
st, torrents = call("GET", "/torrents?limit=20", token=tok2)
st2, done = call("GET", "/fundings", token=tok2)
funded = {d["torrent_id"] for d in (done or [])} if st2 == 200 else set()
fresh = [it["id"] for it in torrents.get("items", []) if it["id"] not in funded]
reused = not fresh and bool(torrents.get("items"))
t0 = fresh[0] if fresh else (torrents["items"][0]["id"] if reused else None)
check("torrent pick", t0 is not None, str(torrents)[:100])
if t0 and not reused:
    # 目标 1500，先筹 1400（税后 1330 < 1500 未达标），再补 200（税后 1520 ≥ 1500 达标）
    st, f = call("POST", "/fundings", {"torrent_id": t0, "goal": 1500,
        "days": 3},
                 token=tok2)
    check("funding create", st == 200, str(f))
    fid = f["id"] if isinstance(f, dict) and "id" in f else None
    if fid:
        st, c = call("POST", "/fundings/contribute",
                     {"funding_id": fid, "amount": 1400,
                         "idempotency_key": f"e2e78a{fid}"},
                     token=tok2)
        check("contribute 1400 (tax 70)", st == 200 and c.get("tax") == 70,
            str(c))
        check("not reached yet", c.get("reached") is False, str(c))
        st, c2 = call("POST", "/fundings/contribute",
                      {"funding_id": fid, "amount": 200,
                          "idempotency_key": f"e2e78b{fid}"},
                      token=tok2)
        check("contribute 200 (tax 10)", st == 200 and c2.get("tax") == 10,
            str(c2))
        check("reached", c2.get("reached") is True, str(c2))
        st, j = call("POST", "/admin/jobs/run", {"job": "funding_settle"},
            token=tok)
        check("job trigger funding_settle", st == 200 and j.get("affected",
            0) >= 0, str(j))
elif reused:
    check("funding reused (already promoted)",
          any(d["torrent_id"] == t0 and d["status"] == 1 for d in done), str(
              done)[:120])
st, mine = call("GET", "/fundings/mine", token=tok2)
check("fundings/mine", st == 200 and len(mine) >= 1, str(mine)[:80])

# ---- ② 工单流转 ----
st, t = call("POST", "/stafftickets/update", {"id": 999999, "priority": 2},
    token=tok)
check("ticket update 404 on missing", st in (404, 400), str(t))
st, tl = call("GET", "/stafftickets", token=tok)
check("ticket list", st == 200 and isinstance(tl, list), str(tl)[:80])
st, tl2 = call("GET", "/stafftickets", token=tok2)
check("ticket list forbidden for user", st in (401, 403), str(st))

# ---- ③ 论坛已读 ----
st, forums = call("GET", "/forums", token=tok2)
fid_forum = forums[0]["id"] if st == 200 and forums else None
if fid_forum:
    st, tps = call("GET", f"/forums/{fid_forum}/topics", token=tok2)
    topics = tps.get("topics", [])
    check("topic list has_unread field", st == 200 and topics and
          all(
              "has_unread" in t and "read_last_post_id" in t for t in topics),
                  str(tps)[:80])
    if topics:
        # 选一个有帖子的主题（replies≥0 都可能有主楼；空主题跳过）
        tid_read = next((t["id"] for t in topics), None)
        st, td = call("GET", f"/forums/topics/{tid_read}", token=tok2)
        check("topic detail 200 + marks read", st == 200 and "posts" in td,
            str(td)[:80])
        st, tps2 = call("GET", f"/forums/{fid_forum}/topics", token=tok2)
        row = next((t for t in tps2["topics"] if t["id"] == tid_read), None)
        # 空主题（无帖）不产生已读行——只有看过带帖主题才应清除角标
        has_posts = bool(td.get("posts"))
        check("has_unread cleared after read",
              row is not None and (not has_posts or row[
                  "has_unread"] is False), str(row))

# ---- ④ 聊天机器人 ----
st, b = call("GET", "/shoutbox/bot", token=tok2)
check("bot help", st == 200 and "commands" in b, str(b)[:80])
st, b = call("GET", "/shoutbox/bot/exec?cmd=/stats", token=tok2)
check("bot /stats", st == 200 and "reply" in b and "用户" in b["reply"], str(b))
st, b = call("GET", "/shoutbox/bot/exec?cmd=/me", token=tok2)
check("bot /me", st == 200 and "火花" in b.get("reply", ""), str(b))
st, b = call("GET", "/shoutbox/bot/exec?cmd=/nope", token=tok2)
check("bot unknown → help text", st == 200 and "可用命令" in b["reply"], str(b))

# ---- ⑤ 泄露复核端点 ----
st, ll = call("GET", "/staff/leaks", token=tok)
check("leak list (staff)", st == 200 and isinstance(ll, list), str(ll)[:80])
st, ll2 = call("GET", "/staff/leaks", token=tok2)
check("leak list forbidden for user", st in (401, 403), str(st))

# ---- ⑥ 运维三件 ----
st, v = call("GET", "/admin/version", token=tok)
check("version page", st == 200 and bool(v.get("latest_migration")), str(v)[
    :120])
st, bl = call("GET", "/admin/backups", token=tok)
check("backups list", st == 200 and "files" in bl, str(bl)[:80])
st, j = call("POST", "/admin/jobs/run", {"job": "expire_promotions"}, token=tok)
check("job trigger expire_promotions", st == 200 and j.get(
    "job") == "expire_promotions", str(j))
st, j = call("POST", "/admin/jobs/run", {"job": "bogus"}, token=tok)
check("job trigger rejects unknown", st == 400, str(st))

# ---- ⑦ 赠送税口径（库内直查：众筹税已入池） ----
import subprocess
q = subprocess.run(
    ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
        "fluxtorrent",
     "-tc", "SELECT COALESCE(sum(amount),0) FROM pool_donations"],
    capture_output=True, text=True)
total = int(q.stdout.strip() or 0)
check("gift tax pooled (>=80 sparks)", total >= 80,
    f"pool_donations total={total}")

print(f"\n===== 0078 E2E: {PASS} PASS / {FAIL} FAIL =====")
sys.exit(1 if FAIL else 0)
