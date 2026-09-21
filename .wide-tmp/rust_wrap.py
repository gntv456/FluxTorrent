"""Rust 宽行收编 v4（状态机组收集版）。

组 = 从一个宽行里的开引号起，跨若干物理行（字符串内 `\\`+newline 续行），
到闭引号为止。闭引号后的文本为 trailing（`,` 或 `)` 等）。
产出：按顶层逗号重新分配，物理行以 `\\` 结尾续行（Rust 字符串内合法），
仅末行带闭引号。
限制：字符串体内不得含 `"`（SQL 串不含）。
"""
import io
import re
import sys

BS = chr(92)
WIDTH = 80


def scan_close(s, start):
    """在 s[start:] 中找闭引号；返回 (idx, closed)。处理 \\ 转义。"""
    j = start
    while j < len(s):
        if s[j] == BS:
            j += 2
            continue
        if s[j] == '"':
            return j, True
        j += 1
    return -1, False


def collect_group(lines, i):
    """lines[i] 是含字符串开引号的（超宽）行。
    返回 (indent, prefix, sql, trailing, next_i) 或 None。"""
    l = lines[i]
    qi = l.find('"')
    if qi < 0:
        return None
    indent = re.match(r'\s*', l).group(0)
    prefix = l[:qi]
    # 起始行：检查本行是否闭合
    ci, closed = scan_close(l, qi + 1)
    if closed:
        sql = l[qi + 1:ci]
        trailing = l[ci + 1:].strip()
        return indent, prefix, sql, trailing, i + 1
    # 未闭合：字符串跨行（行尾必是 `\`）
    seg = l[qi + 1:]
    parts = [seg.rstrip()[:-1].rstrip() if seg.rstrip().endswith(BS) else seg]
    j = i + 1
    while j < len(lines):
        c = lines[j]
        qi2 = c.find('"')
        if qi2 >= 0:
            ci2, closed2 = scan_close(c, qi2 + 1)
            if closed2:
                parts.append(c[:qi2])
                trailing = c[ci2 + 1:].strip()
                return indent, prefix, ' '.join(x for x in parts if x), trailing, j + 1
            else:
                # 引号开新串？（组内不该出现）保守拼接
                parts.append(c.rstrip()[:-1] if c.rstrip().endswith(BS) else c)
        else:
            stripped = c.rstrip()
            if stripped.endswith(BS):
                parts.append(stripped[:-1].rstrip().lstrip())
            else:
                parts.append(stripped.strip())
        j += 1
    return None


def split_top(s):
    parts, depth, cur, instr = [], 0, '', False
    i = 0
    while i < len(s):
        ch = s[i]
        if ch == "'":
            if instr and i + 1 < len(s) and s[i + 1] == "'":
                cur += "''"
                i += 2
                continue
            instr = not instr
        if not instr:
            if ch == '(':
                depth += 1
            elif ch == ')':
                depth -= 1
            elif ch == ',' and depth <= 0:
                parts.append(cur + ',')
                cur = ''
                i += 1
                continue
        cur += ch
        i += 1
    if cur.strip():
        parts.append(cur)
    return [p.strip() for p in parts if p.strip()]


def greedy(cont_limit, chunks, first_limit):
    rows, cur = [], ''
    limit = first_limit
    for c in chunks:
        if cur:
            cand = cur + ' ' + c
            if len(cand) > limit:
                rows.append(cur)
                cur = c
                limit = cont_limit
            else:
                cur = cand
        else:
            if len(c) > limit:
                for w in c.split(' '):
                    if cur and len(cur + ' ' + w) > limit:
                        rows.append(cur)
                        cur = w
                        limit = cont_limit
                    elif cur:
                        cur = cur + ' ' + w
                    else:
                        cur = w
            else:
                cur = c
    if cur:
        rows.append(cur)
    return rows


def wrap_group(indent, prefix, sql, trailing):
    if '"' in sql:
        raise ValueError('double quote in literal')
    chunks = split_top(re.sub(r'\s+', ' ', sql).strip())
    first_limit = WIDTH - len(indent) - len(prefix) - 3
    cont_limit = WIDTH - len(indent) - 2
    rows = greedy(cont_limit, chunks, first_limit)
    out = []
    for idx, r in enumerate(rows):
        head = indent + prefix + '"' if idx == 0 else indent + ' '
        if idx < len(rows) - 1:
            out.append(head + r + ' ' + BS)
        else:
            out.append(head + r + '"' + trailing)
    return out


def process_file(path):
    lines = io.open(path, encoding='utf-8').read().split('\n')
    out = []
    i = 0
    while i < len(lines):
        l = lines[i]
        if len(l) <= 80 or '"' not in l:
            out.append(l)
            i += 1
            continue
        g = collect_group(lines, i)
        if g is None:
            out.append(l)
            i += 1
            continue
        indent, prefix, sql, trailing, ni = g
        try:
            out.extend(wrap_group(indent, prefix, sql, trailing))
        except ValueError:
            out.append(l)
        i = ni
    io.open(path, 'w', encoding='utf-8', newline='\n').write('\n'.join(out))


if __name__ == '__main__':
    for p in sys.argv[1:]:
        before = sum(1 for x in io.open(p, encoding='utf-8').read().split('\n')
                     if len(x) > 80)
        process_file(p)
        after = sum(1 for x in io.open(p, encoding='utf-8').read().split('\n')
                    if len(x) > 80)
        print(f'{p}: wide {before} -> {after}')
