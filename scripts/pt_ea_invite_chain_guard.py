# -*- coding: utf-8 -*-
"""EA 闸门（批八十四）：邀请码链路的三条硬护栏——**一次实测结论转成常驻判据**。

本轮（2026-10-14，打在站方现网容器 `api:latest`）实测结论是「无洞」，
但「无洞」只在有人反复打它时才成立。邀请码是**可被抢用的凭证**，
这一族的三条不变量必须有常驻判据：
  EA1 一枚码只能用一次（第二次必须 4xx，且库里状态真的是已用）；
  EA2 过期码必须被拒，且**不许留下半注册账号**（拒了还建号=双重洞）；
  EA3 跨用户读不到别人的明文码（列表按 inviter_id 收窄、详情口 404）；
  EA4 用码注册成功时 `users.invited_by` 必须落对（邀请关系与奖励的锚点）；
  EA5 面板 `/invites/status` 的三个计数必须与该 inviter 的真实码一致
      （「显示的名额/已用/未用」是会员据以决策的数字，不能是另一套口径）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_ea_invite_chain_guard.py
写库面：本轮签的邀请码按 id 当场删；探针成员收尾 status=2 墓碑
（注册要过邀请码+验证码，注册口本身会限流，所以全程只用两次注册）。
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
TS = int(time.time()) % 99199
VICTIM = "eav%d" % TS           # 受害者用的码（不消耗）
SPARE = "eap%d" % TS            # 夹具码（被第一次注册消耗）


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
        "username": name, "email": "%s@ea-gate.local" % name,
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
made = ((call("POST", "/admin/invites", {"user_id": 1, "count": 2},
              token=ROOT)[1].get("data") or {}).get("codes")) or []
IDS = [c["id"] for c in made]
CODE = {c["id"]: c["code"] for c in made}
ok("EA0 前置：root 能签 2 枚码（id=%s）" % IDS, len(made) == 2,
   "签了 %d 枚" % len(made))
if len(made) != 2:
    summary()
    sys.exit(1)
victim_id, spare_id = IDS[0], IDS[1]

try:
    NEW = "eau%d" % TS
    s1, b1 = register(NEW, CODE[spare_id])
    uid = (b1 or {}).get("data") or {}
    UID = uid.get("user_id")
    ok("EA4a 用夹具码注册成功（http=%s uid=%s）——后面三条都靠这一条站住"
       % (s1, UID), s1 == 200 and UID, "http=%s uid=%s" % (s1, UID))
    rel = psql("SELECT invited_by FROM users WHERE id=%s" % UID)
    ok("EA4b 邀请关系落对：新用户 `users.invited_by` 必须是邀请人 1（root），"
       "实测=%s（这是被邀请奖励与「我邀请来的人」台账的唯一锚点）" % rel,
       rel == "1", "invited_by=%s" % rel)

    # ===== EA1 一枚码只能用一次 =====
    s2, b2 = register("eaurerun%d" % TS, CODE[spare_id])
    st = psql("SELECT status || '/' || used_by FROM invites WHERE id=%d"
              % spare_id)
    ok("EA1 护栏：同一枚码二次注册必须被拒（实测 http=%s code=%s "
       "msg=%s），且库里状态确实是已用（status/used_by=%s）"
       % (s2, (b2 or {}).get("code"), (b2 or {}).get("message"), st),
       s2 >= 400 and st.startswith("1/"), "http=%s 库里=%s" % (s2, st))
    ghost = psql("SELECT count(*) FROM users WHERE username='eaurerun%d'" % TS)
    ok("EA1b 被拒的那次**不许留下账号**（半注册=脏数据+可登录影子号）：%s"
       % ghost, ghost == "0", "残留=%s" % ghost)

    # ===== EA2 过期码必须被拒 =====
    psql("UPDATE invites SET expires_at = now() - interval '1 hour' "
         "WHERE id=%d" % victim_id)
    s3, b3 = register("eaexp%d" % TS, CODE[victim_id])
    ghost2 = psql("SELECT count(*) FROM users WHERE username='eaexp%d'" % TS)
    ok("EA2 护栏：过期码注册必须被拒（实测 http=%s code=%s msg=%s）"
       "且不许建号（残留=%s）"
       % (s3, (b3 or {}).get("code"), (b3 or {}).get("message"), ghost2),
       s3 >= 400 and ghost2 == "0", "http=%s 残留=%s" % (s3, ghost2))

    # ===== EA3 跨用户读不到别人的明文码 =====
    MT = login_as(NEW)
    ok("EA3a 探针成员可登录（拿 token=%s）——没有观众身份，EA3 整组不可判"
       % bool(MT), bool(MT))
    if MT:
        s4, b4 = call("GET", "/invites", None, token=MT)
        txt4 = str((b4 or {}).get("data"))
        ok("EA3b 成员打 GET /invites 只能看到**自己**的码：响应含 root 的"
           "明文码=%s（http=%s，体=%s）"
           % (CODE[victim_id] in txt4 or CODE[spare_id] in txt4, s4,
              txt4[:70]),
           s4 == 200 and CODE[victim_id] not in txt4
           and CODE[spare_id] not in txt4)
        s5, b5 = call("GET", "/invites/%d" % spare_id, None, token=MT)
        ok("EA3c 成员打他人名下码的详情口必须是 **404/4xx**（修前若能 200 "
           "并回明文 code，任何人都能抢用别人的码）：实测 http=%s code=%s "
           "体=%s" % (s5, (b5 or {}).get("code"),
                     str((b5 or {}).get("data"))[:60]),
           s5 >= 400)

        # ===== EA5 面板计数与真实数据同口径 =====
        s6, b6 = call("GET", "/invites/status", None, token=MT)
        d6 = (b6 or {}).get("data") or {}
        real = psql("SELECT count(*) FILTER (WHERE status=0 AND expires_at>"
                    "now()) || '/' || count(*) FILTER (WHERE status=1) "
                    "FROM invites WHERE inviter_id=%s" % UID)
        ok("EA5 /invites/status 的 unused/used 必须等于按 inviter_id 数出来的"
           "真值（面板 unused=%s used=%s，库内=%s）——会员按这三个数决定"
           "要不要花魔力兑码，口径不能是另一套"
           % (d6.get("unused"), d6.get("used"), real),
           s6 == 200 and "%s/%s" % (d6.get("unused"), d6.get("used")) == real,
           "库内=%s 面板=%s/%s" % (real, d6.get("unused"), d6.get("used")))
finally:
    for cid in IDS:
        psql("DELETE FROM invites WHERE id=%d" % cid)
    psql("UPDATE users SET status = 2 WHERE username LIKE 'eau%%' "
         "OR username LIKE 'eaurerun%%' OR username LIKE 'eaexp%%'")
    left_u = psql("SELECT count(*) FROM users WHERE (username LIKE 'eau%' "
                  "OR username LIKE 'eaurerun%' OR username LIKE 'eaexp%') "
                  "AND status<>2")
    left_i = psql("SELECT count(*) FROM invites WHERE id IN (%s)"
                  % (",".join(str(x) for x in IDS)))
    ok("EA6a 本轮邀请码删净（按 id 回查残留=%s）" % left_i, left_i == "0")
    ok("EA6b 探针成员全部墓碑（可登录残留=%s）" % left_u, left_u == "0")

summary()
