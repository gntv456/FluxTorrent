# -*- coding: utf-8 -*-
"""闸门 BI：求字幕悬赏的发起/凑赏/交付相关性/认领工作流。

选题订正：上一批按「pt_audit_* 常驻闸门里搜不到字面量」判它零覆盖，严格说是
「**没有活体覆盖**」——曾有 scripts/e2e_0148_workflow.py 走过这条链，但它依赖
的三个演示账号（users id 2/3/4）已不在库（实测只剩 id 1），脚本第一步就退出；
而且它把「请求 lang=qae、交付字幕 lang=chs 也照样结算」当成预期行为钉住了。

  P-177（P0）交付与请求**是不是同一件事**没人判。旧直交付口 `fulfill` 只查
        「字幕是你本人上传且已过审」，不比语言、不比种子；本站默认
        `subtitle_workflow=0` 时它是唯一活着的结算口，连认领都不必 ⇒ 任何人
        拿任意一条自己的字幕（甚至一条 http 外链）就能把别人凑出来的悬赏池
        整池领走。交稿口 `deliver` 同病，且叠加「逾期自动验收结算」＝错货也发钱。
  P-178（P2）`POST /subtitles/requests` 的 `torrent_id` 直接进 INSERT，撞
        `torrent_id → torrents(id)` 外键成 500（与批四十三 P-171 同族）。

修法是一处判据两处接：`subtitles_util::ensure_req_match`（种子必须同一部；
语言按字面量或按 `subtitle_langs` 字典 id 对齐），`fulfill` 与 `deliver` 都调；
`fulfill` 额外要求字幕有文件本体（`file_ref` 以 `attach://` 开头）—— 外链字幕
不参与当场结算，因为这条通道不等验收就把钱发了。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bi_subtitle_requests.py
"""
import json
import os
import re
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
INV, SUB, REQ = [], [], []


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], capture_output=True, text=True,
        encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:56], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


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


