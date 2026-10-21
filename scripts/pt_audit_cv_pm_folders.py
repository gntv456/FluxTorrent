# -*- coding: utf-8 -*-
"""CV 闸门：自建信箱文件夹三条链路（P-489 / P-490 / P-491）。

现网实测（2026-10-21，容器 `api:latest` 05:13Z，root 名下 0 个文件夹）：

  A) 收件方把一封「发件方仍存副本」的信（saved=1）放进自建文件夹，
     `POST /messages/boxes {id, name:""}` 删夹后：
       该行被**物理删除**，sender 的发件箱副本随之消失。
     删文件夹只是收件方这一侧的批量删除；messages 一行两态
     （receiver 的 location / sender 的 saved），双方都删才可物理清
     ——`message_box.rs` 删信口就是这么做的，夹子删除绕过了它。

  B) 本人两个夹子 A、B，把 B 改名成 A：
       http=404 {"code":1004,"message":"资源不存在"}
     `pmboxes` 有 UNIQUE(user_id,name)，改名触发唯一键冲突后被
     `unwrap_or_default()` 吞成 0 行 ⇒ 「名字被占用」演成「文件夹不存在」。

  C) 新建一个已存在的名字：
       http=200 {"id": <旧夹子 id>}
     `ON CONFLICT DO UPDATE ... RETURNING id` 把重名当成功，界面弹
     「文件夹已创建」而列表一封没多。

判据：
  CV0 前置：root 名下文件夹数 <3（新建撞名用例需要走得到 insert）；
  CV1 新建撞名必须 1002「已有同名文件夹」且同名单夹只有一行（修前 200）；
  CV2 改名撞名必须 1002 且两个夹子名字都没动（修前 404 资源不存在）；
  CV3a 删夹请求本身 200；
  CV3b 发件方仍存副本的信必须留行：location=0、folder=NULL、saved=1，
       且 sender 发件箱还能查到（修前行被物理删除）；
  CV3c 发件方已删（saved=0）的信才物理清掉（防「修成一个都不删」）；
  CV3d 夹子行消失、另一个夹子不受牵连；
  CV4 零残留：本轮探针信与探针夹一条不剩。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_cv_pm_folders.py
写库面：只造 root 的自建文件夹与两封带本轮标记的探针信（sender 取一个
在册非 root 用户，直插不触发 60s 防刷），收尾按 id 删净并回读残留。
"""
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOT = login()
TAG = str(int(time.time()) % 100000)
MARK = "cv%s" % TAG
BOX_A = "%sA" % MARK
BOX_B = "%sB" % MARK
CREATED_MSG = []
CREATED_BOX = []


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
         "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    # psql -tAc 跑 INSERT ... RETURNING 时，stdout 是「返回值行 + INSERT 0 1」
    # 两行；只取第一行，否则 int() 会把命令行标签一起解析而直接炸。
    out = [ln for ln in (r.stdout or "").splitlines() if ln.strip()]
    return out[0].strip() if out else ""


def mkbox(name):
    bid = psql("INSERT INTO pmboxes (user_id,name) VALUES (%s,'%s') "
               "RETURNING id" % (ROOTID, name))
    if bid:
        CREATED_BOX.append(int(bid))
    return int(bid) if bid else 0


def mkmsg(box, subject, saved):
    mid = psql("INSERT INTO messages (sender_id,receiver_id,subject,body,"
               "location,saved,unread,folder) VALUES (%s,%s,'%s','probe',"
               "1,%d,false,%d) RETURNING id"
               % (SID, ROOTID, subject, saved, box))
    if mid:
        CREATED_MSG.append(int(mid))
    return int(mid) if mid else 0


def row(mid, cols="location,saved,folder"):
    return psql("SELECT %s FROM messages WHERE id=%d" % (cols, mid))


ROOTID = int(psql("SELECT id FROM users WHERE username='root'") or 0)
SID = int(psql("SELECT id FROM users WHERE id <> %d AND status < 2 "
               "ORDER BY id LIMIT 1" % ROOTID) or 0)
print("root=%s sender=%s marker=%s" % (ROOTID, SID, MARK))
if not (ROOTID and SID):
    sys.exit("前置失败：拿不到 root/sender id")

