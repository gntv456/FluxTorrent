"""把设计稿 CSS 主块转成路由 CSS：选择器统一加 .gx 前缀，剔除画框/评审栏。"""
import re, pathlib

SRC = pathlib.Path(r"D:\FluxTorrent\design\gacha\抽卡页-高保真样图.html")
OUT = pathlib.Path(r"D:\FluxTorrent\apps\web\app\styles\gacha-stage.css")

html = SRC.read_text(encoding="utf-8")
start = html.rfind("<style")
end = html.rfind("</style>")
css = html[start + 7:end]

DROP = (".review-bar", ".vbtn", ".jbtn", ".frame", ".screen", ".wrap",
        ".vbar", ".frame-cap", ".shot")

def keep(sel: str) -> bool:
    s = sel.strip()
    if not s:
        return False
    for p in DROP:
        if s == p or s.startswith(p + "[") or s.startswith(p + ":") \
           or s.startswith(p + " ") or s.startswith(p + ".") or s.startswith(p + "#"):
            return False
    return True

def scope(sel: str) -> str:
    s = sel.strip()
    if s in (":root", "html", "body"):
        return ".gx"
    if s.startswith(".gx"):
        return s
    return ".gx " + s

out = []
i, n = 0, len(css)
depth = 0            # 花括号深度
at_stack = []        # 最近进入的 at-rule 名
buf = ""

while i < n:
    ch = css[i]
    if ch == "{":
        head = buf.strip()
        buf = ""
        if head.startswith("@"):
            at_stack.append(head)
            out.append(head + " {")
            depth += 1
        else:
            in_kf = any(k.startswith("@keyframes") or k.startswith("@-webkit-keyframes")
                        for k in at_stack)
            if in_kf:
                out.append(head + " {")
            else:
                sels = [x for x in head.split(",") if keep(x)]
                if not sels:
                    # 整条丢弃：跳过块内容
                    d = 1
                    i += 1
                    while i < n and d:
                        if css[i] == "{": d += 1
                        elif css[i] == "}": d -= 1
                        i += 1
                    # 吃掉可能的 trailing ;
                    while i < n and css[i] in " \n\t;": i += 1
                    continue
                out.append(", ".join(scope(x) for x in sels) + " {")
            depth += 1
    elif ch == "}":
        out.append("}")
        depth -= 1
        if at_stack and depth == len(at_stack) - 1 + 1 - 1:
            pass
        if depth < len(at_stack):
            at_stack.pop()
    else:
        buf += ch
    i += 1

body = "".join(out)
body = re.sub(r"\n{3,}", "\n\n", body)

header = """/* gacha-stage.css —— /gacha 抽卡页「星轨卡册」生产样式
   设计来源：design/gacha/抽卡页-高保真样图.html（Aurora 蓝 + 金，单屏 + 弹窗）。
   全部选择器已加 .gx 作用域，避免与站内 .card/.panel/.btn 互相污染；
   样图的画框与评审栏规则不进入生产。
   局部令牌与断点都来自样图；改样式直接改本文件。 */
"""
OUT.write_text(header + body, encoding="utf-8")
print("ok", OUT, len(body))
