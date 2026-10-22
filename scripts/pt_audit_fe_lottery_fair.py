# -*- coding: utf-8 -*-
"""FE 闸门：论坛抽奖「可验证公平」链路（批一百六十八新增）。
三段生死：① 售票把「占位行 / 票据哈希链 / 票价」同事务落库——旧写法先
autocommit 插占位，再让 `issue_ticket` 自开事务去插那条冲突必中的
ON CONFLICT DO NOTHING（⇒ 哈希永不落库、链恒空），扣费又只判 Err 而放过
Replayed；② `store_proof` / `mark_won` 落库失败要出声，否则验证页永远停在
「开售中」而奖其实已开完；③ 生产者缺口（`open_fair_round` 零调用方，
P-396）按钉桩判，不许做半接线。只跑静态：FE_LIVE=0。
"""
import hashlib
import io
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import call, login, ok, summary, clear_register_rl  # noqa

LIVE = os.environ.get("FE_LIVE", "1") != "0"
JOIN = "apps/api/src/community_http/lottery.rs"
DRAW = "apps/api/src/community_http/lottery_draw.rs"
FAIR = "apps/api/src/repo/lottery_fair.rs"
CREATE = "apps/api/src/community_http/topic_create.rs"
SEED = "cd" * 32
GENESIS = "0" * 64
RUN = int(time.time()) % 100000   # 探针号名要按轮次唯一，否则重跑撞「用户名已占用」
PROBE_UIDS: list = []

def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()

def cut(s):
    """剥 // 行注释：不先剥就会被自己刚写下的说明文字喂出假数据。"""
    return "\n".join(
        ln for ln in s.split("\n") if not ln.strip().startswith("//"))

def ticket_hash(seed, prev, nonce):
    h = hashlib.sha256()
    h.update(seed.encode())
    h.update(b"|")
    h.update(prev.encode())
    h.update(b"|")
    h.update(nonce.to_bytes(4, "big"))
    return h.hexdigest()

# ---------- 静态：① 同事务 + 键锚行 ----------
jt, ft, dt = cut(rd(JOIN)), cut(rd(FAIR)), cut(rd(DRAW))
ok("FE1 join 走单事务并自持场次 advisory 锁",
   "let mut tx" in jt and ".begin()" in jt and "tx.commit()" in jt
   and "pg_advisory_xact_lock" in jt,
   "三段各自 autocommit 的旧形状＝扣不掉也白占名额、链断了照样回一张票")
ok("FE2 票价键锚在行上（created_at 微秒）且 Replayed 走 fail-close",
   "timestamp_micros" in jt and "spend_spark_tx" in jt
   and "SpendOutcome::Spent" in jt,
   "常量键 forum-lottery-ticket:{uid}:{topic} 配可删的占位行＝白拿/死端两端")
ok("FE3 补偿式 DELETE 占位行不得复活（由事务回滚取代）",
   "DELETE FROM lottery_entries" not in jt,
   "靠补偿 DELETE 回滚时，删除一失败就留下一张没付钱的票")
ok("FE4 售票并入调用方事务：issue_ticket_tx 里不许再有 begin",
   "issue_ticket_tx" in ft and "issue_ticket_tx" in jt
   and ".begin()" not in ft.split("fn issue_ticket_tx")[1][:800],
   "自开事务看不到调用方的占位行，只能靠 ON CONFLICT 兜住——然后就空转")
ok("FE5 旧死因不得复活：票据那条语句带 (topic_id, user_id) DO NOTHING",
   "(topic_id, user_id) DO NOTHING" not in ft,
   "占位行已存在 ⇒ 那条 INSERT 必然冲突 ⇒ 哈希恒 NULL、链恒空")
ok("FE6 链补不上要报错，不许回一张空票",
   "未能接上哈希链" in ft,
   "空串回给用户＝「响亮地给出假证明」")

# ---------- 静态：② 落库失败要出声 ----------
ok("FE7 store_proof / mark_won 两处 `let _ =` 不得复活",
   "let _ = repo_fair::store_proof" not in dt
   and "let _ = repo_draw::mark_won" not in dt,
   "证明没落库＝验证页永远停在开售中；标记没落库＝已派彩的人显示未中奖")

