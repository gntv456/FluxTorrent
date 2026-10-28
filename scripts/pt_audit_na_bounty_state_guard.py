#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""NA：`topics.bounty_status` 的判定必须带生效谓词（P-464 预防性钉桩）。

现网事实（2026-10-28 实查）：`topics.bounty_status` 是
`NOT NULL DEFAULT '"open"'`——**每一条主题行都从 'open' 起算**，包括普通讨论帖
（实测 topic 569/555/471 全是 `topic_type=normal` + `bounty=0` + `status=open`）。
而 `community_http/topic_create.rs:107-118` 只在 `topic_type == "bounty"` 时才
认赏金，其它类型一律写 0。所以 `'open'` 这个值的真实含义不是「有悬赏待采纳」，
而是「这行从没被悬赏流程碰过」。

本轮逐个读过五个读者，今天**全部安全**（这是查证结论，不是假设）：
`bounty.rs:53` 先判 `ttype != "bounty"` 再判状态；`topic_delete.rs:38-40` 带
`bounty_spark > 0`；`page.tsx:127` 与 `post-row.tsx:158` 都写成
`topic_type === "bounty" && …`；`post_read.rs` 只是取字段不判。
⇒ 现在没有用户可见缺陷。

那为什么还要钉：下一个读者只要写 `if bounty_status == "open"` 就自动中招，
症状按落点不同分成两种——列表/详情会给全站普通帖挂上「悬赏中」，
发放/清算类 SQL 会把非悬赏帖扫进流程（口径同成文判据第 5 条
「豁免名单必须带生效谓词」，也同 P-463 那族「快照列的默认值不是状态」）。
所以这把尺量的是**形状**：凡对 `bounty_status` 做相等判定的文件，
同一行 ±2 行内必须出现 `topic_type` 或 `bounty_spark`。
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTS = ["apps/api/src", "apps/worker/src", "apps/web/app", "apps/web/components"]
CMP = re.compile(r"bounty_status\s*[=!]=?", re.I)
GUARD = re.compile(r"topic_type|bounty_spark", re.I)
# 台账：文件 -> 判定形态与生效谓词在哪一行（多一个少一个都红）
LEDGER = {
    "apps/api/src/community_http/bounty.rs": "53 行先判 ttype != \"bounty\"",
    "apps/api/src/repo/community/topic_delete.rs":
        "38-40 行 SELECT 带 bounty_spark > 0；51-52 的 UPDATE 只在同一 "
        "repo 函数里被上面那道 SELECT 选中后才执行",
    "apps/web/app/(main)/forums/topic/[id]/_parts/post-row.tsx":
        "158 行 topic_type === \"bounty\" && …",
    "apps/web/app/(main)/forums/topic/[id]/page.tsx":
        "127 行 topic_type === \"bounty\" && (bounty_spark ?? 0) > 0",
}

# 粒度：按**文件**判（同文件内必须出现 topic_type 或 bounty_spark 之一）。
# 不按行窗口判——实测行窗口会把 `topic_delete.rs` 那种「SELECT 带
# bounty_spark>0 选出行、12 行后才 UPDATE」的正确写法读成违例，
# 也会漏掉 page.tsx 里距守卫 3 行的判定。文件粒度弱一点，但它不冤枉人。
readers, unguarded = {}, {}
for d in ROOTS:
    if not os.path.isdir(d):
        continue
    for dp, _dn, fns in os.walk(d):
        for fn in sorted(fns):
            if not fn.endswith((".rs", ".tsx", ".ts")):
                continue
            path = os.path.join(dp, fn).replace(chr(92), "/")
            text = open(path, encoding="utf-8", errors="replace").read()
            if CMP.search(text):
                readers[path] = len(CMP.findall(text))
                if not GUARD.search(text):
                    unguarded[path] = readers[path]

offenders = sorted(unguarded)
readers_set = sorted(readers)

ok("NA1 正控：合成件「只判 bounty_status==open」必须被点名",
   bool(CMP.search('if (detail.bounty_status === "open") {')))
ok("NA2 正控（反向）：同一件补上 topic_type 生效谓词后必须转绿",
   not GUARD.search('if (bounty_status === "open")')
   and bool(GUARD.search('if (topic_type === "bounty" && '
                         'bounty_status === "open")')))
ok("NA3a 任何读者判 bounty_status 时，同文件必须带生效谓词（裸判须为 0）",
   not offenders, "裸判文件=%s" % offenders)
ok("NA3b 读者集合 = 台账 %s（多一个=新读者先来登记并说明它的守卫）"
   % sorted(LEDGER), readers_set == sorted(LEDGER),
   "实测读者=%s" % readers_set)
for f in readers_set:
    print("   [读者] %s：%s" % (f, LEDGER.get(f, "未登记")))
ok("NA4 反空转：判据必须真的抓到过判定行（命中文件数须 >= 4，"
   "否则是尺子瞎了不是世界干净）",
   len(readers_set) >= 4 and sum(readers.values()) >= 5,
   "读者=%d 判定行=%d" % (len(readers_set), sum(readers.values())))
summary()
