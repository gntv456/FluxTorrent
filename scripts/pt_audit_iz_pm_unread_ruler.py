# -*- coding: utf-8 -*-
"""常驻闸门 IZ：私信「未读数」必须用站内角标那把尺（P-545）。

会员视角：手机上挂 PT-Plugin-Plus 或开放 API 的移动壳，未读提醒只有一个数。
现网读数（10-22 只取数）：库里有 3 行「仍未读、但已经不在收件箱」的信
（location<>1），外部出口把它们照算未读，而本人站内角标是 0 ⇒ 客户端
角标挂着一个站内没有任何入口能清掉的数。旧写法只筛 `read_at IS NULL`，
既不看删除态也不看文件夹。两形都验：移入文件夹（活体红证；站内删除走
「一行两态双删」会物理抹行，所以回收站那一形用同形状夹具行来验）。

修法（本批只落地载体干净的口）：
  compat_http/ptpp.rs 的 messageCount 改为
  `unread AND location = 1 AND folder IS NULL`，与 /me、/me/settings、
  community_http/message_stream.rs::unread_inbox 同一把尺。
在册未收口（openapi_http/me.rs 正被站方整文件排版，本批不动它）：
  L47 /open/me/overview 的 unread_messages 与 L206 /open/me/messages 的
  unread=1 过滤（列表本体同用那只 fw，删掉的私信正文照样外发）。

判据分层：
  IZ0 前提   库里 unread 与 read_at IS NULL 零分歧 ⇒ 分叉只可能来自
             location/folder 而不是「列选错」；一旦有分歧说明写入侧改了
             形状，这把尺要重选，本条先红提醒。
  IZ1 正控   探针收 3 封真未读 ⇒ 站内角标 3、PT++ 3（不许过滤成 0）
  IZ2a 活体   删 2 封 + 移 1 封进文件夹 ⇒ 站内角标必须归零（前提）
  IZ2b 活体   文件夹那一形：角标 0 ⇒ PT++ 必须等于角标
  IZ2c 活体   回收站那一形：插 1 行 location=0 的未读（与现网同形状）
              ⇒ 角标仍 0 ⇒ PT++ 仍须 0
              旧镜像现网 2b/2c 都红（读到 1、2）＝缺陷本体，不是产品账
  IZ3 静态   ptpp.rs 的未读 SQL 带 location = 1 与 folder IS NULL
  IZ4 台账   全仓 `read_at IS NULL` 只许出现在在册那两只口（多一个红、
             少一个也红——少了一个说明已修，要同步升真判据）
  IZ5 读数   现网 /open/me/overview 的未读数与站内角标的差（WARN）
收尾：撤探针 Token（数 revoked_at IS NULL）、物理删探针信与文件夹、删探针号，
并断言残料为 0。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_iz_pm_unread_ruler.py
      站方重建 api 之后重跑，IZ2 应转绿。
"""
import io
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import (BASE, call, login, ok, psql_stdin,
                          summary)  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "apps", "api", "src")
PW_OLD = "Gate#123456"
PW_NEW = "Gate#2345678"
PREFIX = "IZ探针" + str(int(time.time()))[-6:]
TOKEN_NAME = "iz-" + PREFIX

# 在册未收口台账：文件 → 该文件里 `read_at IS NULL` 的字面出现次数
LEDGER = {"openapi_http/me.rs": 2}


def psql(sql):
    return psql_stdin(sql if sql.strip().endswith(";") else sql + ";")


def read_src(rel):
    with io.open(os.path.join(SRC, rel), encoding="utf-8") as f:
        return f.read()


def strip_line_comments(src):
    """整行 // 注释剥掉（本批补丁的说明行会提到 read_at IS NULL，
    不剥就会被静态判据当成「旧形状仍在册」——注释洗白真洞那一族）。"""
    keep = []
    for ln in src.splitlines():
        if ln.strip().startswith("//"):
            continue
        keep.append(ln)
    return "\n".join(keep)


def files_with(rel_suffix=None):
    out = []
    for dirpath, _dirs, names in os.walk(SRC):
        for n in names:
            if not n.endswith(".rs"):
                continue
            rel = os.path.relpath(os.path.join(dirpath, n), SRC)
            rel = rel.replace("\\", "/")
            if rel_suffix is None or rel.endswith(rel_suffix):
                out.append(rel)
    return out


def ext_get(path, token):
    """开放 API Token 走 `Authorization: Token <secret>`，与 lib.call 的
    Bearer（会话 JWT）不是一把尺，所以单开一个取值口。"""
    req = urllib.request.Request(BASE + path)
    req.add_header("Authorization", "Token " + token)
    try:
        r = urllib.request.urlopen(req, timeout=30)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:  # noqa: BLE001
            return e.code, {"raw": raw[:200].decode("utf-8", "replace")}