# ---------- 静态：③ 生产者在册缺口（钉桩） ----------
def rs_all():
    for dp, _d, fs in os.walk(os.path.join(ROOT, "apps", "api", "src")):
        for fn in fs:
            if fn.endswith(".rs"):
                yield os.path.relpath(os.path.join(dp, fn), ROOT).replace(
                    "\\", "/")

callers = sum("open_fair_round(" in cut(rd(p)) for p in rs_all()
              if not p.endswith("repo/lottery_fair.rs"))
ok("FE8 钉桩：公平场生产者仍是 0 个调用方（P-396 未结）", callers == 0,
   "接线时把 FE8/FE9 与验证页的「未登记」判定一起改，别做半接线")
ct = cut(rd(CREATE))
ok("FE9 建帖口不得悄悄塞一个「看着像公平」的假字段",
   not re.search(r"(?i)fair", ct.replace("forum", "")),
   "语义没对上的档宁可不接；只写 commit 不写 seed 的接线比不接更坏")

# ---------- 活体：售票 → 链 → 开奖 → 复算 ----------
CLEAN = []
tid = 0
ROOT_BAL0 = None


def sql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        raise RuntimeError("psql 失败: " + (r.stderr or "")[:300])
    return (r.stdout or "").strip()


def dat(p):
    return p.get("data") or {}


def login_backoff(user, tries=10):
    for _ in range(tries):
        s, r = call("POST", "/auth/login",
                    {"username": user, "password": "password123"})
        tok = dat(r).get("token")
        if tok:
            return tok
        time.sleep(8 if r.get("code") == 1015 or s == 429 else 3)
    return None


