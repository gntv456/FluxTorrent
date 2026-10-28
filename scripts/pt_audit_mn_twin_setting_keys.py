# -*- coding: utf-8 -*-
"""MN：设定「近亲键对」——面板并排两把只差几个字母的卡，一把真被读一把没人读（P-451）。

比一般死卡更坑的地方：站长**改的是那把没人读的**。他会以为「私信每页条数」
已经设成 30 了，而真正生效的键叫 `topics_per_page_default`（论坛版块每页主题数），
或者这个屏压根用的是 `?limit=` 参数 / 用户偏好列。

现网实锤（2026-10-27，逐把核过消费者）：`*_per_page_default` 一族四把卡里
只有 `topics_per_page_default` 被读（community_http/topic.rs:191，且全仓无
`format!` 拼这族键名 ⇒ 不是动态读取的假阴），另外三把零引用：
  pm_per_page_default / posts_per_page_default / torrents_per_page_default

判据 = difflib 高相似(≥0.82) + 同族（共享首段或尾段）+ 一侧被引用一侧零引用。
两道控制：
  MN1 正控：原型对 (topics_/torrents_per_page_default) 必须被抓到
  MN2 反控：无关键对 (subtitle_workflow vs announce_interval) 不得被配对
动态拼名族（arule_*_enabled/_action、module_*）整族排除——上一轮就是它把
零引用数撑到 254，不排就会把"被 format! 读走的键"误判成死卡。
"""
import difflib
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

CODE_DIRS = ["apps/api/src", "apps/worker/src", "apps/tracker/src", "crates"]
EXCLUDE = [re.compile(r"^arule_[a-z_]+_(enabled|action)$"),
           re.compile(r"^module_[a-z_]+$")]
VERIFIED_PAIRS = {
    ("topics_per_page_default", "torrents_per_page_default"),
    ("topics_per_page_default", "pm_per_page_default"),
    ("topics_per_page_default", "posts_per_page_default"),
}

blob = []
for d in CODE_DIRS:
    for dp, dn, fns in os.walk(d):
        dn[:] = [x for x in dn if x not in ("target", "__pycache__")]
        for f in fns:
            if f.endswith(".rs"):
                p = os.path.join(dp, f)
                try:
                    blob.append(open(p, encoding="utf-8",
                                     errors="replace").read())
                except OSError:
                    pass


def refs(k):
    return sum(1 for t in blob if re.search(re.escape(k), t))


keys = [ln.strip() for ln in psql_stdin(
    "SELECT m.name FROM settings_meta m JOIN site_settings s USING (name) "
    "ORDER BY m.name;").splitlines() if ln.strip()]
pool = [k for k in keys if not any(rx.match(k) for rx in EXCLUDE)]
cache = {k: refs(k) for k in pool}

pairs = []
for i, a in enumerate(pool):
    for b in pool[i + 1:]:
        if (cache[a] > 0) == (cache[b] > 0):
            continue
        sa, sb = a.split("_"), b.split("_")
        if not (sa[0] == sb[0] or sa[-1] == sb[-1]):
            continue
        r = difflib.SequenceMatcher(None, a, b).ratio()
        if r >= 0.82:
            live, deadk = (a, b) if cache[a] else (b, a)
            pairs.append((live, deadk))

ok("MN1 正控：原型对必须被抓到（topics_/torrents_per_page_default）",
   ("topics_per_page_default", "torrents_per_page_default") in pairs,
   "实测对=%s" % sorted(pairs))
ok("MN2 反控：无关键不得被配对",
   ("subtitle_workflow", "announce_interval") not in pairs
   and ("announce_interval", "subtitle_workflow") not in pairs, "见上")
ok("MN3 已核实的近亲对必须仍在（接了线或撤了卡就删号，多一对=新分叉）",
   set(pairs) == VERIFIED_PAIRS,
   "实测=%s" % sorted(pairs))
# 真被读的那把必须确实有引用（防止引用面整体失效时三条断言集体空转）
ok("MN4 反空转：活键一侧的引用数必须 > 0（否则整把尺在空集上跳舞）",
   all(cache.get(l, 0) > 0 for l, _d in VERIFIED_PAIRS)
   and cache.get("topics_per_page_default", 0) >= 1,
   "topics_per_page_default 引用=%s" % cache.get("topics_per_page_default"))

print("[读数] 参与配对的键=%d，命中近亲对=%d" % (len(pool), len(pairs)))
for live, deadk in sorted(pairs):
    print("   活=%-28s(%d处)  死=%s" % (live, cache[live], deadk))
summary()
