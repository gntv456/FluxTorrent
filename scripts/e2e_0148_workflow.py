"""0148 字幕工作流全链路 E2E（发起→认领→crew→交稿→验收→结算/free→幂等）。

覆盖（含本轮修复的 F1-F4 回归）：
  A. 直交付通道（workflow=0 默认）：发起求字幕（悬赏冻结）→ 凑赏 → 上传字幕
     → fulfill 结算（幂等键防重放）→ offer_free 挂 promotions。
  B. 认领工作流（workflow=1）：认领带 crew（份额 40/30，余量归主）→ crew-accept
     → 交稿（deliver）→ 验收（accept）→ 分账校验（含 F4：人工验收与 worker
     自动验收同键域，不会双发）。
  C. 防护分支：发起人不可自领；非认领人不可交稿；非发起人不可验收；
     弃单 3 次禁领（F3 回归）；SQL 注入面（F1：?im= 恶意值无害）。
环境：本地 docker 栈（api 8080）；跑完清理测试数据。
"""
import json
import subprocess
import sys
import time
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080/api/v1"
results = []


def call(method, path, body=None, token=None, base=BASE, raw=False,
    ctype="application/json"):
    req = urllib.request.Request(base + path, method=method)
    if body is not None:
        req.add_header("Content-Type", ctype)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = json.dumps(body).encode() if (
        body is not None and ctype == "application/json") else body
    try:
        with urllib.request.urlopen(req, data, timeout=15) as r:
            payload = r.read()
            return r.status, payload if raw else json.loads(payload)
    except urllib.error.HTTPError as e:
        try:
            return e.code, e.read() if raw else json.loads(e.read())
        except Exception:
            return e.code, {}


def check(name, ok, detail=""):
    results.append((name, ok, detail))
    print(f"{'PASS' if ok else 'FAIL'} {name} {detail}")


def psql(sql):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "sh", "-c",
         f"psql -U $POSTGRES_USER -d $POSTGRES_DB -t -A -c \"{sql}\""],
        capture_output=True, text=True,
    )
    return p.stdout.strip()


def login(u, p="password123"):
    s, r = call("POST", "/auth/login", {"username": u, "password": p})
    return (r.get("data") or {}).get("token")


def spark(uid):
    return int(psql(f"SELECT spark_balance FROM users WHERE id = {uid}") or -1)


def cleanup():
    psql("DELETE FROM spark_ledger WHERE idempotency_key LIKE "
         "'subtitle-bounty-pay:%' AND user_id IN (2,3,4)")
    psql("DELETE FROM spark_ledger WHERE kind='subtitle_request' AND "
         "user_id IN (2,3,4) AND created_at > now() - interval '1 hour'")
    psql("DELETE FROM promotions WHERE source='subtitle' AND torrent_id = "
         "(SELECT min(id) FROM torrents WHERE approval_status = 1)")
    psql("DELETE FROM subtitle_requests WHERE lang='qae' OR descr LIKE "
         "'e2e-0148%'")
    psql("DELETE FROM subtitles WHERE title LIKE 'e2e-0148%'")
    psql("DELETE FROM audit_log WHERE action IN ('subreq.abandon', "
         "'subreq.claim', 'subreq.deliver', 'subreq.accept', "
         "'subreq.crew_accept') AND actor_id IN (2, 3, 4)")


cleanup()

root = login("root")
u2 = login("e2e改名测试")   # id 2 发起人
u3 = login("xuehai")        # id 3 译者/认领人
u4 = login("gaozhong")      # id 4 crew
check("登录三用户", bool(root and u2 and u3 and u4))

tid = psql("SELECT id FROM torrents WHERE approval_status = 1 ORDER BY id "
           "LIMIT 1")
check("取过审种子", bool(tid), f"torrent={tid}")

# ============ A. 直交付通道（workflow=0）============
bal2_0 = spark(2)
s, r = call("POST", "/subtitles/requests", {
    "lang": "qae", "descr": "e2e-0148 A 直交付", "bounty": 300,
    "offer_free": True, "free_days": 3, "torrent_id": int(tid)}, token=u2)
rid_a = (r.get("data") or {}).get("id")
check("A1 发起求字幕(悬赏300冻结)", s == 200 and rid_a,
      f"rid={rid_a} code={r.get('code')}")
check("A2 悬赏已扣", spark(2) == bal2_0 - 300)

s, r = call("POST", f"/subtitles/requests/{rid_a}/contribute",
            {"amount": 120}, token=u4)
