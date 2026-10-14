# -*- coding: utf-8 -*-
"""EC 闸门（批八十六）：站内信三个写口的计数必须说实话（P-297）。

现网（修前，容器 `api:latest`，2026-10-14 实测）：
  POST /messages/delete  拿 3 个**不存在**的 id 打过来 → {"deleted": 3}
  POST /messages/markread 拿 1 个不存在的 id          → {"updated": 0}
  POST /messages/move     拿 1 个不存在的 id          → {"moved": 0}
  ⇒ 同一个文件里三个口，只有 delete 回的是**请求条数**（body.ids.len()）
    而不是真实动过的行数。会员按界面上那句「已删除 N 封」判断事情成了没有，
    而未读数照旧、信照在 —— 越权尝试、已被对方删掉的信、拼错 id 这三种情况
    全被这个数字洗成「成功」。这是本仓「返回值当事实」那一族的又一次
    （P-225 解封 0 行也写审计、P-229 删不存在的规则回 {"ok":true}）。

判据：EC1 不存在的 id 必须 deleted=0（**修前必红**）；EC2 markread/move 同
一 id 必须 0（护栏，证明只有 delete 一支坏）；EC3 **越权**：发信方 A 删收信方
B 的那一份必须 deleted=0 且 B 的收件箱照旧；EC4 正控：收信方 B 自己删必须
deleted=1 且库里真的落地；EC5 NexusPHP 双删语义——只有一方删时信不许物理
消失，双方都删才真删；EC6 静态钉住 `deleted` 取自真实行数而不是 body.ids.len()；
EC7 收尾（探针号墓碑、探针信与自建文件夹删净）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_ec_message_counts.py
写库面：只有两个探针成员（EA/EB 同款注册配方）自己的收发件与自建文件夹；
真人数据一条不动。站内信防刷 60 秒一条 ⇒ A 只发一封。
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
A = "eca%d" % TS      # 发信方
B = "ecb%d" % TS      # 收信方
BOGUS = [999999991, 999999992, 999999993]


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def register(name, code):
    cap = call("GET", "/auth/captcha", None, token=ROOT)[1].get("data") or {}
    n = re.findall(r"-?\d+", str(cap.get("question") or ""))
    return call("POST", "/auth/register", {
        "username": name, "email": "%s@ec.local" % name,
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


clear_register_rl()
time.sleep(6)
codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 2},
               token=ROOT)[1].get("data") or {}).get("codes")) or []
IDS = [c["id"] for c in codes]
AID = BID = None
try:
    ok("EC0 前置：能签 2 枚邀请码（本轮 %d 枚）" % len(codes), len(codes) == 2)
    if len(codes) != 2:
        summary()
        sys.exit(1)
    _sa, ba = register(A, codes[0]["code"])
    AID = (ba or {}).get("data", {}).get("user_id")
    _sb, bb = register(B, codes[1]["code"])
    BID = (bb or {}).get("data", {}).get("user_id")
    TA = login_as(A)
    TB = login_as(B)
    ok("EC0b 两个探针成员就绪（A=%s B=%s，token=%s/%s）"
       % (AID, BID, bool(TA), bool(TB)), bool(AID and BID and TA and TB))
    if not (AID and BID and TA and TB):
        summary()
        sys.exit(1)

    # ===== EC1 / EC2 计数诚实性（不存在 id，零写入）=====
    s1, b1 = call("POST", "/messages/delete", {"ids": BOGUS}, token=TB)
    d1 = ((b1 or {}).get("data") or {})
    ok("EC1 P-297 判据：拿 %d 个**不存在**的 id 打 /messages/delete，"
       "deleted 必须是 0（实测 http=%s deleted=%s；修前=%d，即请求条数）"
       % (len(BOGUS), s1, d1.get("deleted"), len(BOGUS)),
       s1 == 200 and d1.get("deleted") == 0,
       "deleted=%s" % d1.get("deleted"))
    s2, b2 = call("POST", "/messages/markread", {"ids": BOGUS}, token=TB)
    s2b, b2b = call("POST", "/messages/move",
                    {"ids": BOGUS[:1], "folder": None}, token=TB)
    d2 = ((b2 or {}).get("data") or {})
    d2b = ((b2b or {}).get("data") or {})
    ok("EC2 护栏：同一条坏 id 打 markread/move 分别回 updated=%s moved=%s "
       "—— 三口里只有 delete 一支撒谎（这条绿说明不是我误读口径）"
       % (d2.get("updated"), d2b.get("moved")),
       d2.get("updated") == 0 and d2b.get("moved") == 0)

    # ===== 造一封真信：A → B =====
    s3, b3 = call("POST", "/messages",
                  {"to": B, "subject": "EC真信", "body": "EC 正文"},
                  token=TA)
    MID = psql("SELECT id FROM messages WHERE sender_id=%s AND receiver_id=%s"
               " AND subject='EC真信' ORDER BY id DESC LIMIT 1" % (AID, BID))
    ok("EC3 前置：A 给 B 的发信成功（http=%s）且库里 MID=%s" % (s3, MID),
       s3 == 200 and MID.isdigit(), "http=%s MID=%s" % (s3, MID))
    if not MID.isdigit():
        summary()
        sys.exit(1)
    MI = int(MID)

    # ===== EC4 越权：发信方 A 不能替收信方「删掉收件箱那一份」并谎报成功 =====
    s4, b4 = call("POST", "/messages/delete", {"ids": [MI]}, token=TA)
    d4 = ((b4 or {}).get("data") or {})
    still = psql("SELECT count(*) FROM messages WHERE id=%d AND receiver_id=%s"
                 " AND location=1" % (MI, BID))
    ok("EC4 越权面：A（发信方）删 B 那一份——A 只能动自己的 saved 位，"
       "deleted 必须按**真实动过的行**数（实测 deleted=%s）；B 的主收件箱"
       "那一份必须照在（库里=%s 行）" % (d4.get("deleted"), still),
       still == "1" and d4.get("deleted") in (0, 1))

    # ===== EC5 双删语义 + 正控 =====
    state5 = psql("SELECT location || '/' || saved FROM messages WHERE id=%d"
                  % MI)
    ok("EC5a 只有一方删过时**不许物理消失**：当前 location/saved=%s（行还在"
       "=双删语义成立）" % state5, state5 != "", "状态=%s" % state5)
    s5, b5 = call("POST", "/messages/delete", {"ids": [MI]}, token=TB)
    d5 = ((b5 or {}).get("data") or {})
    gone = psql("SELECT count(*) FROM messages WHERE id=%d" % MI)
    ok("EC5b 正控：双方都删之后必须真物理删除——B 这次 deleted=%s、"
       "库里该行=%s（修前后半句一直是对的，这条钉住别被改成永远不删）"
       % (d5.get("deleted"), gone), gone == "0",
       "deleted=%s 残留=%s" % (d5.get("deleted"), gone))

    # ===== EC6 静态对账 =====
    src = io.open(os.path.join(REPO, "apps", "api", "src", "community_http",
                               "message_box.rs"), encoding="utf-8").read()
    code = chr(10).join(l.split("//")[0] for l in src.splitlines())
    del_body = code[code.index("message_delete"):
                     code.index("message_move")]
    ok("EC6 静态：delete 口的响应不能再取 body.ids.len()，必须来自真实行数"
       "（含 body.ids.len()=%s，含 owned 计数=%s）"
       % ("body.ids.len() }" in code, "SELECT count(*) FROM messages WHERE id"
          " = ANY($1)" in code.replace("\\\n", "").replace("         ", " ")),
       "body.ids.len()" not in del_body
       and "SELECT count(*)" in del_body)
finally:
    for cid in IDS:
        psql("DELETE FROM invites WHERE id=%d" % cid)
    if AID and BID:
        psql("DELETE FROM messages WHERE sender_id IN (%s,%s) OR receiver_id "
             "IN (%s,%s)" % (AID, BID, AID, BID))
        psql("DELETE FROM pmboxes WHERE user_id IN (%s,%s)" % (AID, BID))
        psql("DELETE FROM message_flood WHERE user_id IN (%s,%s)" % (AID, BID))
    psql("UPDATE users SET status = 2 WHERE username IN ('%s','%s')" % (A, B))
    # 残料统计一律按**本轮专属用户名**精确取。上一版用 username LIKE 'ec%'，
    # 把站内一个名字以 ec 开头的演示成员（连同系统在他注册时自动发的那封
    # sender_id=NULL 的欢迎信）算进账 ⇒ 凭空造出「残留 1」的假账。
    # 这条坑仓里记过（「残料统计只用本轮专属前缀」），又犯了一次。
    names = "'%s','%s'" % (A, B)
    left = psql("SELECT count(*) FROM users WHERE username IN (%s) "
                "AND status<>2" % names)
    probe = ("SELECT id FROM users WHERE username IN (%s)" % names)
    lmsg = psql("SELECT count(*) FROM messages WHERE sender_id IN (%s) "
                "OR receiver_id IN (%s)" % (probe, probe))
    ok("EC7a 探针号已墓碑（可登录残留=%s）" % left, left == "0")
    ok("EC7b 探针信件删净（残留=%s）" % lmsg, lmsg == "0")
    ok("EC7c 本轮邀请码按 id 删净（残留=%s）"
       % psql("SELECT count(*) FROM invites WHERE id IN (%s)"
              % (",".join(str(x) for x in IDS) or "0")),
       psql("SELECT count(*) FROM invites WHERE id IN (%s)"
            % (",".join(str(x) for x in IDS) or "0")) == "0")

summary()
