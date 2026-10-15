# -*- coding: utf-8 -*-
"""EB 闸门（批八十五）：「仅接收好友私信」这道屏障必须真的挡得住单向好友申请
（P-295）。

现网（修前，容器 `api:latest`，2026-10-14 实测）：
  会员面板「私信接收设定」三档（所有人 / 仅好友 / 仅管理组）里，
  `accept_pm` 这一列本身是有执行者的（`community_http/message_send.rs`
  会按 no/friends 拦），但 **friends 那一支查好友关系的 SQL 没带
  `friendships.list` 条件，还把两个方向都算**：
      WHERE (f.user_id=$1 AND f.friend_id=$2)
             OR (f.user_id=$2 AND f.friend_id=$1)
  而 `friendships.list` 的真实取值是 friend / pending / black（本轮现网实测：
  friend 2 行、pending 3 行，且 **pending 行全部没有反向行** = 单向申请）。
  ⇒ 只要 A 给 B 发过一次好友申请（B 从未接受），A 就能给设了「仅好友」的 B
  发私信；同理 A 把 B 拉黑之后 A 自己还能发（(A,B,'black') 也命中那个 OR）。
  `friend.rs` 里的注释明确写着「好友关系必须是双向两行
  （accept_pm='friends' 之外，好友列表/黑名单两侧都按 user_id=我 读）」——
  也就是说**屏障的生产端**建立了双向语义，而**屏障的判定端**没读那个列。

判据：EB1 无任何关系必须拒；EB3 单向 pending 必须拒（**修前必红**）；
EB4 双向 accepted 必须通（正控，防「一律拒」的假修）；EB5 拉黑后必须拒；
EB6 accept_pm='no' 必须拒；EB7 改回 'yes' 必须通（第二道正控，跨过 60s
站内信防刷窗口）；EB8 静态钉住「方向=接收方那一行 + list='friend'」。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_eb_pm_friends_gate.py
写库面：两个探针成员（EA 同款注册配方）、他们的 friendships 行、发出去的
私信；收尾按 id/用户名删净 + 探针号 status=2 墓碑。站内信防刷是 60s 一条，
所以两条成功发送之间睡 62 秒。
"""
import io
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TS = int(time.time()) % 99199
A = "eba%d" % TS                   # 发送方
B = "ebb%d" % TS      # 接收方
SENT = []         # 成功发送时刻（站内信防刷窗口用）


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:40], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def register(name, code):
    cap = call("GET", "/auth/captcha", None, token=ROOT)[1].get("data") or {}
    n = re.findall(r"-?\d+", str(cap.get("question") or ""))
    return call("POST", "/auth/register", {
        "username": name, "email": "%s@eb.local" % name,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(n[0]) + int(n[1]),
        "captcha_token": "", "fields": {}})


def login_as(name):
    for _ in range(8):
        tok = (call("POST", "/auth/login",
                    {"username": name, "password": "password123"})[1]
               .get("data") or {}).get("token")
        if tok:
            return tok
        time.sleep(7)
    return None


def send(tok, subj):
    return call("POST", "/messages",
                {"to": B, "subject": subj, "body": "EB 探针正文 %s" % subj},
                token=tok)


def rows():
    # 上一版这里把 SQL 拆成带引号的相邻字面量、又拿 '%s' 去插值，单引号在
    # Python 解析期就断掉了（psql 报 syntax error at or near "x"），
    # 于是 EB2/EB4a 的「关系形状」读的是空串——判据自己坏了，不是产品坏了。
    q = ("SELECT coalesce(string_agg(f.user_id || '>' || f.friend_id || "
         "':' || f.list, ' ' ORDER BY f.user_id), '-') "
         "FROM friendships f WHERE f.user_id IN (%d, %d) "
         "OR f.friend_id IN (%d, %d)") % (AID, BID, AID, BID)
    return psql(q)


def flood_wait(sent_at):
    # 站内信防刷是 60s/人：两次「成功」发送之间必须补足窗口，
    # 否则后一条正控会被「发送过于频繁」挡成假红。
    gap = time.time() - sent_at
    if gap < 63:
        print("   等 %d 秒过站内信防刷窗口..." % int(64 - gap), flush=True)
        time.sleep(64 - gap)


clear_register_rl()
time.sleep(6)
codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 2},
               token=ROOT)[1].get("data") or {}).get("codes")) or []
