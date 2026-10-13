# -*- coding: utf-8 -*-
"""闸门 BJ：街机后台写口的形状与范围（奖池档位 / 确定侧奖励 / 物品折算价）。

选题：`/admin/arcade` 一族 8 条端点在常驻闸门里搜不到字面量（`grep -rn
"/admin/arcade" scripts/pt_audit_*.py` 只命中本批新建的 BJ）。这批不是「没人测」，
而是**测过的都是读侧**：`arcade_pool_gate` / `arcade_economy_invariants` 那一族钉的是
「池子 EV 是否 <1、发放是否幂等」，写侧只被同域的一处纪律照顾到 —— `games::
validate_crop` 的注释白纸黑字写着「上限是列宽（INT4）：越界必须由这里点名，
不能让调用方 `as i32` 绕成负数」，但奖池档位与物品折算价这两条写口没跟上。

  P-180（P1）`POST /admin/arcade/pool` 的档位三个值不判：
        · `weight ≤ 0` —— EV 校验里被 `max(0)` 吞成「没有这一档」，落库撞
          `arcade_pool_entries_weight_positive` CHECK ⇒ **500**（面板上像「保存被吞」）
        · `side` 不在 {any,win,triple,lose}（0265 之前的旧值 `tie` 是最真实的一例）
          ⇒ 撞 `side_known` CHECK ⇒ **500**
        · `payout < 0` —— 负倍率把整池 EV 拉低，闸门按「EV < 1」放行 ⇒ **200 落库**
          `mult_permille < 0` 的档，玩家中奖时反而被扣魔力
  P-181（P1）`POST /admin/arcade/rewards` 的 `reward_spark` 只判负不判上限：一行
        里程碑/周常配 1e12 就是之后每次领取的数额，且这条路径不经 EV 闸。
  P-182（P1）`POST /admin/arcade/items` 的 `anchor` 无范围：它既是引用它的池的
        EV 分母，也是「物品回落成魔力」的单价（`spark += anchor * qty`）。

修法是同域既有纪律的复用：`games_http::helpers` 里加 `ensure_grant_range`（口径抄
管理端手工调整的 P-154：火花单次 ±10 亿）与 `ensure_pool_shapes`，三处写口都调 ——
判据一处实现，不在每个入口重写一遍。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bj_arcade_admin_write.py
"""
import json
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
INV, KEYS = [], []
BIG = 10 ** 12


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], capture_output=True, text=True,
        encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:56], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


def cnt(t, w=None):
    return psql("SELECT count(*) FROM %s%s"
                % (t, "" if w is None else " WHERE " + w))


def blob(r):
    return json.dumps(r, ensure_ascii=False) if r is not None else ""


def data(r):
    if isinstance(r, tuple):
        r = r[1]
    return (r or {}).get("data") or {}


def msg(r):
    return str((r[1] or {}).get("message") or "")


def post(p, b=None, tok=None):
    return call("POST", p, b, token=tok or ROOT)


