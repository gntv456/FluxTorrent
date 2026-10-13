# -*- coding: utf-8 -*-
"""BZ 闸门（批六十）：权限矩阵的「能力真相」与代码消费点对不上。

站长的控制面就是这张矩阵：勾一个权限位 = 认为该角色获得了这项能力。
目录里 `implemented` 这一列（0217 G3 之外另一条防线）专门标「功能尚未实装」，
web 面板 `staff-tools-perm.tsx:211` 只在 implemented=false 时挂「未实装」徽章
⇒ **只要 implemented=true 而代码里没有消费点，站长勾了不生效、界面还不提示**。

0062 迁移就是为这类漂移写的（「修正 implemented 标记：upload_special / repost
尚无代码生效点（0 处 perm:: 调用）」），但它漏了 `torrent.set_price`：
`authz/perm.rs` 自己带着 `#[allow(dead_code)]` 与注释「付费种子改价端点未实装
（价格仅发布时设置），接线时删除本 allow」，而目录仍是 implemented=true。

判据纪律：
  - 消费点两种形态都要认：Rust 常量引用 `perm::NAME`，以及 worker/API 里
    SQL 内的 `user_can(…, 'key')`（`hr.exempt` 只有后者，第一版没认它、
    误报成零消费——判据要认全既有口径，不能只按一种写法扫）。
  - Z2 是**现网读口**证据（矩阵返回体），不是只看库；Z3 反向钉「已实装却
    标 false」这另一种漂移，今天应零命中，是正控。
  - 闸门是静态+读口，不写库 ⇒ 无探针行、无收尾清理。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bz_perm_truth.py
"""
import io
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, login, ok, summary)

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def rglob(paths, pattern):
    """在给定源码树里按正则捞文本（跨 api/worker 两 crate）。"""
    out = []
    for p in paths:
        full = os.path.join(REPO, p)
        for root, dirs, files in os.walk(full):
            dirs[:] = [d for d in dirs if d not in ("target", "node_modules")]
            for fn in files:
                if fn.endswith(".rs"):
                    out.append(io.open(os.path.join(root, fn),
                                       encoding="utf-8", errors="replace")
                               .read())
    return "\n".join(out)


def consumers():
    """权限键的消费点：perm::NAME 与 SQL 里的 user_can(…, 'key')。"""
    perm = io.open(os.path.join(REPO, "apps", "api", "src", "authz",
                                "perm.rs"), encoding="utf-8").read()
    name2val = dict(re.findall(
        r"pub const ([A-Z0-9_]+)\s*:\s*&str\s*=\s*\"([^\"]+)\"", perm))
    trees = rglob([os.path.join("apps", "api", "src"),
                   os.path.join("apps", "worker", "src")], None)
    used = set()
    for m in re.finditer(r"perm::([A-Z0-9_]+)", trees):
        if m.group(1) in name2val:
            used.add(name2val[m.group(1)])
    for m in re.finditer(r"user_can\([^)]*?'([a-z_.]+)'", trees):
        used.add(m.group(1))
    return used


USED = consumers()
ok("Z0 前提：消费点扫描认全两种写法（perm::X 与 SQL user_can('k')）",
   "hr.exempt" in USED and len(USED) > 20,
   "已识别消费点 %d 个；hr.exempt 命中=%s" % (len(USED),
                                             "hr.exempt" in USED))

s, b = call("GET", "/admin/permission-matrix", None, token=ROOT)
d = (b or {}).get("data") or {}
perms = d.get("permissions") or []
ok("Z1 现网读口：矩阵 200 且返回 permissions 段（界面据此渲染能力表）",
   s == 200 and len(perms) > 0, "http=%s 条数=%d" % (s, len(perms)))

lie = [p["key"] for p in perms if p.get("implemented")
       and p["key"] not in USED]
ok("Z2 P-230：矩阵里 implemented=true 却没有代码消费点的权限位"
   "（站长勾了不生效，而界面只在 implemented=false 时才挂「未实装」徽章 ⇒ "
   "这一类漂移对站长完全不可见）——命中=%s" % lie,
   not lie, "命中=%d 处 %s" % (len(lie), lie))

sp = [p for p in perms if p["key"] == "torrent.set_price"]
ok("Z2b 具体那一处：torrent.set_price 的 implemented 必须是 false"
   "（perm.rs 自陈「改价端点未实装」，0062 那批修正漏了它）：%s"
   % (sp[0] if sp else "不在清单里（模块过滤掉了？）"),
   bool(sp) and sp[0].get("implemented") is False,
   "implemented=%s" % (sp[0].get("implemented") if sp else "-"))

rev = [p["key"] for p in perms if not p.get("implemented")
       and p["key"] in USED]
ok("Z3 反向漂移（已实装却还标 false ⇒ 能力被埋）：命中=%s（正控，今天应为空）"
   % rev, not rev, "命中=%d 处" % len(rev))

# ===== Z4 取证：职员读口的闸门档位对照（本批只数不修，列待拍板） =====
coarse, named, where = 0, 0, []
for f in ("admin_http/site.rs", "staff_http/site.rs"):
    t = io.open(os.path.join(REPO, "apps", "api", "src", f),
                encoding="utf-8").read()
    c = t.count("let _auth = staff(")
    n = len(re.findall(r"crate::authz::perm::[A-Z_]*VIEW", t))
    coarse += c
    named += n
    where.append("%s：粗闸 %d / 专属 VIEW %d" % (f, c, n))
print("   取证 %s" % "；".join(where))
ok("Z4 取证不算红：/admin/cheaters（admin_http/site.rs）只有粗闸 "
   "`let _auth = staff(`，而 staff_http/site.rs 的 uploaders/"
   "notconnectable/conn-dist 各有专属 *.view 档 ⇒ 最低档职员即可读"
   "「疑似作弊名单 + 每人流量增速」与全量授权矩阵；口径差在册，"
   "修法要新增权限键（迁移+目录+面板三处同步），本批不夹带",
   coarse >= 1 and named >= 3, "粗闸=%d 专属=%d" % (coarse, named))

# ===== Z5 收尾：矩阵条目数与目录一致（确认扫描读的是同一张表） =====
r = subprocess.run(
    ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
     "fluxtorrent", "-tAc",
     "SELECT count(*) FROM permissions WHERE implemented"],
    stdout=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
db_true = (r.stdout or "").strip()
matrix_true = len([p for p in perms if p.get("implemented")])
ok("Z5 矩阵返回的 implemented=true 条数与库一致（%s vs %s，差值=模块关掉的域）"
   % (db_true, matrix_true), matrix_true <= int(db_true or -1),
   "库=%s 矩阵=%s" % (db_true, matrix_true))

summary()
