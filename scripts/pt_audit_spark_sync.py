"""常驻闸门：会动魔力余额的写口，前端必须统一广播顶栏回读。

判什么（每段都可单独核对）：
  G1 后端扫描出「经余额原语的会员侧 POST 端点」集合 B ⊆ 前端登记表 F
     —— 少登记一条 = 用户扣了魔力而顶栏数字不动（本轮修的缺陷本体）；
  G2 F ⊆ B —— 表里有条目却在后端找不到对应写口 = 路由改名/删了而表没跟上；
  G3 表自身形状：非空、无重复、不含 /api/v1 前缀（api-client 按剥前缀匹配）；
  G4 正控：`/shop/buy`、`/attendance/checkin` 必须同时在 B 与 F 里
     —— 抓不到正控说明这把尺坏了，结论作废；
  G5 接线：api-client 的 request() 成功路径上必须真的调用
     maybeNotifySparkChanged（剥注释后再扫，且在 `return body.data` 之前）。

用法：python scripts/pt_audit_spark_sync.py
自测（证明判据承重）：python scripts/pt_audit_spark_sync.py --selftest
  临时从表里抹掉一条真实写口，闸门必须红；随后必须还原并复跑为绿。
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
API = os.path.join(ROOT, "apps", "api", "src")
WEB = os.path.join(ROOT, "apps", "web")
EV = os.path.join(WEB, "lib", "spark-event.ts")
CLI = os.path.join(WEB, "lib", "api-client.ts")
PRIM_FILES = [os.path.join("economy_http", "spend.rs"),
              os.path.join("economy_http", "ledger.rs"),
              os.path.join("economy_http", "funding.rs"),
              os.path.join("economy_http", "clawback.rs"),
              os.path.join("torrents", "charge.rs")]
PRIM_WORDS = re.compile(
    r"debit|credit|spend|refund|charge|clawback|grant|pay|earn|settle", re.I)
SKIP_DIRS = ("node_modules", os.sep + ".next", os.sep + "tests" + os.sep,
             os.sep + "e2e" + os.sep)


def read(p):
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


def strip_comments(src):
    """按行剥注释（**不要**用 `/*…*/` 整段正则：本仓源码里有含 `/*`
    的字符串与正则字面量，非贪婪匹配会把中间的代码整段吃掉——实测把
    api-client.ts 的 `return body.data;` 直接抹掉，判据自己造出假红）。"""
    out, in_block = [], False
    for line in src.splitlines():
        s = line.strip()
        if in_block:
            if "*/" in s:
                in_block = False
            continue
        if s.startswith("/*"):
            if not s.endswith("*/"):
                in_block = True
            continue
        if s.startswith("//") or s.startswith("*"):
            continue
        out.append(line)
    return "\n".join(out)


def backend_write_paths():
    prims = set()
    for rel in PRIM_FILES:
        for m in re.finditer(r"pub (?:crate )?(?:async )?fn (\w+)",
                             read(os.path.join(API, rel))):
            if PRIM_WORDS.search(m.group(1)):
                prims.add(m.group(1))
    assert prims, "余额原语一个都没扫到＝判据坏了"
    out = set()
    for dirpath, _d, files in os.walk(API):
        if any(s in dirpath for s in SKIP_DIRS):
            continue
        for fn in files:
            if not fn.endswith(".rs"):
                continue
            p = os.path.join(dirpath, fn)
            txt = read(p)
            if not any(re.search(r"\b%s\(" % re.escape(n), txt)
                       for n in prims):
                continue
            for m in re.finditer(r'#\[post\("([^"]+)"\)\]', txt):
                out.add(m.group(1))
    # 只保留会员侧：职员面（admin/staff）与外部兼容层由各自面板口径负责
    member = {p for p in out if not p.startswith("/admin")
              and "/staff" not in p and not p.startswith("/compat")
              and not p.startswith("/openapi")}
    return out, member


def frontend_table():
    src = read(EV)
    m = re.search(r"SPARK_WRITE_PATHS[^=]*=\s*\[(.*?)\];", src, re.S)
    if not m:
        raise SystemExit("前端登记表 SPARK_WRITE_PATHS 找不到＝尺坏了")
    return re.findall(r'"([^"]+)"', m.group(1))


def main():
    raw, member = backend_write_paths()
    table = frontend_table()
    tset = set(table)

    print("后端写口(全部 %d，会员侧 %d) / 前端登记表 %d"
          % (len(raw), len(member), len(tset)))

    # G4 正控先行：只验「两把尺都能读到东西」。
    # 「后端有、表里没有」是**缺陷**，归 G1 判红；把它算成尺坏会把
    # 真缺陷的读数改写成「判据不承重」，反而掩盖问题。
    miss_scan = [c for c in ("/shop/buy", "/attendance/checkin")
                 if c not in member]
    ok("G4a 正控：后端能扫到已知扣魔力的写口", not miss_scan,
       "扫不到=%s（会员侧命中=%d）" % (miss_scan, len(member)))
    ok("G4b 正控：前端登记表解析得出且非平凡", len(table) >= 40,
       "解析出 %d 条" % len(table))
    if miss_scan or len(table) < 40:
        print("!! 判据不承重，本轮结论作废")
        return 2

    missing = sorted(member - tset)
    ok("G1 会员侧写口全部登记（漏一条=顶栏数字不动）", not missing,
       "未登记 %d 条：%s" % (len(missing), ", ".join(missing[:12])))

    stale = sorted(t for t in tset if t not in raw)
    ok("G2 表里没有已消失的条目（路由改名要跟上）", not stale,
       "表内找不到后端对应 %d 条：%s" % (len(stale), ", ".join(stale[:12])))

    bad_shape = [t for t in table if not t.startswith("/")
                 or t.startswith("/api/v1") or "{" in t and "}" in t
                 and not re.fullmatch(r"[\w/{}.\-]+", t)]
    ok("G3 表形状：绝对路径、不带 /api/v1、字符合法", not bad_shape,
       "异常条目=%s" % bad_shape[:6])
    ok("G3b 表无重复", len(table) == len(tset),
       "%d 项里有重复" % len(table))

    cli = strip_comments(read(CLI))
    at = cli.find("maybeNotifySparkChanged(")
    ret = cli.find("return body.data;")
    ok("G5 api-client 成功路径上真的广播（在 return 之前）",
       at != -1 and (ret == -1 or at < ret),
       "广播调用位置=%s return 位置=%s" % (at, ret))

    summary()
    return 0


def selftest():
    """抹掉表里一条真实写口 ⇒ 闸门必须红；随后还原并复跑为绿。

    必须用子进程跑：`summary()` 在有红账时直接 sys.exit(1)，
    在父进程里调 main() 会跳过还原，把源码留在翻转态。
    """
    import subprocess
    import sys as _s
    src = read(EV)
    victim = "/shop/buy"
    old = '"%s",' % victim
    if src.count(old) != 1:
        print("锚点不唯一，放弃自测：%d" % src.count(old))
        return 2
    cmd = [_s.executable, os.path.abspath(__file__)]
    open(EV, "w", encoding="utf-8", newline="").write(src.replace(old, ""))
    try:
        flipped = subprocess.run(cmd, capture_output=True, text=True,
                                 encoding="utf-8", errors="replace")
        print("[自测] 抹掉 %s ⇒ rc=%s（必须非 0）"
              % (victim, flipped.returncode))
        tail = [l for l in (flipped.stdout or "").splitlines()
                if "FAIL" in l or "未登记" in l]
        for l in tail[:3]:
            print("   ", l.strip()[:160])
    finally:
        open(EV, "w", encoding="utf-8", newline="").write(src)
    back = subprocess.run(cmd, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")
    print("[自测] 还原后 rc=%s（必须 0）" % back.returncode)
    return 0 if (flipped.returncode != 0 and back.returncode == 0) else 2


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
