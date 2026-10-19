# -*- coding: utf-8 -*-
"""闸门 DM：花掉资产的写入必须核对结果行数（补签白耗 / 勋章白买这一族）。

批一百六十五 P-349/P-350。会员视角两条：
  ① 用补签卡补漏签的那天。并发护栏做得很足（P-93 唯一约束 (user_id,
     target_date)、P-94 先判已签再判持有、P-95 `FOR UPDATE OF o` 只锁订单、
     娱乐屋双源扣减先锁发放行），**唯独最后那条出勤写入**是
     `INSERT INTO attendance … ON CONFLICT (user_id,date) DO NOTHING` 且不看
     行数，而「该日是否已签」判在事务外：用户在检查与本事务之间正常签了当天
     （双标签页/并发）时，resub_uses 插得进、attendance 写 **0 行**、卡照样扣、
     接口回 200 ⇒ 一张补签卡白耗。
  ② 自购限时勋章（/medals/buy）。它与赠送口（/medals/gift，P-345）同形：
     「已拥有」判据只看未过期的行，而 user_medals 主键是 (user_id, medal_id)，
     过期件重买时 `ON CONFLICT DO NOTHING` 写 **0 行** ⇒ 扣全款、回 200、
     勋章没复活。两只口现在都：过期件 DO UPDATE 复活重算有效期；仍 0 行则
     退款并报中文错误。

判据六层：
  DM1 两处修的形状都在（attendance / medal_buy 都有 rows_affected 闸门）；
  DM2 真库 A/B：同一套语句在「当天已正常签到」下 attendance 必写 0 行
      （证明闸门必要）、在未签的那天必写 1 行；读完 ROLLBACK；
  DM3 资产授予表的 DO NOTHING 必须核对行数（幂等初始化表进豁免清单并写明理由）；
  DM4 三处「补签卡持有数」双源口径同形；
  DM5 P-94 顺序钉：先判该日已签再判持有数；
  DM6 两只勋章口同形：buy 与 gift 的发放语句都要有复活分支 + 退款键，
      防「只修一半」。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dm_asset_debit.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOTDIR, "apps", "api", "src")
MISC = os.path.join(SRC, "gaps_http", "misc.rs")
BUY = os.path.join(SRC, "community_http", "medal_buy.rs")
GIFT = os.path.join(SRC, "community_http", "medal_gift.rs")
HOLDERS = [os.path.join(SRC, "economy_http", "checkin.rs"), MISC,
           os.path.join(SRC, "http", "home_attendance.rs")]
# 真正的「资产授予表」：写一行＝用户多了一件东西，写 0 行＝没发到
GRANT_TABLES = ("user_medals", "user_dressups", "user_vouchers",
                "arcade_item_grants", "torrent_purchases", "user_achievements")
# 幂等初始化/去重语义的表：DO NOTHING 是对的，但必须写明理由（豁免清单）
EXEMPT = {
    "topic_favorites": "收藏去重，非资产",
    "gacha_ticket_balance": "余额行初始化",
    "arcade_pets": "宠物行初始化",
    "arcade_fishing_rods": "钓竿行初始化",
    "resub_uses": "唯一约束即业务口径（一人一天一次）",
    "user_donation_tiers": "档位行 CAS，0 行走 info 分支",
    "magic_pool": "按月 UPSERT 累加",
    "attendance": "有 rows_affected 闸门（P-349）",
}


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def rs_files():
    out = []
    for root, _d, names in os.walk(SRC):
        if "target/" in root.replace("\\", "/"):
            continue
        out += [os.path.join(root, n) for n in names if n.endswith(".rs")]
    return out


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:220])
    return (r.stdout or "").strip()


def revive_and_check(text):
    return bool(re.search(
        r"DO UPDATE SET[\s\S]{0,220}expires_at <= now\(\)", text)) \
        and "rows_affected()" in text


m_body, b_body, g_body = cut(MISC), cut(BUY), cut(GIFT)

# ---------- DM1 ----------
ok("DM1 补签的出勤写入带行数闸门、0 行时说明卡未被消耗",
   bool(re.search(r"INSERT INTO attendance[\s\S]{0,420}?DO NOTHING"
                  r"[\s\S]{0,420}?rows_affected\(\)", m_body))
   and "本次未消耗补签卡" in m_body,
   "回到「DO NOTHING 不看行数」＝补签卡可以被白耗（P-349 复发）")
ok("DM1b 自购勋章带复活分支 + 行数闸门 + 退款键",
   revive_and_check(b_body) and "medal-buy-refund:" in b_body,
   "扣全款却写 0 行＝勋章没复活（P-350 复发）")

# ---------- DM2 真库 A/B ----------
CEN = ("SELECT 'att=' || count(*) || '|resub=' || (SELECT count(*) "
       "FROM resub_uses) FROM attendance")
snap_before = psql(CEN)
AB = r"""
SET lock_timeout = '5s';
BEGIN;
CREATE TEMP TABLE ab AS
  SELECT (SELECT min(id) FROM users WHERE status < 2) AS uid,
         ((CURRENT_DATE - interval '2 days')::date) AS d;
INSERT INTO attendance (user_id, date, streak, reward, makeup)
SELECT uid, d, 3, 100, FALSE FROM ab;
WITH i AS (INSERT INTO resub_uses (user_id, target_date, idempotency_key)
  SELECT uid, d, 'dm-ab:' || uid FROM ab
  ON CONFLICT (user_id, target_date) DO NOTHING RETURNING 1)
