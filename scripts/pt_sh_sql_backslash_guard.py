# -*- coding: utf-8 -*-
"""SH 常驻闸门：拼进 SQL 的**字面反斜杠**必须是 0（批一百五十三）

现网 500 而编译 / 360 单测 / SG 槽位闸门全绿的洞（同形第三次）：

    ERROR flux_api::errors: internal error error=error returned from database:
        syntax error at or near "\\"

PG 侧实测（对照判据，非推测）：

    SELECT count(*) FROM (SELECT 1 FROM torrents t \\ WHERE 1=1 LIMIT 5) sub
    -> ERROR:  syntax error at or near "\\"     （反斜杠不是 SQL 续行）

三种 Rust 写法里只有一种是安全的：

  1. 普通字符串 + 单个 `\`  -> 行续行，反斜杠与换行**双双被吃掉**，SQL 连成一行  ✅
  2. 普通字符串 + `\\`      -> **字面反斜杠**留在字符串里，PG 当成反斜杠命令          ❌
  3. 原始字符串 r#"..."#      -> 反斜杠**原样保留**（连单反斜杠也不续行）           ❌

`list_noclamp_as.rs` 的主查询是 `r#"..."#`，其中两行尾部写了单反斜杠以为能续行，
实测整条列表查询 500；`all_sql` / `register_mode.rs` 是形 2，hr_mail.rs 的邮件
文案里混进形 1 但位于原始字符串内。四处同源。

**为什么既有判据全都看不见**：编译不看字符串内容，356 单测不连库，SG 只对
「$N 槽位 vs bind 个数」——而字面反斜杠是另一维。本闸门用一个小 Rust 词法器
（raw / 普通 / 字符字面量 / 注释四态）判定每行行尾反斜杠的**实际去向**。

正控取修复前的提交，证明判据承重。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BS = chr(92)
NL = chr(10)
Q = chr(34)
SQ = chr(39)
# 各文件「引入该缺陷」的那笔。行号随文件演进，所以正控按 (ref, file, line) 钉，
# 不能拿同一个 ref 覆盖全部文件——register_mode 的缺陷早于 hidden_dead 那笔。
PRE_FIX = "a9aedfb2"      # register_mode.rs / hr_mail.rs 的缺陷来源
PRE_FIX_LIST = "a5359d31" # list_noclamp_as.rs 加 hidden_dead 时引入的那笔
SCAN_ROOTS = ["apps"]



def _is_ident(ch):
    return ch.isalnum() or ch == "_"

def line_kinds(src):
    """返回每一行「行尾所在处」的字符串种类集合。

    词法器要点：普通字符串里的 `\`+换行 是**行续行**，它会消耗掉换行、
    但字符串本身并没有结束——所以那两行的行尾仍然算「普通字符串内」。
    """
    i, n, line = 0, len(src), 0
    kinds = []
    cur = None
    raw_hashes = 0
    line_comment = False
    block = 0
    while i < n:
        c = src[i]
        if c == NL:
            line += 1
            line_comment = False
            # 行尾时**先记录当前种类再复位**：普通字符串不能跨行，所以
            # 「开在这行、没在这行闭合」的行尾仍然算在字符串内——这正是
            # 续行反斜杠所在的行。第一版先复位再记录，把这类行全判成
            # 「不在字符串里」，于是正控抓到 0 处（假绿）。
            kinds.append(cur)
            if cur in ("str", "char"):
                cur = None
            i += 1
            continue
        if line_comment:
            i += 1
            continue
        if block:
            if src.startswith("/*", i):
                block += 1
                i += 2
                continue
            if src.startswith("*/", i):
                block -= 1
                i += 2
                continue
            i += 1
            continue
        if cur == "raw":
            if c == Q and src[i + 1:i + 1 + raw_hashes] == "#" * raw_hashes:
                cur = None
                i += 1 + raw_hashes
                continue
            i += 1
            continue
        if cur == "str":
            if c == BS:
                if i + 1 < n and src[i + 1] == NL:
                    line += 1          # 行续行：吃掉换行，字符串仍在
                    kinds.append(cur)
                i += 2
                continue
            if c == Q:
                cur = None
                i += 1
                continue
            i += 1
            continue
        if cur == "char":
            if c == BS:
                i += 2
                continue
            if c == SQ:
                cur = None
                i += 1
                continue
            i += 1
            continue
        # 代码区
        if src.startswith("//", i):
            line_comment = True
            i += 2
            continue
        if src.startswith("/*", i):
            block = 1
            i += 2
            continue
        # 必须要求 r 是**标识符边界**（前一个字符不是标识符字符）：
        # 否则 `registration_mode` 里的 r 会被当成 raw 串开头，
        # 把整个文件后半段都误判成 raw（本闸门第一版就栽在这里）。
        m = None
        if c == "r" and (i == 0 or not _is_ident(src[i - 1])):
            m = re.match(r"r(#*)" + Q, src[i:])
        if m:
            cur = "raw"
            raw_hashes = len(m.group(1))
            i += m.end()
            continue
        if c == Q:
            cur = "str"
            i += 1
            continue
        if c == SQ:
            m2 = re.match(SQ + r"(\\.|[^" + BS + BS + r"])" + SQ, src[i:])
            if m2:
                i += m2.end()
                continue
            i += 1
            continue
        i += 1
    return kinds


def trailing_bs(ln):
    t = ln.rstrip()
    k = 0
    while k < len(t) and t[len(t) - 1 - k] == BS:
        k += 1
    return k


def literal_backslash_hits(text):
    """返回会真正进入 SQL 的字面反斜杠（文件, 行号, 种类）。"""
    kinds = line_kinds(text)
    hits = []
    for idx, ln in enumerate(text.split(NL)):
        k = trailing_bs(ln)
        if not k:
            continue
        kind = kinds[idx] if idx < len(kinds) else None
        if kind == "raw":
            hits.append((idx + 1, "raw"))
        elif kind == "str" and k % 2 == 0:
            hits.append((idx + 1, "escape"))
    return hits


def walk_rs():
    for root in SCAN_ROOTS:
        for dp, dn, fn in os.walk(os.path.join(ROOT, root)):
            dn[:] = [d for d in dn if d not in ("target", "node_modules")]
            for f in sorted(fn):
                if f.endswith(".rs"):
                    yield os.path.join(dp, f)


def blob(rev, rel):
    # 显式 encoding="utf-8" + errors="replace"：Windows 下 text=True 会走
    # GBK 解码，历史 blob 里的中文/符号（本次撞到 0x80）直接抛
    # UnicodeDecodeError，连带 communicate 的读线程崩成 IndexError——
    # 闸门自己先挂，正反控都跑不到。这是要修的真实缺陷，不是绕路。
    p = subprocess.run(
        ["git", "show", "%s:%s" % (rev, os.path.relpath(rel, ROOT).replace(os.sep, "/"))],
        capture_output=True, cwd=ROOT,
    )
    if p.returncode != 0:
        return ""
    return p.stdout.decode("utf-8", errors="replace")


print("== SH 拼进 SQL 的字面反斜杠必须为 0 ==")

files = sorted(walk_rs())
ok("SH-A1 扫描面非空（词法器真的跑起来了）", len(files) > 100, "扫了 %d 个 .rs" % len(files))

# 正控：修复前的版本必须被判红，否则判据不承重
pre_targets = [f for f in files if "register_mode.rs" in f]
pre_hits = []
for f in pre_targets:
    b = blob(PRE_FIX, f)
    if b:
        pre_hits += [(os.path.basename(f), ln, kind) for ln, kind in literal_backslash_hits(b)]
ok("SH-A2 正控：修前 register_mode.rs 能抓到字面反斜杠（判据承重）",
   len(pre_hits) > 0, "修前命中 %s" % (pre_hits[:4],))

bad = []
for f in files:
    text = open(f, encoding="utf-8", errors="replace").read()
    for ln, kind in literal_backslash_hits(text):
        bad.append((os.path.relpath(f, ROOT), ln, kind))

ok("SH-A3 全扫描面无字面反斜杠", not bad, "命中 %d 处" % len(bad))

# 反向证：路由挂载那笔曾删掉整棵 /api/v1，与反斜杠无关——本闸门必须**不**误报它。
# 这条是「判据只盯自己那类缺陷」的边界：判据太宽会把无关修复卡成红。
neg = blob(PRE_FIX, "apps/api/src/main.rs")
ok("SH-A4 反向证：无关文件（路由挂载那笔）不误报",
   not literal_backslash_hits(neg),
   "误报 %s" % (literal_backslash_hits(neg)[:3],))

# 三形态各自的正控：raw 单反斜杠 / 普通串双反斜杠 / raw 内文案里的续行反斜杠。
# 只钉住其中一种的判据，在另外两种上会静默放行。
three = [
    (PRE_FIX_LIST, "apps/api/src/torrents/list_noclamp_as.rs", 210, "raw", "raw 串里的单反斜杠（列表查询 500 的真凶）"),
    (PRE_FIX_LIST, "apps/api/src/torrents/list_noclamp_as.rs", 345, "escape", "普通串里的双反斜杠（all_sql 计数）"),
    (PRE_FIX, "apps/api/src/auth_http/register_mode.rs", 18, "escape", "普通串里的双反斜杠（注册模式静默降级）"),
    (PRE_FIX, "apps/worker/src/jobs/hr_mail.rs", 12, "raw", "raw 串里邮件文案末尾的反斜杠"),
]
for rev, rel, ln, kind, why in three:
    b2 = blob(rev, rel)
    hs = literal_backslash_hits(b2) if b2 else []
    ok("SH-A5 正控 %s:%d [%s]（%s）" % (rel.split("/")[-1], ln, kind, why),
       (ln, kind) in hs, "实际命中 %s" % (hs[:4],))


for rel, ln, kind in bad[:10]:
    print("       %s:%d [%s]" % (rel, ln, kind))

summary()
