# -*- coding: utf-8 -*-
"""闸门 AC：论坛权限门与关注流（2026-10-10 批四实测固化）。

钉住三件事：
  F1 板块抬高 minclassread 后，六个读侧都不许漏出该板块的标题
     （/forums、/forums/boards、直读板块、/forums/feed、/forums/search、
      /rss/forum/{passkey}）——RSS 出口不在模块/权限网关里，历来是漏点；
  F2 版主治理：普通用户 manage/删他人主题一律 403；站长锁帖后回帖被拒，
     解锁后恢复；sticky/digest 落库；
  F3 关注链路：关注用户后对方发新主题要有站内信；关注主题后有人回帖要有站内信；
     自关注被拒、关注不存在的目标被拒；/follows/mine 读得到。

前置数据自己造、跑完复原（板块门槛值先读后写，探针号置 status=2——
audit_log 防篡改链不允许真删号）。注册/登录有 5/min 与 429 限流，
脚本按节拍等待而不是并发硬撞。

跑法：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_ac_forum_perm.py
"""
import os
import re
import subprocess
import sys
from urllib.parse import quote
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (call, clear_register_rl, login, ok,  # noqa: E402
                         summary)

ROOT = login()
TAG = int(time.time()) % 100000
CLEAN = []


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip())
    return r.stdout.strip()


def set_gate(v):
    exec_sql("UPDATE forums SET minclassread = %s WHERE id = %s" % (v, FID))


def manage(tid, tok, **kw):
    path = "/forums/topics/%s/manage" % tid
    return call("POST", path, kw, token=tok)


def follow(tok, ttype, target):
    body = {"target_type": ttype, "target_id": target}
    return call("POST", "/follows", body, token=tok)


def pmsg(uid):
    n = psql("SELECT count(*) FROM messages WHERE receiver_id = %s" % uid)
    return int(n or 0)


def brief(x):
    return str(x)[:94]


def exec_sql(q):
    subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", q],
        capture_output=True, text=True, encoding="utf-8")


def cleanup():
    for stmt in reversed(CLEAN):
        try:
            exec_sql(stmt)
        except BaseException as e:  # noqa: BLE001
            print("   ! 收尾失败: %s -> %s" % (stmt[:64], str(e)[:70]))


def login_backoff(user, tries=10):
    for _ in range(tries):
        s, r = call("POST", "/auth/login",
                    {"username": user, "password": "password123"})
        tok = (r.get("data") or {}).get("token")
        if tok:
            return tok
        time.sleep(8 if r.get("code") == 1015 or s == 429 else 3)
    return None


def new_member(tag, gap=14):
    """注册限流：按节拍建号，拿不到就整条闸门判红。

    清额度这步是 #75 的正解（批一百五十三）：24h 同 IP 限额数的是
    reg_ip_signups 表，清 Redis 不顶用；不清就是「上一支留下的两个号」
    把本支直接顶到上限，红在「两个探针成员可准备」而非被测口径。
    """
    clear_register_rl()
    time.sleep(gap)
    s, r = call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                token=ROOT)
    codes = ((r.get("data") or {}).get("codes")) or []
    if not codes:
        return None
    code, cid = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None
    user = "fc%s%d" % (tag, TAG)
    reg = {"username": user, "email": "%s@fc.local" % user,
           "password": "password123", "invite_code": code,
           "captcha_id": cap.get("captcha_id"),
           "captcha_answer": int(nums[0]) + int(nums[1]),
           "captcha_token": "", "fields": {}}
    s, rr = call("POST", "/auth/register", reg)
    uid = (rr.get("data") or {}).get("user_id")
    if not uid:
        return None
    CLEAN.append("UPDATE users SET status = 2 WHERE id = %s" % uid)
    CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
    tok = login_backoff(user)
    if not tok:
        return None
    return (tok, uid, user)


def topic(tok, uid, name, **kw):
    exec_sql("DELETE FROM forum_flood WHERE user_id = %s" % uid)
    body = {"forum_id": FID, "title": "AC闸门%d%s" % (TAG, name), "body": "正文"}
    body.update(kw)
    s, r = call("POST", "/forums/topics", body, token=tok)
    d = r.get("data") or {}
    return d.get("topic_id") or d.get("id"), s, r


boards = call("GET", "/forums/boards", token=ROOT)[1].get("data") or []
FID = boards[0]["id"]
LOW = new_member("low")
HI = new_member("hi")
if not LOW or not HI:
    ok("两个探针成员可准备（注册/登录限流下）", False,
       "LOW=%s HI=%s" % (bool(LOW), bool(HI)))
    summary()
    cleanup()
    sys.exit(1)
LT, LUID, _ = LOW
HT, HUID, HUSER = HI
CLEAN.append("DELETE FROM follows WHERE user_id IN (%s,%s)" % (LUID, HUID))
CLEAN.append("DELETE FROM posts WHERE topic_id IN "
             "(SELECT id FROM topics WHERE title LIKE 'AC闸门%d%%')"
             % TAG)
CLEAN.append("DELETE FROM topics WHERE title LIKE 'AC闸门%d%%'" % TAG)

print("F1 受限板块的六个读侧都不许漏标题")
TID, s, r = topic(HT, HUID, "受限帖")
TITLE = psql("SELECT title FROM topics WHERE id = %s" % TID)
ok("F1 前置：受限板块能发帖（默认门槛 0）", bool(TID) and s == 200, brief(r))
# 受限前的投票主题（抬门槛后再建就发不出去了）
PTID, _, _ = topic(HT, HUID, "受限投票", topic_type="poll",
                   poll_options=["甲", "乙"])