SELECT 'resub_rows=' || count(*) FROM i;
WITH i AS (INSERT INTO attendance (user_id, date, streak, reward, makeup)
  SELECT uid, d, 0, 0, TRUE FROM ab
  ON CONFLICT (user_id, date) DO NOTHING RETURNING 1)
SELECT 'att_rows=' || count(*) FROM i;
WITH i AS (INSERT INTO resub_uses (user_id, target_date, idempotency_key)
  SELECT uid, d + 1, 'dm-ab2:' || uid FROM ab
  ON CONFLICT (user_id, target_date) DO NOTHING RETURNING 1)
SELECT 'resub_rows2=' || count(*) FROM i;
WITH i AS (INSERT INTO attendance (user_id, date, streak, reward, makeup)
  SELECT uid, d + 1, 0, 0, TRUE FROM ab
  ON CONFLICT (user_id, date) DO NOTHING RETURNING 1)
SELECT 'att_rows2=' || count(*) FROM i;
ROLLBACK;
"""
rd2 = {}
for ln in psql(AB).split("\n"):
    ln = ln.strip()
    if "=" in ln:
        k, v = ln.split("=", 1)
        rd2[k.strip()] = v.strip()
ok("DM2 当天已正常签到时：resub 写 %s 行、attendance 写 %s 行（后者须 0 ⇒ "
   "闸门必要）" % (rd2.get("resub_rows"), rd2.get("att_rows")),
   rd2.get("resub_rows") == "1" and rd2.get("att_rows") == "0",
   "读数不齐＝取样失败/唯一键不在 (user_id,date)/psql 报错")
ok("DM2b 未签的那天两步各写 1 行（resub=%s att=%s）"
   % (rd2.get("resub_rows2"), rd2.get("att_rows2")),
   rd2.get("resub_rows2") == "1" and rd2.get("att_rows2") == "1", "-")
snap_after = psql(CEN)
ok("DM2c A/B 残料：回滚后账面与事前逐字一致（前 %s ／ 后 %s）"
   % (snap_before, snap_after),
   snap_before == snap_after and snap_before.startswith("att="),
   "回滚没吃干净＝闸门在往现网写数据；空读数＝psql 报错")

# ---------- DM3 资产授予表普查（逐处定性，不放宽判据） ----------
# 已核过的豁免：写 0 行不构成「收了钱/耗了卡却没发货」，理由必须写在这里
GRANT_EXEMPT = [
    ("arcade_stubs.rs", "user_achievements",
     "成就解锁记录，重复解锁没有损失，也不收钱"),
    ("tiers.rs", "user_medals",
     "档位勋章已有则不重发；同档重复发放由 user_donation_tiers 的 CAS 拦"),
    ("charge.rs", "torrent_purchases",
     "主键 (user_id,torrent_id) 冲突＝本就拥有；同事务的 spark 幂等触发器"
     "对并发第二发直接 RAISE（DH3 已钉），不存在「扣了钱没写购买行」"),
]
bad, seen, used = [], 0, set()
for f in rs_files():
    s2 = cut(f)
    base = os.path.relpath(f, ROOTDIR).replace("\\", "/").split("/")[-1]
    for m in re.finditer(r'"INSERT INTO (\w+)[\s\S]{0,320}?DO NOTHING"', s2):
        tbl = m.group(1)
        if tbl not in GRANT_TABLES:
            continue
        seen += 1
        if "rows_affected()" in s2[m.end():m.end() + 700]:
            continue
        hit = [(fl, t, why) for fl, t, why in GRANT_EXEMPT
               if fl == base and t == tbl]
        if hit:
            used.add((base, tbl))
        else:
            bad.append("%s:%s" % (base, tbl))
stale = [(fl, t) for fl, t, _w in GRANT_EXEMPT if (fl, t) not in used]
ok("DM3 资产授予表的 DO NOTHING %d 处：核对行数或已逐处定性（违例 %d、"
   "过期豁免 %d）" % (seen, len(bad), len(stale)),
   seen >= 3 and not bad and not stale,
   "违例：%s ／ 过期豁免（该处已不再这么写了，理由要重写）：%s"
   % ("、".join(bad) or "-", "、".join("%s:%s" % x for x in stale) or "-"))

# ---------- DM4 双源同形 ----------
seg = []
for f in HOLDERS:
    s = rd(f).replace("\r\n", " ")
    if not ("NOT EXISTS (SELECT 1 FROM resub_uses" in s
            and "arcade_item_grants" in s and "arcade_item_uses" in s):
        seg.append(os.path.basename(f))
ok("DM4 三处补签卡持有数都是双源口径（走偏 %d）" % len(seg), not seg,
   "、".join(seg) + " 少一段＝看得见却用不了（或反之）")

# ---------- DM5 P-94 顺序 ----------
ok("DM5 先判该日已签、再判持有数（P-94 顺序）",
   0 <= m_body.find("该日已有签到记录") < m_body.find("owned"),
   "反了会把「日期已占」误报成「没有卡」")

# ---------- DM6 两只勋章口同形 ----------
ok("DM6 自购与赠送两只口的发放同形（防只修一半）",
   revive_and_check(b_body) and revive_and_check(g_body)
   and "medal-gift-refund:" in g_body and "medal-buy-refund:" in b_body,
   "buy=%s gift=%s" % (revive_and_check(b_body), revive_and_check(g_body)))

summary()
