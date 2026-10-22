# -*- coding: utf-8 -*-
"""普查：写进 audit_log 的 action 键，后台审计页有没有一句人话标签。

会员/职员视角：`dict.auditLabels[row.action] ?? row.action`
（apps/web/app/(main)/admin/_parts/admin-overview-panel.tsx:218）。
表本身的成文纪律（apps/web/i18n/audit-labels.ts 头注释）：未收录的动作
**显示原始 key 并加「未收录动作」提示**——不白屏，但职员看到的仍是字段名，
正是这张表 2026-09-30 被建出来要治的那个反馈。

判据口径（三条防呆，缺一就把工具自己的噪声当缺陷）：
  * 只认「实参位上的字符串字面量」：`.audit(actor, "k", ref)` 第 2 个实参、
    `.audit_detail(...)` 第 2 个；模块内自建 `audit()` 助手按它的形参名
    `action` 所在下标取，不猜。
  * `format!` / 变量 / 插件动态名 → 记 skipped 并**报出来**，不当缺陷也不当已覆盖。
  * 先剥 `//` 与 `///` 注释再扫（自己写的说明会造候选，别人注释里的键名会洗白洞）。
  * 写入口有三族：Repo 原语、SQL 里内联的单引号字面量（export/groups/worker）、
    插件动态名——三族都要覆盖，只扫第一族会漏。

用法：python scripts/audit_action_labels.py [--json]
"""
import json
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DIRS = [os.path.join(ROOT, "apps", "api", "src"),
        os.path.join(ROOT, "apps", "worker", "src")]
LABELS = os.path.join(ROOT, "apps", "web", "i18n", "audit-labels.ts")

CALL = re.compile(r"(?<!fn\s)(\.)?\b(audit|audit_detail)\s*\(")
FNDEF = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)")
# SQL 字面量里内联的 action：VALUES (nextval(...), $1|NULL, '键', $N::jsonb)
SQL_ACT = re.compile(
    r"VALUES\s*\(\s*nextval\s*\([^)]*\)\s*,\s*(?:\$[0-9]+|NULL)"
    r"\s*,\s*'([^']+)'")
KEY_OK = re.compile(r"^[0-9A-Za-z][A-Za-z0-9_.]*(?::[A-Za-z0-9_.]+)?$")
STR_LIT = re.compile(r'"([^"\\\n]*)"')
# 测试专用文件不是产线写入口（domain/terms/tests.rs 那种整份是测试的）
TEST_FILE = re.compile(r"(^|[\\/])tests\.rs$|[\\/]tests[\\/]")


def strip_comments(text):
    """按行剥 // 注释：只在双引号之外才认行注释；逐字符跟引号状态。"""
    out = []
    for line in text.split("\n"):
        res, in_str, esc, i = [], False, False, 0
        while i < len(line):
            ch = line[i]
            if in_str:
                res.append(ch)
                if esc:
                    esc = False
                elif ch == "\\":
                    esc = True
                elif ch == '"':
                    in_str = False
                i += 1
                continue
            if ch == '"' and line[:i].rstrip().endswith("r#") is False:
                in_str = True
                res.append(ch)
                i += 1
                continue
            if ch == "/" and i + 1 < len(line) and line[i + 1] == "/":
                break
            res.append(ch)
            i += 1
        out.append("".join(res))
    return "\n".join(out)


def match_paren(text, open_idx):
    """从左括号起配平（跳过双引号内容），返回括号内文本与右括号下标。"""
    depth, i, in_str, esc = 0, open_idx, False, False
    while i < len(text):
        ch = text[i]
        if in_str:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
            i += 1
            continue
        if ch == '"':
            in_str = True
        elif ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
            if depth == 0:
                return text[open_idx + 1:i], i
        i += 1
    return None, None


def split_args(inner):
    parts, depth, cur, i = [], 0, [], 0
    in_str, esc = False, False
    while i < len(inner):
        ch = inner[i]
        if in_str:
            cur.append(ch)
            if esc:
                cur.append(inner[i + 1])
                i += 2
                esc = False
                continue
            if ch == "\\":
                cur.append("")
                esc = True
            elif ch == '"':
                in_str = False
            i += 1
            continue
        if ch == '"':
            in_str = True
            cur.append(ch)
            i += 1
            continue
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
        i += 1
    parts.append("".join(cur))
    return [p.strip() for p in parts]


def literal_of(arg):
    m = re.match(r'^r#*"(.*)"$', arg, re.S) or re.match(r'^"(.*)"$', arg, re.S)
    return m.group(1) if m else None


def action_param_index(body):
    """自建助手：形参里叫 action 的那个是第几个（从 0 数）。"""
    names = []
    for p in split_args(body):
        p = re.sub(r"<[^>]*>", "", p)
        m = re.match(r"^(?:&mut\s+\*\*)?(\w+)", p.strip())
        names.append(m.group(1) if m else "")
    return names.index("action") if "action" in names else None


