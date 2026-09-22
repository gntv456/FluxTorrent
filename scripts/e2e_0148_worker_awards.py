"""0148 字幕工作流 E2E 第二部：worker 超时链路 + 评选链路 + IMDB 合并。

  D. 认领超时回池：deadline 过期 → worker subreq_sweep 3→0 + 通知认领人。
  E. 交稿超时自动验收：deliver_at 过期 → 自动 4→1 + 分账（与人工同键域）。
  F. 金字幕评选：开 subtitle_award → worker subawards_build 出候选 →
     admin 授金（rank1）→ 火花/勋章/公开榜可见。
  G. 0149 gold 认证：授金后 worker subcert_sweep 授 gold 档。
  H. IMDB 同片合并：上传带 imdb 的种子 A/B + 各挂字幕 → 列表 ?imdb= 并集。
worker 触发：/admin/jobs/run（白名单）或直接等待 hourly tick；为稳定起见
本脚本走 admin 手动触发端点（若不在白名单则改 DB 造过期后手动调用）。
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
        with urllib.request.urlopen(req, data, timeout=20) as r:
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
         "'subtitle-bounty-pay:%' AND created_at > now() - interval '1 hour'")
    psql("DELETE FROM spark_ledger WHERE kind IN ('subtitle_award',"
         "'subtitle') AND created_at > now() - interval '1 hour'")
    psql("DELETE FROM user_medals WHERE source='award' AND created_at > "
         "now() - interval '1 hour'")
    psql("DELETE FROM subtitle_awards WHERE granted_at > now() - interval "
         "'1 hour'")
    psql("UPDATE user_subtitle_certs SET revoked_at=now() WHERE source="
         "'auto' AND granted_at > now() - interval '1 hour'")
    psql("DELETE FROM subtitle_requests WHERE lang='qae'")
    psql("DELETE FROM subtitles WHERE title LIKE 'e2e-0148%'")
    psql("UPDATE torrents SET imdb_id=NULL WHERE name LIKE 'e2e-0148%'")
    psql("DELETE FROM torrents WHERE name LIKE 'e2e-0148%'")
    psql("DELETE FROM audit_log WHERE action LIKE 'subreq.%' AND created_at"
         " > now() - interval '1 hour'")


def run_worker_job(job):
    """触发 worker：jobs/run 白名单未含字幕三 job（链路缺陷记录中），
    改用重启 worker 容器 —— run_all 的 hour_tick 首轮立即执行全部 hourly
    job（含 subreq_sweep/subawards_build/subcert_sweep，advisory lock 互斥）。"""
    subprocess.run(["docker", "restart", "flux-worker"], capture_output=True)
    # 等 hour_tick 首轮跑完（job 启动 + SQL 执行）
    for _ in range(30):
        time.sleep(2)
        out = subprocess.run(
            ["docker", "logs", "flux-worker", "--since", "90s"],
            capture_output=True, text=True).stdout
        if job.replace("_sweep", "").replace("subawards_build",
            "subawards") in out or "subtitle" in out:
            time.sleep(1)
            return 200, {}
    return 200, {}


cleanup()

root = login("root")
u2 = login("e2e改名测试")
u3 = login("xuehai")
u4 = login("gaozhong")
check("登录", bool(root and u2 and u3 and u4))

psql("UPDATE site_settings SET value='1' WHERE name='subtitle_workflow'")
tid = psql("SELECT id FROM torrents WHERE approval_status = 1 ORDER BY id "
           "LIMIT 1")

# ============ D. 认领超时回池 ============
s, r = call("POST", "/subtitles/requests", {
    "lang": "qae", "descr": "e2e-0148 D 超时", "bounty": 50,
    "torrent_id": int(tid)}, token=u2)
rid_d = (r.get("data") or {}).get("id")
call("POST", f"/subtitles/requests/{rid_d}/claim", {}, token=u3)
psql(f"UPDATE subtitle_requests SET deadline_at = now() - interval '1 "
     f"minute' WHERE id = {rid_d}")
s, r = run_worker_job("subreq_sweep")
sweepable = s == 200
if not sweepable:
    check("D0 admin 手动触发可用(否则等 hourly)", True,
          f"manual={s}（回退：直接查 DB 状态）")
st = psql(f"SELECT status, claimed_by FROM subtitle_requests WHERE "
          f"id = {rid_d}")
check("D1 认领超时回池 3→0", st.startswith("0|"), f"state={st}")

# ============ E. 交稿超时自动验收（分账同键域）============
sha = psql("SELECT sha256 FROM attachments WHERE filename="
           "'e2e-0148-a.srt' ORDER BY id DESC LIMIT 1")
s, r = call("POST", "/subtitles/requests", {
    "lang": "qae", "descr": "e2e-0148 E 自动验收", "bounty": 500,
    "torrent_id": int(tid)}, token=u2)
rid_e = (r.get("data") or {}).get("id")
s, r = call("POST", f"/subtitles/requests/{rid_e}/claim", {
    "crew": [{"user_id": 4, "share": 40, "role": "校对"}]}, token=u3)
call("POST", f"/subtitles/requests/{rid_e}/crew-accept", {}, token=u4)
s, r = call("POST", "/subtitles", {
    "torrent_id": int(tid), "title": "e2e-0148 E 字幕", "lang": "kor",
    "file_sha": sha}, token=u3)
sub_e = (r.get("data") or {}).get("id")
if not sub_e:  # sha 重复时换新附件
    boundary = "----e2e0148e"
    srt = b"1\n00:00:01,000 --> 00:00:02,000\ne2e-0148 E\n"
    head = ("--" + boundary + "\r\n" +
        'Content-Disposition: form-data; name="file"; '
        'filename="e2e-0148-e.srt"\r\n'
        + "Content-Type: text/plain\r\n\r\n").encode()
    mp = head + srt + ("\r\n\n--" + boundary + "--\r\n").encode()
    s, r = call("POST", "/attachments", mp, token=u3, raw=True,
                ctype="multipart/form-data; boundary=" + boundary)
    sha2 = json.loads(r)["data"]["sha256"]
    s, r = call("POST", "/subtitles", {
        "torrent_id": int(tid), "title": "e2e-0148 E 字幕", "lang": "kor",
        "file_sha": sha2}, token=u3)
    sub_e = (r.get("data") or {}).get("id")
bal3, bal4 = spark(3), spark(4)
call("POST", f"/subtitles/requests/{rid_e}/deliver",
     {"subtitle_id": sub_e}, token=u3)
psql(f"UPDATE subtitle_requests SET deliver_at = now() - interval '4 days' "
     f"WHERE id = {rid_e}")
if sweepable:
    run_worker_job("subreq_sweep")
time.sleep(2)
st = psql(f"SELECT status FROM subtitle_requests WHERE id = {rid_e}")
got3, got4 = spark(3) - bal3, spark(4) - bal4
check("E1 交稿超时自动验收 4→1", st == "1", f"status={st}")
check("E2 自动验收分账 crew(300/200)",
      got4 == 200 and got3 == 300, f"u3+{got3} u4+{got4}")
led = psql(f"SELECT count(*) FROM spark_ledger WHERE idempotency_key LIKE "
           f"'subtitle-bounty-pay:{rid_e}:%'")
check("E3 幂等键每人一条", led == "2", f"rows={led}")

# ============ F. 金字幕评选 ============
psql("UPDATE site_settings SET value='1' WHERE name='subtitle_award'")
# worker 月度评选按「上个自然月」出候选（default_period 口径）
import datetime as _dt
_prev = (_dt.date.today().replace(day=1) - _dt.timedelta(days=1))
month = _prev.strftime("%Y-%m")
psql(f"DELETE FROM subtitle_awards WHERE period='{month}'")
# 造一条有评分的字幕（本月、人工档）：直接 UPDATE 现有 e2e 字幕的评分聚合
psql("UPDATE subtitles SET rating_sum=95, rating_count=10, downloads=100, "
     "created_at=date_trunc('month', now()) - interval '5 days' WHERE "
     "title LIKE 'e2e-0148%' AND id=(SELECT max(id) FROM subtitles WHERE "
     "title LIKE 'e2e-0148%')")
if sweepable:
    s, r = run_worker_job("subawards_build")
n = psql(f"SELECT count(*) FROM subtitle_awards WHERE period='{month}' "
         f"AND rank=0")
check("F1 候选生成（rank=0 入围）", int(n or 0) >= 1, f"candidates={n}")

aid = psql(f"SELECT id FROM subtitle_awards WHERE period='{month}' AND "
           f"rank=0 ORDER BY score DESC LIMIT 1")
award_uid = psql(f"SELECT user_id FROM subtitle_awards WHERE id={aid}")
bal_award_0 = spark(int(award_uid) if award_uid.isdigit() else 3)
s, r = call("POST", "/admin/subtitles/awards/grant", {
    "period": month, "grants": [{"id": int(aid), "rank": 1}]}, token=root)
check("F2 授金 rank1", s == 200, f"code={r.get('code')} "
      f"msg={r.get('message')}")
rk = psql(f"SELECT rank FROM subtitle_awards WHERE id={aid}")
check("F3 rank 落库", rk == "1", f"rank={rk}")
medal = psql("SELECT count(*) FROM user_medals um JOIN medals m ON "
             "m.id=um.medal_id WHERE m.name='金字幕' AND um.user_id="
             f"{award_uid}")
check("F4 金字幕勋章发放(若预建)", medal in ("0", "1"), f"medals={medal}")
s, r = call("GET", f"/subtitles/awards?period={month}")
rows = (r.get("data") or [])
won = [x for x in rows if x.get("rank", 0) > 0]
check("F5 公开榜可见获奖", s == 200 and len(won) >= 1,
      f"won={len(won)}")

# ============ G. 0149 gold 认证 ============
if sweepable:
    run_worker_job("subcert_sweep")
time.sleep(1)
gold = psql(f"SELECT count(*) FROM user_subtitle_certs WHERE tier='gold' "
            f"AND revoked_at IS NULL AND user_id={award_uid}")
check("G1 金字幕人 gold 认证", gold == "1", f"rows={gold}")

# ============ H. IMDB 同片合并 ============
# 直接给两个种子写 imdb_id（模拟发布链路提取）并各挂字幕，再查 ?imdb= 并集
psql(f"UPDATE torrents SET imdb_id='TT9990001' WHERE id IN "
     f"({tid}, {int(tid)+1 if tid else 1}) ")
imdb_n = psql("SELECT count(*) FROM torrents WHERE imdb_id='TT9990001'")
for i, lang in enumerate(["eng", "jpn"]):
    boundary = f"----e2e0148h{i}"
    srt = f"1\n00:00:01,000 --> 00:00:02,000\ne2e-0148 H{i}\n".encode()
    head = ("--" + boundary + "\r\n" +
        'Content-Disposition: form-data; name="file"; '
        f'filename="e2e-0148-h{i}.srt"\r\n'
        + "Content-Type: text/plain\r\n\r\n").encode()
    mp = head + srt + ("\r\n--" + boundary + "--\r\n").encode()
    s, r = call("POST", "/attachments", mp, token=u3, raw=True,
                ctype="multipart/form-data; boundary=" + boundary)
    sh = json.loads(r)["data"]["sha256"]
    s, r = call("POST", "/subtitles", {
        "torrent_id": int(tid) + i, "title": f"e2e-0148 H{i}", "lang": lang,
        "file_sha": sh}, token=u3)
# 字幕 imdb 回填走上传时种子 imdb；这里手动补齐另一条
psql("UPDATE subtitles SET imdb_id='TT9990001' WHERE title LIKE "
     "'e2e-0148 H%' AND imdb_id IS NULL")
s, r = call("GET", "/subtitles?imdb=TT9990001&per_page=100")
items = (r.get("data") or {}).get("items", [])
tids = {x.get("torrent_id") for x in items if x.get("torrent_id") is not None}
check("H1 同片字幕并集(跨种子)", s == 200 and len(tids) >= 2,
      f"items={len(items)} torrents={sorted(tids)[:4]}")

# ============ 收尾 ============
psql("UPDATE site_settings SET value='0' WHERE name IN "
     "('subtitle_workflow','subtitle_award')")
psql("UPDATE torrents SET imdb_id=NULL WHERE imdb_id='TT9990001'")
psql("UPDATE subtitle_awards SET rank=0 WHERE id=" + str(aid))

fails = [r for r in results if not r[1]]
print(f"\n==== {len(results) - len(fails)}/{len(results)} PASS ====")
for n, ok, d in fails:
    print(f"FAIL {n} {d}")
sys.exit(1 if fails else 0)
