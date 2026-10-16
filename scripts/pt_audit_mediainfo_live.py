# -*- coding: utf-8 -*-
"""P-346 / P-347 常驻防复发闸门：MediaInfo 一致性判定必须真的在跑。

两个形状，同一个根因——写完就归档，没人调：

* **P-346 未接线**：`mediainfo_service::check_and_store` 全仓零调用点。MediaInfo 原样存下去
  就没人判过，`torrents.mediainfo_check` 恒为 NULL（实测：编辑口写入 392 字符
  后该列仍是 NULL）。站方提交说的“机器抓标题党”实际一台都没跑。
* **P-347 判定器漏判**：扫描器只认 `Stream #0:0: Video: ...` 一套流头，不认
  现代版的 `Video` / `ID : Video 1` 段落格式 → 分辨率与编码全抽空。屏幕上的
  「假 4K」恰好从悄悄地漏过（实测：两套输出格式实测分辨率均为 None）。

本闸门分两层：S0 模拟代码形状对账（不启动服务）；
S1 活体 HTTP 证据（需真进程 + 库里造样本）。
"""

import io
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# = S0 代码形状对账（不需服务）====

# 真实 MediaInfo 现代格式样本（就是用户会粘进来的那种）
MODERN_MI = """General
Complete name                            : /home/admin/secret/user_x264_show

Video
ID                                       : Video 1
Format                                   : AVC
Width                                    : 1 920 pixels
Height                                   : 1 080 pixels
Color space                              : YUV
"""

src = io.open(
    os.path.join(REPO, "apps/api/src/domain/mediainfo/mod.rs"),
    encoding="utf-8").read()

# S0a 判定器必须认第二套输出格式
has_helper = "fn stream_kind_of" in src
has_modern = "bare == \"video\"" in src or "bare == \"video\"" in src
ok("S0a 判定器必须认 MediaInfo 现代段落格式（`Video` / `ID : Video 1`）"
   "，否则「假 4K」从缐缐地漏过：helper=%s" % has_helper,
   has_helper and has_modern)

# S0b 千分位数字（`1 920 pixels`）必须能解析
has_num = "fn parse_num" in src
num_impl = "is_ascii_digit" in src
ok("S0b 千分位空格 + 单位后缀必须能解析"
   "（parse_num=%s）", has_num and num_impl)

# S0c 旧的单一流头闸门不得留下
leftover = src.count("eq_ignore_ascii_case(\"stream \")")
ok("S0c 扫描器不得存在「只认 Stream #0 流头」的旧闸门"
   "（当前 %d 处）" % leftover, leftover == 0)

# S0d 接线：check_and_store 必须有调用点
total_callers = 0
caller_files = []
for root, _d, fs in os.walk(os.path.join(REPO, "apps")):
    if os.sep + "target" in root:
        continue
    for fn in fs:
        if not fn.endswith(".rs"):
            continue
        p = os.path.join(root, fn)
        if fn == "mediainfo_service.rs":
            continue
        t = io.open(p, encoding="utf-8", errors="replace").read()
        n = t.count("check_and_store")
        if n:
            total_callers += n
            caller_files.append(os.path.basename(p))
ok("S0d 判定器必须有调用点（check_and_store 被调 %d 次，"
   "文件 %s）——零调用点就是「一台机器都没跑」"
   % (total_callers, caller_files or "无"),
   total_callers > 0)

# S0e 巡检口必须已注册
# 路由 service 注册在 **admin_http/mod.rs** 自己的 service 链里（模块本身归
# 管道统一注册），不在 http/mod.rs。上一版判据反了这个文件，
# 自检时立刻拿出 S0e 假红——新闸门必须跑自检。
admin_mod = io.open(os.path.join(REPO, "apps/api/src/admin_http/mod.rs"),
                    encoding="utf-8").read()
has_service = ".service(mediainfo_mismatches)" in admin_mod
has_mod = "mod mediainfo_audit;" in admin_mod
ok("S0e 巡检口必须注册（判定落库后无人可见 → 白存）"
   "：service=%s mod=%s" % (has_service, has_mod),
   has_service and has_mod)

# = S1 活体证据（需真进程）====

ROOT = None
try:
    ROOT = login()
except Exception as e:
    print("   SKIP S1：登录失败（%s）——本次没跑活体验证"
      % str(e)[:60])

if ROOT:
    def psql(q):
        r = subprocess.run(
            ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
             "-d", "fluxtorrent", "-tAc", q],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            universal_newlines=True, encoding="utf-8")
        if r.returncode != 0:
            print("   psql 失败：%s" % (r.stderr or "").strip()[:70])
        return (r.stdout or "").strip()

    # 找一个本进程能编辑的种子作为承载
    tid = psql("SELECT id FROM torrents WHERE id=40049") or "40049"

    # S1a 真「假 4K」：标题写 2160p，内容是 1080p
    st, _b = call("PUT", "/torrents/%s" % tid,
                  {"name": "P346.Probe.2160p.WEB-DL.x265-GRP",
                   "mediainfo": MODERN_MI}, token=ROOT)
    raw = psql("SELECT COALESCE(mediainfo_check::text, 'NULL') "
               "FROM torrents WHERE id=%s" % tid)
    caught = False
    detail = raw[:40]
    if raw and raw != "NULL":
        try:
            d = json.loads(raw)
            fields = [m.get("field") for m in d.get("mismatches", [])]
            caught = "resolution" in fields
            detail = "分辨率=%s 编码=%s 不一致=%s" % (
                d["summary"].get("resolution"), d["summary"].get("video_codec"),
                fields)
        except Exception:
            pass
    ok("S1a 真「假 4K」（现代 MediaInfo 格式）必须被拦住："
       "http=%s 实测分辨率=%s" % (st, detail),
       st == 200 and caught, detail)

    # S1b 真 4K 对照：不得误报（分辨率不得报不一致）
    real4k = (MODERN_MI.replace("1 920 pixels", "3 840 pixels")
              .replace("1 080 pixels", "2 160 pixels")
              .replace("Format                                   : AVC",
                       "Format                                   : HEVC"))
    st, _b = call("PUT", "/torrents/%s" % tid,
                  {"name": "P346.Probe.2160p.WEB-DL.x265-GRP",
                   "mediainfo": real4k}, token=ROOT)
    raw = psql("SELECT COALESCE(mediainfo_check::text, 'NULL') "
               "FROM torrents WHERE id=%s" % tid)
    ok("S1b 真 4K 对照组：分辨率不得报错"
       "（误报会把合法片源指成标题党）",
       st == 200 and raw != "NULL" and "resolution" not in [
           m.get("field")
           for m in json.loads(raw).get("mismatches", [])],
       "分辨率不一致项数=%d" % len([
           m for m in json.loads(raw).get("mismatches", [])
           if m.get("field") == "resolution"]) if raw != "NULL" else raw[:30])

    # S1c 巡检口必须能读出来（判定落库不等于看得见）
    st, b = call("GET", "/admin/mediainfo/mismatches?limit=5", None, token=ROOT)
    dd = (b or {}).get("data") or {}
    rows = dd.get("mismatches") or []
    ok("S1c 巡检口必须能读出来（http=%s，条数=%d）"
       "——其中中文注释不得乱码" % (st, len(rows)),
       st == 200 and isinstance(rows, list)
       and "提示" in str(dd.get("note", "")),
       "note=%s" % str(dd.get("note", ""))[:40])

summary()
