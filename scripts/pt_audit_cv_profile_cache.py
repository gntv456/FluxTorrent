# -*- coding: utf-8 -*-
"""CV 闸门：site-profile 整包缓存（flux:site_profile:v1）的写失效覆盖面。

缺陷本体（批一百五十六 P-330，2026-10-19 实测）：552da295 把 site-profile
的 20+ 条串行 SQL 整包塞进 Redis（300s TTL），写失效只挂在 4 处，而后台
「系统设置」页的主保存口 PUT /admin/settings/groups 不在其中。实测：库里已
落新值（default_language chs→cht），前台档案仍回旧值；同一时刻单项口
PUT /admin/settings 立刻生效；手工 DEL 缓存键后分组口的值即刻可见
⇒ 归因在缓存层，不是写失败。同族还漏三处：视图布局 PUT /admin/view-hidden、
站型包应用回滚 POST /admin/site-type-packs/applies/{id}/rollback、引导与安装
（POST /admin/onboarding、setup）。

判据两层：
  静态普查——凡写 site_settings / categories / site_type_packs 的处理器，函数
    体里必须能走到 invalidate_site_profile_cache（直接或经 settings_http 的
    invalidate_cache 助手），否则必须在 ALLOW 台账里写明「为什么不影响档案」。
  活体 A/B——分组保存口、视图布局口保存后立刻读档案，必须反映新值。

正控（钉死在 552da295，不随 HEAD 前进而失效）：把普查跑在那个提交的
groups.rs 上，必须抓出这条洞；抓不出来即判据是假的。

已知残留（本闸门只打到 API 档案出口，没打到渲染层）：apps/web 对 site-profile
另有一层 60s 的 next.revalidate 取数缓存（site-profile.ts:10
PROFILE_REVALIDATE），全站没有 revalidatePath/revalidateTag 的调用点 ⇒ 站长
保存后，**页面**上的站名/导航仍可能滞后 ≤60s（不是本批的回归，是 2026-10-18
P1-2 提速时定下的对价）。要在渲染层收口，需要「后台保存 → 通知 web 失效」这条
新链路（server action 或回调），属设计决策，交站长拍板。
"""
import glob
import os
import re
import subprocess
import sys

sys.path.insert(0, "scripts")
from pt_audit_lib import call, login, ok, FAILURES, dig  # noqa: E402

API = "apps/api/src"
CTRL_REV = "552da295"  # 引入整包缓存、当时只挂 4 处失效的那个提交
WRITE_RE = (r"(INSERT INTO site_settings|UPDATE site_settings|"
            r"INSERT INTO categories|UPDATE categories|"
            r"DELETE FROM categories|UPDATE site_type_packs)")
CALL = "invalidate_site_profile_cache"
# 台账：普查命中但确实不影响档案读出面的写口（动判据前先核这张表）
ALLOW = {
    "admin_p3_http/sections.rs": "只写 mode_id，档案只透出 5 列",
    "admin_p3_http/section_public.rs": "只写 mode_id/auto_approve，不在档案",
    "http/home_layout.rs": "写 home_layout 键，档案不读（走 home_shared）",
    "admin_p2_http/menus.rs": "写 nav.* 键，走 /menu-items 出口",
    "repo/cheat_ruleset.rs": "写 cheat_* 规则键，档案不读",
    # 内容包导入/回滚会重写 categories + 主题键，但代码在两个存量超限文件里
    # （pack_store.rs 461/461、pack_import.rs 643/643），净增 1 行即触门禁
    # ⇒ 本批不代改，钉桩待「下一批拆域回收」时补，见 CV9
    "settings_http/pack_store.rs": "PENDING：内容包导入重写分类，待拆域补失效",
    "settings_http/pack_import.rs": "PENDING：同上（调用方）",
    # 0400–0402 自动审种批次写入的键全在 auto_review 组（{rule}_action /
    # auto_review_mode / 预置覆盖），而 site-profile 只读 site_type、site_name、
    # default_language、module_*、metadata_sources、subtitle_kind、site_tagline
    # 这几类键 ⇒ 档案读出面不受影响，不需要挂档案失效。
    "admin_http/auto_review_preset.rs": "写 auto_review 组预置覆盖键，档案不读",
    "admin_http/review_auto.rs":
    "写 auto_review 组（{rule}_action/auto_review_mode），档案不读",
}


def sh(args):
    return subprocess.run(args, capture_output=True, encoding="utf-8",
                          errors="replace")


