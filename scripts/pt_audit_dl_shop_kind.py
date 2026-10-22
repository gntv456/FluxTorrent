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
import json
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
# 「有核销口的卡类」白名单在代码侧（shop_guards::card_kind_probe → 下面的 cards）。
# 这张台账钉的是**反向**：在售、但全仓找不到任何核销口的卡类——rename_card（P-477）。
# 站方若实现「用户自助改名 + 卡消耗」，就把它从这里删掉并加回 card_kind_probe；
# 若选择下架，也从这里删掉（那时 DB 里不再有这一 kind，DL1 自然收口）。
NO_REDEEM = {"rename_card": "全仓无核销口：用户侧没有自助改名端点"}
PLUMBING = ("economy_http/shop", "economy_http/shop_guards",
            "economy_http/shop_list", "economy_http/my_orders",
            "admin_p3_http/increment", "admin_http/user_grant_item",
            "games_http/arcade_cfg")
ASKERS = [os.path.join(ROOTDIR, "apps", "api", "src", "admin_p3_http",
                       "shop.rs"),
          os.path.join(ROOTDIR, "apps", "api", "src", "games_http",
                       "item_use.rs"), GUARDS]


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-v", "ON_ERROR_STOP=1", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0 or "ERROR" in (r.stderr or ""):
        raise RuntimeError("   psql 失败：%s" % (r.stderr or "").strip()[:220])
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
missing = covered(db_kinds, HE | cards | set(NO_REDEEM))
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

# ---------- DL6 档位金额：品类被认领 ≠ 这件商品发得出货（P-476） ----------
# 生效链对「发放数量」用的是 config 里的键；算不出来时 charity/gift_spark/
# upload_credit 三档的兜底是 **0**，于是 `if amount > 0` 静默跳过：钱照扣、
# 货不发、接口 200。现网实测：0004 起「慈善捐赠」的 config 一直是 '{}'，
# root 三笔订单（id 33/35/49，各 1000 火花）五分钟内进池记录全空。
AMOUNT_KEYS = {
    "charity": ("spark", "sparks", "amount", "price"),
    "gift_spark": ("spark", "sparks", "amount"),
    "upload_credit": ("gb", "upload_gb", "gb_per_unit"),
}


def resolve(cfg, keys):
    for k in keys:
        v = cfg.get(k)
        if isinstance(v, int) and v > 0:
            return v
    return 0


rows6 = psql("SELECT id || '@' || kind || '@' || price || '@' || "
             "coalesce(config::text, '{}') FROM shop_items WHERE active "
             "AND kind IN ('charity','gift_spark','upload_credit') "
             "ORDER BY id").splitlines()
bad6, seen6 = [], 0
for ln in rows6:
    if "@" not in ln:
        continue
    iid, kind, price, cfgtxt = ln.split("@", 3)
    seen6 += 1
    try:
        cfg = json.loads(cfgtxt)
    except Exception:  # noqa: BLE001
        bad6.append("%s(%s) config 不是合法 JSON" % (iid, kind))
        continue
    got = resolve(cfg, AMOUNT_KEYS[kind])
    if got == 0:
        # charity 允许退到「档位售价」，其余两档没有默认值＝静默不发
        bad6.append("%s(%s) 算不出发放数额（config=%s price=%s）"
                    % (iid, kind, cfgtxt[:44], price))
ok("DL6 在售的计量档位（%d 件）都必须算得出正数发放额（不可发 %d）"
   % (seen6, len(bad6)), seen6 >= 3 and not bad6, "; ".join(bad6[:3]))
# 正控：同一把尺必须能抓出「config 空」那件（否则 DL6 是空判据）
ctl = resolve({}, AMOUNT_KEYS["charity"])
ctl2 = resolve(json.loads('{"spark": 1000}'), AMOUNT_KEYS["charity"])
ok("DL6z 正控：空 config 算得出 0（实测 %s）、补好金额后算得出 1000（实测 %s）"
   % (ctl, ctl2), ctl == 0 and ctl2 == 1000,
   "两值相同＝这把尺没装刻度")

# ---------- DL7 卡/券类：发得出 ≠ 花得掉（P-477） ----------
def list_kinds(text, fn_name):
    """从 Rust 源码里抓一个 matches!(kind, "a" | "b") 型判据的品类清单。"""
    i = text.index("fn " + fn_name + "(")
    body = text[i:]
    stop = body.index(chr(10) + "}")
    return set(re.findall(r'"([a-z_]+)"', body[:stop]))


def voucher_issued(text):
    """apply_item_effect 里「往 user_vouchers 插一张券」的那些臂。"""
    i = text.index("pub async fn apply_item_effect")
    seg = text[i:]
    out = set()
    for m in re.finditer(r'((?:"[a-z_]+"\s*\|?\s*)+)\s*=>', seg):
        head = m.group(1)
        tail = seg[m.end():m.end() + 400]
        if "INSERT INTO user_vouchers" in tail:
            out |= set(re.findall(r'"([a-z_]+)"', head))
    return out


GU = rd(GUARDS)
# P-477 之后清单收在共享层（flux-shared），api 侧的 shop_guards 只是转发：
# 判据必须读**清单的本家**，否则「转发改了、本家没改」这种漂移量不出来。
REDEEM = list_kinds(src, "redeem_port_exists")
ISSUED = voucher_issued(src) | cards | list_kinds(src, "issues_voucher")
unredeemed = sorted(ISSUED - REDEEM)
ok("DL7 「发得出但花不掉」的品类恰为台账 %d 档（实测 %s／发卡券侧共 %d 档）"
   % (len(NO_REDEEM), unredeemed, len(ISSUED)),
   unredeemed == sorted(NO_REDEEM) and len(ISSUED) >= 4,
   "多出来的档＝新上的券没人核销还在卖；少了＝台账该销账（同批删一行）")
IU = rd(os.path.join(ROOTDIR, "apps", "api", "src", "games_http",
                     "item_use.rs"))
ok("DL7a 两个入口都问这把尺（商城售卖口 + 娱乐屋奖品绑定口）",
   GU[GU.index("fn ensure_deliverable"):].count(
       "voucher_without_redeem_port") >= 1
   and IU.count("voucher_without_redeem_port") >= 1,
   "只在售卖侧写清单＝半个闸门：奖品侧照样能把废纸发到会员手上")
drift = sorted(cards - REDEEM)
ok("DL7b 卡白名单 %d 档每档都在「花得掉」清单里（漂移=%s）"
   % (len(cards), drift), not drift, "白名单=%s" % sorted(cards))
ctl = sorted((ISSUED | {"zzz_card"}) - REDEEM)
ok("DL7c 正控：合成一张没人核销的 zzz_card 必须被同一算式抓到（抓到=%s）"
   % ctl, "zzz_card" in ctl and ctl != unredeemed,
   "抓不住＝DL7 是空判据（算式对合成档没有反应）")
rn = psql("SELECT coalesce(string_agg('id' || id || '/价' || price"
          " || '/active=' || active::text, ' '), '无') FROM shop_items "
          "WHERE kind = 'rename_card'")
sold = psql("SELECT count(*) FROM shop_orders o JOIN shop_items i "
            "ON i.id = o.item_id WHERE i.kind = 'rename_card'")
vch = psql("SELECT count(*) FROM user_vouchers WHERE kind = 'rename_card'")
ok("DL7d 现网读数在册：改名卡 %s；累计成交 %s 张、手上券 %s 张（停售闸门随镜像生效）"
   % (rn, sold, vch), rn != "无",
   "此前下单=扣 100000 火花拿一张没人认的券；成交 0 说明还是潜伏")

summary()