try:
    n0 = int(psql("SELECT count(*) FROM pmboxes WHERE user_id=%s" % ROOTID))
    ok("CV0 前置：root 名下文件夹数=%d（新建撞名用例需要 <3）" % n0, n0 < 3)

    # ===== CV1 新建撞名 =====
    s, r = call("POST", "/messages/boxes", {"name": BOX_A}, ROOT)
    id_a = ((r or {}).get("data") or {}).get("id")
    if id_a:
        CREATED_BOX.append(int(id_a))
    s2, r2 = call("POST", "/messages/boxes", {"name": BOX_A}, ROOT)
    code2 = (r2 or {}).get("code")
    msg2 = (r2 or {}).get("message") or ""
    same = psql("SELECT count(*) FROM pmboxes WHERE user_id=%s AND name='%s'"
                % (ROOTID, BOX_A))
    ok("CV1 新建撞名：必须 422/1002「已有同名文件夹」（实测 http=%s "
       "code=%s msg=%s；修前 200 且回旧 id），同名单夹必须仍只有 %s 行"
       % (s2, code2, msg2[:24], same),
       s2 == 422 and code2 == 1002 and "同名" in msg2 and same == "1",
       "http=%s code=%s same=%s" % (s2, code2, same))

    # ===== CV2 改名撞名 =====
    id_b = mkbox(BOX_B)
    s3, r3 = call("POST", "/messages/boxes", {"id": id_b, "name": BOX_A}, ROOT)
    code3 = (r3 or {}).get("code")
    msg3 = (r3 or {}).get("message") or ""
    name_b = psql("SELECT name FROM pmboxes WHERE id=%d" % id_b)
    name_a = psql("SELECT name FROM pmboxes WHERE id=%s" % id_a)
    ok("CV2 改名撞名：必须 422/1002「已有同名文件夹」且两个夹子名字都不动"
       "（实测 http=%s code=%s msg=%s；修前 404 资源不存在）；"
       "B=%s A=%s" % (s3, code3, msg3[:24], name_b, name_a),
       s3 == 422 and code3 == 1002 and "同名" in msg3
       and name_b == BOX_B and name_a == BOX_A,
       "http=%s code=%s B=%s A=%s" % (s3, code3, name_b, name_a))

    # ===== CV3 删夹：双删语义 =====
    m1 = mkmsg(id_a, "%s-keep" % MARK, 1)
    m2 = mkmsg(id_a, "%s-gone" % MARK, 0)
    s4, r4 = call("POST", "/messages/boxes", {"id": id_a, "name": ""}, ROOT)
    d4 = (r4 or {}).get("data") or {}
    ok("CV3a 删夹请求成功（http=%s deleted=%s）" % (s4, d4.get("deleted")),
       s4 == 200 and d4.get("deleted") is True)
    st1 = row(m1)
    outbox = psql("SELECT count(*) FROM messages WHERE id=%d AND sender_id=%s "
                  "AND saved=1" % (m1, SID))
    ok("CV3b 发件方仍存副本的信必须留行且只摘收件方一侧："
       "location/saved/folder=%s（期望 0/1/空），sender 发件箱=%s 行"
       "（修前整行被物理删除）" % (st1 or "<行已消失>", outbox),
       st1 == "0|1|" and outbox == "1",
       "state=%s outbox=%s" % (st1, outbox))
    st2 = row(m2, "count(*)")
    ok("CV3c 发件方已删（saved=0）的信才物理清掉：库里=%s 行（期望 0）"
       % st2, st2 == "0", "rows=%s" % st2)
    gone_a = psql("SELECT count(*) FROM pmboxes WHERE id=%s" % id_a)
    alive_b = psql("SELECT count(*) FROM pmboxes WHERE id=%d" % id_b)
    ok("CV3d 夹子行消失=%s、另一个夹子照在=%s" % (gone_a, alive_b),
       gone_a == "0" and alive_b == "1")
finally:
    for mid in CREATED_MSG:
        psql("DELETE FROM messages WHERE id=%d" % mid)
    for bid in CREATED_BOX:
        psql("DELETE FROM pmboxes WHERE id=%d" % bid)
    # 兜底：按本轮专属 marker 清（中途异常时上面两份清单可能漏记）
    psql("DELETE FROM messages WHERE subject LIKE '%s%%'" % MARK)
    psql("DELETE FROM pmboxes WHERE user_id=%s AND name LIKE '%s%%'"
         % (ROOTID, MARK))
    lmsg = psql("SELECT count(*) FROM messages WHERE subject LIKE '%s%%'"
                % MARK)
    lbox = psql("SELECT count(*) FROM pmboxes WHERE name LIKE '%s%%'" % MARK)
    ok("CV4 零残留：探针信=%s 行、探针夹=%s 行" % (lmsg, lbox),
       lmsg == "0" and lbox == "0", "msg=%s box=%s" % (lmsg, lbox))

summary()
