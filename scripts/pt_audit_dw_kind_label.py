# -*- coding: utf-8 -*-
"""闸门 DW：魔力流水的 kind 必须有译名（P-316 一族的常驻引信）。

批一百七十四。会员视角：打开「我的魔力流水」，某一行来源写着
`social_team_reward` / `forum-topic-del` / `orphan_offset`——不是中文。

现网实证（`spark_ledger` 按 kind 去重，对 `apps/web/i18n/zh-CN.ts` 的 ledger
kinds 块求差）：29 个 kind 里 **4 个没有译名**——

| kind | 行数 | 金额合计 |
| --- | --- | --- |
| social_team_reward | 12 | +30000 |
| probe_cleanup | 8 | -450 |
| forum-topic-del | 2 | -4 |
| orphan_offset | 1 | -52374 |

渲染侧是 `dict.kinds[k] ?? k`（见 `apps/web/app/(main)/admin/_parts/*.tsx`、
`dressup/_inner.tsx` 同形），**回落就是原始英文键**，所以「没译名」不是无害的
样式差异，而是会员/职员直接读到内部键名。P-316 那批已经补过 8 个键，
这四条是同族复发 ⇒ 本闸门把它钉成**集合级台账**：多一个红、少一个也红。

`probe_cleanup`（8 行 / -450）是审计闸门自己造的：探针期间从会员账户回收魔力时
图省事写了个内部键。这类行要么走 `adjust`（调账，已有译名），要么就得登记译名——
**不许留内部键在会员账上**；本批不代删（删流水＝改账史，归站方裁）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dw_kind_label.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ZHCN = os.path.join(ROOTDIR, "apps", "web", "i18n", "zh-CN.ts")
SRCDIR = os.path.join(ROOTDIR, "apps", "api", "src")
# 已登记的违例集合（只许缩小，不许变大，也不许偷偷换个名字躲判据）
# 只认**确实没有译名**的：forum-topic-del / forum-post-del 的译名在
# zh-CN.ts 里本来就有（"删主题扣除"/"删帖扣除"），旧正则读不出带引号的键
# 才把它们误列进来（2026-10-20 实测）。
# 2026-10-21 批：上面三个键的译名已登记进三语（apps/web/i18n/{zh-CN,en,zh-TW}.ts
# 的 my.kinds），按本文件自己的口径「少了＝有人补了译文，请把清单同步收缩」
# 收到这里。再出现裸 kind 就是新违例，直接红。三语覆盖由 pt_audit_kind_label_all_lang.py 判。
KNOWN_UNLABELED = set()
# 转账类 kind：扣款与入账必须成对且净额 0（P-360 的常驻形状）
TRANSFER_KINDS = {"forum_tip"}


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-v", "ON_ERROR_STOP=1", "-tA", "-F",
         " | ", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0 or "ERROR" in (r.stderr or ""):
        raise RuntimeError("   psql 失败：%s" % (r.stderr or "").strip()[:240])
    return (r.stdout or "").strip()


def labeled_keys():
    ts = io.open(ZHCN, encoding="utf-8", errors="replace").read()
    i = ts.index("forum_tip")
    blk = ts[max(0, i - 3000): i + 3000]
    # 带引号的键（"forum-post-del": "删帖扣除"）也要认：旧正则要求
    # 「键名紧跟冒号」，于是所有**带引号**的键（正是 0388 之后新加的那批）
    # 全都读不出来 ⇒ 明明有译名却被判成缺译名（2026-10-20 实测
    # forum-post-del 在 zh-CN.ts 里明明有译名）。
    return {m for m in re.findall(r'"?([a-z_][a-z0-9_-]*)"?\s*:', blk)}


live = [ln.strip() for ln in psql(
    "SELECT DISTINCT kind FROM spark_ledger ORDER BY 1").split("\n")
    if ln.strip()]
labels = labeled_keys()
missing = set(live) - labels

# ---------- DW1 现网 kind 必须有译名（集合级台账） ----------
ok("DW1 现网 %d 个 kind，缺译名的集合 %s 必须等于已登记的 %s"
   % (len(live), sorted(missing), sorted(KNOWN_UNLABELED)),
   missing == KNOWN_UNLABELED,
   "多出来＝新 kind 没带译名就上线（P-316 同族复发）；"
   "少了＝有人补了译文，请把本清单同步收缩")
for k in sorted(missing):
    row = psql("SELECT count(*) || ' 行 / ' || coalesce(sum(amount), 0) "
               "FROM spark_ledger WHERE kind = '%s'" % k.replace("'", ""))
    print("      [在册] %s：%s" % (k, row.replace("\n", "")))

# ---------- DW2 代码侧引信：写死的 kind 也要有译名 ----------
used = set()
for root, _, files in os.walk(SRCDIR):
    for fn in files:
        if not fn.endswith(".rs"):
            continue
        t = io.open(os.path.join(root, fn), encoding="utf-8",
                    errors="replace").read()
        for m in re.finditer(
                r"(?:spend_spark|earn_spark|spend_spark_tx|earn_spark_tx)\s*\(",
                t):
            seg = t[m.end():m.end() + 400]
            # kind 实参：非 tx 版是 (db, uid, amount, "kind", key)，
            # 事务版是 (tx, uid, amount, "kind", key, ...)——取第 4 个字符串量
            strs = re.findall(r'"([a-z_][a-z0-9_-]{2,32})"', seg.split(")")
                               [0] if ")" in seg else seg)
            for s in strs[:1]:
                used.add(s)
unlabeled_code = {k for k in used if k not in labels}
# 取值是「spend/earn 调用之后第一个字符串量」的位置启发式：`forum-like` 已人工
# 核实（community_http/like.rs:103 `earn_spark(&db, author, 1, "forum-like",
# &idem)`——点赞给作者 +1 魔力，会员流水页会直出英文键）；其余三条来自同一启发
# 式的相邻位置（ref_type 或邻近调用），**未逐条核实**，所以记成在册候选，
# 不冒充判据。硬判据是 DW1（真库出现过的 kind 必须有译名，集合级台账）。
KNOWN_CODE_UNLABELED = {"forum-like", "forum-post-del", "forum-topic-del",
                        "task_fee"}
new_code = unlabeled_code - KNOWN_CODE_UNLABELED
ok("DW2 代码里写进 spend/earn 的 kind 共 %d 个，无译名在册 %d 个，新增 %s"
   % (len(used), len(unlabeled_code), sorted(new_code) or "无"),
   not new_code,
   "引信只拦新违例：在册那批要逐条核实后才收进 DW1 的硬判据")
for k in sorted(unlabeled_code):
    print("      [候选] %s%s" % (k, " ← 已人工核实（点赞奖励 1 魔力，会员可读）"
                                 if k == "forum-like" else ""))

# ---------- DW3 渲染侧回落的是原键（所以缺译名会直出） ----------
hits = []
for root, _, files in os.walk(os.path.join(ROOTDIR, "apps", "web")):
    if "node_modules" in root or ".next" in root:
        continue
    for fn in files:
        if not fn.endswith((".tsx", ".ts")):
            continue
        t = io.open(os.path.join(root, fn), encoding="utf-8",
                    errors="replace").read()
        if re.search(r"kinds\[[^\]]+\]\s*\?\?\s*", t):
            hits.append(fn)
ok("DW3 界面用 kinds 映射渲染来源且**回落到原始键**（%d 个文件，如 %s）"
   % (len(hits), sorted(set(hits))[:2]),
   len(hits) >= 1,
   "找不到回落形状＝判据前提要重查（界面也许已经改成别的口径）")

# ---------- DW4 转账类 kind 必须成对且净额 0 ----------
for k in sorted(TRANSFER_KINDS):
    row = psql("SELECT count(*) FILTER (WHERE amount>0) || '/' || "
               "count(*) FILTER (WHERE amount<0) || '/' || "
               "coalesce(sum(amount),0) FROM spark_ledger WHERE kind='%s'" % k
               ).split("\n")[0].strip()
    up, dn, net = (row.split("/") + ["?", "?", "?"])[:3]
    ok("DW4 转账类 %s：入 %s 笔 / 出 %s 笔 / 净额 %s（必须成对且为 0）"
       % (k, up, dn, net), up == dn and net == "0",
       "入账多于扣款＝凭空增发（P-360 本体形状）")

# ---------- DW5 残料：不许有闸门自造的 kind 落在真会员账上 ----------
probe_rows = psql("SELECT u.id FROM spark_ledger l JOIN users u ON u.id = "
                  "l.user_id WHERE l.kind IN ('probe_cleanup','orphan_"
                  "offset') AND u.status = 2 GROUP BY u.id ORDER BY 1")
print("   [提示] 历史探针/对账 kind 落在在册正式会员 %s 个（删流水＝改账史，"
      "归站方裁；本闸门只负责让它可见）"
      % (len([x for x in probe_rows.split("\n") if x.strip()]) or 0))

summary()
