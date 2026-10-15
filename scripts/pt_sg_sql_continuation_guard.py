# -*- coding: utf-8 -*-
"""SG 常驻闸门：format! 拼的 SQL 槽位与 bind 链必须对齐（批一百五十二）

现网 500 而编译 / 356 单测 / 门禁全绿的洞（三次同形）：

  1. CSRF 端点挂错 scope —— 整棵 /api/v1 路由树集体 404
  2. hidden_dead 的 all_sql 少绑 $6 —— 槽位整体错位，PG 报 syntax error
  3. hidden_dead 落地时 count_sql 续行重排，误删 medium/alive/approval/status
     四个谓词 —— total 虚高、hidden_dead 恒 0（不报错，只是悄悄说谎）

共同点：判据全在「字符串层面」（槽位编号 vs bind 个数 / 谓词是否齐全），
编译与单测都看不见。

本闸门分两类判据：

  A. **槽位对齐**（替代旧版）：把 format! 里 `{var}` 逐个求值 —— 局部直接
     可算的（字符串字面量、`format!` 生成的谓词）做一次插值展开，再数
     `$N` 的最大值 vs bind 个数。旧版只扫字面量，`approval_gate_count` 里
     的 `$7` 是插值进来的，静态扫不到 —— 那正是 2 号洞能溜过去的原因。

  B. **谓词齐全**：count_sql / all_sql 必须与列表侧 sql 携带同一套过滤
     谓词。3 号洞里 total 之所以说谎，就是因为 count_sql 少了一半谓词。

正控取 724b6020^（all_sql 少绑 $6）与 a5359d31（count_sql 少四个谓词）
两次真洞的修前版本，证明判据承重。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BS = chr(92)
QQ = chr(34)
SQ = chr(39)
FIX_BIND = "724b6020"   # 修前：all_sql 少绑 $6
FIX_PRED = "a5359d31"   # 修后引入：count_sql 误删四个谓词
TARGET = "apps/api/src/torrents/list_noclamp_as.rs"
# 扫描面：所有用 format! 拼 SQL 的文件（新增即自动纳管）
SCAN = [
    TARGET,
    "apps/api/src/rss_http.rs",
]

SLOT_RE = re.compile(r'[$](\d+)')
BIND_RE = re.compile(r'\.bind\(')


# `let <name> = format!(` 的声明式匹配（复用，避免每处超宽行）
FMT_DECL = r'let\s+%s\s*(?::[^=]+)?=\s*format!\('
LIT_DECL = (r'let\s+%s\s*(?::\s*&?str)?\s*=\s*'
            r'"((?:[^"\\]|\\.)*)"')
NAME_BRACE = r'\{[A-Za-z_][A-Za-z0-9_]*\}'


def flatten(text):
    """Rust 续行反斜杠压成空格，让 SQL 连成一行。"""
    return text.replace(BS + BS, QQ).replace(BS, QQ)


def decl_span(src, name):
    """取 `let <name> = format!( ... );` 的完整源码片段。"""
    m = re.search(FMT_DECL % re.escape(name), src)
    if not m:
        return None
    rest = src[m.end():]
    depth, i, in_str = 1, 0, False
    while i < len(rest) and depth > 0:
        ch = rest[i]
        if ch == BS:
            i += 2
            continue
        if ch == QQ:
            in_str = not in_str
        elif not in_str:
            if ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
                if depth == 0:
                    return flatten(rest[:i])
        i += 1
    return None


def chain_bind_count(src, after_name, limit=40):
    """某个 SQL 声明之后的 .bind() 链长度。"""
    m = re.search(FMT_DECL % re.escape(after_name), src)
    if not m:
        return None
    tail = src[m.end():]
    qm = re.search(r'sqlx::query', tail)
    if not qm:
        return None
    after = tail[qm.end():]
    fm = after.find(QQ + ".fetch_")
    seg = after[:fm] if fm > 0 else after[:1200]
    return len(BIND_RE.findall(seg))


def _strip_interp(src, name):
    """取 `let <name> = ...;` 这一条赋值语句的源码（不做插值）。

    用于判断「被 format! 插进去的这个变量会不会带进新的 $N 槽位」。
    找不到赋值语句返回 ''，调用方按「无槽位」处理。
    """
    m = re.search(r'let\s+' + re.escape(name) + r'\b', src)
    if not m:
        return ""
    tail = src[m.end():]
    # 语句结束：第一个顶层分号（跳过括号/引号内）
    depth, i, in_str = 0, 0, False
    while i < len(tail):
        ch = tail[i]
        if ch == BS:
            i += 2
            continue
        if ch in (QQ, SQ):
            in_str = not in_str
        elif not in_str:
            if ch in "([{":
                depth += 1
            elif ch in ")]}":
                depth -= 1
            elif ch == ";" and depth <= 0:
                return tail[:i]
        i += 1
    return tail[:400]


def interpolate(src, sql_txt, depth=0):
    """把 format! 里的 {var} 就地展开（深度受限）。

    只展开同一文件里能静态求值的字符串字面量：
      - `let x = "...";` / `let x = format!("...", ..)`（无嵌套 {}）
      - `format!(...)` 直接产出的变量（谓词生成器的返回值）
    求不出来的保留原样（``），宁可少判据也不误报。
    """
    if depth > 4:
        return sql_txt

    def repl(m):
        var = m.group(1)
        # 1) 直接字面量赋值
        lit = re.search(
            LIT_DECL % re.escape(var), src)
        if lit:
            return flatten(lit.group(1))
        # 2) format!(...) 产出（谓词生成器：内部若还含 {} 则放弃展开）
        fm = re.search(
            r'let\s+' + re.escape(var) + r'\s*(?::[^=]+)?=\s*format!\(', src)
        if fm:
            rest = src[fm.end():]
            d, i, ins, end = 1, 0, False, 0
            while i < len(rest) and d > 0:
                ch = rest[i]
                if ch == BS:
                    i += 2
                    continue
                if ch == QQ:
                    ins = not ins
                elif not ins:
                    if ch == "(":
                        d += 1
                    elif ch == ")":
                        d -= 1
                        if d == 0:
                            end = i
                            break
                i += 1
            if end:
                inner = flatten(rest[:end])
                if "{" in inner:
                    # 嵌套插值：只要被插进去的变量**不带 $N 槽位**
                    # （如白名单枚举拼出的 in_list），外层槽位集合就已
                    # 完全确定 —— 用一层占位替换后继续求值。
                    names = re.findall(r'\{([A-Za-z_][A-Za-z0-9_]*)\}', inner)
                    if names and all(
                            SLOT_RE.search(_strip_interp(src, n)) is None
                            for n in names):
                        inner = re.sub(NAME_BRACE, ' ', inner)
                        return inner
                    return var  # 真的还会带进新槽位，放弃
                return inner
        # 3) 链式调用产出（白名单几处已知谓词生成器）
        chain = re.search(
            r'let\s+' + re.escape(var) + r'\s*(?::[^=]+)?=\s*'
            r'(?:super::)?[a-z_:]+\([^;]*?\)\s*;', src, re.S)
        if chain:
            return var  # 结构不可静态求值：不贡献槽位
        return var

    out = re.sub(r'\{([A-Za-z_][A-Za-z0-9_]*)\}', repl, sql_txt)
    return out if out == sql_txt else interpolate(src, out, depth + 1)


def blob(rev, path):
    p = subprocess.run(["git", "show", "%s:%s" % (rev, path)],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", cwd=ROOT)
    return (p.stdout or "") if p.returncode == 0 else ""


def read(rel):
    p = os.path.join(ROOT, rel)
    if not os.path.exists(p):
        FAILURES.append("SG 取文失败 %s" % rel)
        return ""
    return open(p, encoding="utf-8", errors="replace").read()


def max_slot(txt):
    slots = [int(x) for x in SLOT_RE.findall(txt)]
    return max(slots) if slots else 0


print("== SG 格式串 SQL 槽位 / 谓词 判据 ==")

def extra_slots(src):
    """所有 `extra_preds(ExtraSlots { .. })` 声明的槽位集合。

    这是 SQL 参数范围的**声明式真源**：extra_preds 按这些编号生成
    `$N IS NULL OR ...` 谓词。比「扫字面量数 $N」强得多 —— 谓词是拼
    出来的，字面量扫不到，而这里的编号是明写的。

    返回 [(起始位置, 最高槽位), ..]；列表侧与计数侧各一处，靠位置区分。
    """
    out = []
    for m in re.finditer(r'ExtraSlots\s*\{', src):
        depth, i = 1, m.end()
        while i < len(src) and depth > 0:
            if src[i] == "{":
                depth += 1
            elif src[i] == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        seg = src[m.end():i]
        nums = [int(x) for x in re.findall(r':\s*(\d+)', seg)]
        if nums:
            out.append((m.start(), max(nums)))
    return out


# ---------- A1 正控（真洞）：bind 链必须覆盖 ExtraSlots 声明的全部槽位。
#
# 724b6020 的洞：all_sql 手抄 count_sql 的 bind 链时漏了 $6（pattern），
# bind 数 24 而 ExtraSlots 声明 rating_min=25 ⇒ 差 1。
# 注意：这类洞**扫字面量抓不到** —— 漏绑 $6 后 SQL 里出现的最大字面量槽位
# 仍是 $7（来自 approval_gate_count 的插值），$7 <= 24 判据照样通过。
# 真正的信号是「bind 个数 < 声明的最高槽位」。
pre_bind_src = blob(FIX_BIND + "^", TARGET)
_pre_slots = extra_slots(pre_bind_src)
pre_decl = _pre_slots[-1][1] if _pre_slots else 0
pre_n = chain_bind_count(pre_bind_src, "all_sql")
ok("SG-A1 正控：修前 all_sql 的 bind 数少于 ExtraSlots 声明槽位（判据承重）",
   pre_n is not None and pre_decl > 0 and pre_n < pre_decl,
   "修前 bind=%s / 声明最高槽位=$%d" % (pre_n, pre_decl))

# ---------- A2 现网：每条 SQL 的 bind 数 >= 它声明的槽位上限
now_src = read(TARGET)
_now_slots = extra_slots(now_src)
now_decl_list = _now_slots[0][1] if _now_slots else 0   # 列表侧
now_decl = _now_slots[-1][1] if _now_slots else 0      # 计数侧
ok("SG-A2 ExtraSlots 声明了计数侧槽位（>=25）", now_decl >= 25,
   "声明最高槽位=$%d" % now_decl)

# 列表侧 sql 有自己的 bind 链（游标/LIMIT 额外占槽，只做上界校验）
n_list = chain_bind_count(now_src, "sql")
ok("SG-A3 列表侧 sql 的 bind 数覆盖声明槽位",
   n_list is not None and n_list >= now_decl_list,
   "列表侧 bind=%s / 声明最高槽位=$%d" % (n_list, now_decl_list))

# ---------- A4 共用 bind 链：count_bind.rs 的 add! 次数 == 声明槽位数
# 这是本批修复的结构性收口：count_sql / all_sql 不再各写一遍 bind 链，
# 两者共用 count_bind::CountArgs，槽位数在 add! 宏调用处一目了然。
cb_rel = "apps/api/src/torrents/count_bind.rs"
cb_src = read(cb_rel)
cb_adds = len(re.findall(r'^\s*add!\(', cb_src, re.M))
ok("SG-A4 共用 bind 链 add! 次数 == ExtraSlots 声明槽位数",
   cb_adds == now_decl,
   "count_bind add!=%d / 声明最高槽位=$%d" % (cb_adds, now_decl))

# ---------- A5 两条计数 SQL 都必须走共用 bind 链（不再手抄 .bind 链）
for nm in ("count_sql", "all_sql"):
    s = decl_span(now_src, nm)
    n = chain_bind_count(now_src, nm)
    ok("SG-A5 %s 走共用 bind 链（无独立 .bind 链）" % nm,
       s is not None and n is None, "仍带 %s 个 .bind" % n)

# ---------- A6 兜底：仍对字面量做一次上界扫描（防新写的小 SQL 越界）
bad_scan = 0
for rel in SCAN:
    txt = read(rel)
    if not txt:
        continue
    pat = FMT_DECL % r'(\w*(?:sql|Sql|SQL)\w*)'
    for nm in set(re.findall(pat, txt)):
        s = decl_span(txt, nm)
        n = chain_bind_count(txt, nm)
        if s is None or n is None:
            continue
        mx = max_slot(interpolate(txt, s))
        if mx > n:
            bad_scan += 1
            ok("SG-A6 %s::%s $%d <= bind %d"
               % (os.path.basename(rel), nm, mx, n), False)
ok("SG-A6 全扫描面无字面量槽位越界", bad_scan == 0, "越界 %d 处" % bad_scan)

# ---------- B. 正控 + 判据：count_sql 必须与列表侧同谓词
# a5359d31 引入的洞：count_sql 少了 medium/alive/approval/status。
# 取「修前」(= a5359d31 当时的版本) 证明判据能抓到。
pre_pred = blob(FIX_PRED, TARGET)
NOW_SQL = {
    "medium_pred": "medium_id",
    "alive_pred": "seeders > 0",
    "approval_pred": "approval_status IN",
    "status_pred": "snatches",
}
REQ_PRED = ["{medium_pred}", "{alive_pred}", "{approval_pred}", "{status_pred}"]


def count_body(src):
    s = decl_span(src, "count_sql") or ""
    return s


ok("SG-B0 正控：修前 count_sql 缺 medium/alive/approval/status（判据承重）",
   sum(1 for k in REQ_PRED if k in count_body(pre_pred)) <= 2,
   "修前命中 %d/4" % sum(1 for k in REQ_PRED if k in count_body(pre_pred)))

cur_body = count_body(now_src)
missing = [k for k in REQ_PRED if k not in cur_body]
ok("SG-B1 count_sql 与列表侧同谓词（medium/alive/approval/status）",
   not missing, "缺: %s" % missing)

# all_sql 是 hidden_dead 的「去掉存活」版本：其余谓词必须齐全，
# 但 alive_pred 必须缺席（否则 all_total==total，hidden_dead 恒 0）。
all_body = decl_span(now_src, "all_sql") or ""
all_missing = [k for k in REQ_PRED if k not in all_body and k != "{alive_pred}"]
ok("SG-B2 all_sql 谓词齐全（除存活谓词外）", not all_missing,
   "缺: %s" % all_missing)
ok("SG-B3 all_sql 不含存活谓词（否则差值恒 0）",
   "{alive_pred}" not in all_body, "all_sql 不该带 alive_pred")

summary()
