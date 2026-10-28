# -*- coding: utf-8 -*-
"""闸门 MD：接口收了、但整仓没人读的字段（批一百八十六~一百八十七）。

形状：`Q<T>/web::Query<T>`（查询参数）与 `web::Json<T>/web::Form<T>/OptJson<T>`
（写口载荷）把用户输入反序列化进字段，然后**没有任何代码读它**。后果分两档：
  · 查询侧＝请求照收、HTTP 200、结果完全没按它筛（本仓 #70 那条
    `?limit=100&offset=100` 与 `?limit=100` 回一模一样的 100 行就是同族）；
  · 写口侧＝用户填了框、保存成功、啥也没生效（比 400 难发现得多）。

尺子的两条硬约束（缺一读数就不作数）：
  · **先剥字符串（只剥行内闭合的）、再剥注释**。反过来的话 `"https://…"` 会先
    被按行剥成失衡引号，正则一路吞到下一个引号，真代码整段从读数里消失——
    第一版就把 `q.category_ids` 吞了，把三条「确实有人读」的参数报成零读。
  · **基数 + 已知读点两条自证**（MD1/MD2）。任何一条掉下限＝尺坏了。

主判据取**整仓标识符零出现**（同名字段只会让数偏大 ⇒ 只漏报不误报）；
台账里的项必须带理由，新增一项＝新假开关，销掉一项＝有人真去读它了。

**这条尺子的能力上限要写清楚，别把「全绿」读成「没有死字段」**：标识符在别处
出现过（别的结构体同名字段、局部变量、SQL 别名）就不出声，所以它只抓得到
**名字独一份**的漏接件。查询侧实测抓到 1 条、写口侧实测 0 条——后者的 0 是
「尺子看不见」而不是「站里一定没有」；要判「这一格填了到底生效没」仍需按
行为取证（同一口换参数值打两次比结果集，见记忆里的「收了不用」那条）。
"""
import os
import re
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
API_SRC = os.path.join(ROOT, "apps", "api", "src")
QRX = re.compile(r"(?:web::(?:Query|Qs)|\bQ)\s*<\s*([A-Za-z0-9_]+)")
BRX = re.compile(r"(?:web::(?:Json|Form)|\bOptJson)\s*<\s*"
                 r"([A-Za-z0-9_]+)")
DEF = re.compile(r"(?:pub\s+)?struct\s+([A-Za-z0-9_]+)\s*\{")
FLD = re.compile(r"^(?:pub(?:\([a-z_]+\))?\s+)?([a-z_][a-z0-9_]*)\s*:\s*[^:;]")
TOK = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")

# 台账：实测零读、但**已确证是有意为之**的项。值为理由，理由不成立就该出声。
LEDGER_Q = {("CaptchaVerify", "_code"):
            "gaps_http/captcha.rs 注释写明「预留：算术题在 issue 时校验」"}
LEDGER_B = {}


def strip_strings(txt):
    """逐行剥**行内闭合**的字符串字面量；跨行续写宁可漏剥（=漏报）也不吞代码。"""
    out = []
    for ln in txt.split("\n"):
        ln = re.sub(r'r#"(.)*?"#"', " ", ln)
        ln = re.sub(r'r"(\\.|[^"\\])*"', " ", ln)
        out.append(re.sub(r'"(?:\\.|[^"\\\n])*"', " ", ln))
    return "\n".join(out)


def strip_comments(txt):
    txt = re.sub(r"/\*.*?\*/", " ", txt, flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in txt.split("\n"))


def scrub(txt):
    return strip_comments(strip_strings(txt))


files = []
for dirpath, _dirs, names in os.walk(API_SRC):
    files += [os.path.join(dirpath, n) for n in names if n.endswith(".rs")]
assert len(files) > 200, "扫到的 .rs 太少＝ROOT 指错（%d）" % len(files)

raw, stripped = {}, {}
for f in files:
    t = open(f, encoding="utf-8", errors="replace").read()
    raw[f], stripped[f] = t, scrub(t)
ALLTXT = " ".join(stripped.values())
# 一次分词出全仓频次：逐字段跑正则扫 3MB 文本要几分钟（写口普查首跑就是这么卡死的）
FREQ = Counter(TOK.findall(ALLTXT))

defs, used_q, used_b = {}, set(), set()
for f, txt in raw.items():
    s = stripped[f]
    used_q.update(QRX.findall(s))
    used_b.update(BRX.findall(s))
    for m in DEF.finditer(s):
        name, i, depth, j = m.group(1), m.end(), 1, m.end()
        while j < len(s) and depth:
            depth += (s[j] == "{") - (s[j] == "}")
            j += 1
        for ln in s[i:j - 1].splitlines():
            st = ln.strip()
            mm = FLD.match(st)
            if mm and not st.startswith("#"):
                defs.setdefault(name, {})[mm.group(1)] = (f, st)


def reads(fd, defline):
    """整仓出现次数 − 定义行自身出现次数＝真正的读点数。"""
    return max(0, FREQ.get(fd, 0) - TOK.findall(defline).count(fd))


def census(names):
    out = []
    for name in sorted(names & set(defs)):
        for fd, (f, defline) in sorted(defs[name].items()):
            if reads(fd, defline) == 0:
                out.append((name, fd, f))
    return out


nfields = sum(len(v) for k, v in defs.items()
              if k in used_q or k in used_b)
ok("MD1 基数自证：查询/写口结构体 ≥80、字段 ≥250（掉下限＝尺子读不到代码）",
   len(used_q | used_b) >= 80 and nfields >= 250,
   "结构体=%d 字段=%d 文件=%d" % (len(used_q | used_b), nfields,
                                   len(files)))

CTRL = [("ListQuery", "category_ids"), ("ListQuery", "category_id_alias"),
        ("RssQuery", "linktype"), ("UserListQ", "fval"),
        ("ListQuery", "per_page"), ("ListQuery", "limit")]
ctl = {"%s.%s" % (n, fd): reads(fd, defs[n][fd][1])
       for n, fd in CTRL if n in defs and fd in defs[n]}
low = {k: v for k, v in ctl.items() if v < 1}
ok("MD2 剥法自证：六条「确实有人读」的参数读点必须 ≥1（第一版在这里露馅）",
   len(ctl) >= 5 and not low, "实测=%s 掉线=%s" % (ctl, low or "-"))

cq = census(used_q)
got = {(n, fd) for n, fd, _ in cq} - set(LEDGER_Q)
stale = set(LEDGER_Q) - {(n, fd) for n, fd, _ in cq}
ok("MD3 查询参数零读集合 = 台账（多一个=新假开关，少一个=接线了要销号）",
   not got and not stale,
   "新增=%s 已消失=%s 台账=%d" % (sorted(got), sorted(stale), len(LEDGER_Q)))

cb = census(used_b)
gotb = {(n, fd) for n, fd, _ in cb} - set(LEDGER_B)
staleb = set(LEDGER_B) - {(n, fd) for n, fd, _ in cb}
ok("MD4 写口字段零读集合 = 台账（多一个=用户填了不生效，少一个=补了读要销号）",
   not gotb and not staleb,
   "新增=%s 已消失=%s 写口结构体=%d"
   % (sorted(gotb), sorted(staleb), len(used_b)))
for nm, fd, f in cb:
    print("    [写口零读] %-30s %-22s %s" % (nm, fd, f))

summary()
