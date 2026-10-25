# -*- coding: utf-8 -*-
"""EK 常驻闸门：每条 module_* 设定都必须有执行路径读它（防「死开关」再长出来）

来历（2026-10-15 批一百零七实测）：
  发现现网 `site_settings` 里有 `module_funbox = yes` 这一行，但全仓三条执法通道
  都不认它：
    1) 网关中间件 `modules/gateway.rs` 的两张前缀/精确表里无 funbox
       （`/api/v1/fun` 刻意归 games 口径，见 gateway.rs 注释）；
    2) `modules.rs` 的 `key::ALL` 无 FUNBOX 常量（`modules` 表按它校验，未知键直接
       报「未知模块键 …（可用键见后台「模块开关」）」）；
    3) 前端无任何 `modules("funbox")` 读者（grep 到的 funbox 全是 CSS 类名与
       字典标题；首页那个是 home_sections 的板块键，另一套白名单）。
  ⇒ 单看现网它不改变任何行为，不是「关掉后接口照旧」那种洞。真正要防的是
  `modules.rs::load` 把 `module_*` **无白名单**地 strip 前缀灌进 `profile.modules`
  （layout.tsx 的 `modules(k)` 就吃这张表）：哪天有人按 `modules("funbox")` 做
  界面门禁，就会长出「导航关、接口不关、且两者键名不同」的错位——与本仓反复出现
  的「读口参数名 ≠ 写口字段名」同族。

闸门做三件事：
  EK1 每条 module_* 行都要在「网关两张表 / handler 的 module_disabled / 后端
      enabled / module_on_sql / 前端 modules(...)」至少一处被引用；违例只许是
      KNOWN_INERT 里登记过的（现网 = funbox，删行或补执法后请把集合清空）。
  EK2 `key::ALL` 与网关引用的键集互相兜住：ALL 里的键要么有执法、要么在
      KNOWN_FRONTEND_ONLY（showcase = 首页板块级门禁，无独立 API 面）。
  EK3 profile.modules 的键集 = module_* 行集（无过滤这一事实钉住：改了要复核前端
      所有 modules(...) 读者）。
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SF = os.path.join(ROOT, "apps", "api", "src")
WEB = os.path.join(ROOT, "apps", "web")

# 现网已知「无人执行的设定行」：站方删行或在网关补一条规则后，这里应清空
KNOWN_INERT = {"funbox"}
# ALL 里合法地只在**前端板块层**生效的键（后端无独立 API 面）
KNOWN_FRONTEND_ONLY = {"showcase"}


def psql(sql):
    p = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if p.returncode != 0:
        raise RuntimeError("   psql 失败：%s / %s" % (sql[:60], p.stderr.strip()[:180]))
    return p.stdout.strip()


def read(path):
    if not os.path.exists(path):
        return ""
    return io.open(path, encoding="utf-8", errors="replace").read()


def strip_comments(text):
    out = []
    for line in text.splitlines():
        i = line.find("//")
        if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
            line = line[:i]
        out.append(line)
    return "\n".join(out)


def rust_all():
    """整个 api 源码（剥注释）拼成一块，供引用面搜索"""
    buf = []
    for dirpath, _dirs, files in os.walk(SF):
        for f in files:
            if f.endswith(".rs"):
                buf.append(strip_comments(read(os.path.join(dirpath, f))))
    return "\n".join(buf)


def web_all():
    buf = []
    for dirpath, dirs, files in os.walk(WEB):
        dirs[:] = [d for d in dirs if d not in (".next", "node_modules")]
        for f in files:
            if f.endswith((".ts", ".tsx")):
                buf.append(strip_comments(
                    read(os.path.join(dirpath, f))))
    return "\n".join(buf)


rows = psql("SELECT name || '=' || value FROM site_settings "
            "WHERE name LIKE 'module_%'")
live = {}
for line in rows.splitlines():
    if "=" in line:
        k, v = line.split("=", 1)
        live[k[len("module_"):]] = v
allsrc = rust_all()
wsrc = web_all()
gw = strip_comments(read(os.path.join(SF, "modules", "gateway.rs")))
gkeys = set(re.findall(r"key::(\w+)", gw))

mods = read(os.path.join(SF, "modules.rs"))
mk = re.search(r"pub mod key \{(.+?)\n\}", mods, re.S)
consts = dict(re.findall(r"pub const (\w+): &str = \"([a-z_]+)\";",
                         mk.group(1) if mk else ""))
all_list = re.search(r"pub const ALL[^=]*= *&\[([^\]]*)\]",
                     mods, re.S)
ALL = {consts.get(k, k) for k in
       re.findall(r"\b([A-Z][A-Z0-9_]*)\b", all_list.group(1))} \
    if all_list else set()


def value_keys(names):
    """网关表里写的是**常量名**（key::BANK），ALL 里是**值**（"bank"）：
    两套命名空间不能直接比——先按常量表翻成值再对。"""
    return {consts.get(n, n).lower() if n.isupper() else n for n in names}


CALLS = [
    r'module_disabled\(\s*[^;]*?,\s*"%s"\s*\)',
    r'\.enabled\(\s*[^;]*?,\s*"%s"\s*\)',
    r'enabled\(\s*"%s"\s*\)',
    r'module_on_sql\(\s*"%s"\s*\)',
]


def enforced(name):
    """窄匹配：键名必须出现在**执法调用的实参位**上。
    负控自证：宽匹配（全文找 "funbox"）会被 home_sections 的板块键
    `key: "funbox"` 蒙绿——那不是执法，清掉 KNOWN_INERT 也不会红。"""
    if name in value_keys(gkeys):
        return True
    for pat in CALLS:
        if re.search(pat % re.escape(name), allsrc, re.S):
            return True
    for pat in (r'module_on\(\s*"%s"', r'modules\(\s*"%s"',
                r'modules\?\.\s*%s\b', r'modules\?\[\s*"%s"',
                r'modules\?\.\["%s"\]', r'modules\[%s\]'):
        if re.search(pat % re.escape(name), wsrc):
            return True
    return False


GV = value_keys(gkeys)
print("== S1 每条 module_* 设定都要有读者 ==")
print("   现网 module_* 行=%d 条，key::ALL=%d 条，网关引用键=%d 个（翻成值后 %d）"
      % (len(live), len(ALL), len(gkeys), len(GV)))
inert = sorted(n for n in live if not enforced(n))
ok("EK1 无「没人执行的 module_* 行」（违例只许 KNOWN_INERT：%s）"
   % sorted(KNOWN_INERT), set(inert) <= KNOWN_INERT,
   "实测违例=%s" % inert)
extra = sorted(set(inert) - KNOWN_INERT)
if extra:
    print("   新长出的死开关：%s" % extra)

print("== S2 键集互相兜住 ==")
no_enf = sorted(k for k in ALL if k not in GV
                and not enforced(k) and k not in KNOWN_FRONTEND_ONLY)
ok("EK2 key::ALL 的键要么有后端执法、要么登记为纯前端板块键（%s）"
   % sorted(KNOWN_FRONTEND_ONLY), not no_enf, "未兜住=%s" % no_enf)
ghost = sorted(GV - ALL)
ok("EK3 网关引用的键都在 key::ALL 里（否则关掉那个开关根本不存在）",
   not ghost, "幽灵键=%s" % ghost)
ok("EK4 profile.modules 无过滤这一事实（改了要复核全部 modules(...) 读者）",
   'strip_prefix("module_")' in mods)
db_keys = set(psql("SELECT key FROM modules").split())
ok("EK5 modules 表键集覆盖 key::ALL（后台列表能开到的键都注册过）",
   not db_keys or ALL <= db_keys, "表=%d ALL=%d 缺=%s"
   % (len(db_keys), len(ALL), sorted(ALL - db_keys)))
print("   现网读数：funbox=%s showcase=%s social=%s"
      % (live.get("funbox"), live.get("showcase"), live.get("social")))
summary()
