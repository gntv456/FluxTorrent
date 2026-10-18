# -*- coding: utf-8 -*-
"""闸门 DF：站点设置三面一致性（代码读得到 / site_settings 有行 / 面板有这一项）。

批一百五十八普查出的现状：代码从 `site_settings` 读的 117 个键里，有 16 个
**不在 `settings_meta`**（通用设置面板由 `site_settings JOIN settings_meta` 生成，
见 settings_http/meta.rs 的 META_SELECT），其中只有 2 个有专用写口。
剩下 13 个只能靠 SQL 改——有 3 个的代码注释还写着「可配置/后台改」，
这就是「界面那份被改漏」的同族。要不要给它们长出面板控件是**产品决定**
（每项要中文名、类型、取值区间、三语文案），所以本闸门不擅自加字段，
只做三件事：把清单钉死、把口径本身验住、把待裁项逐条打 WARN 报读数。

分类口径（改任何一项都要在这里同步，否则 DF1 红）：
  needs-ui      代码在读、只能 SQL 改 ⇒ 待站方拍板是否上面板
  infra-sql     基础设施级（换后端），按设计就该走配置/迁移
  dedicated-ui  面板没有这一项，但有专用接口在写（不是断链）

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_df_setting_keys.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEDGER = {
    "gift_tax_bp": "needs-ui",
    "subtitle_accept_timeout_days": "needs-ui",
    "subtitle_workflow": "needs-ui",
    "subtitle_award": "needs-ui",
    "subtitle_award_ai_track": "needs-ui",
    "hr_warn_enable": "needs-ui",
    "forum_video_embed": "needs-ui",
    "post_video_max": "needs-ui",
    "resurrection_hours": "needs-ui",
    "shop_freeze_grace_secs": "needs-ui",
    "preserve_bonus_per_day": "needs-ui",
    "games_coupon_seed_hours": "needs-ui",
    "arcade_season_key": "needs-ui",
    "storage_backend": "infra-sql",
    "risk_shadow_mode": "dedicated-ui",
    "view_hidden": "dedicated-ui",
}
# 台账里说「有专用写口」的两把键：写侧不在 site_settings 的字面 SQL 附近，
# 而是各自域的保存接口（读码确认），闸门用 DF2b 断言这两个文件仍引用该键
DEDICATED = {"risk_shadow_mode": "apps/api/src/repo/cheat_ruleset.rs",
             "view_hidden": "apps/api/src/http/view_layout.rs"}
# 正则会从 `name LIKE 'module_%'` 里抓出半个键名，不是真键
ARTIFACTS = {"module_"}
# 面板已有的键：判据必须认得出它们**不是**孤儿（上一轮我把
# attachment_authority 误报成孤儿键，就是因为只 grep 代码没查登记）
PANEL_OK = ("attachment_authority", "attach_max_mib", "attach_quota_mib",
            "enableattach")


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:170])
    return (r.stdout or "").strip()


def rows(q):
    v = psql(q)
    return [x for x in v.split("\n") if x.strip()]


CODE_RX = re.compile(r"site_settings[^']{0,160}?'([a-z0-9_]{3,40})'", re.S)
WRITERS = {}
READERS = {}
code_keys = set()
for base in ("apps/api/src", "apps/worker/src", "apps/tracker/src"):
    for dp, _dn, fn in os.walk(os.path.join(ROOTDIR, base)):
        for f in fn:
            if not f.endswith(".rs"):
                continue
            rel = os.path.relpath(os.path.join(dp, f), ROOTDIR) \
                .replace("\\", "/")
            txt = io.open(os.path.join(dp, f), encoding="utf-8",
                          errors="replace").read()
            for m in CODE_RX.finditer(txt):
                k = m.group(1)
                code_keys.add(k)
                READERS.setdefault(k, set()).add(rel)
                seg = txt[max(0, m.start() - 320):m.start()]
                if "INSERT INTO site_settings" in seg or \
                        "UPDATE site_settings" in seg or "set_setting" in seg:
                    WRITERS.setdefault(k, set()).add(rel)

meta = set(rows("SELECT name FROM settings_meta"))
cand = {k for k in code_keys if k not in meta} - ARTIFACTS
extra = sorted(cand - set(LEDGER))
stale = sorted(set(LEDGER) - cand)
print("   代码读到 %d 个键；面板登记 %d 个；候选（面板没有这一项）%d 个"
      % (len(code_keys), len(meta), len(cand)))
for k in sorted(cand):
    w = ", ".join(sorted(WRITERS.get(k, set()))[:2]) or DEDICATED.get(k, "")
    print("      %-28s %-12s 写口=%s"
          % (k, LEDGER.get(k, "未分类"), w or "无（只能 SQL 改）"))
ok("DF1 候选集与台账一致（新增未分类 %d、已消失 %d）" % (len(extra), len(stale)),
   not extra and not stale,
   "新增=%s｜已消失=%s" % ("、".join(extra), "、".join(stale)))

# ---------- DF2 正控：面板已登记的键绝不该被判成孤儿 ----------
false_pos = [k for k in PANEL_OK if k in cand or k not in meta]
ok("DF2 正控：面板已有的键不许进候选（误报 %d：%s）"
   % (len(false_pos), "、".join(false_pos)), not false_pos)
dead_ui = []
for k, f in DEDICATED.items():
    if k not in cand:
        continue
    src = io.open(os.path.join(ROOTDIR, f), encoding="utf-8",
                  errors="replace").read()
    if ("'%s'" % k) not in src:
        dead_ui.append(k)
ok("DF2b 声称「有专用写口」的键，那个文件确实还引用它（失真 %d）"
   % len(dead_ui), not dead_ui, "、".join(dead_ui))

# ---------- DF3 面板可见性口径本身：两面都要有行 ----------
meta_only = rows("SELECT m.name FROM settings_meta m WHERE NOT EXISTS "
                 "(SELECT 1 FROM site_settings s WHERE s.name = m.name)")
n_meta_only = len(meta_only)
ok("DF3 有登记却无取值的键 = 0（实测 %d）⇒ 面板是 JOIN 生成的，"
   "少一行就整项消失" % n_meta_only, n_meta_only == 0, meta_only[:120])

# ---------- DF4 活体：schema 口的字段集合就是面板真相 ----------
tok = login()
st, body = call("GET", "/admin/settings/schema", token=tok)
fields = []
for grp in ((body or {}).get("data") or {}).get("groups") or []:
    for card in grp.get("cards") or []:
        fields += [f.get("name") for f in card.get("fields") or []]
fs = set(fields)
print("   /admin/settings/schema http=%s 字段 %d 个" % (st, len(fs)))
ok("DF4a 面板真的长出 attachment_authority（判据的对照键）",
   "attachment_authority" in fs, sorted(fs)[:5])
in_panel = sorted(k for k in cand if k in fs)
ok("DF4b 候选键确实一个都不在面板里（实测混进 %d：%s）"
   % (len(in_panel), "、".join(in_panel)), not in_panel)

# ---------- DF5 待裁清单：逐条报现值与读侧，供站方拍板 ----------
needs = sorted(k for k, v in LEDGER.items() if v == "needs-ui" and k in cand)
print("\n   待裁（代码在读、只能 SQL 改，是否给面板加控件请站方定）：")
for k in needs:
    val = psql("SELECT coalesce(value,'<无行>') FROM site_settings "
               "WHERE name = '%s'" % k)
    print("      %-30s 现值=%-14s 读侧 %d 个文件"
          % (k, val[:14], len(READERS.get(k, set()))))
ok("DF5 待裁清单与台账同源（%d 项，逐条 WARN 报读数）" % len(needs),
   len(needs) >= 10, "、".join(needs))

summary()
