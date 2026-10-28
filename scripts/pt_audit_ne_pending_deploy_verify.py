#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""NE：已修待部署的「部署后复验单」——一跑同时验掉本批六处修复的活体面。

为什么要有它：10-28 这一批（P-465/468/469/470/471/472）都只做到「代码与库侧
验证」，镜像由站长重建，改完没人能在推送前看到用户面。于是把「部署后才该转绿」
的那几条写成一张对账单：**实测红集合必须等于 PENDING 台账**——多一条=新坏了
别的东西，少一条=那条已经部署生效、必须把台账删号（棘轮，不许悄悄变绿）。

当前待部署的四条（10-28 14:17Z 实测）：
  · /home 500（P-469 TABLESAMPLE 位置）；
  · /stats 的 class_dist 空（P-468 JOIN 了不存在的 classes）；
  · 热搜计数不涨（P-465 upsert 自引用列歧义）；
  · （合同已绿）→ 不在台账里，若哪天又红会被 NE3 抓成「多一条」。

副作用：热搜那一条会给一个真实词条 +2 hits——语义上就是「搜了两次」，不是造数。
契约两口、/stats 主体、/home 的鉴权面都已绿，作为常数项一起验，防回归。
"""
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (call, jwt_token, ok, psql_stdin,  # noqa
                          summary)

# 已修待部署：检查名 -> 修的是哪一处（空 = 全部已在现网验过）
# 2026-10-28 12:07Z 站长重建 api 后实测三条全绿：/home 200、class_dist 非空、
# 热搜 hits 增长 ⇒ 按棘轮整段删号。再出现红就是新坏的东西。
PENDING = {}


def q(sql):
    return psql_stdin(sql).strip()


def payload(path, token):
    st, body = call("GET", path, token=token)
    return st, (body if isinstance(body, dict) else {})


tok = jwt_token("root")
red = {}
checked = []


def mark(name, good, detail):
    checked.append(name)
    if not good:
        red[name] = detail
    return good


# ---- /home：200 + random_picks 字段在（该查询失败会让整个接口 500）----
st, body = payload("/home", tok)
mark("home", st == 200 and isinstance(body.get("data"), dict)
     and "random_picks" in (body.get("data") or {}),
     "status=%s code=%s" % (st, body.get("code")))

# ---- 契约两口（已绿，常数项防回归）----
for p in ("/contracts/tiers", "/contracts/mine"):
    st, body = payload(p, tok)
    ok("NE1 常数项 %s 必须 200/code=0" % p,
       st == 200 and body.get("code") == 0,
       "status=%s code=%s" % (st, body.get("code")))

# ---- /stats：class_dist 非空（有 60s Redis 缓存，最多等一轮）----
st, body = payload("/stats", tok)
cd = (body.get("data") or {}).get("class_dist")
mark("class_dist", st == 200 and isinstance(cd, list) and len(cd) > 0,
     "status=%s class_dist=%s" % (st, "空" if not cd else len(cd)))

# ---- 热搜计数：真实词条两次联想后 hits 必须涨 ----
term = "test"
for name in q("SELECT name FROM torrents WHERE approval_status = 1 "
              "ORDER BY id DESC LIMIT 40").splitlines():
    m = re.search(r"[A-Za-z0-9][A-Za-z0-9.]{4,}", name or "")
    if m:
        term = m.group(0)
        break
esc = term.replace("'", "''")


def hits():
    return int(q("SELECT COALESCE((SELECT hits FROM search_term_stats "
                 "WHERE term = '%s'), 0)" % esc) or 0)


before = hits()
call("GET", "/torrents/suggest?q=%s" % term, token=tok)
time.sleep(0.2)
call("GET", "/torrents/suggest?q=%s" % term, token=tok)
after = hits()
mark("hot_search", after > before, "term=%s hits %s→%s" % (term, before, after))

# ---- 正控：harness 分得清 404/401/200（不然上面的红绿都不作数）----
st404, _ = call("GET", "/definitely-not-a-route-ne")
st401, _ = call("GET", "/home")
ok("NE2 正控：不存在的路由 404、无 token 的 /home 401（探针不瞎）",
   st404 == 404 and st401 == 401, "404=%s 401=%s" % (st404, st401))

# ---- 主判据：实测红集合 == 待部署台账 ----
ok("NE3 「部署后复验」实测红集合 = PENDING 台账（少一条=已生效须删号）",
   set(red) == set(PENDING),
   "实测红=%s 台账=%s" % (sorted(red), sorted(PENDING)))
for k in sorted(red):
    print("   [待部署] %-11s %s" % (k, PENDING.get(k, "未登记")))
for k in sorted(set(PENDING) - set(red)):
    print("   [已生效] %-11s 请把台账这一条删掉：%s" % (k, PENDING[k]))

ok("NE4 反空转：三项探针必须真的都跑过（台账已清空时也不能靠空集假绿）",
   len(checked) >= 3, "执行=%d 项：%s" % (len(checked), checked))
summary()