def new_member(seq):
    clear_register_rl()
    time.sleep(6)
    codes = dat(call("POST", "/admin/invites",
                     {"user_id": 1, "count": 1}, token=ROOT_TOK)[1]).get(
        "codes") or []
    if not codes:
        return None
    code, cid = codes[0]["code"], codes[0]["id"]
    cap = dat(call("GET", "/auth/captcha", token=ROOT_TOK)[1])
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None
    name = "fe%d%s" % (seq, RUN)
    s, r = call("POST", "/auth/register", {
        "username": name, "email": "%s@fe.local" % name,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    uid = dat(r).get("user_id")
    if not uid:
        print("   [FE 造号失败] http=%s %s" % (s, str(r)[:160]))
        return None
    PROBE_UIDS.append(int(uid))
    CLEAN.append("UPDATE users SET status = 2 WHERE id = %s" % uid)
    CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
    tok = login_backoff(name)
    return (tok, int(uid)) if tok else None


if LIVE:
    ROOT_TOK = login("root")
    ok("FE-L0 root 登录可用", bool(ROOT_TOK), "多半卡在 PoW/限流")
    if ROOT_TOK:
        ROOT_BAL0 = sql("SELECT spark_balance FROM users WHERE id=1")
        CLEAN.append("UPDATE users SET spark_balance = %s WHERE id = 1"
                     % ROOT_BAL0)
        # 建抽奖主题要向发起人扣押金。root 的余额被历轮活体闸门花成了负数
        # （本轮实测 -25006 ⇒ 409 code=4001「余额不足」），FE-L1 一红，
        # 后面 7 条（票落库/重复参与/链复算/派彩/验证页）全是下游假红。
        # 探针期垫到 5000，收尾按 ROOT_BAL0 原值还原（CLEAN 已挂，倒序执行）。
        sql("UPDATE users SET spark_balance = 5000 WHERE id = 1")
        fid = int(sql("SELECT min(id) FROM forums WHERE minclasscreate=0"))
        s, r = call("POST", "/forums/topics", {
            "forum_id": fid, "title": "FE188 fair lottery probe",
            "body": "probe body, removed on exit.", "topic_type": "lottery",
            "lottery_winners": 1, "lottery_prize": 10,
            "lottery_ticket": 5, "lottery_hours": 6}, token=ROOT_TOK)
        tid = int(dat(r).get("topic_id") or dat(r).get("id") or 0)
        f0 = dat(call("GET", "/forums/lottery/%d/fairness" % tid)[1])
        pk = sql("SELECT coalesce(string_agg(kind,','),'空') FROM spark_ledger "
                 "WHERE idempotency_key LIKE 'forum-lottery-pool:%%:%d'" % tid)
        ok("FE-L1 建出主题：未登记承诺→exists=%s、奖池 kind=%s（tid=%d）"
           % (f0.get("exists"), pk, tid),
           tid > 0 and f0.get("exists") is False and pk == "forum_lottery",
           "http=%s %s｜旧形状：exists=true 演「有承诺」；池子记成 "
           "forum_bounty ⇒ 明细标成悬赏、按 kind 对账永不平" % (s, str(r)[:120]))
        CLEAN += [
            "DELETE FROM lottery_entries WHERE topic_id = %d" % tid,
            "DELETE FROM topic_lotteries WHERE topic_id = %d" % tid,
            "DELETE FROM topics WHERE id = %d" % tid,
            "DELETE FROM spark_ledger WHERE ref_id = %d "
            "OR idempotency_key LIKE 'forum-lottery-win:%d:%%' "
            "OR idempotency_key = 'forum-topic:1:%d'" % (tid, tid, tid),
            "DELETE FROM messages WHERE sender_id IS NULL "
            "AND created_at > now() - interval '15 min'"]
        # 生产者未接线：按 open_fair_round 该写的三列直接做夹具，
        # 把「链能不能跑」与「公平场能不能被开出来」分开定性。
        sql("UPDATE topic_lotteries SET fair_enabled=TRUE, commit_hash='%s',"
            " seed_cipher='%s' WHERE topic_id=%d"
            % (hashlib.sha256(SEED.encode()).hexdigest(), SEED, tid))
        M = [new_member(1), new_member(2)]
        if not M[0] or not M[1]:
            ok("FE-L2 两个探针会员", False, "注册窗口/邀请造不出来")
        else:
            for _t, uid in M:
                bal = sql("SELECT spark_balance FROM users WHERE id=%d"
                          % uid)
                CLEAN.append("UPDATE users SET spark_balance = %s "
                             "WHERE id = %d" % (bal, uid))
            # ① 余额不足：占位行不得留下（P-395 的白拿窗口）
            sql("UPDATE users SET spark_balance = 0 WHERE id = %d" % M[0][1])
            s, r = call("POST", "/forums/lottery/join",
                        {"topic_id": tid}, token=M[0][0])
            left = sql("SELECT count(*) FROM lottery_entries WHERE "
                       "topic_id=%d AND user_id=%d" % (tid, M[0][1]))
            ok("FE-L3 扣费失败不留占位（http=%s 残留=%s）" % (s, left),
               s != 200 and left == "0",
               "旧形状靠补偿 DELETE，删除失败就留下一张没付钱的票")
            for _t, uid in M:
                sql("UPDATE users SET spark_balance = 200 WHERE id = %d" % uid)
            got = []
            for k, (tok, uid) in enumerate(M):
                s, r = call("POST", "/forums/lottery/join",
                            {"topic_id": tid}, token=tok)
                h = dat(r).get("ticket_hash")
                dbv = sql("SELECT coalesce(ticket_hash,'NULL')||'/'||"
                          "coalesce(ticket_seq::text,'NULL') "
                          "FROM lottery_entries WHERE topic_id=%d "
                          "AND user_id=%d" % (tid, uid))
                got.append(h)
                ok("FE-L4 第%d张票落库（%s）" % (k + 1, dbv),
                   s == 200 and isinstance(h, str) and len(h) == 64
                   and dbv == "%s/%d" % (h, k),
                   "旧形状：响应 '' / 库里 NULL/NULL")
            s, r = call("POST", "/forums/lottery/join",
                        {"topic_id": tid}, token=M[0][0])
            ok("FE-L5 同一人重复参与被挡（http=%s code=%s）"
               % (s, r.get("code")), r.get("code") == 1002, str(r)[:120])
            b0 = sql("SELECT spark_balance FROM users WHERE id=%d" % M[1][1])
            sql("DELETE FROM lottery_entries WHERE topic_id=%d "
                "AND user_id=%d" % (tid, M[1][1]))
            s, r = call("POST", "/forums/lottery/join",
                        {"topic_id": tid}, token=M[1][0])
            b1 = sql("SELECT spark_balance FROM users WHERE id=%d" % M[1][1])
            ok("FE-L6 占位行消失后重参与＝重新扣款（%s→%s）" % (b0, b1),
               s == 200 and int(b1) == int(b0) - 5,
               "旧形状：Replayed 被当成功 ⇒ http=200 且分文未扣")
            rows = sql("SELECT ticket_seq||'|'||ticket_hash FROM "
                       "lottery_entries WHERE topic_id=%d AND "
                       "ticket_hash IS NOT NULL ORDER BY ticket_seq" % tid)
            chain, prev = [], GENESIS
            for line in [x for x in rows.split("\n") if x.strip()]:
                seqn, hh = line.split("|")
                chain.append((int(seqn), hh))
            recomputed, prev = [], GENESIS
            for sn, hh in chain:
                recomputed.append((sn, hh, ticket_hash(SEED, prev, sn)))
                prev = hh
            ok("FE-L7 第三方独立复算整条链（SHA-256(seed|prev|nonce)）",
               [s for s, _h, _e in recomputed] == list(
                   range(0, len(recomputed)))
               and all(h == e for _s, h, e in recomputed)
               and len(recomputed) == 2,
               "seq 是 0 基（domain 用 seq 当 nonce）；库里=%s 复算=%s"
               % (chain, recomputed))
            s, r = call("POST", "/forums/lottery/draw",
                        {"topic_id": tid}, token=ROOT_TOK)
            win = dat(r).get("winners") or []
            ok("FE-L8 公平场真有人中奖（winners=%s）" % win,
               s == 200 and len(win) == 1,
               "链恒空时 picked 为空 ⇒ 奖池既不派彩也不再退回（已 drawn）")
            s, r = call("GET", "/forums/lottery/%d/fairness" % tid)
            d = dat(r)
            ok("FE-L9 验证页给真材料并复算通过",
               d.get("exists") is True and d.get("stage") == "drawn"
               and d.get("verified") is True
               and d.get("ticket_count") == len(chain),
               "旧形状：ticket_count=0 而 verified=true（假证明）")
            last = chain[-1][1] if chain else ""
            got_root = sql("SELECT coalesce(tickets_root,'') FROM "
                           "topic_lotteries WHERE topic_id=%d" % tid)
            ok("FE-L10 场级 tickets_root = 链尾而非 genesis",
               got_root == last and last != GENESIS,
               "root=%r 链尾=%r" % (got_root, last))

# ---------- 收尾 ----------
if LIVE and tid:
    print("== 收尾 ==")
    for stmt in reversed(CLEAN):
        sql(stmt)
    print("   残料：entries=%s lotteries=%s topics=%s 本场流水=%s" % (
        sql("SELECT count(*) FROM lottery_entries WHERE topic_id=%d" % tid),
        sql("SELECT count(*) FROM topic_lotteries WHERE topic_id=%d" % tid),
        sql("SELECT count(*) FROM topics WHERE id=%d" % tid),
        sql("SELECT count(*) FROM spark_ledger WHERE ref_id=%d" % tid)))
    ids = ",".join(str(x) for x in PROBE_UIDS) or "-1"
    ok("FE-Z1 root 余额与在册会员数复原",
       sql("SELECT spark_balance FROM users WHERE id=1") == ROOT_BAL0
       and sql("SELECT count(*) FROM users WHERE id IN (%s) "
               "AND status < 2" % ids) == "0",
       "在册(status<2)=%s root=%s" % (
           sql("SELECT count(*) FROM users WHERE status < 2"),
           sql("SELECT spark_balance FROM users WHERE id=1")))

summary()