def scan_file(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        raw = f.read()
    raw = raw.split("#[cfg(test)]", 1)[0]          # 测试模块不是产线写入口
    text = strip_comments(raw)
    local = {}
    for m in FNDEF.finditer(text):
        if m.group(1) in ("audit", "audit_detail"):
            idx = action_param_index(m.group(2))
            if idx is not None:
                local[m.group(1)] = idx
    found, skipped = [], []
    for m in CALL.finditer(text):
        prev = text[m.start() - 1] if m.start() else ""
        if prev == ":":                     # `tg::audit(` = 域函数，不是审计写入口
            continue
        has_dot, name = m.group(1), m.group(2)
        inner, _ = match_paren(text, m.end() - 1)
        if inner is None:
            skipped.append((os.path.relpath(path, ROOT), m.group(0), "括号配平失败"))
            continue
        args = split_args(inner)
        if has_dot:
            idx = 1
        else:
            # 模块内自建助手：定义可能写在调用之后（pack_import 就是），
            # 按名字认，不按位置认。
            idx = local.get(name)
        if idx is None or idx >= len(args):
            skipped.append((os.path.relpath(path, ROOT), m.group(0),
                            "助手签名没认出 action 实参位"))
            continue
        lit = literal_of(args[idx])
        # 条件式实参（`if body.approve { "a" } else { "b" }`）也是产线写入口：
        # 把该实参里的每个字符串字面量都算上，一个都不漏。
        cands = [lit] if lit is not None else STR_LIT.findall(args[idx])
        bad = [c for c in cands if not KEY_OK.match(c)]
        cands = [c for c in cands if KEY_OK.match(c)]
        if not cands:
            kind = ("动态名(format!)" if "format!" in args[idx]
                    else "变量/无字面量")
            skipped.append((os.path.relpath(path, ROOT),
                            args[idx][:60].replace("\n", " "), kind))
        for c in bad:
            skipped.append((os.path.relpath(path, ROOT), c,
                            "字面量不像键名"))
        for c in cands:
            found.append((c, os.path.relpath(path, ROOT),
                          text[:m.start()].count("\n") + 1))
    # SQL 内联字面量一族
    for mm in re.finditer(r"INSERT INTO audit_log", text):
        back = text.rfind('"', 0, mm.start())
        fwd = text.find('"', mm.end())
        if back < 0 or fwd < 0:
            continue
        seg = text[back:fwd + 1]
        for sm in SQL_ACT.finditer(seg):
            lit = sm.group(1)
            if KEY_OK.match(lit):
                found.append((lit, os.path.relpath(path, ROOT),
                              text[:back].count("\n") + 1))
            else:
                skipped.append((os.path.relpath(path, ROOT), lit,
                                "SQL 内联值不像键名"))
    return found, skipped


def label_keys(drop=frozenset()):
    """标签表键集；drop 供闸门做正控（内存里抹掉一枚，不落盘改文件）。

    只取 Zh 块（到 Tw 块起始为止）：把范围写成「Zh 到 En」会把 Tw 一起算进来，
    于是「简中缺、繁中有」这种洞会被读成已覆盖——那正是 JK2 要单独钉的洞。
    """
    with open(LABELS, encoding="utf-8", errors="replace") as f:
        s = f.read()
    start = s.index("auditLabelsZh")
    end = s.index("export const auditLabelsTw")
    body = s[start:end]
    keys = re.findall(r'^\s*(?:"([^"]+)"|([A-Za-z0-9_]+)):\s*"', body, re.M)
    return {a or b for a, b in keys if (a or b)} - set(drop)


def written_actions():
    """产线写入口的 action 全集（键 → 写处列表）。"""
    actions, skipped = {}, []
    for d in DIRS:
        for dirpath, _, files in os.walk(d):
            for fn in files:
                if not fn.endswith(".rs"):
                    continue
                p = os.path.join(dirpath, fn)
                if TEST_FILE.search(p.replace("\\", "/")):
                    continue
                f, sk = scan_file(p)
                for lit, rel, line in f:
                    actions.setdefault(lit, []).append("%s:%d" % (rel, line))
                skipped += sk
    return actions, skipped


def main():
    as_json = "--json" in sys.argv
    actions, skipped = written_actions()
    keys = label_keys()
    missing = sorted(k for k in actions if k not in keys)
    dead = sorted(k for k in keys if k not in actions)
    if as_json:
        print(json.dumps({"written": len(actions), "labels": len(keys),
                          "missing": missing, "dead": dead,
                          "locs": {k: actions[k] for k in missing},
                          "skipped": skipped}, ensure_ascii=False, indent=2))
        return
    print("写入侧 distinct action = %d；标签表 = %d" % (len(actions), len(keys)))
    print("\n[未收录] 后台会显示原始 key 的 action（%d 条）：" % len(missing))
    for k in missing:
        print("  %-34s 写于 %s" % (k, ", ".join(actions[k][:3])))
    print("\n[动态名/变量] 静态扫不到，须人工定性（%d 条）：" % len(skipped))
    for rel, arg, why in skipped[:20]:
        print("  %-24s %s  ← %s" % (rel[-24:], arg[:40], why))
    if len(skipped) > 20:
        print("  …另有 %d 条同类" % (len(skipped) - 20))
    print("\n[只在历史里] 标签表有、代码不再写（%d 条，历史行仍要能读）"
          % len(dead))


if __name__ == "__main__":
    main()