orig = psql("SELECT minclassread FROM forums WHERE id = %s" % FID)
set_gate(5)
CLEAN.append("UPDATE forums SET minclassread = %s "
             "WHERE id = %s" % (orig or 0, FID))
try:
    lst = call("GET", "/forums", token=LT)[1].get("data") or []
    lrows = lst if isinstance(lst, list) else (lst.get("forums") or [])
    ok("F1 /forums 不含受限板块",
       not any(x.get("id") == FID for x in lrows), brief(lrows))
    b2 = call("GET", "/forums/boards", token=LT)[1].get("data") or []
    ok("F1 /forums/boards 不含受限板块",
       not any(x.get("id") == FID for x in b2), brief(b2))
    s, r = call("GET", "/forums/%s/topics" % FID, token=LT)
    ok("F1 直读受限板块被拒", s == 403, "http=%s code=%s" % (s, r.get("code")))
    feed = call("GET", "/forums/feed", token=LT)[1].get("data") or {}
    frows = feed.get("items") if isinstance(feed, dict) else feed
    ok("F1 /forums/feed 不含受限主题",
       not any((x.get("title") or "") == TITLE for x in frows or []),
       str(frows)[:120])
    sq = "/forums/search?q=" + quote(TITLE)
    srch = call("GET", sq, token=LT)[1].get("data")
    srows = (srch or {}).get("hits") if isinstance(srch, dict) else srch
    ok("F1 /forums/search 不命中受限主题",
       not any((x.get("title") or "") == TITLE for x in srows or []),
       str(srows)[:120])
    pk = psql("SELECT passkey FROM users WHERE id = %s" % LUID)
    s, body = call("GET", "/rss/forum/" + pk, token=LT, raw=True)
    text = (body or b"").decode("utf-8", "replace")
    ok("F1 RSS 论坛流不含受限主题（RSS 出口不在网关里，必须自证）",
       TITLE not in text, "http=%s len=%d" % (s, len(text)))
    s, r = call("GET", "/forums/%s/topics" % FID, token=ROOT)
    ok("F1 正控：站长读同板块 200", s == 200, "http=%s" % s)
    # 用真 poll 主题验：权限判定要早于业务查询，否则受限板块的
    # 主题类型/存在性会被 404 与 403 的差别泄露
    s, r = call("POST", "/forums/poll/vote",
                  {"topic_id": PTID, "option_index": 0}, token=LT)
    ok("F1 受限板块的 poll 投票被 403（权限先于业务）",
       s == 403, "http=%s %s" % (s, brief(r)))
finally:
    set_gate(orig or 0)

print("F2 版主治理：权限与锁定")
s, r = manage(TID, LT, locked=True)
ok("F2 普通用户 manage 被拒", s == 403, "http=%s %s" % (s, brief(r)))
ok("F2 未落库：locked 仍 false",
   psql("SELECT locked FROM topics WHERE id = %s" % TID) == "f")
s, r = manage(TID, ROOT, locked=True, sticky=True, digest=True)
ok("F2 站长 manage 成功", s == 200, brief(r))
got = psql("SELECT locked::text||'/'||sticky::text||'/'||digest"
             " FROM topics WHERE id = %s" % TID)
ok("F2 三个标记都落库", got == "true/true/true", "库里=%s" % got)
s, r = call("POST", "/forums/topics/%s/reply" % TID,
           {"body": "锁后回帖"}, token=LT)
ok("F2 锁帖后回帖被拒", s == 400 and "锁定" in (r.get("message") or ""),
   "http=%s %s" % (s, brief(r.get("message"))))
s, r = manage(TID, ROOT, locked=False)
s, r = call("POST", "/forums/topics/%s/reply" % TID,
           {"body": "解锁后回帖"}, token=LT)
ok("F2 正控：解锁后能回帖", s == 200, "http=%s %s" % (s, brief(r)))
s, r = call("DELETE", "/forums/topics/%s" % TID, token=LT)
ok("F2 普通用户删他人主题被拒", s == 403, "http=%s %s" % (s, brief(r)))

print("F3 关注链路要发得出通知")
m0 = pmsg(LUID)
s, r = follow(LT, "user", HUID)
ok("F3 关注用户 200", s == 200, brief(r))
NTID, s, r = topic(HT, HUID, "被关注新帖")
m1 = pmsg(LUID)
ok("F3 关注用户后对方发帖 → 关注者收到通知", m1 > m0, "增量=%d" % (m1 - m0))
s, r = follow(LT, "topic", NTID)
ok("F3 关注主题 200", s == 200, brief(r))
exec_sql("DELETE FROM forum_flood WHERE user_id = %s" % HUID)
call("POST", "/forums/topics/%s/reply" % NTID, {"body": "回帖"}, token=HT)
m2 = pmsg(LUID)
ok("F3 关注主题后有人回帖 → 关注者收到通知", m2 > m1, "增量=%d" % (m2 - m1))
s, r = follow(LT, "user", LUID)
ok("F3 自关注被拒", s == 400, "http=%s %s" % (s, brief(r.get("message"))))
s, r = follow(LT, "topic", 99999999)
ok("F3 关注不存在的目标被拒", s == 404, "http=%s" % s)
mine = call("GET", "/follows/mine", token=LT)[1].get("data") or {}
ok("F3 /follows/mine 读得到关注",
   bool((mine.get("users") or [])) and bool((mine.get("topics") or [])),
   str(mine)[:130])

try:
    summary()
finally:
    cleanup()
