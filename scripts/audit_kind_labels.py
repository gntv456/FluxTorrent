"""普查：火花流水 kind 的中文标签是否覆盖全（用户视角=流水页那一列）。

判据口径（避免把统计词当成候选）：
  * 只认「余额原语调用的第 4 个实参是字符串字面量」这一形状
    （earn_spark / spend_spark / clawback_spark 及其 *_tx 变体）；
    传变量的、以及直接 INSERT spark_ledger 的，**记 skipped 并报出来**，
    不当成已覆盖也不当成缺陷。
  * 现网 distinct(kind) 作为第二把尺（静态扫不到的历史值也在这儿）。
  * 标签集 = apps/web/i18n/zh-CN.ts 里 my.kinds 的键
    （`sparkKindLabels` 只读这一处；页面是 `kindLabels[r.kind] ?? r.kind`
     ⇒ 没登记的键会把英文原样甩给会员）。
用法：python scripts/audit_kind_labels.py [--json]
"""
import json
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
API = os.path.join(ROOT, "apps", "api", "src")
I18N = os.path.join(ROOT, "apps", "web", "i18n", "zh-CN.ts")
FUNCS = (r"earn_spark|earn_spark_tx|spend_spark|spend_spark_tx"
         r"|clawback_spark|clawback_spark_tx")
# 前面是 `fn ` 的是**函数定义**（形参里就有 kind: &str），不是调用
CALL = re.compile(r"(?<!fn\s)\b(?:%s)\s*\(" % FUNCS)
# 直插 spark_ledger 的写入口（不走余额原语）：kind 是 VALUES/SELECT 里的
# 字面量列。social_team_reward 就是这一类——只扫原语调用会漏掉它。
INSERT = re.compile(r"INSERT\s+INTO\s+spark_ledger", re.I)
LIT = re.compile(r"'([a-z][a-z0-9_]{2,})'")
# 这些不是 kind（幂等键模板、列名、排除项等），按实际语句里出现过的串排除
NOT_KIND = {"spark_ledger", "social_team_reward_id", "user_id", "kind"}


def read(p):
    try:
        with open(p, encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError:
        return ""


def split_args(text, open_idx):
    """从 '(' 之后按顶层逗号切分（忽略嵌套括号与字符串内的逗号）。"""
    depth, in_str, esc = 0, False, False
    cur, out = [], []
    # 从开括号**之后**开始：把 '(' 也计一层会让内部逗号永远切不开
    i = open_idx + 1
    while i < len(text):
        c = text[i]
        if in_str:
            cur.append(c)
            if esc:
                esc = False
            elif c == "\\":
                esc = True
            elif c == '"':
                in_str = False
            i += 1
            continue
        if c == '"':
            in_str = True
            cur.append(c)
        elif c in "([{":
            depth += 1
            cur.append(c)
        elif c in ")]}":
            if depth == 0:
                out.append("".join(cur))
                return out, i
            depth -= 1
            cur.append(c)
        elif c == "," and depth == 0:
            out.append("".join(cur))
            cur = []
        else:
            cur.append(c)
        i += 1
    return out, i


def code_kinds():
    lit, skipped, ins = set(), [], set()
    for dirpath, _d, files in os.walk(API):
        for fn in files:
            if not fn.endswith(".rs"):
                continue
            p = os.path.join(dirpath, fn)
            txt = read(p)
            rel = os.path.relpath(p, API).replace(os.sep, "/")
            for m in CALL.finditer(txt):
                args, _ = split_args(txt, m.end() - 1)
                if len(args) < 4:
                    skipped.append((rel, "实参不足 4 个"))
                    continue
                a = args[3].strip()
                mm = re.fullmatch(r'"([^"]+)"', a)
                if mm:
                    lit.add(mm.group(1))
                else:
                    skipped.append((rel, "kind 非字面量: "
                                    + a.replace("\n", " ")[:40]))
            for m in INSERT.finditer(txt):
                win = txt[m.end(): m.end() + 420]
                # 只取列清单之后的 VALUES/SELECT 段，避开 (user_id, kind, …)
                body = re.split(r"VALUES|SELECT", win, maxsplit=1)
                if len(body) < 2:
                    continue
                for cand in LIT.findall(body[1]):
                    if cand not in NOT_KIND:
                        ins.add(cand)
    return lit, skipped, ins


def dict_keys(lang="zh-CN"):
    src = read(os.path.join(ROOT, "apps", "web", "i18n", "%s.ts" % lang))
    # my.kinds 这一段是 4 空格缩进的对象、以 `    },` 收尾；按缩进抓边界
    # 比按括号配对稳（段里没有函数调用，不会被 split_args 的 ')' 判定误导）
    m = re.search(r"\n    kinds: \{([\s\S]*?)\n    \}", src)
    if not m:
        raise SystemExit("%s.ts 里找不到 my.kinds＝尺坏了" % lang)
    keys = set()
    for frag in m.group(1).split("\n"):
        mm = re.match(r"\s*(?:\"([^\"]+)\"|'([^']+)'|([A-Za-z_][\w]*))\s*:",
                      frag)
        if mm:
            keys.add(next(g for g in mm.groups() if g))
    return keys


def db_kinds():
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tA", "-c", "select distinct kind from "
         "spark_ledger"],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return {l.strip() for l in (r.stdout or "").splitlines() if l.strip()}


def main():
    lit, skipped, ins = code_kinds()
    keys = dict_keys()
    dbk = db_kinds()
    universe = lit | dbk          # 硬判据只用这两把尺
    missing = sorted(universe - keys)
    unused = sorted(keys - universe)
    print("原语字面量 %d；直插 SQL 候选 %d；现网 %d；标签键 %d"
          % (len(lit), len(ins), len(dbk), len(keys)))
    only_ins = sorted(ins - universe)
    if only_ins:
        print("\n（提示）INSERT spark_ledger 里扫到、但没在现网/原语出现的字面量"
              "——直插语句的列位没核对，不据此判缺陷：")
        print("   " + ", ".join(only_ins))
    print("\n未登记标签（会员会在流水页看到英文原键）：")
    for k in missing:
        src = []
        if k in lit:
            src.append("原语")
        if k in dbk:
            src.append("现网")
        print("   %-26s 来自 %s" % (k, "+".join(src)))
    print("\n登记了但没人再写的历史键（可留可清，不算缺陷）：")
    print("   " + (", ".join(unused) if unused else "无"))
    print("\nskipped（静态求不出，需人工看一眼）：")
    for f, why in skipped[:20]:
        print("   %-46s %s" % (f, why))
    if len(skipped) > 20:
        print("   …共 %d 条" % len(skipped))
    if "--json" in sys.argv:
        print(json.dumps({"missing": missing, "unused": unused,
                          "skipped": skipped}, ensure_ascii=False))
    return missing


if __name__ == "__main__":
    main()
