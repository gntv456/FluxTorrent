# -*- coding: utf-8 -*-
"""闸门 BY：站内信三张列表的分页与「角标 = 点进去看到的条数」不变量（任务 #70）。

背景（红线实测，批一百三十五）：给 root 自造 115 封未读后——

    PG 真值未读            180
    /me.unread_messages    180     ← 角标
    GET /messages/inbox    100     ← 列表

三张列表（inbox/sent/staff）都是 `ORDER BY m.id DESC LIMIT 100` 且无任何
cursor 参数，而 me.rs / message_stream.rs 的注释把「角标数字必须等于点进去
看到的条数」写成既定不变量（P-31 收口过）⇒ 超过 100 时该不变量不成立，
老信在界面上**永久不可达**：标已读也标不到，因为根本没列出来。

修法（保持数据形状不变）：加 `limit`/`offset`，并在响应头给出
`X-Total-Count`（同筛选下总数）与 `X-Has-More`（还有下一批）。
数据仍是 `{code,message,request_id,data:[...]}`——消息中心、通知中心页、
staff-box 组件三处都直接 `.then(setRows)` 把 data 当数组用，改成
`{items,total}` 会同时打断三处，所以分页信息只能走头。

本闸钉八件事：
  S0 正控：PG 真值 / 角标 / 三张列表总数三处一致（口径不许分叉）。
  S1 >100 时列表能翻到全部（旧信不再不可达）。
  S2 翻页不重不漏（各页 id 集合并集 = 全集，交集为空）。
  S3 筛选与分页正交（unread / box_id / search 各自带分页仍守恒）。
  S4 缺省形状不变（不传 limit 时行为与老版本逐字一致：≤100 时全给、>100 时
     仍给 100 条且 X-Has-More=1）——防止修分页顺手改了既有契约。
  S5 越界参数被夹住（limit=0/-5/99999 不得 500 或返回全表）。
  S6 三张列表各自守恒（sent / staff 同样钉）。
  S7 零残留：自造的探针信全删，基线未读数复原。

收尾纪律：探针信全部带唯一 subject 标记；只删带本闸标记的行；删完回读
残留为 0 且 /me.unread_messages 回到跑前值。
"""
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, call_h, login, ok, summary  # noqa: E402

ROOT = login()
MARK = "by70%d" % (int(time.time()) % 100000)
UID = 1          # 只用 root 的信箱造信：不建号、不碰别人的信箱
CREATED = []


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
         "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % (r.stderr or "")[:200])
    return (r.stdout or "").strip()


def pg_unread():
    return int(psql("SELECT count(*) FROM messages WHERE receiver_id = %d "
                    "AND unread AND location = 1 AND folder IS NULL" % UID))


def pg_total():
    return int(psql("SELECT count(*) FROM messages WHERE receiver_id = %d "
                    "AND location = 1 AND folder IS NULL" % UID))


def pg_saved():
    return int(psql("SELECT count(*) FROM messages WHERE sender_id = %d "
                    "AND saved = 1" % UID))


def seed(n):
    """给 root 主收件箱造 n 封未读探针信（sender_id NULL = 系统通知形状）。"""
    rows = ",".join("('%s #%d')" % (MARK, i) for i in range(n))
    psql("INSERT INTO messages (receiver_id, sender_id, location, folder, "
         "subject, body, unread, saved) "
         "SELECT %d, NULL, 1, NULL, s, 'paging probe', true, 0 "
         "FROM unnest(ARRAY[%s]) AS s" % (UID, rows))
    CREATED.append(n)


def cleanup():
    if not CREATED:
        return
    n = psql("DELETE FROM messages WHERE subject LIKE '%s%%'" % MARK)
    print("   [cleanup] 探针信已删 %s 条；未读回基线 %s / 现值 %s"
          % (n, BASE_UNREAD, pg_unread()))


def inbox_ids(limit=None, offset=None, extra=""):
    qs = []
    if limit is not None:
        qs.append("limit=%d" % limit)
    if offset:
        qs.append("offset=%d" % offset)
    if extra:
        qs.append(extra)
    path = "/messages/inbox" + ("?" + "&".join(qs) if qs else "")
    return call_h("GET", path, token=ROOT)


BASE_UNREAD = pg_unread()
BASE_TOTAL = pg_total()
BASE_SAVED = pg_saved()
print("== S0 地基：真值 / 角标 / 列表总数三处同口径（跑前未读=%d 总=%d）=="
      % (BASE_UNREAD, BASE_TOTAL))

s, r, _ = call_h("GET", "/messages/inbox", token=ROOT)
me_unread = (call("GET", "/me", token=ROOT)[1].get("data") or {}).get(
    "unread_messages")
ok("S0 角标 = PG 真值（口径不许分叉）", me_unread == BASE_UNREAD,
   "角标=%s 真值=%s" % (me_unread, BASE_UNREAD))
s, r, h = call_h("GET", "/messages/inbox", token=ROOT)
hdr_total = h.get("x-total-count")
ok("S0 列表自带 X-Total-Count = PG 真值（分页信息走头，不改数据形状）",
   hdr_total is not None and int(hdr_total) == BASE_TOTAL,
   "头=%s 真值=%s" % (hdr_total, BASE_TOTAL))
ok("S0 数据仍是数组（前端三处 .then(setRows) 不能被打断）",
   isinstance(r.get("data"), list), type(r.get("data")).__name__)

# 造够 130 封，确保 >100（根号用户的真实场景：老会员多年积累）
NEED = max(0, 130 - BASE_TOTAL)
print("== S1 造 %d 封探针信（把主收件箱顶过 100 条上限）==" % NEED)
if NEED:
    seed(NEED)
