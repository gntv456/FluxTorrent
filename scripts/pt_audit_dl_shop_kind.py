# -*- coding: utf-8 -*-
"""闸门 DL：商城「这个品类到底发得出东西吗」必须只有一把尺子。

批一百六十四 P-348。会员视角：商城买一件道具，收钱之后由
`flux_shared::apply_item_effect` 决定发什么。它的 match 有 `_ => {}` 兜底
⇒ 不在实现清单里的 kind：**扣全款、回 200、什么也不发**（审计 P1 命名的
「花钱买空气」）。仓里本来就有那把尺子 `has_effect()`（后台建 SKU
admin_p3_http/shop.rs:71 与娱乐屋奖品绑定 item_use.rs:93 都问它），
**唯独收钱的售卖口没问**——库里被直接改出来的 active 行、或「加了 match 臂
却忘了加清单」的漂移，都能绕开后台那道拦。现在售卖口也问同一把尺
（economy_http/shop_guards.rs::ensure_deliverable），卡类（补签卡/改名卡/
临时邀请券）按张插订单行、由各自核销口消费，由 card_kind_probe 认领。

判据五层：
  DL1 覆盖：DB 里出现过的每个 kind 必须被 has_effect ∪ card_kind_probe 认领
      （未认领=0，命中数与基数一起报，防「什么都没匹配」的空绿）；
  DL2 漂移：has_effect 清单与 dispatcher 的 match 臂集合必须互相覆盖
      （卡类三只走专用支，反向比对按台账豁免）；
  DL3 顺序：售卖口 `ensure_deliverable` 必须在 `spend_spark(` 之前
      （拦在扣款之后＝照样收钱，只是回个错）；
  DL4 正控：给检测器一份含「zzz_no_handler」的合成分支清单，必须报未认领；
  DL5 单一尺子：凡收钱/绑奖品的口都问 has_effect，不许自己抄一份清单。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dl_shop_kind.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SHARED = os.path.join(ROOTDIR, "crates", "flux-shared", "src", "lib.rs")
SHOP = os.path.join(ROOTDIR, "apps", "api", "src", "economy_http", "shop.rs")
GUARDS = os.path.join(ROOTDIR, "apps", "api", "src", "economy_http",
                      "shop_guards.rs")
# 卡类：不走 apply_item_effect，按张插订单行；反向比对时豁免
CARD_KINDS = ("makeup_card", "rename_card", "temp_invite")
ASKERS = [os.path.join(ROOTDIR, "apps", "api", "src", "admin_p3_http",
                       "shop.rs"),
          os.path.join(ROOTDIR, "apps", "api", "src", "games_http",
                       "item_use.rs"), GUARDS]


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:220])
    return (r.stdout or "").strip()


src = rd(SHARED)


def has_effect_list():
    i = src.index("pub fn has_effect")
    body = src[i:src.index("\n}\n", i)]
    return set(re.findall(r'"([a-z_]+)"', body))


def dispatcher_arms():
    i = src.index("pub async fn apply_item_effect")
    body = src[i:]
    stop = body.index("_ =>") if "_ =>" in body else len(body)
    kinds = set()
    for m in re.finditer(r'^\s*((?:"[a-z_]+"\s*\|?\s*)+)\s*=>',
                         body[:stop], re.M):
        kinds |= set(re.findall(r'"([a-z_]+)"', m.group(1)))
    # 单行写法（"x" | "y" => 换行）之外的补集
    for m in re.finditer(r'"([a-z_]+)"\s*(?:\|\s*"[a-z_]+")*\s*=>',
                         body[:stop]):
        kinds |= set(re.findall(r'"([a-z_]+)"', m.group(0)))
    return kinds


def covered(kinds, claimed):
    return sorted(k for k in kinds if k not in claimed)


HE = has_effect_list()
ARMS = dispatcher_arms()
cards = set(re.findall(r'"([a-z_]+)"', rd(GUARDS).split(
    "card_kind_probe")[1][:200])) if "card_kind_probe" in rd(GUARDS) else set()

# ---------- DL1 覆盖率 ----------
db_kinds = set(x.strip() for x in psql(
    "SELECT kind FROM shop_items GROUP BY kind ORDER BY kind").split("\n")
    if x.strip())
missing = covered(db_kinds, HE | cards)
ok("DL1 DB 里 %d 个品类都有人认领（未认领 %d）" % (len(db_kinds), len(missing)),
   len(db_kinds) >= 10 and not missing,
   "缺认领：%s ／ 卡类专用支：%s" % ("、".join(missing) or "-",
                                      "、".join(sorted(cards)) or "-"))

# ---------- DL2 漂移 ----------
ghost = sorted(HE - ARMS - cards)      # 清单说有、代码没实现
undoc = sorted(ARMS - HE)              # 代码实现了、清单没说
ok("DL2 has_effect(%d) 与生效链臂(%d) 未漂移（清单虚报 %d、漏登记 %d）"
   % (len(HE), len(ARMS), len(ghost), len(undoc)),
   len(HE) >= 15 and len(ARMS) >= 15 and not ghost and not undoc,
   "清单虚报（扣钱不发）：%s ／ 漏登记（被误停售）：%s"
   % ("、".join(ghost) or "-", "、".join(undoc) or "-"))

# ---------- DL3 顺序：拦在收钱之前 ----------
body = re.sub(r"//.*", "", rd(SHOP), flags=re.M)
i_guard = body.find("ensure_deliverable(")
i_spend = body.find("spend_spark(")
ok("DL3 售卖口在扣款**之前**问这把尺（guard=%s spend=%s）"
   % (i_guard, i_spend),
   0 <= i_guard < i_spend, "顺序反了＝照样收钱，只是回个错（P-348 复发）")

# ---------- DL4 正控：检测器必须抓得住未认领的 kind ----------
probe = covered({"upload_credit", "zzz_no_handler", "vip"}, HE | cards)
ok("DL4 正控：合成品类清单里未认领的必须被抓出（抓到 %s）" % probe,
   probe == ["zzz_no_handler"], "抓不住＝DL1 那条红永远不可能出现（空判据）")

# ---------- DL5 收钱/绑奖的口共用同一把尺 ----------
lazy = [os.path.relpath(f, ROOTDIR).replace("\\", "/") for f in ASKERS
        if "has_effect" not in rd(f)]
ok("DL5 三处入口都问 has_effect（自写清单的 %d 处）" % len(lazy), not lazy,
   "缺尺子：%s" % ("、".join(lazy) or "-"))

summary()
