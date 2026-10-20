# -*- coding: utf-8 -*-
"""常驻闸门：假开关基线不许把「只有 SQL 函数读的键」登记成待清理项（P-461）。

形状：`fake_switch_guard.mjs` 的证据面一直只有 Rust/TS（迁移整份排除，因为
`INSERT INTO settings_meta` 会把每个键都算成"有引用"）。于是 0136 换做种公式
之后，八个参数键 `seeding_base_hourly / seeding_cap_hourly / seeding_bonus_scale
/ seeding_donor_mult / seeding_curve_k / seeding_rarity_k / seeding_rarity_exp /
seeding_vol_log_base_gb` 在源码里一次都不出现——读它们的是迁移 0136/0385/0394
里建的数据库函数 `seeding_params()`（worker 的做种收益 SQL 直接调它）。
八把键全被登记进 `fake_switch_baseline.json` 的「现存假开关」清单。

为什么这比漏报更危险：基线的用途是**清理清单**（0193 fake_switch_cleanup 就是
照它删的）。照单删这八把键，等于当场拆掉做种奖励公式的参数。P-460 是同族的
另一面：那八把 NP 旧键是真死（只读处置），这八把是被误判为死。

修法：门禁的语料加一段「dollar-quoted 函数体且含 `FROM site_settings`」——
只收读法、不收注册语句；基线里那八把随之撤下（285→277，手工只删这八把，
不跑 `--update`，免得把站方在途件的收缩算进本批）。

判据分层：
  F0  权威正控：`pg_proc` 里 seeding_params() 的函数体确实读这八把键（不靠
      grep 猜），且它们已不在基线里、FIX^ 的基线里在
  F1  尺在册：门禁里有 sqlObjectBodies()，且 sourceCorpus 真的把它拼进语料
      （只定义不调用 = 没修）
  F2  尺的边界：函数体过滤条件是 `FROM site_settings`（注册语句不算读者）
  F3  现网门禁实跑 rc=0、存量条数 = 基线长度（读数对账）
  F4  反向钉：这八把键仍是面板可编辑的真参数（visible 且非 readonly）——
      哪天有人把它们跟 P-460 那八把旧键一起标只读，这条要红
跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_fake_switch_sql_readers.py
      负控：同加 --selftest
"""
import io
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GUARD = "scripts/fake_switch_guard.mjs"
BASE = "scripts/fake_switch_baseline.json"
FIX = "5ad48a37"
KEYS = ["seeding_base_hourly", "seeding_cap_hourly", "seeding_bonus_scale",
        "seeding_donor_mult", "seeding_curve_k", "seeding_rarity_k",
        "seeding_rarity_exp", "seeding_vol_log_base_gb"]
IN_LIST = ",".join("'%s'" % k for k in KEYS)