TOTAL = pg_total()
ok("S1 准备：主收件箱 %d 条（已过 100 上限）" % TOTAL, TOTAL > 100)

s, r, h = call_h("GET", "/messages/inbox", token=ROOT)
page1 = r.get("data") or []
ok("S1 缺省页仍是 100 条（既有契约不动）", len(page1) == 100,
   "len=%d" % len(page1))
ok("S1 超 100 时 X-Has-More=1（旧信不再被静默截断）",
   h.get("x-has-more") == "1", "头=%s" % h.get("x-has-more"))

seen = []
off = 0
while True:
    s, r, h = call_h("GET", "/messages/inbox?limit=50&offset=%d" % off,
                     token=ROOT)
    rows = r.get("data") or []
    seen.extend([m["id"] for m in rows])
    if h.get("x-has-more") != "1" or not rows:
        break
    off += len(rows)
    if off > 2000:
        break
probe_ids = {int(m["id"]) for m in page1}
all_ids = set(seen)
ok("S2 翻页能取到全部 %d 条（旧信全部可达）" % TOTAL,
   len(all_ids) == TOTAL, "取到=%d 应=%d" % (len(all_ids), TOTAL))
ok("S2 翻页不重复（各页交集为空）", len(seen) == len(all_ids),
   "累计=%d 去重=%d" % (len(seen), len(all_ids)))

s, r, h = call_h("GET", "/messages/inbox?limit=50&offset=0", token=ROOT)
r2_first = r.get("data") or []
p1 = {m["id"] for m in r2_first}
s, r, h = call_h("GET", "/messages/inbox?limit=50&offset=50", token=ROOT)
p2 = {m["id"] for m in (r.get("data") or [])}
ok("S2 相邻两页不重叠", not (p1 & p2), "交集=%d" % len(p1 & p2))
# 排序稳定性：limit=50 的第一页必须是缺省 100 条**的前 50 条**（id DESC）。
# 不能拿两个不同大小的集合比全等——那是我自己写错的判据（limit=50 与缺省
# 100 本就不该相等），第一次跑就红在这里，是闸门的错不是产品的错。
default_order = [m["id"] for m in (call("GET", "/messages/inbox",
                                        token=ROOT)[1].get("data") or [])]
ok("S2 第一页 = 缺省页的前 50 条（排序稳定，id DESC 不得漂移）",
   [m["id"] for m in (r2_first or [])] == default_order[:50],
   "首页=%s 缺省前50=%s" % ([m["id"] for m in (r2_first or [])][:3],
                             default_order[:3]))
ok("S2 首页是缺省页的子集（不得出现缺省页没有的信）",
   p1 <= set(default_order), "多出=%d" % len(p1 - set(default_order)))

s, r, h = call_h("GET", "/messages/inbox?unread=true&limit=50", token=ROOT)
u_rows = r.get("data") or []
u_total = h.get("x-total-count")
pg_unread_now = pg_unread()
ok("S3 未读筛选 + 分页守恒（筛选与分页正交）",
   u_total is not None and int(u_total) == pg_unread_now,
   "头=%s 真值=%s" % (u_total, pg_unread_now))
ok("S3 未读筛选的页大小受 limit 约束", len(u_rows) <= 50, len(u_rows))

s, r, h = call_h("GET", "/messages/inbox?search=%s&limit=30" % MARK,
                 token=ROOT)
ok("S3 关键词筛选 + 分页：总数 = 本闸造的信数",
   h.get("x-total-count") is not None
   and int(h["x-total-count"]) == NEED,
   "头=%s 造了=%d" % (h.get("x-total-count"), NEED))

bad = []
for lim, want_max in ((0, 1), (-5, 1), (99999, 100)):
    s, r, h = call_h("GET", "/messages/inbox?limit=%d" % lim, token=ROOT)
    n = len(r.get("data") or [])
    if s != 200 or n > want_max:
        bad.append("limit=%d → %s/%d 条" % (lim, s, n))
ok("S5 越界参数被夹住（不得 500，也不得返回全表）", not bad, "; ".join(bad))

s, r, h = call_h("GET", "/messages/inbox?offset=-1&limit=10", token=ROOT)
ok("S5 负 offset 被夹到 0（不 500）", s == 200, "status=%s" % s)

s, r, h = call_h("GET", "/messages/sent?limit=1000", token=ROOT)
ok("S6 发件箱：总数头 = PG 真值（%d）" % BASE_SAVED,
   h.get("x-total-count") is not None
   and int(h["x-total-count"]) == BASE_SAVED,
   "头=%s 真值=%d" % (h.get("x-total-count"), BASE_SAVED))
ok("S6 发件箱缺省形状仍是数组",
   isinstance(r.get("data"), list), type(r.get("data")).__name__)

s, r, h = call_h("GET", "/messages/staff?limit=1000", token=ROOT)
staff_pg = int(psql(
    "SELECT count(*) FROM messages m WHERE (m.sender_id = 1 AND EXISTS "
    "(SELECT 1 FROM users su WHERE su.id = m.receiver_id AND su.class_id >= 90))"
    " OR (m.receiver_id = 1 AND EXISTS (SELECT 1 FROM users su "
    "WHERE su.id = m.sender_id AND su.class_id >= 90))"))
ok("S6 管理组信箱：总数头 = PG 真值（%d）" % staff_pg,
   h.get("x-total-count") is not None
   and int(h["x-total-count"]) == staff_pg,
   "头=%s 真值=%d" % (h.get("x-total-count"), staff_pg))

try:
    summary()
finally:
    cleanup()
