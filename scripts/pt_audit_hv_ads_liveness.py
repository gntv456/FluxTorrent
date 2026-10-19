# -*- coding: utf-8 -*-
"""闸门 HV：广告投放与 ad_free 权益的死配置钉桩（批一百九十六）。

实测（现网 + 全仓普查，2026-10-19）：
  · `ads` 表 0 行；后端**唯一**的 `FROM ads` 在 `staff_http/site.rs`（后台列表），
    五个口 `GET/POST/PUT /admin/ads*` + 开关 + 删除都齐全；
    前端没有任何广告位组件（`AdSlot`/`/ads` 读取 0 命中）。
    ⇒ 站方投放的广告**没有展示层**，`position`(header/footer/sidebar) 与 `sort`、
    `enabled` 都在给一个不存在的渲染器服务。
  · `ad_free`（「15 天去广告」）全仓只出现在一处判定面：
    `games_http/arcade_cfg.rs` 的 `COSMETIC_KINDS` 品类清单——**没有任何
    `user_can(...,'ad_free')` 之类的执法读者**；而前端 `shop-catalog.tsx` 把它
    标成 `right`，i18n 文案写着「全站无横幅与弹窗」。
    ⇒ 单看代码这就是「发空货」形状。现网这件 SKU `active = false`（0295 那批
    运营审计把它下架了，见 590958b7），所以**不是活的资金洞**，是残留死配置。

因此本闸门不擅自加广告位（放哪儿、哪些页面、与 donor 权益怎么让位都是站点口径，
归站方拍板），只做三件事：把「无展示层」「无执法读者」的现状钉住，
并在**有人把 SKU 重新上架而执法读者仍是 0** 时报警。

判据：
  HV1 现状：后端 `FROM ads` 的读者只有 staff_http/site.rs（出现别的读者＝展示层
      建起来了，撤本条）；
  HV2 现状：`ad_free` 在 apps/api 的判定读者为 0（只允许出现在品类清单）；
  HV3 活体：该 SKU 若 `active=true` 而 HV2 仍为 0 ⇒ 红（卖空气）；当前下架 ⇒ 绿，
      同时把 active 现值报出来；
  HV4 正控：同一套 grep 手段必须能在 `users` 上搜出多处读者（防路径写错恒 0）；
  HV5 承重：内存级构造「active=true 且判定读者 0」的形状必须让 HV3 的判函数报红。

用法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hv_ads_liveness.py
      HV_LIVE=0 跳过 HV3 的现网只读查询。
"""
import io
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

LIVE = os.environ.get("HV_LIVE", "1") != "0"
SITE = "apps/api/src/staff_http/site.rs"
SKU = "ad_free"


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def grep_files(pattern, path, glob="*.rs"):
    r = subprocess.run(
        ["grep", "-rl", "--include=" + glob, pattern, path],
        cwd=ROOT, capture_output=True, text=True, errors="replace")
    return [x.strip() for x in (r.stdout or "").split("\n") if x.strip()]


def ads_readers():
    """返回读了 ads 表的后端文件（去掉注释里的 downloads/threads 等噪音）。"""
    hits = []
    for f in grep_files("FROM ads", "apps/api/src"):
        for ln in rd(f).split("\n"):
            t = ln.strip()
            if t.startswith("//") or t.startswith("///"):
                continue
            if "FROM ads" in ln:
                hits.append(f)
                break
    return sorted(set(hits))


def ad_free_enforcers():
    """`ad_free` 的执法读者：出现该串但**不是**品类/货架清单的文件。"""
    out = []
    for f in grep_files(SKU, "apps/api/src"):
        body = rd(f)
        # 品类清单与常量表：把行内只列名字的行剔掉
        for ln in body.split("\n"):
            t = ln.strip()
            if t.startswith("//") or t.startswith("///"):
                continue
            if SKU not in t:
                continue
            is_listing = ("KINDS" in body and t.startswith('"')) or \
                t.endswith(",") and '"' in t and "(" not in t
            if not is_listing:
                out.append("%s: %s" % (f, t[:60]))
                break
    return out


def sku_active(active_value, enforcers):
    """判函数：在售且无执法读者 ⇒ 违例。"""
    if active_value in (True, 1, "t", "1", "true"):
        return (not enforcers), "SKU 在售但没有任何执法读者（发空货）"
    return False, "SKU 未上架，残留属死配置"


readers = ads_readers()
enf = ad_free_enforcers()
ok("HV1 现状钉桩：ads 的读者只有后台管理口（无会员展示层）",
   readers == [SITE],
   "读者=%s（出现别的文件＝展示层建起来了，撤本条）" % (readers,))
ok("HV2 现状钉桩：ad_free 没有执法读者（只活在品类清单里）",
   enf == [], "非清单出现=%s" % (enf,))
bad4 = grep_files("FROM users", "apps/api/src")
ok("HV4 正控：同法在 users 上必须搜出多处读者（防路径写错恒 0）",
   len(bad4) >= 5, "users 读者文件数=%s" % len(bad4))
viol, why = sku_active(True, enf)
ok("HV5 承重：合成「在售 + 零执法读者」的形状必须被判违例", viol, why)
viol2, _ = sku_active(False, [])
ok("HV5b 承重：下架时不报违例（否则判据恒红）", not viol2, "")

if LIVE:
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-q", "-tA", "-F", "|", "-c",
         "SELECT count(*) FILTER (WHERE active), count(*), "
         "COALESCE(string_agg(id::text || ':' || name, ','), '-') "
         "FROM shop_items WHERE kind = '%s'" % SKU],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if "ERROR" in (r.stderr or ""):
        raise RuntimeError(r.stderr[:300])
    on_sale, total, detail = (r.stdout or "0|0|-").strip().split("|")
    ok("HV3 活体：ad_free 若在售必须已接执法（现网在售=%s 共=%s [%s]）"
       % (on_sale, total, detail),
       int(on_sale) == 0 or bool(enf),
       "在售=%s 执法读者=%s" % (on_sale, len(enf)))
else:
    print("[HV3] HV_LIVE=0 跳过现网 SKU 只读查询")

summary()
