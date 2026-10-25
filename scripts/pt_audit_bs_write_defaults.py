# -*- coding: utf-8 -*-
"""闸门 BS：写口「省略即反做」家族普查棘轮（2026-10-25 十三轮）。

形状（十二轮在 `PUT /admin/classes/{id}` 上实发过）：整对象/开关型写口的
请求体字段带 `#[serde(default)]`，客户端**少带一列**不是「不动这一列」，
而是「按 Rust 缺省改写」——bool 变 false、数字变 0。当这个缺省方向的后果是
关掉一个保护开关时，一次拼错的请求就会静默放宽站点的安全行为，且不留错误。

本轮全仓普查（`apps/api/src`，非 Option/Vec 的 `serde(default)` 共 111 处）
按「字段名是安全开关 + 会被写进 site_settings/表」筛出的在册两只：
  * `admin_http/cheat_ruleset_admin.rs` 的 `ShadowReq.on` —— **本轮已修为必填**。
    旧行为：`POST /admin/cheat/shadow` 不带 `on` ⇒ 写 `risk_shadow_mode='0'`
    ⇒ 关掉「只算分不开案」的影子模式，下一轮 `risk_score` 直接对真人开案件；
    读侧缺省是 '1'（`apps/worker/src/jobs/risk_score.rs:62` 的 COALESCE），
    所以「不设」与「被省略」不是一回事，只有显式才该改。
  * `admin_p3_http/classes_preview.rs` 的 `ClassImpactBody.demotable` —— 只读
    影响预览里的「假设值」，省略会让预览按「该档不可降」算，与真实调度不一致；
    属站方口径（改它要动面板契约），先登记不红。

判据（新来者才红，在册的出声）：
  BS1 全仓不再有「非 Option 的安全开关字段带 serde(default）」落在写口上
      （在册白名单外的新命中即红）
  BS2 `ShadowReq.on` 已是 Option + 缺省即拒（修复不回归）
  BS3 `ClassPutBody.demotable / promo_sparks` 仍是 Option（十二轮的洞不回归）
  BS4 worker 影子模式读侧缺省仍是 '1'（开）——新库/清库后默认是保护态
  BS5 现网读数：`risk_shadow_mode` 是否存在、当前值（只报不判）
"""
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.environ.get("BS_REPO") or os.path.dirname(
    os.path.dirname(os.path.abspath(__file__)))
API = os.path.join(ROOT, "apps", "api", "src")
WORKER = os.path.join(ROOT, "apps", "worker", "src", "jobs", "risk_score.rs")
SHADOW = os.path.join(API, "admin_http", "cheat_ruleset_admin.rs")
CLASSES = os.path.join(API, "admin_p3_http", "classes_admin.rs")
POOL = os.path.join(API, "games_http", "arcade_admin_write.rs")
POOL = os.path.join(API, "games_http", "arcade_admin_write.rs")

# 语义是「安全开关/保护位」的名字：这类字段不许有缺省
SAFETY = re.compile(r"^(on|off|enabled|disabled|demotable|require|allow|"
                    r"deny|exempt|protect|shadow|lock|unlock)$", re.I)
# 已审读、暂交站方定夺的在册项（file:struct:field）；新增项必须配理由
LEDGER = {"admin_p3_http/classes_preview.rs:ClassImpactBody:demotable":
          "只读预览的假设值，省略=按不可降算；改它要同步面板契约"}

FAIL = []
PASS = 0


def ok(name, cond, detail=""):
    global PASS
    if cond:
        PASS += 1
        print("[PASS] %s%s" % (name, ("  " + detail) if detail else ""))
    else:
        FAIL.append(name)
        print("[FAIL] %s  %s" % (name, detail))


def struct_body(src, sname):
    """只取某个结构体的正文——同文件里 GET 用的 ClassRow 也有
    `demotable: Option<bool>`，全文正则会把它当成「写口已修」而假绿。"""
    m = re.search("struct\\s+%s\\s*\\{(.*?)\\n\\}" % sname, src, re.S)
    return m.group(1) if m else ""


def read(path):
    return open(path, encoding="utf-8", errors="replace").read() \
        if os.path.exists(path) else ""


def strip(t):
    t = re.sub(r"/\*[\s\S]*?\*/", " ", t.replace("\r\n", "\n"))
    return re.sub(r"(?<!:)//[^\n]*", " ", t)


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-v", "ON_ERROR_STOP=1", "-tA", "-F", "|"],
        input=sql, capture_output=True, text=True, encoding="utf-8",
        errors="replace")
    err = (r.stderr or "").strip()
    if r.returncode != 0 or "ERROR" in err:
        raise RuntimeError("psql 失败：%s" % err[:300])
    return [x for x in (r.stdout or "").strip().splitlines() if x.strip()]