def mpost(path, body, ctype, tok):
    req = urllib.request.Request(BASE + path, method="POST")
    req.add_header("Content-Type", ctype)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        with urllib.request.urlopen(req, body, timeout=30) as rp:
            return rp.status, json.loads(rp.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {"raw": e.reason}


INV = []


def mkprobe(prefix):
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
        "username": name, "email": "%s@bi.local" % name,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    tok = data(rg).get("token") or data(
        post("/auth/login", {"username": name,
                             "password": "password123"})).get("token")
    if not tok:
        return 0, None, "注册/登录 http=%s %s" % (rg[0], blob(rg[1])[:90])
    uid = int(psql("SELECT id FROM users WHERE username = '%s'" % name))
    return uid, tok, ""


clear_register_rl()
A, TA, EA = mkprobe("bia")
B, TB, EB = mkprobe("bib")
C, TC, EC = mkprobe("bic")
ok("I0a 三个成员探针就绪（发起人 A=%s / 白拿者 B=%s / 译者 C=%s）%s"
   % (A, B, C, EA + EB + EC),
   A > 0 and B > 0 and C > 0, "A=%s B=%s C=%s" % (A, B, C))
# A 出悬赏、B/C 凑赏都要有火花：走产品自己的调整口（会落台账，收尾一并清）
post("/admin/users/adjust", {"user_id": A, "spark_delta": 1000})
for u in (B, C):
    post("/admin/users/adjust", {"user_id": u, "spark_delta": 300})
SP0 = dict((u, psql("SELECT spark_balance::text FROM users WHERE id = %d" % u))
           for u in (A, B, C))
TID = psql("SELECT id FROM torrents WHERE approval_status = 1 ORDER BY id "
           "LIMIT 1")
TID2 = psql("SELECT id FROM torrents WHERE approval_status = 1 AND id <> %s "
            "ORDER BY id DESC LIMIT 1" % TID)
ok("I0b 样本种子就绪（%s / 另一部 %s）" % (TID, TID2),
   TID.isdigit() and TID2.isdigit() and TID != TID2, "TID=%s TID2=%s"
   % (TID, TID2))
ok("I0c 火花到位（A=%s B=%s C=%s：四张悬赏单各 100 全由 A 出，B/C 只凑赏）"
   % (SP0[A], SP0[B], SP0[C]),
   int(SP0[A] or 0) >= 400 and int(SP0[B] or 0) >= 20
   and int(SP0[C] or 0) >= 50, blob(SP0)[:100])
LED0 = int(psql("SELECT coalesce(max(id), 0) FROM spark_ledger"))
MSG0 = int(psql("SELECT coalesce(max(id), 0) FROM messages"))
WF0 = psql("SELECT value FROM site_settings WHERE name = 'subtitle_workflow'")


def mkreq(tok, lang="eng", bounty=0, tid=None, **kw):
    b = {"lang": lang, "descr": "BI%d" % TAG, "bounty": bounty}
    if tid is not None:
        b["torrent_id"] = int(tid)
    b.update(kw)
    s, r = post("/subtitles/requests", b, tok)
    rid = data((s, r)).get("id")
    if rid:
        REQ.append(int(rid))
    return s, r, rid


def pool(rid):
    return psql("SELECT bounty::text FROM subtitle_requests WHERE id = %s"
                % rid)


def st(rid):
    return psql("SELECT status::text FROM subtitle_requests WHERE id = %s"
                % rid)


def spark(u):
    return psql("SELECT spark_balance::text FROM users WHERE id = %d" % u)


def ledkey(k):
    return cnt("spark_ledger", "idempotency_key = '%s'" % k)


# ============ I1 发起悬赏的门口 ============
s, r, _ = mkreq(TA, tid=999999999)
ok("I1a 挂一个不存在的种子必须 400 并指出来（不得 500、不得静默当「无种子」）",
   s == 400 and "不存在" in msg((s, r)),
   "http=%s msg=%s" % (s, msg((s, r))[:80]))
s, r, _ = mkreq(TA, bounty=-1, tid=TID)
ok("I1b 悬赏为负必须拒", s == 400 and "悬赏" in msg((s, r)),
   "http=%s msg=%s" % (s, msg((s, r))[:70]))
s, r, _ = mkreq(TA, lang="  ", tid=TID)
ok("I1c 语言留空必须拒", s == 400, "http=%s %s" % (s, blob(r)[:70]))
s, r, _ = mkreq(TA, tid=TID, offer_free=True)
ok("I1d 成员发起时勾「限时免费」（站方出资）必须拒 —— B2 回归",
   s == 400 and "限时免费" in msg((s, r)),
   "http=%s msg=%s" % (s, msg((s, r))[:70]))
sa, ra, RID = mkreq(TA, bounty=100, tid=TID)
ok("I1e 正例：成员发起 eng 悬赏 100 挂在种子 %s 上" % TID,
   sa == 200 and RID and data((sa, ra)).get("bounty_frozen") == 100,
   "http=%s %s" % (sa, blob(ra)[:90]))
ok("I1f 悬赏即时冻结：A 火花 -100、池=100、contributors 记 A、幂等键恰 1 条",
   int(spark(A)) == int(SP0[A]) - 100 and pool(RID) == "100"
   and psql("SELECT contributors::text FROM subtitle_requests WHERE id = %s"
             % RID).find('"user_id": %d' % A) >= 0
   and ledkey("sub-req-bounty:%d:%s" % (A, RID)) == "1",
   "spark=%s pool=%s led=%s" % (spark(A), pool(RID),
                                ledkey("sub-req-bounty:%d:%s" % (A, RID))))

# ============ I2 凑赏 ============
sb, rb = post("/subtitles/requests/%s/contribute" % RID, {"amount": 50}, TC)
ok("I2a 第三人凑赏 50 入池（池=150、C 火花 -50、幂等键恰 1 条）",
   sb == 200 and data((sb, rb)).get("bounty") == 150
   and int(spark(C)) == int(SP0[C]) - 50
   and ledkey("sub-req-contrib:%d:%s:0" % (C, RID)) == "1",
   "http=%s %s sparkC=%s" % (sb, blob(rb)[:80], spark(C)))
ok("I2b 入池金额 0 与负数都拒（且不动火花）",
   post("/subtitles/requests/%s/contribute" % RID, {"amount": 0}, TC)[0]
   == 400 and post("/subtitles/requests/%s/contribute" % RID,
                   {"amount": -30}, TC)[0] == 400
   and int(spark(C)) == int(SP0[C]) - 50, "sparkC=%s" % spark(C))
res = []
tk = [threading.Thread(target=lambda i=i: res.append(
    post("/subtitles/requests/%s/contribute" % RID, {"amount": 20}, TB)[0]))
    for i in range(3)]
for t in tk:
    t.start()
for t in tk:
    t.join()
ok("I2c 同人并发三笔同额入池：不变式「扣款笔数 == 入池笔数」（状态码 %s，"
   "池=%s B 火花=%s）" % (res, pool(RID), spark(B)),
   int(pool(RID)) == 150 + 20 * len([x for x in res if x == 200])
   and int(spark(B)) == int(SP0[B]) - 20 * len([x for x in res if x == 200])
   and 200 in res, "res=%s pool=%s sparkB=%s" % (res, pool(RID), spark(B)))

# ============ I3 交付相关性（本批核心） ============
def upload_srt(tok, title, lang, tid, mark):
    bd = "----bi%d" % TAG
    body = ("--%s\r\nContent-Disposition: form-data; name=\"file\"; "
            "filename=\"bi-%s.srt\"\r\nContent-Type: text/plain\r\n\r\n"
            % (bd, mark)).encode() + (
                "1\n00:00:01,000 --> 00:00:02,000\n%s\n" % mark).encode() + (
                "\r\n--%s--\r\n" % bd).encode()
    s, r = mpost("/attachments", body, "multipart/form-data; boundary=" + bd,
                 tok)
    sha = data((s, r)).get("sha256")
    if not sha:
        return None
    s2, r2 = post("/subtitles", {"torrent_id": int(tid), "title": title,
                                 "lang": lang, "file_sha": sha}, tok)
    sid = data((s2, r2)).get("id")
    if sid:
        SUB.append(int(sid))
    return sid


def ext_sub(tok, title, lang, tid, url):
    s, r = post("/subtitles", {"torrent_id": int(tid), "title": title,
                               "lang": lang, "file_ref": url}, tok)
    sid = data((s, r)).get("id")
    if sid:
        SUB.append(int(sid))
    return sid


EXT = ext_sub(TB, "BI%d 外链英文字幕" % TAG, "eng", TID,
              "https://sub.example.org/bi%d.srt" % TAG)
POOL_X, SPB_X = pool(RID), spark(B)
sx, rx = post("/subtitles/requests/%s/fulfill" % RID,
              {"subtitle_id": EXT}, TB)
ok("I3a 用一条 http 外链字幕去领整池必须拒（本通道当场发钱，只认有本体的字幕）",
   sx == 400 and "外链" in msg((sx, rx)) and pool(RID) == POOL_X
   and spark(B) == SPB_X,
   "http=%s msg=%s 池=%s B火花=%s" % (sx, msg((sx, rx))[:60], pool(RID),
                                      spark(B)))
# 每条判据各一张单（批四十四踩过的坑：第一笔真结算之后，后面全读成「已处理」）
RB_ = mkreq(TA, bounty=100, tid=TID)[2]
SUB_SPA = upload_srt(TB, "BI%d 西语字幕" % TAG, "spa", TID, "BIspa%d" % TAG)
sx2, rx2 = post("/subtitles/requests/%s/fulfill" % RB_,
                {"subtitle_id": SUB_SPA}, TB)
ok("I3b 语言不对（单要 eng、交的是 spa）必须拒，那张单的池一分不动",
   sx2 == 400 and "语言" in msg((sx2, rx2)) and pool(RB_) == "100",
   "http=%s msg=%s 池=%s" % (sx2, msg((sx2, rx2))[:70], pool(RB_)))
RC_ = mkreq(TA, bounty=100, tid=TID)[2]
SUB_OTHER = upload_srt(TB, "BI%d 另一部种子的 eng 字幕" % TAG, "eng", TID2,
                       "BIoth%d" % TAG)
ok("I3z 造样就绪：外链/西语/另一部三条字幕都真的挂上了（%s/%s/%s）"
   % (EXT, SUB_SPA, SUB_OTHER), all([EXT, SUB_SPA, SUB_OTHER]),
   "EXT=%s SPA=%s OTH=%s" % (EXT, SUB_SPA, SUB_OTHER))
sx3, rx3 = post("/subtitles/requests/%s/fulfill" % RC_,
                {"subtitle_id": SUB_OTHER}, TB)
ok("I3c 种子不对（同一部片子才叫同一件事）必须拒",
   sx3 == 400 and "种子" in msg((sx3, rx3)) and pool(RC_) == "100",
   "http=%s msg=%s 池=%s" % (sx3, msg((sx3, rx3))[:70], pool(RC_)))
RD_ = mkreq(TA, bounty=100, tid=TID)[2]
post("/subtitles/requests/%s/contribute" % RD_, {"amount": 50}, TC)
SUB_OK = upload_srt(TB, "BI%d 正主字幕" % TAG, "eng", TID, "BIok%d" % TAG)
# 基线必须在上一条字幕之后取：上传本身就发 +5 火花，早取一格就把奖励算进
# 「领赏所得」里（实测本批 B 连传三条 ⇒ 差 15）
POOL_D, SPB_D = pool(RD_), spark(B)
sok, rok = post("/subtitles/requests/%s/fulfill" % RD_,
                {"subtitle_id": SUB_OK}, TB)
ok("I3d 正例：同种子同语言 + 有本体的字幕照样当场结算（池=%s 全给交付人）"
   % POOL_D,
   sok == 200 and data((sok, rok)).get("bounty_paid") == int(POOL_D)
   and st(RD_) == "1" and int(spark(B)) == int(SPB_D) + int(POOL_D),
   "http=%s %s 池=%s sparkB=%s" % (sok, blob(rok)[:90], pool(RD_), spark(B)))
sdup, rdup = post("/subtitles/requests/%s/fulfill" % RD_,
                  {"subtitle_id": SUB_OK}, TB)
ok("I3e 重复 fulfill 被拒且不双发（B 火花停在 %s）" % spark(B),
   (sdup != 200 or rdup.get("code") != 0)
   and int(spark(B)) == int(SPB_D) + int(POOL_D),
   "http=%s %s" % (sdup, blob(rdup)[:80]))
ok("I3f 结算不变式：池 = 各人实收之和，且本单领赏幂等键只有一条",
   sum(int(p.get("amount") or 0)
       for p in (data((sok, rok)).get("settlement") or [])) == int(POOL_D)
   and cnt("spark_ledger", "idempotency_key LIKE 'subtitle-bounty-pay:%s:%%'"
           % RD_) == "1", blob(data((sok, rok)).get("settlement"))[:110])

# ============ I4 认领工作流（临时开 subtitle_workflow=1） ============
try:
    psql("UPDATE site_settings SET value = '1' WHERE name = "
         "'subtitle_workflow'")
    ok("I4a 造现场：工作流开关已开（复读=%s）"
       % psql("SELECT value FROM site_settings WHERE name = "
              "'subtitle_workflow'"),
       psql("SELECT value FROM site_settings WHERE name = "
            "'subtitle_workflow'") == "1", "wf=%s" % WF0)
    s2, r2, RID2 = mkreq(TA, bounty=60, tid=TID)
    s2b, r2b = post("/subtitles/requests/%s/claim" % RID2, {}, TA)
    ok("I4b 发起人不能认领自己发的单（防自薅悬赏）",
       s2b == 400 and "自己" in msg((s2b, r2b)) and st(RID2) == "0"
       and psql("SELECT coalesce(claimed_by::text,'NULL') FROM "
                "subtitle_requests WHERE id = %s" % RID2) == "NULL",
       "http=%s msg=%s st=%s" % (s2b, msg((s2b, r2b))[:60], st(RID2)))
    s2c, r2c = post("/subtitles/requests/%s/claim" % RID2, {"crew": [
        {"user_id": C, "share": 90, "role": "翻译"},
        {"user_id": B, "share": 60, "role": "校对"}]}, TC)
    ok("I4c 协作份额合计 150%% 必须拒（池是按份额拆的，超 100 就是凭空多付）",
       s2c == 400 and "100" in msg((s2c, r2c)),
       "http=%s msg=%s" % (s2c, msg((s2c, r2c))[:70]))
    s2d, r2d = post("/subtitles/requests/%s/claim" % RID2, {"crew": [
        {"user_id": 999999, "share": 10, "role": "时间轴"}]}, TC)
    ok("I4d 队伍里有不存在的用户必须拒", s2d == 400 and "不存在" in
       msg((s2d, r2d)), "http=%s msg=%s" % (s2d, msg((s2d, r2d))[:70]))
    s2e, r2e = post("/subtitles/requests/%s/claim" % RID2, {}, TC)
    ok("I4e 正例：第三人认领成功（status=3、claimed_by=C、期限落库）",
       s2e == 200 and st(RID2) == "3"
       and psql("SELECT claimed_by::text FROM subtitle_requests WHERE id = %s"
                % RID2) == str(C)
       and psql("SELECT coalesce(deadline_at::text,'NULL') FROM "
                "subtitle_requests WHERE id = %s" % RID2) != "NULL",
       "http=%s st=%s %s" % (s2e, st(RID2), blob(r2e)[:80]))
    CSB_OTH = upload_srt(TC, "BI%d C 的另一部字幕" % TAG, "eng", TID2,
                         "BIco%d" % TAG)
    s2f, r2f = post("/subtitles/requests/%s/deliver" % RID2,
                    {"subtitle_id": CSB_OTH}, TC)
    ok("I4f 交稿也要过相关性尺：C 认领了种子 %s 的单，却交另一部种子的字幕"
       "（st=%s）" % (TID, st(RID2)),
       s2f == 400 and "种子" in msg((s2f, r2f)) and st(RID2) == "3",
       "http=%s msg=%s st=%s" % (s2f, msg((s2f, r2f))[:70], st(RID2)))
    CSB_SPA = upload_srt(TC, "BI%d C 的西语字幕" % TAG, "spa", TID,
                         "BIcs%d" % TAG)
    s2g, r2g = post("/subtitles/requests/%s/deliver" % RID2,
                    {"subtitle_id": CSB_SPA}, TC)
    ok("I4g 语言错配的字幕同样交不进（st=%s，逾期会自动验收结算，所以必须"
       "在交稿这一步就挡）" % st(RID2),
       s2g == 400 and "语言" in msg((s2g, r2g)) and st(RID2) == "3",
       "http=%s msg=%s st=%s" % (s2g, msg((s2g, r2g))[:70], st(RID2)))
    SUB_C = upload_srt(TC, "BI%d C 的正主字幕" % TAG, "eng", TID,
                       "BIc%d" % TAG)
    s2h, r2h = post("/subtitles/requests/%s/deliver" % RID2,
                       {"subtitle_id": SUB_C}, TB)
    ok("I4h 非认领人不能替人交稿", s2h == 400,
       "http=%s %s" % (s2h, blob(r2h)[:70]))
    s2i, r2i = post("/subtitles/requests/%s/deliver" % RID2,
                       {"subtitle_id": SUB_C}, TC)
    MINE = psql("SELECT count(*) FROM messages WHERE receiver_id = %d AND "
                "id > %d AND subject = '字幕已交稿，请验收'" % (A, MSG0))
    ok("I4i 正例：认领人交匹配字幕 → 3→4 并通知发起人验收",
       s2i == 200 and st(RID2) == "4" and MINE == "1",
       "http=%s st=%s msg=%s" % (s2i, st(RID2), blob(r2i)[:70]))
    s2j, r2j = post("/subtitles/requests/%s/accept" % RID2, {}, TB)
    ok("I4j 第三人不能验收（只有发起人或 staff）", s2j == 403,
       "http=%s %s" % (s2j, blob(r2j)[:70]))
    SPC = spark(C)
    s2k, r2k = post("/subtitles/requests/%s/accept" % RID2, {}, TA)
    ok("I4k 正例：发起人验收 → 整池 60 结算给主认领人 C（C 火花 %s->%s）"
       % (SPC, spark(C)),
       s2k == 200 and st(RID2) == "1"
       and int(spark(C)) == int(SPC) + 60,
       "http=%s %s" % (s2k, blob(r2k)[:100]))
    s2l, r2l = post("/subtitles/requests/%s/accept" % RID2, {}, TA)
    ok("I4l 重复验收被拒且不双发", (s2l != 200 or r2l.get("code") != 0)
       and int(spark(C)) == int(SPC) + 60,
       "http=%s %s" % (s2l, blob(r2l)[:80]))
    s2m, r2m = post("/subtitles/requests/%s/abandon" % RID, {}, TB)
    ok("I4m 弃单只认认领人本人（B 弃 C 的单必须拒）", s2m == 400,
       "http=%s %s" % (s2m, blob(r2m)[:70]))
finally:
    psql("UPDATE site_settings SET value = '%s' WHERE name = "
         "'subtitle_workflow'" % WF0)
    ok("I4n 工作流开关已复原（复读=%s，与开局 %s 一致）"
       % (psql("SELECT value FROM site_settings WHERE name = "
               "'subtitle_workflow'"), WF0),
       psql("SELECT value FROM site_settings WHERE name = "
            "'subtitle_workflow'") == WF0, "wf=%s" % WF0)

# ============ I5 收尾与残留 ============
psql("DELETE FROM subtitle_requests WHERE descr LIKE 'BI%d%%'" % TAG)
psql("DELETE FROM subtitles WHERE title LIKE 'BI%d%%'" % TAG)
psql("DELETE FROM attachments WHERE filename LIKE 'bi-%%' AND id > 0 AND "
     "user_id IN (%d, %d, %d)" % (A, B, C))
psql("DELETE FROM messages WHERE id > %d AND (receiver_id IN (%d, %d, %d) "
     "OR sender_id IN (%d, %d, %d))" % (MSG0, A, B, C, A, B, C))
psql("DELETE FROM spark_ledger WHERE id > %d AND user_id IN (%d, %d, %d)"
     % (LED0, A, B, C))
for u, v in SP0.items():
    psql("UPDATE users SET spark_balance = %s WHERE id = %d" % (v, u))
# 火花必须**在墓碑之前**复读：产品自己的 user_purge 那一步会把余额冲平，
# 放在墓碑之后比就成了「我的账没复原」的假红（批四十四同款账）
NOWSP = dict((u, psql("SELECT spark_balance::text FROM users WHERE id = %d"
                       % u)) for u in SP0)
ok("I5b 三个探针火花逐字回基线（%s vs %s）" % (SP0, NOWSP),
   NOWSP == SP0 and cnt("spark_ledger", "id > %d AND user_id IN (%d, %d, %d)"
                        % (LED0, A, B, C)) == "0", blob(NOWSP)[:120])
for u in (A, B, C):
    post("/admin/users/status", {"user_id": u, "status": 2})
    call("DELETE", "/admin/users/%d" % u, None, token=ROOT)
    psql("DELETE FROM spark_ledger WHERE user_id = %d" % u)
    psql("DELETE FROM traffic_ledger WHERE user_id = %d" % u)
if INV:
    psql("UPDATE invites SET status = 2 WHERE id IN (%s)"
         % ",".join(str(int(i)) for i in INV))
ok("I5a 本轮请求/字幕已清空（请求残留 %s 条、字幕残留 %s 条）"
   % (cnt("subtitle_requests", "descr LIKE 'BI%d%%'" % TAG),
      cnt("subtitles", "title LIKE 'BI%d%%'" % TAG)),
   cnt("subtitle_requests", "descr LIKE 'BI%d%%'" % TAG) == "0"
   and cnt("subtitles", "title LIKE 'BI%d%%'" % TAG) == "0")
TOMB = psql("SELECT count(*) FROM users WHERE id IN (%d, %d, %d) AND "
            "status >= 2 AND passkey LIKE 'deleted-%%'" % (A, B, C))
ok("I5c 三个探针都走产品自己的墓碑（%s/3），本轮站内信残留 %s 条"
   % (TOMB, cnt("messages", "id > %d" % MSG0)),
   TOMB == "3" and cnt("messages", "id > %d" % MSG0) == "0",
   "tomb=%s msgs=%s" % (TOMB, cnt("messages", "id > %d" % MSG0)))
ok("I5d 站点设定没被改动：subtitle_workflow 复读=%s（开局 %s）"
   % (psql("SELECT value FROM site_settings WHERE name = "
           "'subtitle_workflow'"), WF0),
   psql("SELECT value FROM site_settings WHERE name = "
        "'subtitle_workflow'") == WF0)
summary()
