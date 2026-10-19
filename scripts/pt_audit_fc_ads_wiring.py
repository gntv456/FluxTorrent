# -*- coding: utf-8 -*-
"""闸门 FC：广告位（ads）的「后台齐全、前台没人读」与接线前置。

批一百八十六（会员/站长双视角）。站长在「站点工具 - 广告位」建一条头幅广告，
勾了启用、排好顺序、写了 HTML——**前台永远不会出现它**。

  P-375（本批查实，属未接线而非可代改的代码缺陷）：`ads` 表只有
  `staff_http/site.rs` 的后台 CRUD 在读（列表/新增/改/开关/删），
  全站**没有公开读口**（无 `GET /ads`、无 `FROM ads` 的会员面查询），
  前端也**没有任何渲染点**（`dangerouslySetInnerHTML` 的 8 处里没有 ads）。
  面板却给了 position（header/footer/sidebar）、sort 与 enabled 开关，
  这套控件暗示「配了就会上首页」，与实际相反。
  接线属新功能（要定位置、样式、CSP 与出站消毒口径），且必须走
  `home_news::html_clean` 那把尺（P-341 的教训：ads.html 是原样存的），
  因此本批不代改，只把它钉成判据，避免「以后有人加了读口就以为接好了」。

  在册一处小缺陷（P-376，本批登记未改，附原因）：`ad_delete` 不判
  `rows_affected` ⇒ 删不存在的 id 也回 200，并落一条「ad.delete」审计
  （审计说删了、库里本来没有 = 账实不符）。同文件的 update/toggle 都判了
  0 行回 404，是这一族里被改漏的那一份；`site.rs` 行数基线 326 恰好等于
  当前行数（只许瘦不许胖），补 4 行判定要先在别处腾 4 行 ⇒ 需拆文件，
  不挤在本批做，登记在此防遗忘。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_fc_ads_wiring.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
APISRC = os.path.join(ROOTDIR, "apps", "api", "src")
WEBSRC = os.path.join(ROOTDIR, "apps", "web")
SITE = os.path.join(APISRC, "staff_http", "site.rs")
# 在册：ads 后台里未判 rows_affected 的写口数（只降不升；修掉请同步收小）
DELETE_ROWS_CEIL = 1


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-q", "-d", "fluxtorrent", "-tA", "-c", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:200])
    return (r.stdout or "").strip()


site = cut(SITE)

# ---------- FC1 后台三口的护栏齐不齐（正例钉死） ----------
i_del = site.find("async fn ad_delete")
delfn = site[i_del:site.find("async fn", i_del + 10)] if i_del >= 0 else ""
ok("FC1 广告后台三口都要求 ADS_MANAGE 且都落审计（命中 %d/%d）"
   % (site.count("perm::ADS_MANAGE"), site.count('audit(Some(auth.id), "ad.')),
   site.count("perm::ADS_MANAGE") >= 3
   and site.count('audit(Some(auth.id), "ad.') >= 3,
   "后台写口掉权限位或掉审计＝谁改了站点广告无人可查")
n_ok = len(re.findall(r"rows_affected\(\)", delfn))
ok("FC2 `ad_delete` 判 rows_affected（在册 %d 处待修，须 ≤ %d）"
   % (max(DELETE_ROWS_CEIL - n_ok, 0), DELETE_ROWS_CEIL),
   n_ok <= DELETE_ROWS_CEIL,
   "不判就是「删不存在的 id 也回 200 + 落一条已删除审计」；"
   "同文件 update/toggle 都判了 0 行回 404")

# ---------- FC3 接线状态：读口与渲染点必须同进同退 ----------
public_read = []
render = []
for root, _, fs in os.walk(APISRC):
    for fn in fs:
        if not fn.endswith(".rs"):
            continue
        p = os.path.join(root, fn)
        if "staff_http" in p.replace("\\", "/"):
            continue
        if re.search(r"FROM ads\b", cut(p)):
            public_read.append(os.path.relpath(p, ROOTDIR).replace("\\", "/"))
for root, _, fs in os.walk(WEBSRC):
    rp = root.replace("\\", "/")
    if "node_modules" in rp or ".next" in rp:
        continue
    for fn in fs:
        if not fn.endswith((".tsx", ".ts")):
            continue
        t = rd(os.path.join(root, fn))
        # 「/ads」在后台面板里是**管理 API 路径**（/api/v1/admin/ads），
        # 不是前台渲染点：staff-tools-* 与 admin* 文件一律排除，
        # 否则这条判据会把后台自身误判成「已接线」（第一版就是这么错的）。
        low = fn.lower()
        if low.startswith("staff-") or low.startswith("admin"):
            continue
        if re.search(r"/ads\b", t) and "api/v1/admin" not in t:
            render.append(fn)
wired = bool(public_read) and bool(render)
print("   [接线状态] 非后台读口 %s；前端消费文件 %s ⇒ %s"
      % (public_read or "无", render or "无",
         "已接线" if wired else "未接线（站长配了不会显示）"))
if not public_read and not render:
    ok("FC3 未接线现状已登记（读口与渲染点同时缺失，判据只防半接线）", True,
       "")
else:
    ok("FC3 半接线检测：读口 %d 处 / 渲染点 %d 处必须同时存在"
       % (len(public_read), len(render)),
       bool(public_read) == bool(render),
       "只加读口 = 数据出得去但没人显示（假象接线）；只加渲染 = 前端 404")

# ---------- FC4 接线前置：出站必须过 html_clean ----------
if public_read:
    hits = []
    for rel in public_read:
        t = cut(os.path.join(ROOTDIR, rel))
        if "html_clean" not in t:
            hits.append(rel)
    ok("FC4 已出现公开读口，则每一处都必须过 html_clean（缺 %s）" % (hits or "无"),
       not hits,
       "ads.html 是原样存的；前台一旦渲染，未消毒就是存储型 XSS（P-341 同族）")
else:
    print("   [前置在册] 尚无公开读口；将来接线时必须同批把出站消毒接到 "
          "home_news::html_clean 那把尺上（FC4 会自动转硬）")

# ---------- FC5 现网数据 ----------
rows = psql("SELECT count(*) FROM ads")
en = psql("SELECT count(*) FROM ads WHERE enabled")
print("   [现网账] ads：%s 条（启用 %s 条）；未接线期间这个数字涨到多少都不会"
      "影响前台——面板却给了 position/sort/enabled 全套控件"
      % (rows, en))

summary()