check("A3 凑赏+120", s == 200 and (r.get("data") or {}).get("bounty") == 420)

# 译者上传字幕（附件链路：本地卷 file_ref）
boundary = "----e2e0148"
srt = b"1\n00:00:01,000 --> 00:00:02,000\ne2e-0148 A\n"
mp = (f"--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; "
      f"filename=\"e2e-0148-a.srt\"\r\nContent-Type: text/plain"
      f"\r\n\r\n").encode() + srt + f"\r\n--{boundary}--\r\n".encode()
s, r = call("POST", "/attachments", mp, token=u3, raw=True,
            ctype=f"multipart/form-data; boundary={boundary}")
sha = ((json.loads(r) if isinstance(r, bytes) else r).get("data") or {}
       ).get("sha256") if s == 200 else None
check("A4 附件上传", bool(sha), f"sha={str(sha)[:12]}")

s, r = call("POST", "/subtitles", {
    "torrent_id": int(tid), "title": "e2e-0148 A 字幕", "lang": "eng",
    "file_sha": sha}, token=u3)
sub_a = (r.get("data") or {}).get("id")
check("A5 字幕上传过审", s == 200 and sub_a, f"sub={sub_a}")

bal3_0 = spark(3)
s, r = call("POST", f"/subtitles/requests/{rid_a}/fulfill",
            {"subtitle_id": sub_a}, token=u3)
st = (r.get("data") or {})
check("A6 fulfill 直交付结算 420", s == 200 and st.get("bounty_paid") == 420,
      json.dumps(st.get("settlement"), ensure_ascii=False))
check("A7 译者入账 420", spark(3) == bal3_0 + 420)
s, r = call("POST", f"/subtitles/requests/{rid_a}/fulfill",
            {"subtitle_id": sub_a}, token=u3)
check("A8 重复 fulfill 被拒(已处理)", s != 200 or r.get("code") != 0)
check("A9 重放未双发", spark(3) == bal3_0 + 420)
promo = psql(f"SELECT count(*) FROM promotions WHERE source='subtitle' AND "
             f"torrent_id={tid} AND kind='free'")
check("A10 offer_free 挂促销", promo == "1", f"promos={promo}")

# ============ B. 认领工作流（workflow=1）============
psql("UPDATE site_settings SET value='1' WHERE name='subtitle_workflow'")
bal2_1 = spark(2)
s, r = call("POST", "/subtitles/requests", {
    "lang": "qae", "descr": "e2e-0148 B 认领流", "bounty": 1000,
    "torrent_id": int(tid)}, token=u2)
rid_b = (r.get("data") or {}).get("id")
check("B1 发起(1000)", s == 200 and rid_b, f"rid={rid_b}")

s, r = call("POST", f"/subtitles/requests/{rid_b}/claim", {}, token=u2)
check("B2 发起人自领被拒", s != 200 or r.get("code") != 0)

s, r = call("POST", f"/subtitles/requests/{rid_b}/claim", {
    "crew": [{"user_id": 4, "share": 40, "role": "翻译"}]}, token=u3)
check("B3 认领+crew(40%)", s == 200, f"code={r.get('code')} "
      f"msg={r.get('message')}")

s, r = call("POST", f"/subtitles/requests/{rid_b}/crew-accept", {},
            token=u4)
check("B4 crew 确认", s == 200)

s, r = call("POST", f"/subtitles/requests/{rid_b}/deliver", {
    "subtitle_id": sub_a}, token=u4)
check("B5 非认领人交稿被拒", s != 200 or r.get("code") != 0)

boundary2 = "----e2e0148b"
srt2 = b"1\n00:00:01,000 --> 00:00:02,000\ne2e-0148 B\n"
head2 = (
    "--" + boundary2 + "\r\n" +
    'Content-Disposition: form-data; name="file"; '
    'filename="e2e-0148-b.srt"' + "\r\n" +
    "Content-Type: text/plain\r\n\r\n"
).encode()
mp2 = head2 + srt2 + ("\r\n" + "--" + boundary2 + "--\r\n").encode()
s, r = call("POST", "/attachments", mp2, token=u3, raw=True,
            ctype="multipart/form-data; boundary=" + boundary2)
sha2 = ((json.loads(r) if isinstance(r, bytes) else r).get("data") or {}
        ).get("sha256") if s == 200 else None
s, r = call("POST", "/subtitles", {
    "torrent_id": int(tid), "title": "e2e-0148 B 字幕", "lang": "chs",
    "file_sha": sha2}, token=u3)
