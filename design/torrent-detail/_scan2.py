# -*- coding: utf-8 -*-
"""补充扫描：体系归属、loading 覆盖、响应式盲区、页面头模式"""
import io, os, re, glob, json

WEB = r"D:\FluxTorrent\apps\web"
APP = os.path.join(WEB, r"app\(main)")
COMPS = os.path.join(WEB, "components")

def read(p):
    return io.open(p, encoding="utf-8", errors="replace").read()

tsx_files = []
for root in (APP, COMPS):
    for dp, dn, fn in os.walk(root):
        for f in fn:
            if f.endswith(".tsx"):
                tsx_files.append(os.path.join(dp, f))

out = {}

# 1) 各体系（前缀）归属页面：统计每个 tsx 用到的前缀
prefixes = ["td-", "up-", "pting-", "bzx-", "ucx-", "tsb-", "upx-", "rssx-", "ntf-", "funx-", "brand-", "task-", "pghd"]
sysmap = {p: [] for p in prefixes}
for f in tsx_files:
    c = read(f)
    rel = os.path.relpath(f, WEB)
    for p in prefixes:
        if p in c and "app\\(main)\\" in rel:
            sysmap[p].append(rel.replace("app\\(main)\\", "").split("\\")[0])
out["system_pages"] = {p: sorted(set(v)) for p, v in sysmap.items()}

# 2) 有 page.tsx 但无同层 loading 的路由
routes = [d for d in os.listdir(APP) if os.path.isdir(os.path.join(APP, d))]
no_loading = []
for d in routes:
    has_page = os.path.exists(os.path.join(APP, d, "page.tsx"))
    has_loading = os.path.exists(os.path.join(APP, d, "loading.tsx"))
    if has_page and not has_loading:
        no_loading.append(d)
out["routes_no_loading"] = sorted(no_loading)

# 3) 响应式盲区：页面级 page.tsx 中完全无断点前缀的
re_bp = re.compile(r"(?:sm|md|lg|xl|2xl):|max-(?:sm|md|lg|xl)|min-(?:sm|md|lg|xl)")
blind = []
for d in routes:
    p = os.path.join(APP, d, "page.tsx")
    if os.path.exists(p):
        c = read(p)
        if not re_bp.search(c):
            blind.append(d)
out["responsive_blind_pages"] = sorted(blind)

# 4) emoji 在页面/组件中的分布（U+1F300+ 与 ⚙📣 等）
emoji_re = re.compile(
    "[\U0001F300-\U0001FAFF\u2600-\u27BF\u2B50\uFE0F]"
)
emoji_files = {}
for f in tsx_files:
    c = read(f)
    for m in emoji_re.finditer(c):
        rel = os.path.relpath(f, WEB)
        emoji_files.setdefault(rel, set()).add(m.group(0))
out["emoji_files"] = len(emoji_files)
out["emoji_top"] = sorted(((k, sorted(v)) for k, v in emoji_files.items()), key=lambda x: -len(x[1]))[:20]

# 5) 页面头模式统计：pghd / 自定义 header 前缀
head_files = {"pghd": [], "pting-board__head": [], "up-hero": [], "brand-panel__head": []}
for f in tsx_files:
    c = read(f)
    rel = os.path.relpath(f, WEB)
    if "app\\(main)\\" not in rel:
        continue
    for k in head_files:
        if k in c:
            head_files[k].append(rel.replace("app\\(main)\\", "").split("\\")[0])
out["head_patterns"] = {k: sorted(set(v)) for k, v in head_files.items()}

with io.open(r"D:\FluxTorrent\design\torrent-detail\_scan2.json", "w", encoding="utf-8") as f:
    json.dump(out, f, ensure_ascii=False, indent=1)
print("done")