def read(rel, ref=None):
    if ref:
        return subprocess.run(["git", "show", "%s:%s" % (ref, rel)],
                              capture_output=True, text=True,
                              encoding="utf-8", errors="replace").stdout
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def main():
    body = psql_stdin("SELECT prosrc FROM pg_proc p JOIN pg_namespace n "
                      "ON n.oid=p.pronamespace WHERE n.nspname='public' "
                      "AND p.proname='seeding_params';")
    hit = [k for k in KEYS if k in body]
    base = set(json.loads(read(BASE))["dead"])
    old_base = set(json.loads(read(BASE, FIX + "^"))["dead"])
    ok("F0 权威正控：seeding_params() 函数体读到 %d/8 把键；FIX^ 基线里 %d 把"
       "在册、现在 0 把在册" % (len(hit), sum(1 for k in KEYS if k in old_base)),
       len(hit) == 8 and sum(1 for k in KEYS if k in old_base) == 8
       and not [k for k in KEYS if k in base],
       "函数体读不到＝做种公式换了读者，本判据的键清单要重新对账")
    guard = read(GUARD)
    ok("F1 尺在册且真被调用：sqlObjectBodies() 定义 + sourceCorpus 里 push",
       "function sqlObjectBodies()" in guard
       and "parts.push(sqlObjectBodies());" in guard,
       "只定义不接入＝八把键明天又被登记回基线")
    fn_body = guard[guard.index("function sqlObjectBodies()"):] \
        if "function sqlObjectBodies()" in guard else ""
    fn_body = fn_body[:fn_body.index("\n}\n")] if "\n}\n" in fn_body else ""
    # 两个标记都要在：`FROM site_settings` 是「只收读法」的过滤器；
    # 惰性配对 [\s\S]*? 是「按 dollar-quote 成对截取」而不是整份文件拼进去
    # （少了任何一个，门禁要么把注册语句当读者、要么整个失明）。
    ok("F2 尺的边界：只收含 `FROM site_settings` 的 dollar-quote 成对区域"
       "（读法过滤=%s / 惰性配对=%s）"
       % ("FROM site_settings" in fn_body, r"[\s\S]*?" in fn_body),
       "FROM site_settings" in fn_body and r"[\s\S]*?" in fn_body,
       "把整份迁移都拼进语料＝门禁失明")
    p = subprocess.run(["node", os.path.join(ROOT, GUARD)],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", timeout=180, cwd=ROOT)
    out = (p.stdout or "") + (p.stderr or "")
    m = re.search(r"存量基线 (\d+) 条", out)
    ok("F3 门禁实跑 rc=%s，读数=%s（基线实际 %d 条）"
       % (p.returncode, m.group(1) if m else "?", len(base)),
       p.returncode == 0 and m is not None and int(m.group(1)) == len(base),
       "读数与文件不符＝基线被手改过而门禁没重算")
    st = psql_stdin("SELECT count(*) FROM settings_meta WHERE name IN ("
                    + IN_LIST + ") AND COALESCE(visible,true) AND NOT "
                    "COALESCE(readonly,false);")
    ok("F4 八把键仍是面板可编辑的真参数（visible 且非 readonly =%s/8）" % st,
       st == "8", "被标只读＝把它们当成 P-460 那批旧键处理了，两者不能混")
    if "--selftest" in sys.argv:
        selftest()
    summary()


def selftest():
    """把 sqlObjectBodies 从语料里摘掉 ⇒ 门禁必须把八把键报成新增假开关。"""
    p = os.path.join(ROOT, GUARD)
    snap = io.open(p, "rb").read()
    txt = snap.decode("utf-8")
    a = "  parts.push(sqlObjectBodies());"
    if txt.count(a) != 1:
        ok("S0 负控锚点唯一（命中 %d）" % txt.count(a), False,
           "拼接点搬家了，先看 sourceCorpus 现在的形状")
        return
    try:
        io.open(p, "wb").write(txt.replace(a, "  // (selftest) 暂时不拼 SQL 读者",
                                            1).encode("utf-8"))
        r = subprocess.run(["node", p], capture_output=True, text=True,
                           encoding="utf-8", errors="replace", timeout=180,
                           cwd=ROOT)
        out = (r.stdout or "") + (r.stderr or "")
        named = [k for k in KEYS if k in out]
        ok("S1 负控：撤掉 SQL 读者语料后门禁变红并点名这几把键（%d 把）"
           % len(named), r.returncode != 0 and len(named) == 8,
           "翻红了没点名/没红＝这条尺不承重。输出：%s" % out[:160])
    finally:
        io.open(p, "wb").write(snap)
    back = io.open(p, "rb").read() == snap
    r2 = subprocess.run(["node", p], capture_output=True, text=True,
                        encoding="utf-8", errors="replace", timeout=180,
                        cwd=ROOT)
    ok("S2 负控还原：按字节回到翻转前（%s）且门禁复跑 rc=%s"
       % (back, r2.returncode), back and r2.returncode == 0,
       "还原不干净＝把翻转态留在了工作树")


if __name__ == "__main__":
    main()