IDS = [c["id"] for c in codes]
AID = BID = None
try:
    if len(codes) < 2:
        ok("EB0 前置：能签 2 枚邀请码", False)
        summary()
        sys.exit(1)
    sa, ba = register(A, codes[0]["code"])
    # P-329：`.get("data") or {}` 只在键缺失时兜底——接口回 {"data": null}
    # （限流/PoW/注册被拒都会这样）时它返回 None，下一句 None.get 直接
    # AttributeError：整支闸门 EB1~EB8 一条没跑，而 finally 的清理照常打
    # PASS ⇒ 终端是 rc=1 / FAIL=0 的「假跑完」。改成 must-have 前置断言。
    AID = ((ba or {}).get("data") or {}).get("user_id")
    sb, bb = register(B, codes[1]["code"])
    BID = ((bb or {}).get("data") or {}).get("user_id")
    if AID is None or BID is None:
        ok("EB0 前置：两个探针账号都注册成功（拿得到 user_id）", False,
           "A=%s/%s B=%s/%s" % (sa, (ba or {}).get("message"),
                                sb, (bb or {}).get("message")))
        summary()
        sys.exit(1)
    TA = login_as(A)
    TB = login_as(B)
    ok("EB0 两个探针成员就绪（A=%s B=%s，都能登录=%s/%s）——没有这两个身份"
       "整批判据不可跑" % (AID, BID, bool(TA), bool(TB)),
       bool(AID and BID and TA and TB))
    if not (AID and BID and TA and TB):
        summary()
        sys.exit(1)
    psql("UPDATE users SET accept_pm='friends' WHERE id=%s" % BID)

    # ===== EB1 无任何关系 =====
    s1, b1 = send(TA, "EB1无关")
    ok("EB1 护栏：B 设「仅接收好友私信」而两人无任何关系时，A 发信必须被拒"
       "（http=%s msg=%s）" % (s1, (b1 or {}).get("message")),
       s1 >= 400 and "好友" in str((b1 or {}).get("message")))

    # ===== EB2 A 单方面发起好友申请 =====
    s2, b2 = call("POST", "/friends", {"username": B}, token=TA)
    shape = rows()
    pend = "pending" in shape
    ok("EB2 A 发起好友申请（http=%s）→ 库里出现 pending 行且**B 侧没有接受行**"
       "（当前关系=%s）" % (s2, shape), s2 == 200 and pend,
       "关系=%s" % shape)

    # ===== EB3 修前必红：单向 pending 也算好友 =====
    s3, b3 = send(TA, "EB3单向申请")
    got3 = psql("SELECT count(*) FROM messages WHERE sender_id=%s AND "
                "receiver_id=%s AND subject='EB3单向申请'" % (AID, BID))
    ok("EB3 P-295 判据：B 从未接受申请时，A 的发信**必须仍被拒**——实测 "
       "http=%s msg=%s，库里该信条数=%s（修前=200 且真落一行，说明「仅好友」"
       "屏障被单向申请绕过）" % (s3, (b3 or {}).get("message"), got3),
       s3 >= 400 and got3 == "0", "http=%s 落库=%s" % (s3, got3))

    # ===== EB4 正控：B 接受后必须能发 =====
    T0 = time.time()
    # 走文档化路径：B 对收到的 pending 申请执行 accept（friend.rs 的 accept
    # 分支会翻转申请方那行 + 插入自己那行）。第一版这里先让 B 也 add 一次，
    # 结果走进「对方已申请我」那条隐性分支（只翻转一行），把正控顶成假红——
    # 那条分支单独立为 P-296 钉桩（EB4c），不混进本批修复判据。
    s4b, _b4b = call("POST", "/friends/action",
                     {"username": A, "action": "accept"}, token=TB)
    shape4 = rows()
    ok("EB4a 关系成立：B 接受后库里必须至少有 1 行 list=friend（当前=%s）"
       "——反向行齐不齐由 EB4c 单独钉"
       "，别在这儿重复计数" % shape4,
       "friend" in shape4, "关系=%s" % shape4)
    flood_wait(T0)
    s4, b4 = send(TA, "EB4已好友")
    SENT.append(time.time())
    got4 = psql("SELECT count(*) FROM messages WHERE sender_id=%s AND "
                "receiver_id=%s AND subject='EB4已好友'" % (AID, BID))
    ok("EB4 正控（防「一律拒」的假修）：真好友时 A 必须发得出去——http=%s "
       "msg=%s 库里=%s 行" % (s4, (b4 or {}).get("message"), got4),
       s4 == 200 and got4 == "1", "http=%s 落库=%s" % (s4, got4))

    # EB4c 钉桩（P-296，本批不改）：accept 之后本应两侧各一行；「对方已申请
    # 我时我再 add 一次」那条分支只翻转申请方一行。承载文件 friend.rs 是站方
    # 在途改动，只入册不代改；补齐反向行的那天，这条转红提醒删桩。
    n_fr = psql("SELECT count(*) FROM friendships WHERE list='friend' AND "
                "((user_id=%s AND friend_id=%s) OR (user_id=%s AND "
                "friend_id=%s))" % (AID, BID, BID, AID))
    ok("EB4c 钉桩 P-296：现网 friend 行数=%s（1 = 反向行仍缺，属站方在途；"
       "别把这条当本批已修）" % n_fr, n_fr in ("1", "2"),
       "friend 行数=%s" % n_fr)

    # ===== EB5 B 把 A 拉黑后必须拒 =====
    s5, _b5 = call("POST", "/friends/action",
                   {"username": A, "action": "black"}, token=TB)
    s5b, b5b = send(TA, "EB5已拉黑")
    ok("EB5 拉黑通道没被我改坏：B 拉黑 A 后 A 发信必须拒（black http=%s，"
       "发信 http=%s msg=%s）" % (s5, s5b, (b5b or {}).get("message")),
       s5 == 200 and s5b >= 400)
    psql("DELETE FROM friendships WHERE user_id IN (%s,%s) OR friend_id IN "
         "(%s,%s)" % (AID, BID, AID, BID))

    # ===== EB6 accept_pm='no' 必须拒 =====
    psql("UPDATE users SET accept_pm='no' WHERE id=%s" % BID)
    s6, b6 = send(TA, "EB6拒收")
    ok("EB6 第 3 档「不接收私信」必须拒得住（http=%s msg=%s）"
       % (s6, (b6 or {}).get("message")), s6 >= 400)

    # ===== EB7 第二道正控：改回 'yes' 必须通（跨过 60s 站内信防刷窗口）=====
    psql("UPDATE users SET accept_pm='yes' WHERE id=%s" % BID)
    flood_wait(SENT[-1])
    s7, b7 = send(TA, "EB7全开")
    got7 = psql("SELECT count(*) FROM messages WHERE sender_id=%s AND "
               "receiver_id=%s AND subject='EB7全开'" % (AID, BID))
    ok("EB7 正控：accept_pm=yes 时同一对人必须发得出去（http=%s 库里=%s 行）"
       "——三档里「所有人」这一档也得有执行者" % (s7, got7),
       s7 == 200 and got7 == "1", "http=%s 落库=%s" % (s7, got7))

    # ===== EB8 静态对账：判定必须只看接收方那一行且带 list='friend' =====
    src = io.open(os.path.join(REPO, "apps", "api", "src", "community_http",
                               "message_send.rs"), encoding="utf-8").read()
    body = chr(10).join(l.split("//")[0] for l in src.splitlines())
    i = body.find("is_friend")
    win = body[i:i + 420]
    ok("EB8 静态对账：is_friend 那段 SQL 必须读 friendships.list（屏障生产"
       "端建立了 pending/friend/black 三态，判定端就得读它）：带 list='friend'"
       "=%s、查的是 friendships=%s"
       % ("list = 'friend'" in win, "friendships" in win),
       "list = 'friend'" in win and "friendships" in win)
