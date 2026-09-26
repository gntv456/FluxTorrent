#!/usr/bin/env python3
"""后端校验详情本地化棘轮（四审 L7 第二批的「不再变坏」门）。

两类串分开管，因为修法不同：
  static  —— `Validation("中文…")` 整句字面量 ⇒ 按原句查
             `apps/api/i18n/validation_details.tsv`，
             调用点零改动；**新写的静态校验串必须进表**，否则这里红。
  dynamic —— `Validation(format!("…{x}…"))` ⇒ 没有固定原句可查，必须先参数化成 key；
             本门只锁它的**条数不增**。

基线只记「当前还没译的文本集合 + 动态串条数」，且**按文本**而不是行号记账
（`i18n_guard` 按行号记，插删一行就把存量判成新增，是它现有的坑，这里不重犯）。

用法：
    python scripts/validation_i18n_guard.py            # 门禁（CI 用）
    python scripts/validation_i18n_guard.py --report    # 只出数据，不判成败
    python scripts/validation_i18n_guard.py --update    # 译完一批后收缩基线
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "apps" / "api" / "src"
TABLE = ROOT / "apps" / "api" / "i18n" / "validation_details.tsv"
BASELINE = ROOT / "scripts" / "validation_i18n_baseline.json"
CJK = re.compile(r"[\u4e00-\u9fff]")
CTORS = ("DomainError::Validation(", "DomainError::TorrentInvalid(")


def balanced_end(s, open_idx):
    """从 '(' 起找到配对 ')' 的后一位（忽略字符串字面量里的括号）。"""
    depth, i, in_str, esc = 0, open_idx, False, False
    while i < len(s):
        c = s[i]
        if in_str:
            if esc:
                esc = False
            elif c == "\\":
                esc = True
            elif c == '"':
                in_str = False
        else:
            if c == '"':
                in_str = True
            elif c == "(":
                depth += 1
            elif c == ")":
                depth -= 1
                if depth == 0:
                    return i + 1
        i += 1
    return len(s)


def scan():
    static, dynamic = [], []
    for f in sorted(SRC.rglob("*.rs")):
        text = f.read_text(encoding="utf-8")
        if not CJK.search(text):
            continue
        rel = str(f.relative_to(ROOT)).replace("\\", "/")
        for ctor in CTORS:
            start = 0
            while True:
                k = text.find(ctor, start)
                if k < 0:
                    break
                start = k + len(ctor)
                expr = text[start:balanced_end(text, k + len(ctor) - 1) - 1]
                parts = re.findall(r'"((?:[^"\\]|\\.)*)"', expr)
                zh = [p for p in parts if CJK.search(p)]
                if not zh:
                    continue
                is_fmt = "format!" in expr
                single_static = (
                    not is_fmt and len(zh) == 1 and expr.strip().startswith('"')
                )
                for frag in zh:
                    clean = frag.replace('\\"', '"').replace("\\\\", "\\")
                    (static if single_static else dynamic).append(
                        {"file": rel, "text": clean if single_static else ""}
                    )
    return static, dynamic


def table_keys():
    if not TABLE.exists():
        print(f"FAIL 找不到译文表 {TABLE.relative_to(ROOT)}", file=sys.stderr)
        sys.exit(1)
    keys = set()
    for line in TABLE.read_text(encoding="utf-8").splitlines():
        t = line.strip()
        if t and not t.startswith("#"):
            keys.add(t.split("\t")[0].strip())
    return keys


def main():
    mode = "--update" if "--update" in sys.argv else (
        "--report" if "--report" in sys.argv else "gate")
    static, dynamic = scan()
    uniq = sorted({r["text"] for r in static if r["text"]})
    known = table_keys()
    uncovered = [t for t in uniq if t not in known]
    covered = len(uniq) - len(uncovered)
    pct = (100.0 * covered / len(uniq)) if uniq else 100.0
    print(f"静态详情句 {len(uniq)} 条（出现 {len(static)} 处）／已译 {covered} "
          f"→ 覆盖率 {pct:.1f}%；动态 format! 串 {len(dynamic)} 处（需先参数化 key）")

    if mode == "--report":
        return 0
    if mode == "--update":
        BASELINE.write_text(json.dumps({
            "_comment": "按文本记账（不是行号）；译完一批用 --update 收缩",
            "uncovered_static": uncovered,
            "dynamic_count": len(dynamic),
        }, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        print(f"WROTE {BASELINE.name}: uncovered={len(uncovered)} "
              f"dynamic={len(dynamic)}")
        return 0

    if not BASELINE.exists():
        print("FAIL 缺基线文件，先跑 --update 生成", file=sys.stderr)
        return 1
    base = json.loads(BASELINE.read_text(encoding="utf-8"))
    old = set(base["uncovered_static"])
    now = set(uncovered)
    bad = 0

    added = sorted(now - old)
    for t in added:
        print(f"FAIL 新增未译的校验串（请进 validation_details.tsv 或改走既有句子）："
              f"{t}", file=sys.stderr)
        bad += 1
    if len(dynamic) > base["dynamic_count"]:
        print(f"FAIL 动态校验串从 {base['dynamic_count']} 涨到 {len(dynamic)}："
              "format! 拼出来的句子无法按原句查表，请用已译的字面量或参数化 key",
              file=sys.stderr)
        bad += 1
    fixed = sorted(old - now)
    if fixed and not bad:
        print(f"提示：{len(fixed)} 条已译完，可跑 --update 收缩基线")
    if not bad:
        print("OK: 覆盖率未回退，且没有新增未译串")
    return 1 if bad else 0


sys.exit(main())
