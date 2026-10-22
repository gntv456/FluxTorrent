#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""闸门：psql **从 stdin 读 SQL** 的脚本不许吞错误（基线棘轮）。

为什么单拎 stdin 形态：psql 从 **stdin** 读脚本时，只要脚本本身有语法/运行
错误，psql 会把错误打到 stderr、但**退出码仍是 0**（`-c` 形态才会返回 1）。
于是「用 stdin 喂 SQL、只取 stdout」的闸门，遇到一条写错的 SQL 时拿到的是
**空串**而不是报错——判据拿着空串继续跑，轻则莫名其妙红，重则**假绿**
（期望值恰好是空/0 时直接判过）。

2026-10-20 实测被这条咬到：隔离复现 H&R 阶梯限下时，探针 INSERT 写错了列名
（block_invoke vs 真列 block_invite），psql 退出码 0、INSERT 静默没落库，
复现得出「阶梯限下在位也会恢复下载权限」的**错误结论**，差点把 P-445 的
正确设计当成缺陷改掉。

硬规矩（stdin 形态必须同时满足）：
  ① 失败要**抛异常**（print 后继续＝假绿）；
  ② 要么 argv 带 `-v ON_ERROR_STOP=1`，要么显式查 stderr 里的 ERROR。
     **`-e ON_ERROR_STOP=1` 不算**（P-552 实测）：那是设给 docker exec 的
     环境变量，psql 不读它，语句报错照样 rc=0、ERROR 只在 stderr。

存量放行：`scripts/psql_stdin_baseline.json`（路径 → 放行理由）。
新写的脚本不在基线里就直接红；基线只减不增（谁修好谁从基线里划掉）。
"""
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCRIPTS = os.path.join(ROOT, 'scripts')
BASELINE = os.path.join(SCRIPTS, 'psql_stdin_baseline.json')

INPUT = re.compile(r'input\s*=\s*([^,\)\n]+)')
# 「失败有信号」的几种本仓写法：抛异常 / 记进 FAILURES 清单 /
# DO 块里把报错转成 PREPARE_FAIL 通知（sql 预检闸门靠它逐条归因）。
SIGNAL = re.compile(r'raise\s+\w+\(|FAILURES\.append|PREPARE_FAIL|sys\.exit')


def stdin_psql_files():
    """找出「psql 从 stdin 读」的脚本：同一次 subprocess.run 里既有 psql
    又有非 None 的 input=。"""
    out = []
    for name in sorted(os.listdir(SCRIPTS)):
        if not name.endswith('.py'):
            continue
        text = open(os.path.join(SCRIPTS, name), encoding='utf-8',
                    errors='replace').read()
        if 'psql' not in text or 'psql_stdin' in text:
            continue
        for m in INPUT.finditer(text):
            val = (m.group(1) or '').strip()
            if val in ('None', 'none', ''):
                continue
            head = text[max(0, m.start() - 700):m.start()]
            if 'psql' not in head:
                continue
            out.append(name)
            break
    return out


def offenders():
    base = {}
    if os.path.exists(BASELINE):
        base = json.load(open(BASELINE, encoding='utf-8'))
    bad = []
    for name in stdin_psql_files():
        text = open(os.path.join(SCRIPTS, name), encoding='utf-8',
                    errors='replace').read()
        why = []
        if not SIGNAL.search(text):
            why.append('失败不抛异常（print 后继续 → 假绿）')
        if not (re.search(r'"-v",\s*"ON_ERROR_STOP=1"', text)
                or '"ERROR" in' in text):
            why.append('既没用 `-v ON_ERROR_STOP=1` 也不查 stderr'
                       '（`-e` 是 docker 环境变量，psql 不认：报错也 rc=0）')
        if why and name not in base:
            bad.append((name, '；'.join(why)))
    return bad


def main():
    bad = offenders()
    if bad:
        print('psql stdin 闸门红：%d 个脚本可能在吞 SQL 错误' % len(bad))
        for name, why in bad:
            print('  ', name, '—', why)
        print('修法：失败要 raise，且 psql argv 带 -v ON_ERROR_STOP=1'
          '（或显式查 stderr；docker 的 -e 不算）')
        sys.exit(1)
    print('OK: psql stdin 形态的脚本都会在失败时抛异常')


if __name__ == '__main__':
    main()