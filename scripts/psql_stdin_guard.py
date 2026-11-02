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

硬规矩（每个 stdin 调用点各自必须满足）：
  ① 失败要**抛异常**（print 后继续＝假绿）；
  ② 要么 argv 带 `-v ON_ERROR_STOP=1`，要么显式查 stderr 里的 ERROR。
     **`-e ON_ERROR_STOP=1` 不算**（P-552 实测）：那是设给 docker exec 的
     环境变量，psql 不读它，语句报错照样 rc=0、ERROR 只在 stderr。

2026-11-02 改版（P-xxx，站长问「为什么 master CI 绿着这两道门却红」）：
  · 判据从**整文件**降到**调用点**：一个文件里两处 psql，一处带 flag、
    一处裸跑，旧写法会被带 flag 那处洗白（实测 pt_audit_dz_offer_vote.py
    正是这个形状）；
  · 认 `"ERROR" in` 也认 `"ERROR:" in`（psql 的错误行带 `psql:<stdin>:N:`
    前缀，不少人按 `ERROR:` 匹配，旧字面量把这批判成了假红）；
  · 公共层豁免只看**真调用**（`psql_stdin(`），文件里出现这个词但不调用它
    不再白拿豁免——否则棘轮能被一句注释掏空。

存量放行：`scripts/psql_stdin_baseline.json`（路径 → 放行理由）。
新写的脚本不在基线里就直接红；基线只减不增（谁修好谁从基线里划掉）。

不算数的形状（都在代码里注了原因）：
  · `-c` / `-tAc` argv 形态——那种写法 psql 出错**确实**返回非 0，rc 可信；
  · `sh(args, stdin=None)` / `run(cmd, …, input=)` 这类包装站点——argv 与抛点
    都在调用方，按整文件找信号（真实 stdin 调用点仍逐个量）；
  · 扫描器自己（源码里就写着 `input=` 与 `"psql"` 字样）。
2026-11-02 建基线时三条放行理由都是**人工核对过**的，不是「先塞进基线再说」：
backup_drill / pt_ep_notice_pref_guard 的 stdin 调用点确实带开关（EP 的
`-tAq` 两处是本批补上的——那是「SET 静默失败 → SELECT 读旧值 → 判成写了 0 行」
的真假绿形状），pt_audit_nc_sql_plannable 是把 ERROR 文本当测量数据的探针。
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
# argv 里给 psql 的 ON_ERROR_STOP（跨行也算，但必须是 `-v` 成对出现）
STOP = re.compile(r'"-v",\s*"ON_ERROR_STOP=1"')
# 显式查 stderr 的几种字面量写法（ERROR 与 ERROR: 等价）
STDERR = re.compile(r'''['"]ERROR:?['"]\s+in''')
# 用了公共层（真调用，不是提一嘴）
USES_LIB = re.compile(r'psql_stdin\s*\(')
# 调用点向上找 argv 起点、向下找 raise 的距离上限（实测本仓助手都在这个范围）
HEAD, TAIL = 700, 600


def resolve_argv(text, head):
    """把 `subprocess.run(PSQL, input=…)` 这类**间接 argv** 展开。

    本仓有 29 个文件把 psql 命令表提成模块级常量（`PSQL = [...]`）再传给
    subprocess——常量定义往往在几十行开外，落在 700 字符窗口之外，于是
    「明明带了 `-v ON_ERROR_STOP=1`」被判成裸跑（假红）。按标识符回表取
    那段列表字面量拼进窗口，判据才量到真实 argv。
    """
    i = head.rfind('subprocess.run(')
    if i < 0:
        return head
    seg = head[i + len('subprocess.run('):]
    m = re.match(r'\s*([A-Za-z_][\w\.]*)', seg)
    if not m:
        return head
    # 常量也可能定义在函数体里（backup_drill 的 `psql = [...]` 就在 main 内），
    # 只认顶格会把带着 flag 的调用判成裸跑。
    dm = re.search(r'^[ 	]*%s\s*=\s*\[' % re.escape(m.group(1)), text, re.M)
    if not dm:
        return head
    k = dm.end() - 1
    depth = 0
    while k < len(text):
        if text[k] == '[':
            depth += 1
        elif text[k] == ']':
            depth -= 1
            if depth == 0:
                break
        k += 1
    return head + "\n" + text[dm.start():k + 1]


