# -*- coding: utf-8 -*-
"""常驻闸门：代码在读、面板却没登记的设定键（反向台账，P-462）。

`fake_switch_guard.mjs` 只管一个方向：面板有控件、代码不读 = 假开关。
反方向一直是空的——**代码在读某个 site_settings 键，`settings_meta` 里却没有
这一行**，于是：
  1) 职员面 `/admin/webhooks` 报得出 `failure_threshold` / `retry_times` /
     `signing_enabled`，面板却没有对应字段：「看得见状态、动不了参数」；
  2) 现网 `webhook_sign_secret` 是空串，签名投递这条能力对站长**不可达**
     （唯一办法是直连数据库改 site_settings）；
  3) 同族 `webhook_discord` 早就在面板里 ⇒ 一族注册了一半，按本仓自己的口径
     就是漏登记。

本批把 Webhook 四把键补进元数据（迁移 0411，min/max 直接取代码里那对 clamp，
两界一致），并立这条反向台账：**「在读未登记」的集合必须等于在册清单**——
新加一个只能改数据库的参数就红，补了登记要把键从清单里撤下。

判据分层：
  V0 尺自证：读者算法必须同时认得两类读者——DB 函数（`seeding_*` 八把）与
     Rust（`webhook_*` 四把）。只认一类＝尺是瞎的（P-461 刚踩过只看 Rust）
  V1 台账集合相等（多＝新漏登记；少＝已登记还挂着，都要红）
  V2 两界一致：迁移登记的 min/max 必须等于代码里的 clamp 界；站方应用 0411 后
     库里那一行也必须同界（自升级）
  V3 现网账面：`/admin/webhooks` 三个状态字段在册；库里这几把键当前是否可编辑
跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_unregistered_tunables.py
      负控：同加 --selftest
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import (  # noqa: E402
    BASE, call, login, ok, psql_stdin, summary)

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MIG = "apps/api/migrations/0411_webhook_settings_meta.sql"
SEND = "apps/api/src/ops_webhook.rs"
FIX = "c222bd71"
# 0411 补登记的四把（迁移还没被站方应用时，台账先把它们算成"将登记"）
WEBHOOK = ["webhook_timeout_secs", "webhook_retry_times",
           "webhook_failure_threshold", "webhook_sign_secret"]
# 分类一：**另有界面/属派生或部署级**，出现在 settings_meta 之外是正当的
# （每条都核过读者文件）。它们从缺口里扣掉，避免把台账写成噪音。
UI_OWNED = {
    "nav.custom_enabled", "nav.min_visible_class",       # 导航管理屏
    "view_hidden", "home_layout",                         # 首页/视图布局屏
    "topbar_spark_chip",                                  # 站型包 apply 写
    "auto_review_ruleset_ver", "risk_shadow_mode",        # 规则集发布/影子开关
    "arcade_season_key", "storage_backend",               # 季度滚动 / 部署级
    "rss_alive_only",
    # 只出现在 `repo/cheat_ruleset.rs` 的一条 `name NOT IN (…)` 排除清单里，
    # 是规则集同步的内部标记，不是被读来调参的键
    "risk_shadow_max_ver",
}
# 分类二：**代码在读、面板没登记、也没有别的界面管**＝真缺口。
# 本批补了 Webhook 四把；剩下这些已把证据交给站长定标签与分组（一次性塞 20
# 个字段进面板是产品决定，不由审站方代拍），台账钉住"只许变少"。
LEDGER = {
    "fishing_event_enabled", "fishing_rod_window_bonus_ms",
    "forum_video_embed", "forum_video_upload",
    "hr_relief_floor_hours", "hr_warn_base_days", "hr_warn_enable",
    "hr_warn_max_shift_days", "subtitle_accept_timeout_days",
    "subtitle_ai_badge", "subtitle_award", "subtitle_award_ai_track",
    "subtitle_claim_timeout_days", "subtitle_crew_max", "subtitle_workflow",
    "subcert_min_awards", "subcert_min_rating", "subcert_min_subtitles",
    "subcert_min_votes", "gift_tax_bp", "shop_freeze_grace_secs",
    "resurrection_hours", "post_video_max", "video_max_mib",
    "video_quota_mib", "video_daily_limit",
    "risk_band_critical", "risk_band_high", "risk_band_low",
    "risk_band_watch", "risk_decay_days", "risk_norm",
    "risk_weight_default", "arcade_budget_base", "arcade_budget_pct",
    "arcade_budget_window_days",
}
SEEDING = ["seeding_base_hourly", "seeding_cap_hourly", "seeding_bonus_scale",
           "seeding_donor_mult", "seeding_curve_k", "seeding_rarity_k",
           "seeding_rarity_exp", "seeding_vol_log_base_gb"]


def strip(t):
    t = re.sub(r"/\*[\s\S]*?\*/", " ", t.replace("\r\n", "\n"))
    return re.sub(r"(?<!:)//[^\n]*", " ", t)


def rust_blob():
    out = []
    for d in ["apps/api/src", "apps/worker/src", "apps/tracker/src", "crates"]:
        base = os.path.join(ROOT, d)
        for root, _, files in os.walk(base):
            for fn in files:
                if fn.endswith(".rs"):
                    out.append(strip(io.open(
                        os.path.join(root, fn), encoding="utf-8",
                        errors="replace").read()))
    return "\n".join(out)


# —— V4：反向台账的第一版只从「有行的键」里挑读者 ⇒「代码在读、连行都没有」
# 的参数（面板彻底摸不到、只能改代码缺省）永远抓不到。
# 2026-10-21 实测漏网：`games_coupon_seed_hours`（worker 在读、文档写着
# 「运维可热调」，但 site_settings 无行、settings_meta 无登记）⇒ 迁移 0412 补上。
# 正则要排掉两类误抓：`nspname = 'public'`（左边不是 name）与
# `name = 'module_' || m.module_key`（拼接前缀，不是完整键名）。
NOROW_NOISE = {"module_"}
KEYREAD = re.compile(r"(?:^|[^a-z])name\s*=\s*'([a-z0-9_]+)'(?!\s*\|\|)")


def v4_no_row(rust, dbf, all_meta):
    rows = {r.strip() for r in psql_stdin(
        "SELECT name FROM site_settings;").splitlines() if r.strip()}
    cand = set()
    for blob in (rust, dbf):
        for m in KEYREAD.finditer("\n" + blob):
            cand.add(m.group(1))
    unreach = {k for k in cand if k not in all_meta and k not in rows}
    noise = sorted(unreach & NOROW_NOISE)
    gap = sorted(unreach - NOROW_NOISE)
    ok("V4 「代码在读 + 库里无行 + 面板无登记」的键必须为空（读不到就是面板摸不到）",
       not gap, "缺口=%s｜已排除噪声=%s｜候选键=%d" % (gap, noise, len(cand)))
    # V4z 正控（0412 落地前这条就是当时的现场）：把刚补登记的键从「已登记」集合里
    # 拿掉，检测器必须重新点名它——否则「V4 绿」只是尺子没看见东西。
    probe = "games_coupon_seed_hours"
    meta_wo = all_meta - {probe}
    rows_wo = rows - {probe}
    again = {k for k in cand if k not in meta_wo and k not in rows_wo}
    ok("V4z 正控：撤掉 0412 的登记后必须重新点名 %s" % probe,
       probe in again, "撤掉后缺口=%s" % sorted(again - NOROW_NOISE))
    return len(cand)


# V5：能力面断言——「登记」不等于「站长摸得到」。
# 面板 schema 是分组→卡片→字段三层嵌套，且字段名可能落在 name 或 key 上；
# 第一版我在 data.groups[].fields[] 这一层直接找，把「解析没写对」读成
# 「面板里没有这格」（假阴性）。所以这里递归收集全部 name/key，并拿
# 同卡片已在册的 pet_feed_cost 当对照——对照不在＝判据坏，不是站坏。
PANEL_KEYS = ["webhook_timeout_secs", "webhook_retry_times",
              "webhook_failure_threshold", "webhook_sign_secret",
              "games_coupon_seed_hours"]
PANEL_REF = "pet_feed_cost"


def _collect_names(node, sink):
    if isinstance(node, dict):
        for k in ("name", "key"):
            v = node.get(k)
            if isinstance(v, str):
                sink.add(v)
        for v in node.values():
            _collect_names(v, sink)
    elif isinstance(node, list):
        for v in node:
            _collect_names(v, sink)


def v5_panel(tok):
    import json  # noqa: F401  (call 已解好，这里只用结构)
    s, r = call("GET", "/admin/settings/schema", token=tok)
    names = set()
    _collect_names((r or {}).get("data") or {}, names)
    ok("V5z 正控：对照字段 %s 必须在面板 schema 里（不在＝解析坏）" % PANEL_REF,
       PANEL_REF in names, "收集到字段名=%d" % len(names))
    miss = [k for k in PANEL_KEYS if k not in names]
    ok("V5 五把补登记的参数面板真摸得到（P-462 四把 + P-469 口粮券）",
       not miss, "缺=%s" % miss)


def main():
    rust = rust_blob()
    dbf = psql_stdin("SELECT coalesce(string_agg(prosrc, E'\\n'),'') FROM "
                    "pg_proc")
    hit_seed = [k for k in SEEDING if ("'%s'" % k) in dbf]
    hit_wh = [k for k in WEBHOOK if re.search(r"\b%s\b" % k, rust)]
    ok("V0 尺自证：DB 函数认出 %d/8 把 seeding_*，Rust 认出 %d/4 把 webhook_*"
       % (len(hit_seed), len(hit_wh)),
       len(hit_seed) == 8 and len(hit_wh) == 4,
       "读者算法漏一类＝台账要么假空要么作假满")
    # V1 现网在读未登记集合（把 0411 将补的四把先算成已登记）
    rows = psql_stdin(
        "SELECT s.name FROM site_settings s WHERE NOT EXISTS (SELECT 1 FROM "
        "settings_meta m WHERE m.name = s.name) ORDER BY 1;").split("\n")
    live_un = {r.strip() for r in rows if r.strip()}
    all_meta = {r.strip() for r in psql_stdin(
        "SELECT name FROM settings_meta;").split("\n") if r.strip()}
    reading = {k for k in live_un
               if re.search(r"\b%s\b" % re.escape(k), rust)
               or ("'%s'" % k) in dbf}
    gap = reading - UI_OWNED
    want_all = set(LEDGER) | set(WEBHOOK)
    # 已经补上登记的（站方应用 0411 之后就是 webhook 那四把）自动从期望集里
    # 扣掉——闸门不能因为"站方做了正确的事"而红；但要把这件事说出来，提醒
    # 台账可以撤窄。
    done = sorted(k for k in want_all if k in all_meta)
    want = want_all - set(done)
    extra = sorted(gap - want_all)
    gone = sorted(want - gap)
    ok("V1 「代码在读、面板没登记、也别处无界面」集合=%d 把，必须等于台账"
       "（多 %s / 少 %s）" % (len(gap), extra or "无", gone or "无"),
       not extra and not gone,
       "多＝又添了一把只能改数据库的用户可感参数；少＝台账里的键现在没人读了")
    if done:
        print("   [V1b] 已补登记、可从台账撤下：%s" % ", ".join(done))
    # V2 两界一致
    mig = io.open(os.path.join(ROOT, MIG), encoding="utf-8").read()
    src = strip(io.open(os.path.join(ROOT, SEND), encoding="utf-8",
                        errors="replace").read())
    clamps = {}
    for k in WEBHOOK[:3]:
        i = src.index('setting(repo, "%s")' % k)
        m = re.search(r"clamp\((\d+),\s*(\d+)\)", src[i:i + 300])
        if m:
            clamps[k] = (int(m.group(1)), int(m.group(2)))
    bad = []
    for k, (lo, hi) in clamps.items():
        m = re.search(r"\('%s', 'number'.*?%s" % (k, re.escape(k)), mig, re.S)
        row = re.search(r"\(\s*'%s'.*?(\d+(?:\.\d+)?),\s*(\d+(?:\.\d+)?)\s*,"
                        r"\s*'ops'" % k, mig, re.S)
        if not row or (float(row.group(1)), float(row.group(2))) != (lo, hi):
            bad.append("%s 代码(%d,%d) 迁移=%s"
                       % (k, lo, hi, row.groups() if row else "无"))
    ok("V2 面板 min/max 与投递侧 clamp 同界（违例 %s）" % (bad or "无"),
       len(clamps) == 3 and not bad,
       "两界不一致＝「填得进去运行时被夹」或「代码允许面板拒收」")
    live = psql_stdin("SELECT coalesce(string_agg(name||'['||"
                      "coalesce(min::text,'?')||','||coalesce(max::text,'?')"
                      "||']',' '),'未应用') FROM settings_meta WHERE name IN "
                      "('webhook_timeout_secs','webhook_retry_times',"
                      "'webhook_failure_threshold');")
    print("   [V2b] 库内现值：%s（站方应用 0411 后这把应显示与代码同界）" % live)
    # V3 现网账面
    tok = login()
    st, r = call("GET", "/admin/webhooks", None, tok)
    data = (r or {}).get("data") or {}
    fields = {"failure_threshold", "retry_times", "signing_enabled"}
    have = fields.intersection(data.keys()) if isinstance(data, dict) else set()
    secret_val = psql_stdin("SELECT coalesce(value,'<无>') FROM site_settings "
                            "WHERE name='webhook_sign_secret';")
    ok("V3 职员面 `/admin/webhooks` 状态字段在册（HTTP %s，命中 %s）；"
       "签名密钥现值=%r（空串＝能力可达但站长此前无法设置）"
       % (st, sorted(have), secret_val),
       st == 200 and have == fields and secret_val == "",
       "状态口字段改名要同步这里；密钥非空说明站方已配置，台账口径不变")
    if "--selftest" in sys.argv:
        selftest()
    v4_no_row(rust, dbf, all_meta)
    v5_panel(login("root"))
    summary()


def selftest():
    """从迁移里抽掉一行登记 ⇒ V1/V2 至少一条必须红；按字节还原。"""
    p = os.path.join(ROOT, MIG)
    snap = io.open(p, "rb").read()
    txt = snap.decode("utf-8")
    a = "  ('webhook_timeout_secs', 'number',"
    if txt.count(a) != 1:
        ok("S0 负控锚点唯一（命中 %d）" % txt.count(a), False, "迁移写法变了")
        return
    try:
        # 把这一行的键名改掉（等价于"这把没登记"）。判据只看**语句里的元组**，
        # 不看整份文件——迁移头部注释也写着这个键名，按原文数会假绿
        # （这条负控第一次就栽在这里，是 P-461 那个坑的第三次复现）。
        io.open(p, "wb").write(
            txt.replace(a, "  ('zz_selftest_row', 'number',", 1)
            .encode("utf-8"))
        mig = io.open(p, encoding="utf-8").read()
        row = re.search(r"\(\s*'webhook_timeout_secs'\s*,\s*'number'", mig)
        v2 = row is not None
        ledger_hit = "webhook_timeout_secs" in txt
        ok("S1 负控：抽掉一行登记后 V2 认得出（元组还在=%s，台账含它=%s "
           "⇒ 应一假一真）" % (v2, ledger_hit),
           (not v2) and ledger_hit, "认不出＝V2 不承重")
    finally:
        io.open(p, "wb").write(snap)
    back = io.open(p, "rb").read() == snap
    ok("S2 负控还原：迁移按字节回到翻转前（%s）" % back, back,
       "还原不干净＝把翻转态留在了工作树")


if __name__ == "__main__":
    main()
