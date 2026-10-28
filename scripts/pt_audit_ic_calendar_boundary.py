# -*- coding: utf-8 -*-
"""常驻闸门 IC：日历界必须显式站点时区（UTC+8），除非登记在台账。

2026-10-27 现场：`invite_quota(period)` 这张表的周期键写的是
`date_trunc('week', now())::date`，而本库会话时区是 **UTC**（SHOW timezone=UTC），
站点周（娱乐周榜/经济大盘）一律 `AT TIME ZONE 'Asia/Shanghai'`。
后果：北京每周一 00:00–08:00 这两套周并行——周榜已经翻周，邀请周配额还算上周，
会员看到「本周已用完」而按站点周是新的一周；反向也在周日 00:00–08:00 提前翻周
（每周多 8 小时的早重置）。同族的日界事故本仓已收过三次（P-465 首页今日榜、
P-472 refundable 同日退两次、P-473 大盘日界），根因都是「拿会话时区切用户
看得见的日历」。

口径分工（别说第二遍）：
  IC1 尺自证（正控）：HEAD 的 invite_http 必须被这把尺抓到 ≥3 处裸周界；
  IC2 「写进库里的日历键 / 配额周期」裸界必须为 0——这条是硬线，不设棘轮；
  IC3 台账集合钉：剩下的裸界只许是 REGISTERED 那几支（银行计息内部自洽的
      UTC 日算），文件集合多一个少一个都红；
  IC4 前提承重：DB 会话时区仍是 UTC。哪天库/连接改了时区，台账里那些
      「UTC 日界」的语义会集体翻转，必须重算——所以把前提写进判据本身。

口径分工（与 DB《站点日界》别说第二遍）：
  DB（pt_audit_db_day_window.py）判**行为**——把窗口表达式喂给固定时刻做
    纯函数 A/B，并逐口验「今日/本周」的实际行集（DB12~DB16 那些）。
    它的台账（DB13）盯的是「裸 current_date 比较」这一族，看不见
    `date_trunc('week', now())` 这种带函数名的写法——invite_quota 就是这么
    在 DB 恒绿的情况下带着 UTC 周界跑了很久。
  IC（本闸门）判**形状**：全仓（api+worker+crates）扫裸日历界，
    ① 把周期键写进库的一律硬线为零；② 其余按文件集合钉台账；
    ③ 顺带把「本库会话时区=UTC」这个前提钉住（前提变了台账要集体重算）。
  两边共用同一个「对」的形状：`date_trunc(<unit>, x AT TIME ZONE
  'Asia/Shanghai') [AT TIME ZONE 'Asia/Shanghai']`。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ic_calendar_boundary.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BARE = re.compile(
    r"date_trunc\(\s*'(?:week|day|month)'\s*,\s*(?:now\(\)|CURRENT_"
    r"TIMESTAMP|CURRENT_DATE)\s*\)"
    r"|(?:now\(\)|CURRENT_TIMESTAMP)::date"
    r"|(?<![.\w])CURRENT_DATE(?!\s*-\s*interval)", re.I)
SHIFT = ("AT TIME ZONE", "interval '8 hours'")
# 台账：银行计息族按 UTC 日界自洽推进（`last_interest_date`/`due_at::date`
# 这些 date 列与 CURRENT_DATE 同一天花板，内部不打架）。改成站点日会移动
# 计息边界＝动钱，属站长决定，所以这里登记+报读数，不默许扩大。
# （checkin 的 `current_date - interval '30 days'` 只是日历条的取数窗口、
#   bank_jobs/mod.rs 的 run_date 已经写站点日，两支都不算裸界，不在台账。）
REGISTERED = {
    "apps/worker/src/bank_jobs/deposit.rs",
    "apps/worker/src/bank_jobs/loan.rs",
    "apps/api/src/economy_http/loan_repay.rs",
    "apps/api/src/economy_http/loans.rs",
    "apps/api/src/economy_http/loans_apply.rs",
    # 契约月是 UTC 自洽口径：签约 current_month()=UTC、worker 结算取上一
    # UTC 月、休眠豁免读本 UTC 月——三处同键同源，动用它会移动结算边界。
    "apps/worker/src/jobs/achv_dormant.rs",
}
PERIOD_KEY = re.compile(r"period\s*=\s*date_trunc\(\s*'week'", re.I)


def strip(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return "\n".join(re.sub(r"(^|\s)//.*", "", ln) for ln in text.split("\n"))


def scan(text):
    """返回 [(行号, 片段, 邻近有无位移)]——位移必须在同一条 SQL 里。"""
    out = []
    for m in BARE.finditer(text):
        seg = text[max(0, m.start() - 260): m.end() + 260]
        out.append((text[:m.start()].count("\n") + 1,
                    " ".join(m.group(0).split())[:46],
                    any(s in seg for s in SHIFT)))
    return out


def walk():
    """返回 (裸界分布, 其中把周期键写进库的文件集合)。"""
    hits = {}
    period_files = set()
    for sub in ("apps/api/src", "apps/worker/src", "crates"):
        for dirpath, _, files in os.walk(os.path.join(ROOT, sub)):
            if any(p in dirpath for p in ("target", "node_modules", ".next")):
                continue
            for f in files:
                if not f.endswith(".rs"):
                    continue
                p = os.path.join(dirpath, f)
                rel = os.path.relpath(p, ROOT).replace("\\", "/")
                raw = io.open(p, encoding="utf-8", errors="replace").read()
                real = [(ln, g) for ln, g, shifted in
                        scan(strip(raw)) if not shifted]
                if not real:
                    continue
                hits[rel] = real
                if PERIOD_KEY.search(raw):
                    period_files.add(rel)
    return hits, period_files


print("== IC1 正控：这把尺必须抓得到修复前的裸周界 ==")
old = subprocess.run(["git", "-C", ROOT, "show",
                      "HEAD:apps/api/src/invite_http.rs"],
                     capture_output=True, text=True, encoding="utf-8",
                     errors="replace")
if old.returncode != 0:
    print("!! git show 失败 rc=%s（128=文件不在 HEAD）" % old.returncode)
    sys.exit(2)
ctl = [x for x in scan(strip(old.stdout)) if not x[2]]
ok("IC1 正控：HEAD 的 invite_http 裸周界 >=3 处（实抓 %d）" % len(ctl),
   len(ctl) >= 3, "抓到=%s" % ctl[:3])

print("== IC2/IC3 现网工作树 ==")
hits, period_files = walk()
ok("IC2 没有任何「写进库里的日历键/周配额」用会话时区切（命中=%s）"
   % (sorted(period_files) or "无"), not period_files,
   "=%s" % {k: hits[k][:2] for k in sorted(period_files)})
extra = sorted(set(hits) - REGISTERED)
gone = sorted(REGISTERED - set(hits))
print("   裸界分布：%s" % {k: len(v) for k, v in sorted(hits.items())})
ok("IC3 裸界文件集合 = 台账（新增=%s 已消失=%s）" % (extra or "无",
                                                  gone or "无"),
   not extra and not gone,
   "台账外的新裸界要先按站点时区改写；台账内消失了要顺手删条目")

print("== IC4 前提：DB 会话时区 ==")
tz = psql_stdin("SHOW timezone;").strip() or "(库不可达)"
print("   [前提读数] SHOW timezone = %s" % tz)
ok("IC4 判据前提仍在：会话时区是 UTC（台账里那些 UTC 日界的语义靠它）",
   tz in ("UTC", "Etc/UTC") or tz == "(库不可达)",
   "若改成 +08，IC3 台账的语义集体翻转，必须重算")

summary()
