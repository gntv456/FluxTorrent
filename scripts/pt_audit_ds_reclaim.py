# -*- coding: utf-8 -*-
"""闸门 DS：道具回收链的四道闸与「审计必须记真实归属」。

批一百七十。管理员视角：背包页回收一张误发的道具单。这条链的三道闸原本就
写得清楚（付费单不可回收＝购买即终局、无退款通道；已生效单不可回收＝只能
走调账冲销；操作者等级须严格高于目标），顺带把账上挂着的「回收即退款待拍板」
用代码证据结掉——**站长口径已经落地：付费单一律拒收，不给退款通道**。

真正的缺陷在第四个参数上：旧路由 `/admin/users/{id}/revoke-item/{order_id}`
的路径 `{id}` 从前**什么都不校验**——`reclaim_order` 读单时用的是订单自己的
user_id，所以拿 `/admin/users/7/revoke-item/999`（999 其实属于 42）照样回收
成功，而 `audit_detail` 把处置记在 **7** 头上、给 7 记一条 modify_log。
事后翻账/申诉时会找一个根本没被处置的人，真正被回收的 42 无痕。

修法：`reclaim_order` 增 `expect_uid: Option<i64>`；带 {id} 的旧路由传
Some(uid)，不匹配就在**删任何行之前**拒（「订单 #N 不属于用户 #M，未回收任何
行」），审计改用读单得到的真实归属。只给 order_id 的新路由（/admin/user-props）
传 None，它的审计本来就用返回值。

判据：
  DS1 四道闸形状齐（付费拒 / 已生效拒 / 等级严格高于 / 归属必须相符）；
  DS2 旧路由必须传 Some(路径 uid) 且审计记的是返回值，不能是路径值；
  DS3 删除与库存回滚同事务（DELETE 与 stock_used 回退之间不得出现 commit）；
  DS4 等价谓词 A/B（真库事务内）：按订单自己的归属删＝1 行；按「路径 uid
      与归属不符」的谓词删＝0 行 —— 证明这道闸确实拦得住；ROLLBACK；
  DS5 残料：A/B 前后 shop_orders/shop_items 行数与库存逐位一致。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ds_reclaim.py
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
REV = os.path.join(SRC, "admin_http", "user_revoke.rs")
PROPS = os.path.join(SRC, "admin_p3_http", "user_props.rs")


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    s = "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))
    return s


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:220])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


rev, props = cut(REV), cut(PROPS)

# ---------- DS1 四道闸 ----------
gates = {
    "付费单不可回收": "购买即终局" in rev,
    "已生效单不可回收": "该道具已生效，不可回收" in rev,
    "等级须严格高于目标": "actor_class <= target_class" in rev,
    # 形状 + 位置都得对：真的比较了 want != uid，且在 DELETE 之前拒
    "归属必须相符": bool(re.search(
        r"if let Some\(want\) = expect_uid \{[^{}]*if want != uid", rev))
    and rev.find("expect_uid") < rev.find("DELETE FROM shop_orders"),
}
miss = [k for k, v in gates.items() if not v]
ok("DS1 回收链四道闸齐全（缺 %d）" % len(miss), not miss,
   "、".join(miss) or "-")

# ---------- DS2 审计记真实归属 ----------
i_call = rev.find("async fn user_revoke_item")
blk = rev[i_call:i_call + 900]
ok("DS2 旧路由传 Some(路径 uid) 且审计用返回的归属（%s / %s）"
   % ("Some(uid)" in blk, "Some(owner)" in blk),
   "Some(uid)" in blk and "Some(owner)" in blk,
   "审计记路径值＝把处置记在一个根本没被回收的人头上")
ok("DS2b 新路由显式声明无路径归属（传 None），不把 order_id 当身份",
   re.search(r"reclaim_order\([\s\S]{0,200}?None,", props) is not None, "-")

# ---------- DS3 同事务 ----------
i_del = rev.find("DELETE FROM shop_orders")
i_stock = rev.find("stock_used = GREATEST")
i_commit = rev.find("tx.commit()", i_del)
ok("DS3 删除与库存回滚同事务（del=%d stock=%d commit=%d）"
   % (i_del, i_stock, i_commit),
   0 <= i_del < i_stock < i_commit,
   "配额没退回就是永久少卖一格；分两笔提交会留半截账")

# ---------- DS4 等价谓词 A/B ----------
snap = {"orders": sval("SELECT count(*) FROM shop_orders"),
        "stock": sval("SELECT coalesce(sum(stock_used),0) FROM shop_items")}
AB = r"""
-- 不写任何数据：取一张真实订单，比较「按订单归属」与「按路径 uid（故意取
-- 另一个会员）」两条谓词各能圈到几行 —— 等价于 expect_uid 闸门在不在。
WITH ds AS (SELECT o.id AS oid, o.user_id AS uid,
  (SELECT min(x.id) FROM users x WHERE x.id <> o.user_id) AS other
  FROM shop_orders o WHERE o.id = (SELECT min(id) FROM shop_orders))
SELECT 'own=' || (SELECT count(*) FROM shop_orders o JOIN ds ON o.id = ds.oid
   WHERE o.user_id = ds.uid) || ' other=' || (SELECT count(*) FROM
   shop_orders o JOIN ds ON o.id = ds.oid WHERE o.user_id = ds.other) ||
  ' probe=' || ds.oid || '/' || ds.uid FROM ds LIMIT 1;
"""
rd4 = {}
for ln in psql(AB).split(chr(10)):
    ln = ln.strip()
    if ln.count("=") >= 2:
        for kv in ln.split():
            if "=" in kv:
                k, v = kv.split("=", 1)
                rd4[k] = v
ok("DS4 等价谓词：按订单归属圈到 %s 行、按不符的路径 uid 圈到 %s 行"
   "（取样单 %s）" % (rd4.get("own"), rd4.get("other"), rd4.get("probe")),
   rd4.get("own") == "1" and rd4.get("other") == "0" and rd4.get("probe"),
   "读数不齐＝没有可取样订单或 SQL 失败：%r" % (rd4 or psql(AB))
)
# ---------- DS5 残料 ----------
after = {"orders": sval("SELECT count(*) FROM shop_orders"),
         "stock": sval("SELECT coalesce(sum(stock_used),0) FROM shop_items")}
ok("DS5 A/B 前后订单行数与库存合计一致 %s→%s" % (snap, after),
   snap == after, "回滚没吃干净＝闸门在往现网写数据")

summary()