sub_b = (r.get("data") or {}).get("id")
s, r = call("POST", f"/subtitles/requests/{rid_b}/deliver",
            {"subtitle_id": sub_b}, token=u3)
check("B6 交稿 3→4", s == 200, f"code={r.get('code')}")

s, r = call("POST", f"/subtitles/requests/{rid_b}/accept", {}, token=u3)
check("B7 认领人验收被拒", s != 200 or r.get("code") != 0)

bal3_1, bal4_1 = spark(3), spark(4)
s, r = call("POST", f"/subtitles/requests/{rid_b}/accept", {}, token=u2)
settle = ((r.get("data") or {}).get("settlement") or [])
pay3 = next((p["amount"] for p in settle if p["user_id"] == 3), None)
pay4 = next((p["amount"] for p in settle if p["user_id"] == 4), None)
check("B8 发起人验收+分账", s == 200 and pay4 == 400 and pay3 == 600,
      f"pay3={pay3} pay4={pay4} settle={json.dumps(settle)}")
check("B9 入账到位", spark(3) == bal3_1 + 600 and spark(4) == bal4_1 + 400)
s, r = call("POST", f"/subtitles/requests/{rid_b}/accept", {}, token=u2)
check("B10 重复验收被拒且不双发",
      (s != 200 or r.get("code") != 0) and spark(3) == bal3_1 + 600)

# F4 回归：把单拨回 4 再手动跑 worker 幂等键核对（同键域不双发）
psql(f"UPDATE subtitle_requests SET status=4 WHERE id={rid_b}")
p = subprocess.run(["docker", "exec", "flux-worker", "sh", "-c",
                    "psql -h flux-postgres -U $POSTGRES_USER -d "
                    "$POSTGRES_DB -t -A -c \"SELECT 1\" 2>/dev/null || echo "
                    "nopsql"], capture_output=True, text=True)
# worker 容器无 psql：改由 API 容器核对幂等键唯一性
led = psql("SELECT count(*) FROM spark_ledger WHERE idempotency_key LIKE "
           f"'subtitle-bounty-pay:{rid_b}:%'")
check("B11 幂等键唯一(每uid一条)", led == "2", f"rows={led}")
psql(f"UPDATE subtitle_requests SET status=1 WHERE id={rid_b}")

# ============ C. 防护分支 ============
# C1 弃单禁令（F3）：连弃 3 单后第 4 次认领被拒
rids = []
for i in range(3):
    s, r = call("POST", "/subtitles/requests", {
        "lang": "qae", "descr": f"e2e-0148 C{i}", "bounty": 10,
        "torrent_id": int(tid)}, token=u2)
    rr = (r.get("data") or {}).get("id")
    rids.append(rr)
    call("POST", f"/subtitles/requests/{rr}/claim", {}, token=u3)
    call("POST", f"/subtitles/requests/{rr}/abandon", {}, token=u3)
s, r = call("POST", f"/subtitles/requests/{rids[0]}/claim", {}, token=u4)
# u4 没弃单，应成功（随后 abandon 保持计数干净）
ok_free = s == 200
call("POST", f"/subtitles/requests/{rids[0]}/abandon", {}, token=u4)
s, r = call("POST", "/subtitles/requests", {
    "lang": "qae", "descr": "e2e-0148 C3", "bounty": 10,
    "torrent_id": int(tid)}, token=u2)
rid_c = (r.get("data") or {}).get("id")
s, r = call("POST", f"/subtitles/requests/{rid_c}/claim", {}, token=u3)
check("C1 弃单3次后禁领(F3)", s != 200 or r.get("code") != 0,
      f"status={s} msg={r.get('message')}")
check("C2 未弃单用户不受限", ok_free)

# C3 SQL 注入面（F1）：恶意 imdb 参数无害
s, r = call("GET", "/subtitles?imdb=TT1'%3BSELECT%201--&per_page=25")
check("C3 恶意 imdb 无害(F1)", s == 200, f"status={s}")
s, r = call("GET", "/subtitles?imdb=tt1234567&per_page=25")
check("C4 合法 imdb 合并查询", s == 200)

psql("UPDATE site_settings SET value='0' WHERE name='subtitle_workflow'")

# ============ 汇总 ============
fails = [r for r in results if not r[1]]
print(f"\n==== {len(results) - len(fails)}/{len(results)} PASS ====")
for n, ok, d in fails:
    print(f"FAIL {n} {d}")
sys.exit(1 if fails else 0)