def read(rel, rev=None):
    if rev:
        out = sh(["git", "show", "%s:%s" % (rev, rel)])
        return out.stdout if out.returncode == 0 else None
    try:
        with open(rel, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def fn_span(src, pos):
    """写语句所在的那个 fn 的函数体原文（含签名到下一个 fn 之前）。"""
    heads = [m.start() for m in re.finditer(r"\bfn \w+", src)]
    before = [h for h in heads if h < pos]
    if not before:
        return ""
    after = [h for h in heads if h > pos]
    return src[before[-1]:after[0] if after else len(src)]


def fn_names_with_call(src):
    """本文件内哪些 fn 体里出现档案失效调用（供一跳解析助手）。"""
    return {m.group(1) for m in re.finditer(r"\bfn (\w+)", src)
            if CALL in fn_span(src, m.start())}


def write_files(rev=None):
    """含档案表写语句的 .rs 清单；rev 用来把普查打在历史版本上。"""
    args = ["git", "grep", "-l", "-E", WRITE_RE]
    if rev:
        args.append(rev)
    args += ["--", API]
    out = sh(args)
    rows = {}
    for line in out.stdout.splitlines():
        rel = line.split(":", 1)[1] if rev and ":" in line else line
        if rel.endswith(".rs"):
            rows[rel] = read(rel, rev)
    return rows


def bodies(src):
    """{fn 名: 从签名到下一个 fn 之前的原文}（嵌套 fn 会重叠，够用）"""
    heads = list(re.finditer(r"\bfn (\w+)", src))
    out = {}
    for i, m in enumerate(heads):
        end = heads[i + 1].start() if i + 1 < len(heads) else len(src)
        out.setdefault(m.group(1), src[m.start():end])
    return out


def census(src_map, all_src=None):
    """逐写语句判覆盖面：本体 → 同文件调用方 → 跨文件调用方（一跳到底）。"""
    rows = []
    pool = all_src if all_src is not None else src_map
    gcover = set()
    for s2 in pool.values():
        if s2:
            gcover |= {n for n, b in bodies(s2).items() if CALL in b}
    for rel, src in src_map.items():
        if src is None:
            continue
        bs = bodies(src)
        covering = {n for n, b in bs.items() if CALL in b} | gcover
        for m in re.finditer(WRITE_RE, src):
            span = fn_span(src, m.start())
            fn = re.search(r"\bfn (\w+)", span)
            name = fn.group(1) if fn else "?"
            how = ""
            if CALL in span or any(h + "(" in span for h in covering):
                how = "本体"
            elif name != "?":
                bs[name] = span
                callers = {n for n, b in bs.items()
                           if n != name and re.search(r"\b%s\(" % name, b)}
                hit = sorted(callers & covering)
                if hit:
                    how = "同文件调用方 " + ",".join(hit)
            if not how and name != "?":
                pat = "%s(" % name
                hits = sorted(r for r, s2 in pool.items()
                              if r != rel and s2 and pat in s2)
                if hits and all(CALL in (pool[h] or "") for h in hits):
                    how = "跨文件调用方 " + ",".join(
                        h.split("/")[-1] for h in hits)
            rows.append((rel, m.group(1), name, bool(how), how))
    return rows


def holes(rows):
    return [r for r in rows if not r[3]
            and not any(a in r[0] for a in ALLOW)]


def report(rows, tag):
    h = holes(rows)
    print("%s：档案表写语句 %d 处，其中未覆盖且不在台账 %d 处"
          % (tag, len(rows), len(h)))
    for x in h:
        print("   未覆盖 %-44s %-22s %s" % x[:3])
    return h


print("=" * 68)
print("一、静态普查（覆盖面 + 正控）")
print("=" * 68)
ALLSRC = {p.replace(os.sep, "/"): read(p)
          for p in glob.glob(API + "/**/*.rs", recursive=True)}
now_rows = census(write_files(), ALLSRC)
now_holes = report(now_rows, "CV1 工作树")
how_cnt = {}
for r in now_rows:
    if r[3]:
        how_cnt[r[4].split()[0]] = how_cnt.get(r[4].split()[0], 0) + 1
print("   覆盖面解析出处：%s" % (how_cnt or "无"))
ok("CV1 工作树没有未登记的档案写失效缺口", not now_holes,
   "%d 处未覆盖：%s" % (len(now_holes), [x[2] for x in now_holes]))
ok("CV1b 普查基数可信（档案表写语句 ≥ 20 处，别测在空集合上）",
   len(now_rows) >= 20, "基数=%d" % len(now_rows))

ctrl = read(API + "/settings_http/groups.rs", CTRL_REV)
ctl_rows = census({API + "/settings_http/groups.rs": ctrl})
ctl_holes = report(ctl_rows, "CV2 正控(%s 的 groups.rs)" % CTRL_REV)
ok("CV2 正控：普查在加缓存那一版上必须抓出分组保存口（判据承重）",
   any("UPDATE site_settings" in x[1] and not x[3] for x in ctl_rows),
   "该版 groups.rs 共 %d 处写、%d 处未覆盖" % (len(ctl_rows), len(ctl_holes)))
ok("CV3 修复确实落在这条路径上（工作树 groups.rs 已含档案失效）",
   CALL in (read(API + "/settings_http/groups.rs") or ""),
   "groups.rs 里找不到 %s" % CALL)

# 钉桩：内容包导入这条还没补（文件行数门禁挡住），台账在册即不许它悄悄变多
ok("CV9 钉桩：pack_store.rs 仍在 PENDING 台账里（别把它当已修）",
   "PENDING" in ALLOW["settings_http/pack_store.rs"],
   "补完后请同时删掉这条台账")

print()
if os.environ.get("CV_SKIP_LIVE"):
    print("（CV_SKIP_LIVE=1：跳过活体层，只核静态判据）")
    print("红账：%s" % (FAILURES or "无"))
    sys.exit(1 if FAILURES else 0)
print("=" * 68)
print("二、活体 A/B（打的是 %s）" % os.environ.get(
    "FLUX_API_BASE", "http://127.0.0.1:8080/api/v1"))
print("=" * 68)
K_GRP, GRP = "default_language", "基础信息"
FLIP = {"chs": "cht", "cht": "chs", "en": "chs"}


def psql(sql):
    return sh(["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
               "fluxtorrent", "-Atc", sql]).stdout.strip()


def prof():
    s, r = call("GET", "/site-profile")
    return (dig(r, "data") or {}) if s == 200 else {}


tok = login()
orig = psql("SELECT value FROM site_settings WHERE name='%s'" % K_GRP)
want = FLIP.get(orig)
if not want:
    raise SystemExit("前置不成立：%s 现值 %r 不在可切换集合里" % (K_GRP, orig))
try:
    prof()  # 先铺缓存，否则读到的是首查，测不出失效
    ok("CV10 前置：缓存层确在工作（ttl>0）",
       sh(["docker", "exec", "flux-redis", "sh", "-c",
           'redis-cli -a "$REDIS_PASSWORD" --no-auth-warning ttl '
           'flux:site_profile:v1']).stdout.strip() not in ("-2", "-1", ""),
       "ttl 非正 ⇒ 后面所有判据都可能假绿")
    s1, r1 = call("PUT", "/admin/settings/groups",
                  {"group": GRP, "values": {K_GRP: want}}, tok)
    ok("CV11a 分组保存返回成功", s1 == 200 and r1.get("code") == 0,
       "%s %s" % (s1, str(r1)[:180]))
    ok("CV11b 库里已是新值",
       psql("SELECT value FROM site_settings WHERE name='%s'" % K_GRP)
       == want, "db=%r" % psql(
           "SELECT value FROM site_settings WHERE name='%s'" % K_GRP))
    got = prof().get(K_GRP)
    ok("CV11 分组保存后档案立刻反映新值（P-330 本体）", got == want,
       "档案=%r 期望=%r" % (got, want))

    s2, r2 = call("PUT", "/admin/view-hidden",
                  {"columns": ["size"], "sections": []}, tok)
    vh = prof().get("view_hidden") or {}
    ok("CV12 视图布局保存后档案立刻反映（藏住的列进了档案）",
       s2 == 200 and r2.get("code") == 0
       and "size" in (vh.get("columns") or []),
       "rc=%s code=%s 档案=%r" % (s2, r2.get("code"), vh))
finally:
    s3, r3 = call("PUT", "/admin/view-hidden",
                  {"columns": [], "sections": []}, tok)
    s4, r4 = call("PUT", "/admin/settings/groups",
                  {"group": GRP, "values": {K_GRP: orig}}, tok)
    print("复原 view-hidden rc=%s / %s=%s rc=%s code=%s"
          % (s3, K_GRP, orig, s4, r4.get("code")))
    ok("CV13 复原到位（库与档案都回原值）",
       psql("SELECT value FROM site_settings WHERE name='%s'" % K_GRP)
       == orig and prof().get(K_GRP) == orig,
       "db=%r 档案=%r" % (psql(
           "SELECT value FROM site_settings WHERE name='%s'" % K_GRP),
           prof().get(K_GRP)))
    ok("CV14 复原到位（视图布局列已清空）",
       not (prof().get("view_hidden") or {}).get("columns"),
       "档案=%r" % prof().get("view_hidden"))

print()
print("红账：%s" % (FAILURES or "无"))
sys.exit(1 if FAILURES else 0)
