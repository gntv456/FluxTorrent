"""常驻闸门：spark_ledger 的 kind 必须三语都有中文/对应语言标签。

用户视角：「我的火花流水」类型列写的是 `kindLabels[r.kind] ?? r.kind`
（apps/web/app/(main)/me/sparks/page.tsx）——没登记过的 kind 就把
`forum-post-del`、`bank_loan_repay_partial` 这种裸键直接甩到会员面前，
外语站同理。

尺只有一份：`scripts/audit_kind_labels.py`（按**实参位**取 kind：
余额原语第 4 参的字面量；再并上现网 distinct(kind)——直插 spark_ledger
的写入口只能靠现网这把尺兜住）。本文件只负责判与自测，不另抄一份解析。

断言：
  K1 三语（zh-CN / en / zh-TW）对 kind 全集的缺口都为 0
  K2 正控·现网侧：只由直插 SQL 写出、不走原语的 kind（social_team_reward）
     必须出现在全集里——否则说明尺看不见直插写入口，K1 就是假绿
  K3 正控·只在代码里、现网还没写过的 kind（task_fee）必须在全集里
自测：--selftest 从 en.ts 抹掉一个键 ⇒ K1 必须红；还原后必须绿。

与既有闸门 DW（scripts/pt_audit_dw_kind_label.py）的分工，别说第二遍：
DW 判「现网出现过的 kind 在 zh-CN 有译名」，台账集合钉；本尺判**三语 × 全集**
（含还没上线的原语 kind），补的是 DW 没覆盖的两件事——外语标签、上线前的引信。
两边共用同一个前提：渲染侧回落到原始键。DW 补译名后要同步收缩它的台账，
本批已把 orphan_offset / probe_cleanup / social_team_reward 三个键登记进三语，
所以 DW1 期望的缺译名集合应为空。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from audit_kind_labels import code_kinds, db_kinds, dict_keys  # noqa: E402
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LANGS = ("zh-CN", "en", "zh-TW")
# 两把正控：一个只能靠现网看到，一个只能靠代码看到
PROBE_DB = "social_team_reward"
PROBE_CODE = "task_fee"


def universe():
    lit, _skipped, _ins = code_kinds()
    dbk = db_kinds()
    return lit, dbk, (lit | dbk)


def main():
    lit, dbk, uni = universe()
    print("kind 全集 %d（原语 %d + 现网 %d）" % (len(uni), len(lit), len(dbk)))
    gaps = {}
    for lang in LANGS:
        miss = sorted(uni - dict_keys(lang))
        if miss:
            gaps[lang] = miss
    ok("K1 三语 kind 标签无缺口", not gaps,
       "; ".join("%s 缺 %s" % (k, v) for k, v in gaps.items()))
    ok("K2 正控：直插 SQL 的 kind 也被现网尺看到（%s）" % PROBE_DB,
       PROBE_DB in dbk and PROBE_DB in uni,
       "在现网=%s 在全集=%s" % (PROBE_DB in dbk, PROBE_DB in uni))
    ok("K3 正控：只在代码里的 kind 也被原语尺看到（%s）" % PROBE_CODE,
       PROBE_CODE in lit, "在字面量集=%s" % (PROBE_CODE in lit))
    summary()


def selftest():
    p = os.path.join(ROOT, "apps", "web", "i18n", "en.ts")
    src = open(p, encoding="utf-8").read()
    victim = '      refund: "Refund",'
    if src.count(victim) != 1:
        raise SystemExit("自测锚点不唯一：%d" % src.count(victim))
    open(p, "w", encoding="utf-8", newline="").write(src.replace(victim, ""))
    cmd = [sys.executable, os.path.abspath(__file__)]
    try:
        flipped = subprocess.run(cmd, capture_output=True, text=True,
                                 encoding="utf-8", errors="replace")
        print("[自测] 抹掉 en 的 refund 后 rc=%s（必须非 0）"
              % flipped.returncode)
        for l in [x for x in (flipped.stdout or "").splitlines()
                  if "K1" in x][:2]:
            print("   ", l.strip()[:200])
    finally:
        open(p, "w", encoding="utf-8", newline="").write(src)
    back = subprocess.run(cmd, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")
    print("[自测] 还原后 rc=%s（必须 0）" % back.returncode)
    return 0 if (flipped.returncode != 0 and back.returncode == 0) else 2


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
