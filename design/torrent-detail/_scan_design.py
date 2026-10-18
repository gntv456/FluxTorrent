# -*- coding: utf-8 -*-
"""全局设计一致性量化扫描：apps/web 前台"""
import io, os, re, glob, json

WEB = r"D:\FluxTorrent\apps\web"
STYLES = os.path.join(WEB, r"app\styles")
APP = os.path.join(WEB, r"app\(main)")
COMPS = os.path.join(WEB, r"components")

out = {}

# 1) CSS 文件 @media 数量
css_files = glob.glob(os.path.join(STYLES, "*.css"))
media = {}
for f in css_files:
    c = io.open(f, encoding="utf-8", errors="replace").read()
    media[os.path.basename(f)] = len(re.findall(r"@media", c))
out["css_media"] = media

# 2) 全 tsx 响应式前缀统计（Tailwind sm/md/lg/xl 或 max-md 等）
tsx_files = []
for root in (APP, os.path.join(WEB, "components")):
    for dp, dn, fn in os.walk(root):
        if "node_modules" in dp: continue
        for f in fn:
            if f.endswith(".tsx") or f.endswith(".ts"):
                tsx_files.append(os.path.join(dp, f))
out["tsx_count"] = len(tsx_files)

re_break = re.compile(r"(?:sm|md|lg|xl|2xl):|max-(?:sm|md|lg|xl)|min-(?:sm|md|lg|xl)")
with_break = 0
break_total = 0
for f in tsx_files:
    c = io.open(f, encoding="utf-8", errors="replace").read()
    n = len(re_break.findall(c))
    if n: with_break += 1
    break_total += n
out["breakpoint"] = {"files_with_bp": with_break, "total_bp_uses": break_total}

# 3) h1 使用：h1 总数与「裸 h1」（无类名）
h1_total = 0; h1_bare = 0; h1_examples = []
for f in tsx_files:
    c = io.open(f, encoding="utf-8", errors="replace").read()
    for m in re.finditer(r"<h1([^>]*)>", c):
        h1_total += 1
        attrs = m.group(1)
        if "className" not in attrs and "class" not in attrs:
            h1_bare += 1
            if len(h1_examples) < 10:
                h1_examples.append(os.path.relpath(f, WEB) + " :: " + attrs.strip()[:60])
out["h1"] = {"total": h1_total, "bare": h1_bare, "examples": h1_examples}

# 4) 字符符号图标（非 SVG 的装饰字符）
sym_pat = re.compile(r"[◷●♥☆⧉⬇★▲▼→←↗↙↘↖›»«·⊕⊗≡※✦✿☾☀✓✔✕✖⚠️]")
sym_files = {}
for f in tsx_files:
    c = io.open(f, encoding="utf-8", errors="replace").read()
    for m in sym_pat.finditer(c):
        rel = os.path.relpath(f, WEB)
        sym_files.setdefault(rel, set()).add(m.group(0))
out["symbol_icons"] = {k: sorted(v) for k, v in list(sym_files.items())[:30]}
out["symbol_files_total"] = len(sym_files)

# 5) 旧胶囊按钮样式残留（rounded-full 与 border 组合 / btn-pill / pill）
pill_files = {}
for f in tsx_files:
    c = io.open(f, encoding="utf-8", errors="replace").read()
    if re.search(r"rounded-full", c):
        rel = os.path.relpath(f, WEB)
        pill_files[rel] = len(re.findall(r"rounded-full", c))
out["rounded_full"] = {"files": len(pill_files), "total": sum(pill_files.values())}

# 6) 硬编码颜色（hex 在 tsx 里）
hex_pat = re.compile(r"#[0-9a-fA-F]{3,8}\b")
hex_files = {}
for f in tsx_files:
    c = io.open(f, encoding="utf-8", errors="replace").read()
    hs = hex_pat.findall(c)
    if hs:
        rel = os.path.relpath(f, WEB)
        hex_files[rel] = len(hs)
out["hardcode_hex"] = {"files": len(hex_files), "total": sum(hex_files.values()), "top": sorted(hex_files.items(), key=lambda x: -x[1])[:15]}

# 7) style={{ ... 内联样式数量
inline_files = {}
for f in tsx_files:
    c = io.open(f, encoding="utf-8", errors="replace").read()
    n = len(re.findall(r"style=\{\{", c))
    if n: inline_files[os.path.relpath(f, WEB)] = n
out["inline_style"] = {"files": len(inline_files), "total": sum(inline_files.values())}

# 8) 页面级 loading.tsx 覆盖（路由加载态）
routes = [d for d in os.listdir(APP) if os.path.isdir(os.path.join(APP, d))]
route_no_loading = []
for d in routes:
    has_loading = os.path.exists(os.path.join(APP, d, "loading.tsx")) or os.path.exists(os.path.join(APP, d, "loading.ts"))
    if not has_loading:
        route_no_loading.append(d)
out["routes"] = {"total": len(routes), "no_loading": route_no_loading}

with io.open(r"D:\FluxTorrent\design\torrent-detail\_scan.json", "w", encoding="utf-8") as f:
    json.dump(out, f, ensure_ascii=False, indent=1)
print("done", len(tsx_files), "tsx")