def unread_of(tok, which):
    st, r = ext_get(which, tok)
    d = (r or {}).get("data") or {}
    if which == "/plugins/ptppUserInfo":
        return st, d.get("messageCount"), d
    return st, d.get("unread_messages"), d


def main():
    root = login()
    ts = int(time.time()) % 100000
    user = "iz%d" % ts
    s, r = call("POST", "/admin/adduser", {"username": user,
                                           "password": PW_OLD,
                                           "email": "%s@iz.local" % user},
                token=root)
    if s != 200:
        raise SystemExit("建探针号失败 http=%s %s" % (s, str(r)[:160]))
    uid = int(psql("SELECT id FROM users WHERE username='%s'" % user) or 0)

    tok = None
    tid = None
    box_id = None
    try:
        # 临时密码账号只放行 /me 与改密两条自救路（auth_infra.rs），
        # 所以先改密再签 Token——改密会撤销旧 token，顺序不能颠倒。
        pw = login(user, PW_OLD)
        s, r = call("POST", "/me/password/change",
                    {"old_password": PW_OLD, "new_password": PW_NEW}, token=pw)
        ok("IZ1a 探针改密（临时密码账号写口的前提）", s == 200,
           "http=%s %s" % (s, str(r)[:160]))
        mem = login(user, PW_NEW)

        diverge = psql("SELECT count(*) FROM messages "
                       "WHERE unread <> (read_at IS NULL)")
        ok("IZ0 前提：unread 与 read_at 零分歧（现网 %s 行分叉）" % diverge,
           diverge == "0",
           "有分歧说明写入口改了形状，这把尺要重选，先别照抄本批修法")

        # 3 封真未读（root 是管理组，不受 accept_pm 与 60s 防刷限制）
        for i in range(3):
            call("POST", "/messages", {"to": user,
                                       "subject": "%s-%d" % (PREFIX, i),
                                       "body": "闸门 IZ 夹具，跑完即删"},
                 token=root)
        ids = [int(x) for x in psql(
            "SELECT id FROM messages WHERE receiver_id=%d "
            "AND subject LIKE '%s%%' ORDER BY id" % (uid, PREFIX)).split()]
        ok("IZ1b 夹具 3 封已投递", len(ids) == 3, "ids=%s" % ids)

        s, me1 = call("GET", "/me", None, token=mem)
        badge1 = ((me1 or {}).get("data") or {}).get("unread_messages")
        ok("IZ1c 站内角标 = 3", badge1 == 3, "badge=%s" % badge1)

        s, tr = call("POST", "/me/tokens", {"password": PW_NEW,
                                            "name": TOKEN_NAME,
                                            "scopes": ["read"],
                                            "rate_per_min": 120}, token=mem)
        td = (tr or {}).get("data") or {}
        tok, tid = td.get("token"), td.get("id")
        ok("IZ1d 探针 Token 已签发", s == 200 and bool(tok),
           "http=%s %s" % (s, str(tr)[:160]))
        if not tok:
            raise SystemExit("没有 Token，活体判据无法继续")

        st, pt1, _ = unread_of(tok, "/plugins/ptppUserInfo")
        ok("IZ1e 正控：真未读必须计到（PT++ messageCount=%s，http=%s）"
           % (pt1, st), pt1 == 3,
           "过滤成 0 说明把尺子修成了「永远没有未读」")

        # 删 2 封（进回收站）+ 移 1 封进文件夹，全部保持未读
        s, r = call("POST", "/messages/boxes", {"name": "IZ夹"}, token=mem)
        box_id = psql("SELECT id FROM pmboxes WHERE user_id=%d AND name='IZ夹'"
                      % uid).strip()
        ok("IZ1f 探针文件夹已建", s == 200 and box_id != "",
           "http=%s box=%s" % (s, box_id))
        call("POST", "/messages/move", {"ids": [ids[2]],
                                        "folder": int(box_id)}, token=mem)
        s, dr = call("POST", "/messages/delete", {"ids": ids[0:2]}, token=mem)
        ok("IZ1g 2 封已删除", s == 200, "http=%s %s" % (s, str(dr)[:120]))

        s, me2 = call("GET", "/me", None, token=mem)
        badge2 = ((me2 or {}).get("data") or {}).get("unread_messages")
        ok("IZ2a 站内角标已归零（前提）", badge2 == 0, "badge=%s" % badge2)

        st, pt2, _ = unread_of(tok, "/plugins/ptppUserInfo")
        ok("IZ2b PT++ 未读必须等于站内角标（读到 %s，站内 0）" % pt2,
           pt2 == badge2,
           "差 %s＝已删/已归档的信在外部出口当永久未读，站内没有入口能清；"
           "站方重建 api 镜像后这条才转绿" % ((pt2 or 0) - badge2))

        # 回收站那一形：站内删除走「一行两态双删」（发件方未存副本时直接
        # 物理抹行），所以按现网那 3 行的形状插一行 location=0 的未读信
        psql("INSERT INTO messages(sender_id, receiver_id, subject, body, "
             "unread, location, created_at) "
             "VALUES (NULL, %d, '%s-T', '闸门 IZ 夹具', true, 0, now());"
             % (uid, PREFIX))
        st, me3 = call("GET", "/me", None, token=mem)
        badge3 = ((me3 or {}).get("data") or {}).get("unread_messages")
        st, pt3, _ = unread_of(tok, "/plugins/ptppUserInfo")
        ok("IZ2c 回收站里的未读不得计入（PT++ %s / 站内 %s）" % (pt3, badge3),
           pt3 == badge3 and badge3 == 0,
           "站内已经看不见这封信，外部却多一个清不掉的未读")

        st, ov, _ = unread_of(tok, "/open/me/overview")
        print("[WARN] 在册未收口 /open/me/overview unread_messages=%s "
              "http=%s（本批未改 openapi/me.rs：站方正在排版该文件）"
              % (ov, st))
        st2, lm = ext_get("/open/me/messages?limit=50", tok)
        shown = ((lm or {}).get("data") or {}).get("total")
        print("[WARN] /open/me/messages total=%s http=%s（含回收站行＝"
              "删掉的私信正文仍外发，在册未收口）" % (shown, st2))
    finally:
        if tok and tid:
            pw2 = None
            try:
                pw2 = login(user, PW_NEW)
            except SystemExit:
                pw2 = None
            if pw2:
                call("POST", "/me/tokens/revoke", {"id": tid}, token=pw2)
        psql("DELETE FROM messages WHERE subject LIKE '%s%%';" % PREFIX)
        if box_id:
            psql("DELETE FROM pmboxes WHERE id=%s;" % box_id)
        call("DELETE", "/admin/users/%d" % uid, None, token=root)
        left_pm = psql("SELECT count(*) FROM messages WHERE subject "
                       "LIKE '%s%%'" % PREFIX)
        left_tk = psql("SELECT count(*) FROM api_tokens WHERE name='%s' "
                       "AND revoked_at IS NULL" % TOKEN_NAME)
        left_box = psql("SELECT count(*) FROM pmboxes WHERE name='IZ夹'")
        ok("IZ9 残料归零（私信 %s / 有效 Token %s / 文件夹 %s）"
           % (left_pm, left_tk, left_box),
           left_pm == "0" and left_tk == "0" and left_box == "0")

    # ---- 静态判据（与活体同源，不受镜像滞后影响）----
    ptpp = strip_line_comments(read_src("compat_http/ptpp.rs"))
    m = re.search(r'SELECT count\(\*\) FROM messages[^"]*', ptpp, re.S)
    ok("IZ3a ptpp 未读 SQL 已找到", bool(m), "字面量里没抓到 messages 计数")
    if m:
        clause = m.group(0)
        ok("IZ3b 带 location = 1", "location = 1" in clause, clause[:120])
        ok("IZ3c 带 folder IS NULL", "folder IS NULL" in clause, clause[:120])

    # 站内三只口必须一直是同一把尺（本批没动，防后续被单点改动带偏）
    for rel in ("auth_http/me.rs", "auth_http/me_settings.rs",
                "community_http/message_stream.rs"):
        body = strip_line_comments(read_src(rel))
        hit = re.findall(r'SELECT count\(\*\) FROM messages[^"]*', body, re.S)
        bad = [h for h in hit
               if not ("location = 1" in h and "folder IS NULL" in h)]
        ok("IZ3d 站内尺未失配 %s" % rel, bool(hit) and not bad,
           "抓到 %d 条，违例 %d" % (len(hit), len(bad)))

    census = {}
    for rel in files_with():
        body = strip_line_comments(read_src(rel))
        n = body.count("read_at IS NULL")
        if n:
            census[rel] = n
    ok("IZ4 台账一致：read_at IS NULL 只允许在册的 %s，实测 %s"
       % (LEDGER, census), census == LEDGER,
       "多一个＝新洞；少一个＝那只口已修，请把 IZ2 的活体判据扩到它并撤桩")


if __name__ == "__main__":
    main()
    summary()