def mkprobe(prefix):
    codes = data(post("/admin/invites", {"user_id": 1, "count": 1})
                 ).get("codes") or []
    cap = data(call("GET", "/auth/captcha", token=ROOT))
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(nums) < 2:
        return 0, None, "发码/验证码没准备好"
    INV.append(codes[0]["id"])
    name = "%s%d" % (prefix, TAG * 100 + int(time.time()) % 100)
    rg = post("/auth/register", {
        "username": name, "email": "%s@bj.local" % name,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    tok = data(rg).get("token") or data(
        post("/auth/login", {"username": name,
                             "password": "password123"})).get("token")
    if not tok:
        return 0, None, "注册/登录 http=%s %s" % (rg[0], blob(rg[1])[:90])
    uid = int(psql("SELECT id FROM users WHERE username = '%s'" % name))
    return uid, tok, ""


clear_register_rl()
MV, MTV, EM = mkprobe("bjmv")
ok("B0a 成员探针就绪（uid=%s）%s" % (MV, EM), MV > 0 and MTV, EM)
# 模板池避开 bigsmall / farm：那两款的机制校验各有附加规则（三区千分比、
# 票价由作物定标），克隆出来的档位不一定满足，会把 B1a 的正例带崩
TPL = psql("SELECT e.pool_key FROM arcade_pool_entries e JOIN arcade_pools p"
           " ON p.key = e.pool_key AND p.game NOT IN ('bigsmall','farm')"
           " WHERE e.enabled GROUP BY e.pool_key ORDER BY count(*) DESC"
           " LIMIT 1")
TPL_G = psql("SELECT game FROM arcade_pools WHERE key = '%s'" % TPL)
TPL_T = psql("SELECT ticket FROM arcade_pools WHERE key = '%s'" % TPL)


def tpl_entries(weight=None, payout=None, side=None, n=2):
    """把模板池的若干档位读成请求体（判据测的是形状，不在这里重算 EV）。"""
    raw = psql("SELECT label||'|'||weight||'|'||mult_permille||'|'||kind||"
               "'|'||coalesce(item_key,'')||'|'||qty||'|'||side FROM "
               "arcade_pool_entries WHERE pool_key = '%s' AND enabled "
               "ORDER BY sort LIMIT %d" % (TPL, n))
    out = []
    for line in raw.splitlines():
        q = line.split("|")
        if len(q) < 7:
            continue
        e = {"label": q[0], "weight": int(q[1]), "payout": int(q[2]) / 1000.0,
             "kind": q[3], "qty": int(q[5]), "enabled": True, "side": q[6]}
        if q[4]:
            e["item_key"] = q[4]
        if weight is not None:
            e["weight"] = weight
        if payout is not None:
            e["payout"] = payout
        if side is not None:
            e["side"] = side
        out.append(e)
    return out


def save_pool(tag, entries, game=None, ticket=None):
    key = "bjtest%d%s" % (TAG, tag)
    KEYS.append(key)
    s, r = post("/admin/arcade/pool", {
        "pool_key": key, "game": game or TPL_G, "label": "BJ %s" % tag,
        "ticket": ticket or int(TPL_T or 10), "entries": entries})
    return s, r, key


BASE = dict((t, cnt(t)) for t in ("arcade_pools", "arcade_pool_entries",
                                  "arcade_items", "arcade_quests",
                                  "arcade_milestones"))
ok("B0b 模板池就绪（key=%s game=%s ticket=%s；基线行数 %s）" % (TPL, TPL_G,
   TPL_T, BASE), TPL and TPL_G and int(TPL_T or 0) > 0, blob(BASE)[:120])

# ============ B1 奖池档位的三个不判的值 ============
s0, r0, K0 = save_pool("ok", tpl_entries())
ok("B1a 正例：按现有池的档位另存为新 key（http=%s 回包 ev=%s）"
   % (s0, data((s0, r0)).get("ev")),
   s0 == 200 and cnt("arcade_pool_entries", "pool_key = '%s'" % K0)
   == str(len(tpl_entries())), "%s %s" % (s0, blob(r0)[:100]))
s1, r1, K1 = save_pool("w0", tpl_entries(weight=0))
ok("B1b 档位权重填 0 必须 400 并点名是哪一档（未修版即绿：EV 闸已拦，本轮当回归钉）",
   s1 == 400 and "权重" in msg((s1, r1)),
   "http=%s msg=%s" % (s1, msg((s1, r1))[:80]))
s2, r2, K2 = save_pool("wn", tpl_entries(weight=-5))
ok("B1c 权重为负同样必须 400（同上，回归钉）", s2 == 400 and "权重" in msg((s2, r2)),
   "http=%s msg=%s" % (s2, msg((s2, r2))[:80]))
s3, r3, K3 = save_pool("sd", tpl_entries(side="tie"))
ok("B1d side 用 0265 之前的旧值 tie 必须 400 并报出取值域（不得撞 CHECK 变 500）",
   s3 == 400 and "side" in msg((s3, r3)),
   "http=%s msg=%s" % (s3, msg((s3, r3))[:90]))
NEG = -1.0
s4, r4, K4 = save_pool("neg", tpl_entries(payout=NEG))
NEGMULT = cnt("arcade_pool_entries", "pool_key = '%s' AND mult_permille < 0"
              % K4)
ok("B1e 票价倍数为负必须 400：负档把整池 EV 拉低而绕过闸门，中奖反而扣玩家魔力",
   s4 == 400 and "倍数" in msg((s4, r4)),
   "http=%s msg=%s 库里负倍率档=%s 条" % (s4, msg((s4, r4))[:70], NEGMULT))
ok("B1f 写侧关闸：坏档位一条都不落库（四个坏 key 的行数全为 0）",
   all(cnt("arcade_pool_entries", "pool_key = '%s'" % k) == "0"
       for k in (K1, K2, K3, K4))
   and cnt("arcade_pools", "key IN ('%s','%s','%s','%s')"
           % (K1, K2, K3, K4)) == "0",
   " ".join("%s=%s" % (k, cnt("arcade_pool_entries",
                              "pool_key = '%s'" % k))
            for k in (K1, K2, K3, K4)))
s5, r5, _ = save_pool("ok", tpl_entries())
ok("B1g 同 key 重复保存是覆盖不是追加（ON CONFLICT + 重建档位，行数=%s）"
   % cnt("arcade_pool_entries", "pool_key = '%s'" % K0),
   s5 == 200 and cnt("arcade_pool_entries", "pool_key = '%s'" % K0)
   == str(len(tpl_entries())),
   "%s %s" % (s5, blob(r5)[:80]))

# ============ B2 确定侧奖励 ============
QC = "bjq%d" % TAG
s6, r6 = post("/admin/arcade/rewards", {
    "kind": "quest", "code": QC, "scope": "*", "target": 3,
    "reward_spark": BIG})
ok("B2a 一行周常配 1e12 魔力必须 400（上限口径 = 管理端手工调整的 10 亿）",
   s6 == 400 and "需在 0 ~" in msg((s6, r6)),
   "http=%s msg=%s 库里=%s 条" % (s6, msg((s6, r6))[:80],
                                 cnt("arcade_quests", "code = '%s'" % QC)))
s7, r7 = post("/admin/arcade/rewards", {
    "kind": "quest", "code": QC, "scope": "*", "target": 3,
    "reward_spark": 50})
ok("B2b 正例：合规的一行能存下（库里 reward_spark 读回 50）",
   s7 == 200 and psql("SELECT reward_spark::text FROM arcade_quests WHERE "
                      "code = '%s'" % QC) == "50",
   "http=%s %s" % (s7, blob(r7)[:80]))
ok("B2c 既不发魔力也不发物品、scope 越界都仍被拒（回归：这两条本来就在）",
   post("/admin/arcade/rewards", {"kind": "quest", "code": QC, "scope": "*",
                                  "target": 3, "reward_spark": 0})[0] == 400
   and post("/admin/arcade/rewards", {"kind": "quest", "code": QC,
                                      "scope": "farm_harvest", "target": 3,
                                      "reward_spark": 50})[0] == 400)
s8, r8 = post("/admin/arcade/rewards", {
    "kind": "milestone", "code": "bjm%d" % TAG, "scope": "s1", "target": 5,
    "reward_spark": -1})
ok("B2d 负奖励照旧拒", s8 == 400, "http=%s %s" % (s8, blob(r8)[:70]))

# ============ B3 物品目录的折算价 ============
IK = "bjitem%d" % TAG


def save_item(anchor, kind="cosmetic"):
    return post("/admin/arcade/items", {
        "key": IK, "name": "BJ 探针物品", "kind": kind, "anchor": anchor,
        "anchor_src": "declared", "use_kind": "collect"})


s9, r9 = save_item(BIG)
ok("B3a anchor 配 1e12 必须 400（它是池的 EV 分母，也是物品回落成魔力的单价）",
   s9 == 400 and "0 ~" in msg((s9, r9)),
   "http=%s msg=%s 库里=%s 条" % (s9, msg((s9, r9))[:80],
                                 cnt("arcade_items", "key = '%s'" % IK)))
s10, r10 = save_item(-100)
ok("B3b anchor 为负必须 400（非经济类物品原先无人拦，负 anchor 让 EV 看起来更低）",
   s10 == 400 and "0 ~" in msg((s10, r10)),
   "http=%s msg=%s" % (s10, msg((s10, r10))[:80]))
s11, r11 = save_item(100)
ok("B3c 正例：合规 anchor 存得下且读回一致",
   s11 == 200 and psql("SELECT anchor::text FROM arcade_items WHERE key = "
                       "'%s'" % IK) == "100",
   "http=%s %s" % (s11, blob(r11)[:80]))

# ============ B4 权限与审计 ============
for nm, path, bod in (("pool", "/admin/arcade/pool", {"pool_key": "x",
                                                      "game": "jgg",
                                                      "label": "x",
                                                      "ticket": 10,
                                                      "entries": []}),
                      ("rewards", "/admin/arcade/rewards", {
                          "kind": "quest", "code": "x", "scope": "*",
                          "target": 1, "reward_spark": 1}),
                      ("items", "/admin/arcade/items", {
                          "key": "x", "name": "x", "kind": "cosmetic",
                          "anchor": 1, "anchor_src": "declared"})):
    st = post(path, bod, MTV)[0]
    ok("B4a 成员打不开 %s 写口（403，perm user.adjust 门槛）" % nm, st == 403,
       "http=%s" % st)
AUD = dict((a, cnt("audit_log", "action = '%s'" % a))
           for a in ("arcade.pool.save", "arcade.reward.save",
                     "arcade.item.save"))
save_item(200)
post("/admin/arcade/rewards", {"kind": "quest", "code": QC, "scope": "*",
                              "target": 3, "reward_spark": 50})
save_pool("ok", tpl_entries())
NOWAUD = dict((a, cnt("audit_log", "action = '%s'" % a)) for a in AUD)
ok("B4b 三个写口都留审计（pool/reward/item 各 +1：%s -> %s）" % (AUD, NOWAUD),
   all(int(NOWAUD[a]) == int(AUD[a]) + 1 for a in AUD), blob(NOWAUD)[:120])

# ============ B5 读侧没被写侧改动打坏 + 残留 ============
so, ro = call("GET", "/admin/arcade/overview", token=ROOT)
ok("B5a 后台总览仍可读（http=%s，含池表/赔率读数）" % so, so == 200
   and bool(data((so, ro))), blob(ro)[:90])
ok("B5b 库里负倍率与 0 权重档都不存在（写侧关闸的落库证据）",
   cnt("arcade_pool_entries", "mult_permille < 0") == "0"
   and cnt("arcade_pool_entries", "weight <= 0") == "0",
   "neg=%s zero=%s" % (cnt("arcade_pool_entries", "mult_permille < 0"),
                       cnt("arcade_pool_entries", "weight <= 0")))
psql("DELETE FROM arcade_pool_entries WHERE pool_key LIKE 'bjtest%d%%'" % TAG)
psql("DELETE FROM arcade_pools WHERE key LIKE 'bjtest%d%%'" % TAG)
psql("DELETE FROM arcade_quests WHERE code LIKE 'bjq%%'")
psql("DELETE FROM arcade_milestones WHERE code LIKE 'bjm%%'")
psql("DELETE FROM arcade_items WHERE key LIKE 'bjitem%%'")
if INV:
    psql("UPDATE invites SET status = 2 WHERE id IN (%s)"
         % ",".join(str(int(i)) for i in INV))
post("/admin/users/status", {"user_id": MV, "status": 2})
call("DELETE", "/admin/users/%d" % MV, None, token=ROOT)
psql("DELETE FROM spark_ledger WHERE user_id = %d" % MV)
END = dict((t, cnt(t)) for t in BASE)
drift = dict((k, (BASE[k], END[k])) for k in BASE if BASE[k] != END[k])
ok("B5c 五张配置表行数回基线（残留 %s），探针已墓碑" % (drift or "无"),
   not drift and psql("SELECT status::text FROM users WHERE id = %d" % MV)
   in ("2", "3"), blob(drift)[:120])
summary()