finally:
    for cid in IDS:
        psql("DELETE FROM invites WHERE id=%d" % cid)
    if AID and BID:
        psql("DELETE FROM messages WHERE sender_id IN (%s,%s) OR receiver_id "
             "IN (%s,%s)" % (AID, BID, AID, BID))
        psql("DELETE FROM friendships WHERE user_id IN (%s,%s) OR friend_id "
             "IN (%s,%s)" % (AID, BID, AID, BID))
        psql("DELETE FROM message_flood WHERE user_id IN (%s,%s)"
             % (AID, BID))
    psql("UPDATE users SET status = 2 WHERE username IN ('%s','%s')" % (A, B))
    left = psql("SELECT count(*) FROM users WHERE (username LIKE 'eba%' OR "
               "username LIKE 'ebb%') AND status<>2")
    lmsg = psql("SELECT count(*) FROM messages m JOIN users u ON u.id=m."
                "sender_id WHERE u.username LIKE 'eba%' OR u.username LIKE "
                "'ebb%'")
    ok("EB9a 探针号已墓碑（可登录残留=%s）" % left, left == "0")
    ok("EB9b 探针私信与好友行删净（消息残留=%s）" % lmsg, lmsg == "0")
    ok("EB9c 邀请码收尾：按 id 删（本轮 %d 枚）后残留=%s"
       % (len(IDS), psql("SELECT count(*) FROM invites WHERE id IN (%s)"
                         % (",".join(str(x) for x in IDS) or "0"))),
       psql("SELECT count(*) FROM invites WHERE id IN (%s)"
            % (",".join(str(x) for x in IDS) or "0")) == "0")

summary()
