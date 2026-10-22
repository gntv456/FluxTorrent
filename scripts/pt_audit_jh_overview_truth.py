# -*- coding: utf-8 -*-
"""常驻闸门 JH：控制台概览（GET /me/overview）每个读数都要等于库里真值。

这一族今天已经抓出四条：P-515 等级名恒空（text 列按 JSON 解码）、
P-517 cid 恒 0（int4 按 i64 解码）、P-550 收藏夹硬写 0（没人算）、
P-554 三个取值闭包把解码失败咽成 0/false/""。共同点是**接口回 200、
code=0、界面照常渲染，而那个数是假的**——类型门、编译、单测都抓不到。
所以判据不写「字段存在」，写「等于库里那一枚」，并要求 payload 里
至少有 8 个数非零（否则「库里全是 0」会让对账空绿）。

对账口径一律引站内单点，不自己另算：
  未读  = unread ∧ location=1 ∧ folder IS NULL（与 /me 角标同尺）
  收藏  = bookmarks JOIN torrents 走 visibility（会员只见 1 档，
          管理组另加 0/2/4；3 档谁都不可读）
  勋章  = user_medals 带未过期过滤（与勋章墙同尺，见 JD）
  发布  = approval_status = 1（会员面口径，见 P-549）

判据：
  JH1  逐字段 api == 库值（root 一名 + 普通会员一名）
  JH2  非空前提：root 的 payload 至少 8 个数字非零
  JH3  正控：把某枚期望值改 1，比较器必须只报那一个字段
  JH4  静态：me_settings.rs 的取值闭包必须留声（decode_warn），
       不许退回 unwrap_or 静默——P-515/517 的机制就是它

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_jh_overview_truth.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import call, dig, login, ok, psql_stdin, summary  # noqa

SRC = "apps/api/src/auth_http/me_settings.rs"
PW_OLD = "Gate#123456"
PW_NEW = "Gate#2345678"


def q(sql, uid):
    v = psql_stdin((sql + ";").format(uid=uid))
    return v.strip()


def q2(sql):
    return psql_stdin(sql + ";").strip()


def expect_sqls(is_staff):
    """字段 → 库里真值的取法。收藏/勋章的可见性口径与站内单点一致。"""
    vis = ("(t.approval_status = 1 OR t.approval_status IN (0, 2, 4))"
           if is_staff else "t.approval_status = 1")
    return {
        "uploaded": "SELECT uploaded FROM users WHERE id={uid}",
        "downloaded": "SELECT downloaded FROM users WHERE id={uid}",
        "spark_balance": "SELECT COALESCE(spark_balance,0) FROM users "
                         "WHERE id={uid}",
        "seeding": "SELECT count(*) FROM snatches s WHERE s.user_id={uid} "
                   "AND s.seeding",
        "leeching": "SELECT count(*) FROM snatches s WHERE s.user_id={uid} "
                    "AND s.leeching",
        "uploads": "SELECT count(*) FROM torrents t WHERE t.owner_id={uid} "
                   "AND t.approval_status = 1",
        "comments": "SELECT count(*) FROM comments c WHERE c.user_id={uid}",
        "medals": "SELECT count(*) FROM user_medals m WHERE m.user_id={uid} "
                  "AND (m.expires_at IS NULL OR m.expires_at > now())",
        "invites_pending": "SELECT count(*) FROM invites i WHERE "
                           "i.inviter_id={uid} AND i.status = 0",
        "invites_used": "SELECT count(*) FROM invites i WHERE "
                        "i.inviter_id={uid} AND i.status = 1",
        "unread_messages": "SELECT count(*) FROM messages m WHERE "
                           "m.receiver_id={uid} AND m.unread AND "
                           "m.location = 1 AND m.folder IS NULL",
        "bookmarks": "SELECT count(*) FROM bookmarks b JOIN torrents t "
                     "ON t.id = b.torrent_id WHERE b.user_id={uid} AND " + vis,
    }


def compare(api, exp):
    """返回不一致的字段清单；库里取不到（空串）时不比，避免假红。"""
    bad = []
    for k, want in exp.items():
        if want == "" or k not in api:
            continue
        got = api.get(k)
        if str(got) != str(want):
            bad.append("%s: api=%s 库=%s" % (k, got, want))
    return bad


def one_user(tok, uid, is_staff):
    s, r = call("GET", "/me/overview", None, token=tok)
    api = dig(r, "data") or {}
    exp = {k: q(v, uid) for k, v in expect_sqls(is_staff).items()}
    return s, api, exp


def main():
    root = login("root", "password123")
    s, api, exp = one_user(root, 1, True)
    bad = compare(api, exp)
    ok("JH1a root 概览逐字段等于库里（比 %d 个字段）" % len(exp),
       s == 200 and not bad, "; ".join(bad)[:200])
    nonzero = [k for k, v in exp.items()
               if str(v) not in ("", "0", "None")]
    ok("JH2 非空前提：root 的库里读数至少 8 个非零（否则对账是空的）",
       len(nonzero) >= 8, "只有 %d 个非零：%s" % (len(nonzero), nonzero))

    # ── JH1b/c：普通会员面（本人会话）＋ 自己造的读数 ──────────────
    # 只用本脚本新建的探针号，绝不拿库里现成的号做删除对象（第一版误按
    # `class<90 AND status<2` 挑了 uid 39 去删——那是前几批留下的墓碑号，
    # 幸好管理口按权限/形状拒了没删成，但这种动作本身不许再出现）。
    name = "jh%d" % (int(time.time()) % 100000)
    s2, r2 = call("POST", "/admin/adduser",
                  {"username": name, "password": PW_OLD,
                   "email": "%s@jh.local" % name}, token=root)
    if s2 == 200:
        uid2 = int(q("SELECT id FROM users WHERE username='%s'" % name, 0) or 0)
        sub = "JH探针" + name
        try:
            # 根会话发一封未读私信 + 探针自己收藏一枚过审种
            call("POST", "/messages", {"to": name, "subject": sub,
                                       "body": "闸门 JH 夹具，跑完即删"},
                 token=root)
            p0 = login(name, PW_OLD)
            s2b, _r = call("POST", "/me/password/change",
                           {"old_password": PW_OLD, "new_password": PW_NEW},
                           token=p0)
            ok("JH1b0 探针改密（临时密码账号写口前提，http=%s）" % s2b,
               s2b == 200)
            p1 = login(name, PW_NEW)
            tid = q("SELECT id FROM torrents WHERE approval_status = 1 "
                    "ORDER BY id DESC LIMIT 1", 0)
            s2c, _r2c = call("PUT", "/torrents/%s/bookmark" % tid,
                             {"on": True}, token=p1)
            ok("JH1b1 夹具：收藏这枚过审种（http=%s）" % s2c, s2c == 200)
            s3, r3 = call("GET", "/me/overview", None, token=p1)
            api3 = dig(r3, "data") or {}
            exp3 = {k: q(v, uid2) for k, v in expect_sqls(False).items()}
            bad3 = compare(api3, exp3)
            ok("JH1b 会员概览逐字段等于库里 uid=%d（比 %d 个字段）"
               % (uid2, len(exp3)), s3 == 200 and not bad3,
               "; ".join(bad3)[:200])
            ok("JH1c 会员读到的是自己的账、不串号（uploaded=%s，"
               "收藏=%s，未读=%s）" % (api3.get("uploaded"),
                                      api3.get("bookmarks"),
                                      api3.get("unread_messages")),
               str(api3.get("uploaded")) != str(exp.get("uploaded"))
               and str(api3.get("bookmarks")) == "1"
               and str(api3.get("unread_messages")) == "1")
        finally:
            q2("DELETE FROM messages WHERE subject = '%s'" % sub)
            q2("DELETE FROM bookmarks WHERE user_id = %d" % uid2)
            call("DELETE", "/admin/users/%d" % uid2, None, token=root)
            left = q("SELECT count(*) FROM messages WHERE receiver_id={uid}",
                     uid2)
            leftb = q("SELECT count(*) FROM bookmarks WHERE user_id={uid}",
                      uid2)
            ok("JH9 残料归零（探针私信 %s / 收藏 %s）" % (left, leftb),
               left == "0" and leftb == "0")
    else:
        ok("JH1b 建探针号（普通会员面）", False,
           "http=%s %s" % (s2, str(r2)[:120]))

    base_bad = [x.split(":")[0] for x in compare(api, exp)]
    shift = dict(exp)
    shift["spark_balance"] = str(int(exp["spark_balance"]) + 1)
    pc = [x.split(":")[0] for x in compare(api, shift)]
    ok("JH3 正控：改一枚期望值，比较器必须只多报它一个（spark_balance）",
       len(pc) == len(base_bad) + 1 and "spark_balance" in pc,
       "基线不一致=%s 改后=%s" % (base_bad, pc))

    body = "\n".join(l for l in open(os.path.join(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
        SRC), encoding="utf-8", errors="replace").read().splitlines()
        if not l.strip().startswith("//"))
    ok("JH4 取值闭包解码失败要留声（不许退回 unwrap_or 静默）",
       "decode_warn" in body and body.count("unwrap_or(0)") == 0
       and "unwrap_or(false)" not in body,
       "P-515/P-517 的机制就是这三个闭包吞掉解码失败")


if __name__ == "__main__":
    main()
    summary()
