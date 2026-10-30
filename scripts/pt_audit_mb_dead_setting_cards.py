# -*- coding: utf-8 -*-
"""MB：面板有卡、全站零消费者的设定键（死配置卡片，P-450）。

形状：`settings_meta` 里挂了卡（有 type/label，设置页渲染出来能改），
`site_settings` 里有值，但 api/worker/tracker/crates **任何地方都不读这把键**
⇒ 站长改完一点反应没有。这是「配置写了却不生效」的最后一格，
与假开关（有执法无入口）正好反向。

上一轮同一把粗尺报出 254/551 条，被自己的正控毙了——那批假阳几乎全来自
**动态拼键名**的消费者（`review_auto.rs:239` 的 `gb(&format!("{id}_enabled"))`
就吃掉整族 `arule_*_enabled`）。所以本闸门带两条硬装置：
  MB1 正控A：已知死卡（watermarkpos 等）必须被抓到
  MB2 正控B：已知活键（announce_interval / subtitle_workflow / arule_*_enabled 族）
             不得被误报——误报＝豁免名单写坏了，集合判据就没意义
豁免必须带**生效谓词**（正则 + 读它的代码位置），否则不许进名单。
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

CODE_DIRS = ["apps/api/src", "apps/worker/src", "apps/tracker/src", "crates"]
# 豁免：动态拼出来的键名族。每条都写「谁拼的」，没出处不进名单。
EXEMPT_PATTERNS = [
    (re.compile(r"^arule_[a-z_]+_(enabled|action)$"),
     "review_auto.rs:239 用 gb(&format!(\"{id}_enabled\")) 读走整族"),
    (re.compile(r"^module_[a-z_]+$"),
     "apps/api/src/modules.rs 按 module_<key> 前缀通用读取"),
]
ALIVE_CONTROLS = ["announce_interval", "subtitle_workflow",
                  "arule_ads_keywords_enabled"]
DEAD_CONTROLS = ["seeding_cap_hourly"]


def load_blob():
    blob = []
    for d in CODE_DIRS:
        for dp, dn, fns in os.walk(d):
            dn[:] = [x for x in dn if x not in ("target", "__pycache__")]
            for f in fns:
                if f.endswith(".rs"):
                    p = os.path.join(dp, f)
                    try:
                        blob.append((p, open(p, encoding="utf-8",
                                             errors="replace").read()))
                    except OSError:
                        pass
    return blob


def exempt(name):
    return next((why for rx, why in EXEMPT_PATTERNS if rx.match(name)), None)


def refs(blob, name):
    return [p for p, t in blob if re.search(re.escape(name), t)]


blob = load_blob()
cards = psql_stdin(
    "SELECT m.name FROM settings_meta m JOIN site_settings s USING (name) "
    "ORDER BY m.name;", sep="\t")
keys = [ln.strip() for ln in cards.splitlines() if ln.strip()]
# 只算「有卡有值」的键；代码引用零 ⇒ 死卡
dead = [k for k in keys
        if not refs(blob, k) and not exempt(k)]

hit_a = all(k in dead for k in DEAD_CONTROLS)
ok("MB1 正控A：已知死卡必须被抓到 %s" % DEAD_CONTROLS, hit_a,
   "实测死卡集合含=%s" % [k for k in DEAD_CONTROLS if k in dead])
hit_b = all(refs(blob, k) or exempt(k) for k in ALIVE_CONTROLS)
ok("MB2 正控B：活键/动态键族不得被误报 %s" % ALIVE_CONTROLS, hit_b,
   "被误报=%s" % [k for k in ALIVE_CONTROLS
                 if not (refs(blob, k) or exempt(k))])

# 台账只放**本会话逐把核实过**的。B4（0439）后仅存的实测死卡是 10 把
# 「TS / SQL 函数体消费」的留守键（MB 粗尺只扫 Rust blob，看不见这两面），
# 台账语义 = 粗尺死卡登记 + 消费方明示；真死键已全部摘除（0438/0439）。
LEDGER = {
    "seeding_base_hourly", "seeding_bonus_scale", "seeding_cap_hourly",
    "seeding_curve_k", "seeding_donor_mult", "seeding_rarity_exp",
    "seeding_rarity_k", "seeding_vol_log_base_gb",   # seeding_params() 函数体
    "bitbucket",   # web legacy-tool.ts
    "website",     # web layout.tsx
}
ok("MB3 已核实死卡必须仍是死卡（台账 ⊆ 实测；某把被接了线就该从台账删号）",
   LEDGER.issubset(set(dead)),
   "缺席=%s（说明它已被消费或撤卡，请删号）" % sorted(LEDGER - set(dead)))
# 棘轮：**候选数**上限只降不升。注意 246 是「键名字面量零引用」的候选集，
# **不是** 246 个缺陷——本尺看不见动态拼名（`arule_*_enabled` 就是被
# `format!("{id}_enabled")` 读走的，MB2 专门拿它当反控）。
# 每核实一把：确认为死卡就并入 LEDGER，能接线/该撤卡就处理后从候选里消掉；
# 数字下降只来自「逐把核实」，不来自把尺子调松。
MEASURED_CEIL = 10
ok("MB4 键名零引用的**候选**数不得超过基线 %d（是候选上限，不是缺陷数；"
   "只降不升，降的方式是逐把核实）" % MEASURED_CEIL,
   len(dead) <= MEASURED_CEIL,
   "实测=%d（降到新数后请把基线一起下调）" % len(dead))
ok("MB7 台账不得被悄悄清空：粗尺死卡登记须 ≥ 10 把（现=%d）" % len(LEDGER),
   len(LEDGER) >= 10, "台账=%d" % len(LEDGER))

# 专门钉最尖锐的一条：功能在，旋钮不在链路上
src = open("apps/api/src/admin_http/site.rs", encoding="utf-8",
           errors="replace").read()
ok("MB5 作弊探测：NP 时代『探测等级』卡已于 0438 撤卡（真实阈值写死在"
   " site.rs SQL 的 500GB/100MB/s，档位化记 backlog）——键不得回潮",
   "536870912000" in src and not refs(blob, "cheater_detection_level"),
   "写死阈值在位=%s 键被引用=%s"
   % ("536870912000" in src, bool(refs(blob, "cheater_detection_level"))))
ok("MB6 同屏易混对已收口：torrents/pm/posts_per_page_default 三把假键已于"
   " 0439 摘卡，真键 topics_per_page_default 必须仍在且有读者",
   bool(refs(blob, "topics_per_page_default")),
   "真键引用=%d" % len(refs(blob, "topics_per_page_default")))

print("[读数] settings_meta∩site_settings 键数=%d，零消费者死卡=%d"
      % (len(keys), len(dead)))
print("[读数] 台账已核实=%d，未逐把核实的差额=%d"
      % (len(LEDGER), len(set(dead) - LEDGER)))
summary()
