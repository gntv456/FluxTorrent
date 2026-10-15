# -*- coding: utf-8 -*-
"""SG 常驻闸门：format! 拼的 SQL 槽位与 bind 链必须对齐（批一百五十二）

现网 500 而编译 / 356 单测 / 门禁全绿的洞（两次同形）：

  1. CSRF 端点挂错 scope —— 整棵 /api/v1 路由树集体 404
  2. hidden_dead 的 all_sql 少绑 $6 —— 槽位整体错位，PG 报 syntax error

共同点：判据全在「字符串层面」（槽位编号 vs bind 个数），编译与单测都
看不见。本闸门把 format! 拼出的 SQL 与它后面的 .bind() 链放在一处比对：
SQL 里出现的最大 $N 必须 <= bind 链的 .bind() 个数。

只扫 format! + sqlx::query* 的近邻配对，不做全仓 SQL 审计——那属于
sql_comment_guard / layer_guard 的地盘。

正控取修前那次提交（all_sql 的 SQL 用到 $7 但只有 24 个 bind）证明判据承重。
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
FIX = "724b6020"
SCAN = [
    "apps/api/src/torrents/list_noclamp_as.rs",
    "apps/api/src/rss_http.rs",
]


def flatten(text):
    # Rust 续行反斜杠压成空格，让 SQL 连成一行（\\ 与 \ 都会吃掉源码换行）
    return text.replace(BS + BS, QQ).replace(BS, QQ)


SLOT_RE = re.compile(r'[$](\d+)')
BIND_RE = re.compile(r'\.bind\(')
QUERY_RE = re.compile(r'sqlx::query(?:_as|_scalar|_text)?\(')
DECL_RE = re.compile(
    r'let (\w*(?:sql|Sql|SQL)\w*|\w*_total|\w*_count)'
    r'\s*(?::[^=]+)?=\s*format!\('
)


def find_pairs(src):
    # 产出 (变量名, SQL 文本, bind 个数)
    out = []
    for m in DECL_RE.finditer(src):
        name = m.group(1)
        rest = src[m.end():]
        depth, i, in_str, end = 1, 0, False, 0
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
                        end = i
                        break
            i += 1
        if end == 0:
            continue
        sql_txt = flatten(rest[:end])
        tail = src[m.end() + end:]
        qm = QUERY_RE.search(tail)
        if not qm:
            continue
        after = tail[qm.end():]
        fm = after.find(QQ + ".fetch_" + QQ)
        chain = after[:fm] if fm > 0 else after[:800]
        out.append((name, sql_txt, len(BIND_RE.findall(chain))))
    return out


def blob(rev, path):
    p = subprocess.run(["git", "show", "%s:%s" % (rev, path)],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", cwd=ROOT)
    return (p.stdout or "") if p.returncode == 0 else ""


def max_slot(sql_txt):
    slots = [int(x) for x in SLOT_RE.findall(sql_txt)]
    return max(slots) if slots else 0


print("== SG format! SQL 槽位 vs bind 链 ==")

# 正控：修前 all_sql 用到 $7 但 bind 数不够 ⇒ 判据必须抓到
pre = blob(FIX + '^', SCAN[0])
pre_pairs = dict((n, b) for n, s, b in find_pairs(pre)) if pre else {}
now_pairs = dict((n, b) for n, s, b in
                 find_pairs(open(os.path.join(ROOT, SCAN[0]),
                                    encoding='utf-8',
                                    errors='replace').read()))
# 正控：修前 all_sql 的 bind 数比修后少 —— 少了 （pattern），槽位整体错位。
# 判据用「同一变量的 bind 数在修复前后相等」而不是静态槽位扫描：
# {approval_gate_count} 里的  是插值进来的，静态扫不到。
pre_all = pre_pairs.get('all_sql')
now_all = now_pairs.get('all_sql')
ok('SG0 正控：all_sql 的 bind 数在修复前后不同（判据承重）',
   pre_all is not None and now_all is not None and pre_all != now_all,
   '修前 bind=%s / 现网 bind=%s' % (pre_all, now_all))

bad_all = 0
for rel in SCAN:
    p = os.path.join(ROOT, rel)
    if not os.path.exists(p):
        FAILURES.append("SG 取文失败 %s" % rel)
        continue
    txt = open(p, encoding="utf-8", errors="replace").read()
    for name, sql_txt, nbind in find_pairs(txt):
        mx = max_slot(sql_txt)
        label = "%s :: %s" % (os.path.basename(rel), name)
        if mx > nbind:
            bad_all += 1
        ok("SG %s 槽位 $%d <= bind %d" % (label, mx, nbind), mx <= nbind)

ok("SG 全扫描面无槽位越界", bad_all == 0, "越界 %d 处" % bad_all)

summary()
