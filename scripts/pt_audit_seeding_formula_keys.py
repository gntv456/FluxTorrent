# -*- coding: utf-8 -*-
"""常驻闸门：「做种公式」屏上不许再摆无人读取的旧键（P-460）。

形状（现网实测）：面板 group「做种公式」里八把 NP 时代留下的键
  perseeding(每个做种基础魔力=1) / maxseeding(计费做种上限=7) /
  bonus_per_seed(做种基础魔力（旧键）=0.4) / donortimes(捐赠者魔力倍数=2) /
  tzero / nzero / bzero / l(公式 T0/N0/B0/L)
可写可回显，但**没有任何读者**：真正在算做种火花的是数据库函数
`seeding_params()`，它读的是 `seeding_base_hourly / seeding_cap_hourly /
seeding_bonus_scale / seeding_donor_mult / seeding_curve_k / seeding_rarity_k /
seeding_rarity_exp / seeding_vol_log_base_gb`。比「改了没用」更糟的是两套数
互相矛盾——面板写底薪 1、上限 7，实际按 10、150 发；站长按旧键推算经济，
推的是一个 0136 就已经不存在的公式。

处置沿用本仓既有口径（不发明机制）：`settings_meta.readonly = true` + hint
指认真键，先例是登录防线五条兼容保留键。迁移 0410 落地。

判据分层：
  K0 「谁读」用权威来源：直接取 pg_proc 里 seeding_params() 的函数体——
     必须含满 8 把 seeding_* 键、且**一把旧键都不含**（比 grep 硬：DB 自己
     说的）。这条同时是「旧键无读者」的正控
  K1 Rust / 前端零读者（剥注释后逐键搜；单字母键 `l` 不交给 grep 判，
     子串会满天飞，由 K0 判它）
  K2 迁移在册：文件存在、八把键名齐、只动 readonly/hint 两个列
  K3 事务 dry-run：把迁移语句真跑一遍，断言「只读旧键 0→8」「只读键总数
     =11+8=19 精确等式（没误伤别的键）」「八把键的 site_settings 值一字未动」，
     然后 ROLLBACK 并回读残留 0
  K4 readonly 这个机制真的会被执行（后端逐字段拒绝 + 前端锁控件），否则
     我只是往一列布尔上写了个愿望
  K5 自升级读数：站方应用 0410 之前只报 WARN，之后这条升硬——现网仍是 0
跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_seeding_formula_keys.py
      负控：同加 --selftest
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MIG = "apps/api/migrations/0410_seeding_legacy_readonly.sql"
OLD = ["perseeding", "maxseeding", "bonus_per_seed", "donortimes",
       "tzero", "nzero", "bzero", "l"]
REAL = ["seeding_base_hourly", "seeding_cap_hourly", "seeding_bonus_scale",
        "seeding_donor_mult", "seeding_curve_k", "seeding_rarity_k",
        "seeding_rarity_exp", "seeding_vol_log_base_gb"]
PSQL = ["docker", "exec", "-i", "flux-postgres",
        "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-v",
         "ON_ERROR_STOP=1", "-F", "|",
        "-f", "-"]
# 应用前的既有只读键数（现网实测 11：五条登录兼容键 + 站型 + 四把展示键）
BASE_READONLY = 11


def psql(sql):
    p = subprocess.run(PSQL, input=sql, capture_output=True, text=True,
                       encoding="utf-8", errors="replace", cwd=ROOT)
    if p.returncode != 0:
        raise RuntimeError("psql 失败：%s" % ((p.stderr or "")[:240]))
    return (p.stdout or "").strip()


def cells(text):
    out = {}
    for ln in text.split("\n"):
        if "|" in ln:
            k, v = ln.split("|", 1)
            out[k.strip()] = v.strip()
    return out


def strip(text):
    t = re.sub(r"/\*[\s\S]*?\*/", " ", text)
    return re.sub(r"(?<!:)//[^\n]*", " ", t)


def corpus():
    parts = []
    for d in ["apps/api/src", "apps/worker/src", "apps/tracker/src",
              "crates", "apps/web/components", "apps/web/app", "apps/web/lib"]:
        base = os.path.join(ROOT, d)
        if not os.path.isdir(base):
            continue
        for root, _, files in os.walk(base):
            if "node_modules" in root or ".next" in root:
                continue
            for fn in files:
                if fn.endswith((".rs", ".ts", ".tsx")):
                    parts.append(strip(io.open(
                        os.path.join(root, fn), encoding="utf-8",
                        errors="replace").read()))
    return "\n".join(parts)


def main():
    # ---------- K0 「谁读」问数据库 ----------
    body = psql("SELECT prosrc FROM pg_proc p JOIN pg_namespace n "
                "ON n.oid=p.pronamespace WHERE n.nspname='public' "
                "AND p.proname='seeding_params';")
    hit_real = [k for k in REAL if k in body]
    hit_old = [k for k in OLD if re.search(r"\b%s\b" % k, body)]
    ok("K0 seeding_params() 的函数体：真键命中 %d/8，旧键命中 %s"
       % (len(hit_real), hit_old or "无"),
       len(hit_real) == 8 and not hit_old and "site_settings" in body,
       "公式换了读者却没同步面板，就是这批洞的成因")
    # ---------- K1 Rust / 前端零读者 ----------
    corp = corpus()
    long_old = [k for k in OLD if len(k) > 2]  # `l` 交给 K0 判
    grep_hits = {k: len(re.findall(r"\b%s\b" % k, corp)) for k in long_old}
    bad = {k: n for k, n in grep_hits.items() if n}
    ok("K1 七把长名旧键在 Rust/前端剥注释后零引用（异常 %s）" % (bad or "无"),
       not bad and len(long_old) == 7,
       "有人在读了 yet 面板还锁着＝迁移要撤")
    # ---------- K2 迁移在册 ----------
    mig = io.open(os.path.join(ROOT, MIG), encoding="utf-8").read()
    missing = [k for k in OLD if k not in mig]
    ok("K2 迁移 0410 在册：八把键名齐（缺 %s）、只动 readonly 与 hint"
       % (missing or "无"),
       not missing and mig.count("SET readonly = true") == 5
       and "UPDATE site_settings" not in mig
       and "DELETE" not in mig.upper(),
       "迁移被改窄或开始动值＝这批的口径不再成立")
    # ---------- K3 事务 dry-run ----------
    in_list = ",".join("'%s'" % k for k in OLD)
    dry = psql("""
