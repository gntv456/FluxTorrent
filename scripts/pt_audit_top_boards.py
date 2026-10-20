# -*- coding: utf-8 -*-
"""常驻闸门：排行榜「后宫时魔」必须等于结算实发，不许自己复刻算式。

会员视角：打开 /top，第六张卡片写着「后宫时魔 / 每小时」，榜主是按
「基础 10 + 做种颗数×2 + 做种体积TB，捐赠×2」排出来的——那是 **0133 之前**
的做种收益算式。结算本体早就换成 DB 函数：

    seeding_hourly( Σ seeding_torrent_bonus(…), base, cap, curve_k,
                    donor_mult, donor )

参数在 site_settings（实测现值 base=10 cap=150 curve_k=0.12 vol_base=50
rarity_k=0.6 rarity_exp=0.35 scale=0.4 donor_mult=2），后台可热调。
两套式子并存的后果不是「数字差一点」：榜按颗数×体积排序，恰好把 0133 立档
要堵的「堆小种」排到前面（`apps/worker/src/jobs/seeding.rs` 顶部注释原文：
「旧公式 1000 颗 1MB 能拿 ~205/h，比保 6 颗濒危大种还高」）。
排行榜是站方自己对会员发布的口径，它替套利做了广告。

修法按 seeding.rs 自己那句「权威在流水，快照仅展示」：
榜值取 `spark_ledger` 里每人**最近一条 kind='seeding_reward' 的实发额**，
公式只有一份，永远在 DB 函数里。

判据（正控都在判据本体上，不靠额外用例）：
  T1 源码里不许再出现复刻算式的特征串，且必须读 seeding_reward
  T2 worker 侧那笔流水的 kind 字面量还在（榜的依赖前提）
  T3 现网 SQL 基线非空——堵「夜里没做种 ⇒ 两边都空 ⇒ 假绿」
  T4 GET /top/boards 的 hourly 逐行等于 SQL 回读（id 与数值都对上）
  T5 榜上不许有被处置号（status>=2）
  T6 三个 DB 函数仍在册（哪天改名，榜与结算一起红，不留单侧）
  T7 发种量榜不许把匿名种记到人头上（与主页 uploads 同一个口径）
  T8 发种量榜逐行等于「非匿名已审核种数」回读
自测：--selftest 两处负控——hourly 退回复刻算式 ⇒ T1 红；
      torrents 去掉匿名过滤 ⇒ T7 红；还原后必须绿（只动文本，不碰库）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_top_boards.py
      FLUX_API_BASE 默认 http://127.0.0.1:8080/api/v1（侧进程用 8099）
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import (  # noqa: E402
    BASE, call, login, ok, psql_stdin, summary,
)

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MISC = os.path.join(ROOT, "apps", "api", "src", "content_http", "misc.rs")
SEED = os.path.join(ROOT, "apps", "worker", "src", "jobs", "seeding.rs")
KIND = "seeding_reward"
# 复刻算式的三块骨头：体积 TB 常量、颗数×2、donor 三目
LEGACY = ["1099511627776", "count(*) * 2", "donor_privileged(u.id)"]
# 每人最近一笔实发（榜与判据共用同一条口径 SQL）
BASE_SQL = (
    "SELECT g.user_id, g.amount FROM spark_ledger g "
    "WHERE g.kind = '%s' AND g.id = ("
    "SELECT MAX(g2.id) FROM spark_ledger g2 "
    "WHERE g2.user_id = g.user_id AND g2.kind = '%s') "
    "AND g.amount > 0 AND EXISTS (SELECT 1 FROM users u "
    "WHERE u.id = g.user_id AND u.status < 2) "
    "ORDER BY g.amount DESC, g.user_id LIMIT 10" % (KIND, KIND)
)


def strip_rows(path):
    """按行剥 // 注释（整行与行尾），不碰字符串内容。

    静态判据必须先剥注释：本批的修复说明里就写着旧算式，
    不剥注释会把「我已经改好的形状」当成待改项，也会让旧串躲过检查。
    """
    out = []
    for line in io.open(path, encoding="utf-8", errors="replace"):
        s = re.sub(r"//.*$", "", line.rstrip("\n"))
        out.append(s)
    return "\n".join(out)


def truth_rows():
    txt = psql_stdin(BASE_SQL).strip()
    rows = []
    for ln in txt.splitlines():
        ln = ln.strip()
        if not ln or "|" not in ln:
            continue
        a, b = ln.split("|", 1)
        rows.append((int(a.strip()), float(b.strip())))
    return rows


def main():
    code = strip_rows(MISC)
    # ---------- T1 源码侧不许复刻算式 ----------
    hit = [t for t in LEGACY if t in code]
    reads_ledger = "kind = '%s'" % KIND in code
    ok("T1 榜单 SQL 读 %s 流水、且不含复刻算式（特征串命中 %s）"
       % (KIND, hit or "无"), reads_ledger and not hit,
       "命中旧算式＝榜又变成第二份公式，与结算必然分叉")
    # ---------- T2 依赖前提：worker 仍写这个 kind ----------
    seed_code = strip_rows(SEED)
    ok("T2 worker 结算仍把这笔写成 kind='%s'" % KIND,
       "'%s'" % KIND in seed_code,
       "kind 改了名，榜就恒空——两边必须一起改，本条先红")
    # ---------- T3 现网基线非空（否则 T4 是空对空） ----------
    truth = truth_rows()
    ok("T3 现网按 %s 回读到基线 %d 行（必须 >0，否则判据不承重）"
       % (KIND, len(truth)), len(truth) > 0,
       "一条历史实发都没有＝口径探针失效，先查流水再谈榜")
    # ---------- T4 接口逐行等于回读 ----------
    token = login()
    st, resp = call("GET", "/top/boards", token=token)
    ok("T4 GET /top/boards 200（BASE=%s）" % BASE, st == 200,
       "http=%s %s" % (st, str(resp)[:160]))
    boards = (resp or {}).get("data") or {}
    hourly = boards.get("hourly") or []
    got = [(int(r["id"]), float(r["val"])) for r in hourly]
    same_ids = [x[0] for x in got] == [x[0] for x in truth]
    same_vals = all(abs(a[1] - b[1]) < 1e-9 for a, b in zip(got, truth))
    ok("T4b hourly 榜 %d 行与回读 %d 行：名次与数值完全一致"
       % (len(got), len(truth)),
       same_ids and same_vals,
       "接口=%s 回读=%s" % (got[:4], truth[:4]))
    # ---------- T5 榜上都是在册号 ----------
    ids = [x[0] for x in got] or [x[0] for x in truth]
    listed = psql_stdin(
        "SELECT count(*) FROM users WHERE id IN (%s) AND status >= 2"
        % ",".join(str(i) for i in ids)).strip().splitlines()[0].strip()
    ok("T5 榜上被处置号（status>=2）%s 个（必须 0）" % listed,
       listed == "0", "回读=%s" % listed)
    # ---------- T6 结算本体的三个函数仍在册 ----------
    fn = psql_stdin(
        "SELECT string_agg(proname, ',') FROM pg_proc p "
        "JOIN pg_namespace n ON n.oid = p.pronamespace "
        "WHERE n.nspname = 'public' AND p.proname IN "
        "('seeding_hourly','seeding_torrent_bonus','seeding_params')").strip()
    got_fns = set(x.strip() for x in fn.split(",") if x.strip())
    need = {"seeding_hourly", "seeding_torrent_bonus", "seeding_params"}
    ok("T6 结算公式本体（DB 函数）在册 %d/3（缺 %s）"
       % (len(got_fns & need), sorted(need - got_fns) or "无"),
       got_fns >= need,
       "公式本体没了＝榜的依赖与 worker 的结算同时失联，先定位再谈判据")
    # ---------- T7 发种量榜不许把匿名种记到人头上 ----------
    # 全站对 torrents.anonymous 的处理是一律隐去发布者：详情页/列表/RSS/保种页
    # 都写 CASE WHEN t.anonymous THEN NULL ELSE u.username END，
    # 本人主页的 uploads 与 /users/{id}/torrentlist 都带 NOT t.anonymous。
    # 榜若少了这一条，「匿名发一颗种」的人主页写着「发布 0 种」，
    # 却以真名挂在全站排行榜上（2026-10-21 事务内 A/B：同一颗种翻成匿名后，
    # 旧口径榜上仍有此人 1 行、新口径 0 行）。
    ok("T7 发种量榜的 JOIN 带 NOT t.anonymous（匿名种不记名）",
       "AND NOT t.anonymous" in code and "t.approval_status = 1" in code,
       "漏掉这行＝排行榜替站方把匿名性卖了")
    # ---------- T8 发种量榜逐行等于「非匿名已审核种数」回读 ----------
    tb = boards.get("torrents") or []
    trows = [(int(r["id"]), int(r["val"])) for r in tb]
    back = psql_stdin(
        "SELECT x.uid, x.c FROM (SELECT t.owner_id AS uid, count(*) AS c "
        "FROM torrents t JOIN users u ON u.id = t.owner_id AND u.status < 2 "
        "WHERE t.approval_status = 1 AND NOT t.anonymous "
        "GROUP BY t.owner_id ORDER BY c DESC, t.owner_id LIMIT 10) x"
    ).splitlines()
    want = [(int(a.strip()), int(b.strip()))
            for a, b in (ln.split("|") for ln in back if "|" in ln)]
    ok("T8 torrents 榜 %d 行与「非匿名已审核种数」回读 %d 行一致"
       % (len(trows), len(want)), trows == want,
       "接口=%s 回读=%s" % (trows[:4], want[:4]))
    summary()


def _flip_and_run():
    """跑一遍本闸门，返回 (rc, FAIL 行)。调用方负责还原源码。"""
    cmd = [sys.executable, os.path.abspath(__file__)]
    r = subprocess.run(cmd, capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    return r.returncode, [x for x in (r.stdout or "").splitlines()
                          if "FAIL" in x][:3]


def selftest():
    """两处负控：hourly 退回复刻算式 ⇒ T1 红；torrents 去掉匿名过滤 ⇒ T7 红。
    还原后必须绿。只改文本，不碰库、不重跑结算。"""
    src = io.open(MISC, encoding="utf-8").read()
    plan = [
        ("T1", "FROM spark_ledger g WHERE g.kind = 'seeding_reward' ",
         "FROM snatches s JOIN torrents t ON t.id = s.torrent_id "
         "WHERE 1099511627776 > 0 AND count(*) * 2 > 0 "
         "AND donor_privileged(u.id) "),
        ("T7", "AND t.approval_status = 1 AND NOT t.anonymous",
         "AND t.approval_status = 1"),
    ]
    bad = 0
    for tag, anchor, fake in plan:
        if src.count(anchor) != 1:
            raise SystemExit("自测锚点 %s 不唯一：%d" % (tag, src.count(anchor)))
        flipped = src.replace(anchor, fake)
        with io.open(MISC, "w", encoding="utf-8", newline="") as f:
            f.write(flipped)
        try:
            rc, fails = _flip_and_run()
        finally:
            with io.open(MISC, "w", encoding="utf-8", newline="") as f:
                f.write(src)
        print("[自测] %s 翻转后 rc=%s（必须非 0）" % (tag, rc))
        for l in fails:
            print("   ", l.strip()[:180])
        if rc == 0:
            bad += 1
    back, _ = _flip_and_run()
    print("[自测] 全部还原后 rc=%s（必须 0）" % back)
    return 0 if (not bad and back == 0) else 2


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