def scan():
    """返回 {file:struct:field} —— 安全开关字段带 serde(default) 且非 Option。"""
    hits = {}
    field = re.compile(r'#\[serde\(default(?:\([^)]*\))?\)\]\s*'
                       r'(?:#\[[^\n]*\]\s*)*(?:pub(?:\([a-z]+\))?\s+)?'
                       r'(\w+)\s*:\s*([A-Za-z_][\w:]*(?:<[^>\n]*>)?)', re.S)
    struct = re.compile(r'struct\s+(\w+)\s*\{(.*?)\n\}', re.S)
    for dirpath, _d, files in os.walk(API):
        for fn in files:
            if not fn.endswith(".rs"):
                continue
            path = os.path.join(dirpath, fn)
            rel = os.path.relpath(path, API).replace("\\", "/")
            src = strip(read(path))
            for sm in struct.finditer(src):
                for fm in field.finditer(sm.group(2)):
                    name, typ = fm.group(1), fm.group(2).strip()
                    if typ.startswith("Option") or not SAFETY.match(name):
                        continue
                    hits["%s:%s:%s" % (rel, sm.group(1), name)] = typ
    return hits


def finish():
    print("\n%d 条通过 / %d 条失败" % (PASS, len(FAIL)))
    if FAIL:
        print("失败：" + "、".join(FAIL))
        sys.exit(1)
    sys.exit(0)


def main():
    hits = scan()
    fresh = sorted(set(hits) - set(LEDGER))
    ok("BS1 写口没有新的「省略即反做」安全开关（在册的另计）", not fresh,
       "、".join("%s(%s)" % (k, hits[k]) for k in fresh[:6]))
    for k in sorted(set(hits) & set(LEDGER)):
        print("[WARN] BS1w 在册：%s —— %s" % (k, LEDGER[k]))

    sh = strip(read(SHADOW))
    sb = struct_body(sh, "ShadowReq")
    ok("BS2 ShadowReq.on 已是 Option 且缺省即拒（省略不再关掉影子模式）",
       bool(re.search(r"on\s*:\s*Option<bool>", sb))
       and "body.on.ok_or_else" in sh
       and not re.search(r"#\[serde\(default[^)]*\)\]\s*on\s*:\s*bool",
                         sb),
       "ShadowReq 体=%s" % sb[:110])
    cb = struct_body(strip(read(CLASSES)), "ClassPutBody")
    ok("BS3 ClassPutBody 的 demotable/promo_sparks 仍是 Option",
       bool(re.search(r"demotable\s*:\s*Option<bool>", cb))
       and bool(re.search(r"promo_sparks\s*:\s*Option<i64>", cb)),
       "ClassPutBody 体=%s" % cb[:110])

    pb = struct_body(strip(read(POOL)), "PoolEntryReq")
    ok("BS6 PoolEntryReq.enabled 已必填（省略不再把该档静默停用）",
       bool(re.search(r"pub\s+enabled\s*:\s*bool", pb))
       and not re.search(r"#\[serde\(default[^\)]*\)\]\s*pub\s+enabled", pb),
       "PoolEntryReq 体=%s" % pb[:110])

    pb = struct_body(strip(read(POOL)), "PoolEntryReq")
    ok("BS6 PoolEntryReq.enabled 已必填（省略不再把该档静默停用）",
       bool(re.search(r"pub\s+enabled\s*:\s*bool", pb))
       and not re.search(r"#\[serde\(default[^\)]*\)\]\s*pub\s+enabled",
                         pb), "PoolEntryReq 体=%s" % pb[:110])

    wk = strip(read(WORKER))
    m = re.search(r"WHERE name = 'risk_shadow_mode'\)\s*,\s*'(\d)'", wk)
    ok("BS4 worker 影子模式读侧缺省仍是 '1'（清库后默认是保护态）",
       bool(m) and m.group(1) == "1",
       m.group(0)[:70] if m else "读不到 COALESCE 缺省")

    rows = psql("SELECT COALESCE((SELECT value FROM site_settings "
                "WHERE name='risk_shadow_mode'), '<未落库＝按读侧缺省 1 开>') "
                "|| '|' || (SELECT count(*) FROM site_settings "
                "WHERE name='risk_shadow_mode');")
    val, n = (rows[0].split("|") + ["?"])[:2] if rows else ("?", "?")
    print("[WARN] BS5 现网读数：risk_shadow_mode=%s（行数 %s，"
          "影子模式=开 时真人不会被自动开案）" % (val, n))
    finish()


main()