\\set ON_ERROR_STOP on
BEGIN;
SELECT 'A|' || count(*)::text FROM settings_meta WHERE name IN (%s)
   AND readonly;
%s
SELECT 'B|' || count(*)::text FROM settings_meta WHERE name IN (%s)
   AND readonly;
SELECT 'C|' || count(*)::text FROM settings_meta WHERE readonly;
SELECT 'D|' || count(*)::text FROM site_settings s WHERE s.name IN (%s)
   AND s.value IS NOT NULL;
ROLLBACK;
SELECT 'F|' || count(*)::text FROM settings_meta WHERE name IN (%s)
   AND readonly;
""" % (in_list, mig, in_list, in_list, in_list))
    c = cells(dry)
    # 前提自适应：0410 一旦在现网应用过（K5 那条就是查这个），dry-run 起手
    # 读到的 A 已经是 8 而不是 0——旧判据把「库里还是旧状态」写死，于是在
    # 「迁移已上线」这条时间线上常年假红（本轮 K3/K3b 即是）。两种世界都要成立：
    #   A ∈ {0, 8}（未应用／已应用）；B=8（套上迁移后八把都只读）；
    #   C=BASE_READONLY+8（总数等式，不误伤别的键）；D=8（值没被迁移改写）；
    #   F=A（回滚后回到起手状态，不留改动在现网）。
    a0 = c.get("A") in ("0", str(len(OLD)))
    eq = c.get("C") == str(BASE_READONLY + len(OLD))
    ok("K3 dry-run：旧键只读数 %s→%s；只读键总数 %s（=%d+%d 精确等式，"
       "没误伤）；八把键的值仍在=%s/8"
       % (c.get("A"), c.get("B"), c.get("C"), BASE_READONLY, len(OLD),
          c.get("D")),
       a0 and c.get("B") == str(len(OLD)) and eq and c.get("D") == "8",
       "A 只能是 0（未应用）或 8（0410 已上线）；现 A=%s B=%s C=%s D=%s"
       % (c.get("A"), c.get("B"), c.get("C"), c.get("D")))
    ok("K3b dry-run 已回滚（起手只读旧键 %s → 回滚后 %s）"
       % (c.get("A"), c.get("F")),
       c.get("F") == c.get("A"),
       "F≠A＝dry-run 把没经站方批准的改动留在了现网（0410 已上线时 A 本就是 8，"
       "所以判据是「回到起手状态」而不是「必须归零」）")
    # ---------- K4 readonly 机制真的被执行 ----------
    groups = strip(io.open(os.path.join(
        ROOT, "apps/api/src/settings_http/groups.rs"),
        encoding="utf-8", errors="replace").read())
    ctl = strip(io.open(os.path.join(
        ROOT, "apps/web/components/setting-field-controls.tsx"),
        encoding="utf-8", errors="replace").read())
    ok("K4 readonly 会被执行：后端逐字段拒绝（\"该字段为只读\"）+ 前端锁控件"
       "（field.readonly 进 lock）都在册",
       "该字段为只读" in groups and "meta.readonly" in groups
       and "field.readonly" in ctl,
       "只剩一列布尔而没人执行＝我写的是愿望不是修复")
    # ---------- K5 自升级读数 ----------
    now = psql("SELECT count(*) FROM settings_meta WHERE name IN (%s) "
               "AND readonly;" % in_list)
    if now == "8":
        ok("K5 现网已应用 0410（八把旧键已只读）", True, "")
    else:
        print("   [K5] 现网只读旧键=%s / 8：站方重建 api（跑迁移）后这条升硬"
              % now)
    if "--selftest" in sys.argv:
        selftest(in_list)
    summary()


def selftest(in_list):
    """把迁移里一条 readonly = true 翻掉 ⇒ K3 的「8」必须变 7；按字节还原。"""
    p = os.path.join(ROOT, MIG)
    snap = io.open(p, "rb").read()
    txt = snap.decode("utf-8")
    # 锚点必须落在**语句**上：文件头注释里也写着 "readonly = true"，
    # 取不带 SET 的形态会翻到注释，SQL 一字未动 ⇒ 「翻了没红」的假结论
    if txt.count("SET readonly = true") != 5:
        ok("S0 负控锚点：SET readonly = true 应恰好 5 条（实=%d）"
           % txt.count("SET readonly = true"), False, "迁移写法变了")
        return
    flipped = txt.replace("SET readonly = true", "SET readonly = false",
                          1)
    try:
        io.open(p, "wb").write(flipped.encode("utf-8"))
        dry = psql("\\set ON_ERROR_STOP on\nBEGIN;\n%s\n"
                   "SELECT 'B|' || count(*)::text FROM settings_meta WHERE "
                   "name IN (%s) AND readonly;\nROLLBACK;"
                   % (io.open(p, encoding="utf-8").read(), in_list))
        got = cells(dry).get("B")
        ok("S1 负控：翻掉一条 UPDATE 后 K3 抓到（只读旧键=%s，应为 7）" % got,
           got == "7", "翻红了没抓到＝K3 不承重")
    finally:
        io.open(p, "wb").write(snap)
    back = io.open(p, "rb").read() == snap
    ok("S2 负控还原：迁移文件按字节回到翻转前（%s）" % back, back,
       "还原不干净＝翻转态留在了工作树")


if __name__ == "__main__":
    main()