def psql_blocks(text):
    """每个含 `"psql"` 的 argv 列表字面量（stdin 形态才纳入判据）。

    `-c` / `-tAc` 形态不算：那种写法 psql 出错**确实**返回非 0（本仓 88+ 个
    调用点走它，rc 可信），拿 `-v ON_ERROR_STOP=1` 去要求它们只会造假红。
    """
    out = []
    for m in re.finditer(r'"psql"', text):
        # 向左找这个列表的起点
        i = text.rfind('[', 0, m.start())
        if i < 0:
            continue
        depth, k = 0, i
        while k < len(text):
            if text[k] == '[':
                depth += 1
            elif text[k] == ']':
                depth -= 1
                if depth == 0:
                    break
            k += 1
        block = text[i:k + 1]
        if re.search(r'"-tAc"|"(\w*), ?-c"|-c"|\s"-c"', block):
            continue
        out.append((i, block))
    return out


def stdin_sites(text):
    """该文件里所有「psql 从 stdin 读」的调用点 → (行号, 原因)。"""
    bad = []
    blocks = psql_blocks(text)
    for m in INPUT.finditer(text):
        val = (m.group(1) or '').strip()
        if val in ('None', 'none', ''):
            continue
        head = text[max(0, m.start() - HEAD):m.start()]
        if 'psql' not in head and not blocks:
            continue
        argv_text = resolve_argv(text, head)
        tail = text[m.end():m.end() + TAIL]
        win = argv_text + text[m.start():m.end()] + tail
        # ① 失败必须有信号。`sh(args, stdin=None)` 这类包装函数的抛点
        #    在调用方（隔几十行），按整文件找；直接站点仍按窗口找。
        wrapper = bool(re.search(r'subprocess\.run\(\s*(args|cmd|argv)'
                                 r'\s*[,)]', head))
        sig_ok = SIGNAL.search(text) if wrapper else SIGNAL.search(win)
        if not sig_ok:
            bad.append((text[:m.start()].count('\n') + 1,
                        '失败不抛异常（print 后继续 → 假绿）'))
            continue
        # ② 每个 stdin 形态的 psql argv 都要带 -v ON_ERROR_STOP=1
        #    （整文件级会把「一处对一处裸」的第二个 site 洗白，故逐个量）
        naked = [b for _, b in blocks
                 if not (STOP.search(b) or STDERR.search(b))]
        if 'psql' in argv_text and not (STOP.search(argv_text)
                                        or STDERR.search(win)):
            bad.append((text[:m.start()].count('\n') + 1,
                        '既没用 `-v ON_ERROR_STOP=1` 也不查 stderr'
                        '（`-e` 是 docker 环境变量，psql 不认：报错也 rc=0）'))
        elif blocks and len(naked) != len(blocks) and naked:
            bad.append((text[:m.start()].count('\n') + 1,
                        '同文件里有 %d/%d 处 psql argv 是裸跑的'
                        % (len(naked), len(blocks))))
    return bad


def offenders():
    base = {}
    if os.path.exists(BASELINE):
        base = json.load(open(BASELINE, encoding='utf-8'))
    bad = []
    for name in sorted(os.listdir(SCRIPTS)):
        if not name.endswith('.py') or name == os.path.basename(__file__):
            continue      # 扫描器自己不参与（源码里就写着 input= 与 psql 字样）
        path = os.path.join(SCRIPTS, name)
        text = open(path, encoding='utf-8', errors='replace').read()
        if 'psql' not in text or name in base:
            continue
        sites = stdin_sites(text)
        if not sites:
            continue
        reasons = '；'.join('L%d %s' % (ln, why) for ln, why in sites)
        bad.append((name, reasons))
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