# -*- coding: utf-8 -*-
"""闸门 CT：站内信写口的归属判定（P-340）——删信口能把别人的信物理删掉。

实测红线（批一百五十四）：造一封 receiver=6311、已处于「回收站态」
（location=0, saved=0）的信，root（id=1）调 `POST /messages/delete`：

    HTTP 200  {"deleted": 0}      ← 声称一封没删
    该行已被物理 DELETE           ← 事实相反

根因在 `message_box.rs`：同一个事务里三条写语句，前两条都带归属条件
（`receiver_id = $1` / `sender_id = $1`），最后那条物理删除只按
`id = ANY($1) AND location = 0 AND saved = 0`——**没有归属**。它本意是
「把本人删完剩下的回收站行一并清掉」，但只要存在「别人的、已被删过一次、
saved=0」的信行，任何人对那个 id 调本口都能把它抹掉。

同一文件的 `markread` / `move` 都带归属条件（已核对），只有这条漏了。

本闸钉六件事：
  S1 别人的回收站态信：deleted 必须是 0，**且行必须还在**（红线：行消失）。
  S2 别人的正常态信：同样必须还在（顺带钉前两条 UPDATE 的归属）。
  S3 本人正常态信：deleted=1（正控，防「修成谁都不能删」）。
  S4 本人回收站态信：也能删净（正控，证明归属条件写对了）。
  S5 markread 对别人的信无效（同一族的另两条语句）。
  S6 零残留：本闸造的信一条不剩。

收尾只删带本闸 subject 标记的行。
"""
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import login, ok, summary  # noqa: E402

BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8100/api/v1")
ROOT = login()
MARK = "ct340%d" % (int(time.time()) % 100000)
OTHER = 6311          # 一个在册的非 root 用户（本闸不登录他，只造他的信行）
CREATED = []


def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        r = urllib.request.urlopen(req, data, timeout=25)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:  # noqa: BLE001
            return e.code, {}


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
         "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (r.stdout or "").strip()


def mkmsg(receiver, location):
    """造一封带本闸标记的信。location=1 正常态，0 = 回收站态。"""
    mid = psql("INSERT INTO messages (receiver_id, sender_id, location, folder, "
               "subject, body, unread, saved) VALUES (%d, NULL, %d, NULL, "
               "'%s', 'probe', true, 0) RETURNING id"
               % (receiver, location, MARK)).split("\n")[0].strip()
    if mid:
        CREATED.append(mid)
    return int(mid) if mid else 0


def alive(mid):
    return psql("SELECT count(*) FROM messages WHERE id = %d" % mid) == "1"


print("== S1 别人的回收站态信：物理删除必须不生效（P-340 红线）==")
a = mkmsg(OTHER, 0)
s, r = call("POST", "/messages/delete", {"ids": [a]}, ROOT)
ok("S1 别人的回收站态信：deleted 必须为 0",
   (r.get("data") or {}).get("deleted") == 0,
   "deleted=%s" % (r.get("data") or {}).get("deleted"))
ok("S1 别人的回收站态信：行必须还在（红线：接口说没删，行却没了）",
   alive(a), "alive=%s" % alive(a))

print("== S2 别人的正常态信：归属同样成立 ==")
b = mkmsg(OTHER, 1)
s, r = call("POST", "/messages/delete", {"ids": [b]}, ROOT)
ok("S2 别人的正常态信：deleted=0 且行还在",
   (r.get("data") or {}).get("deleted") == 0 and alive(b),
   "deleted=%s alive=%s" % ((r.get("data") or {}).get("deleted"), alive(b)))

print("== S3/S4 正控：本人两种状态都要能删净（防「修成谁都不能删」）==")
c = mkmsg(1, 1)
s, r = call("POST", "/messages/delete", {"ids": [c]}, ROOT)
ok("S3 本人正常态信：deleted=1",
   (r.get("data") or {}).get("deleted") == 1,
   "deleted=%s" % (r.get("data") or {}).get("deleted"))
d = mkmsg(1, 0)
s, r = call("POST", "/messages/delete", {"ids": [d]}, ROOT)
ok("S4 本人回收站态信：也能删净（行消失即通）",
   (r.get("data") or {}).get("deleted") == 1 and not alive(d),
   "deleted=%s alive=%s" % ((r.get("data") or {}).get("deleted"), alive(d)))

print("== S5 同族另两条：markread 对别人的信无效 ==")
e = mkmsg(OTHER, 1)
psql("UPDATE messages SET unread = true WHERE id = %d" % e)
call("POST", "/messages/markread", {"ids": [e]}, ROOT)
still = psql("SELECT unread FROM messages WHERE id = %d" % e)
ok("S5 markread 不得改别人的信", still == "t", "unread=%s" % still)


def cleanup():
    ids = ",".join(str(x) for x in CREATED) or "-1"
    n = psql("DELETE FROM messages WHERE id IN (%s)" % ids)
    left = psql("SELECT count(*) FROM messages WHERE subject LIKE '%s%%'" % MARK)
    ok("S6 零残留：本闸造的信一条不剩", left == "0",
       "删了 %s 条，残留 %s" % (n, left))


try:
    summary()
finally:
    cleanup()
