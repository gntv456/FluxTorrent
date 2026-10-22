# -*- coding: utf-8 -*-
"""常驻闸门 JJ：errors.rs 发射的每个错误码都要有三份方言的人话（P-555）。

会员视角：`dict.errors[err.code] ?? err.message`（注册页 / 登录页 / 后台通用
兜底，见 apps/web/app/(main)/admin/page.tsx 与 login-form.tsx）。缺一条键不会
白屏，但**英文/繁中用户会收到一句简中服务器文案**——这张表存在的目的正是
不让这种事发生。P-435（流水 kind 直出英文键）与 P-316 是同一族，只是这次
缺的是错误码而非流水类型。

判据：
  JJ1  errors.rs 里 `=> NNNN` 发射的码 ⊆ zh-CN / en / zh-TW 三份 errors 段
  JJ2  反向：字典里不许留着码表已删的键（dead key 会掩盖上一次的改动）
  JJ3  ja 走 deepMergeDict(zhCN, ja) 段级回落（i18n/merge.ts 的成文设计），
       所以 ja **可以**没有 errors 段——但必须显式承认，不许哪天有人以为漏了
  JJ4  正控：从字典里抹掉一枚键，JJ1 必须只报那一枚
"""
import io
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ERRS = os.path.join(ROOT, "apps", "api", "src", "errors.rs")
I18N = os.path.join(ROOT, "apps", "web", "i18n")
LOCALES = ["zh-CN", "en", "zh-TW"]


def emitted_codes():
    src = io.open(ERRS, encoding="utf-8", errors="replace").read()
    return sorted(set(re.findall(r"=>\s*(\d{4})\s*,", src)))


def dict_block(loc):
    """取 `errors: { ... }` 整段并返回 {码: 文案}。"""
    t = io.open(os.path.join(I18N, "%s.ts" % loc), encoding="utf-8",
                errors="replace").read()
    i = t.find("\n  errors: {")
    if i < 0:
        return None
    j = t.index("{", i)
    depth, k = 0, j
    while k < len(t):
        if t[k] == "{":
            depth += 1
        elif t[k] == "}":
            depth -= 1
            if depth == 0:
                break
        k += 1
    return dict(re.findall(r'(\d{4}):\s*"([^"]*)"', t[j:k + 1]))


def main():
    codes = emitted_codes()
    ok("JJ0 errors.rs 确实在发多位错误码（读到 %d 枚）" % len(codes),
       len(codes) >= 20, "抽取器失灵会让下面全是空判据")
    tables = {loc: dict_block(loc) for loc in LOCALES}
    for loc in LOCALES:
        tbl = tables[loc]
        if tbl is None:
            ok("JJ1 %s 有 errors 段" % loc, False, "整个段都不在")
            continue
        missing = [c for c in codes if c not in tbl]
        empty = [c for c, v in tbl.items() if not v.strip()]
        ok("JJ1 %s 覆盖 errors.rs 全部 %d 枚码" % (loc, len(codes)),
           not missing and not empty,
           "缺=%s 空文案=%s" % (missing[:6], empty[:4]))
        dead = sorted(set(tbl) - set(codes))
        ok("JJ2 %s 没有码表已删的死键" % loc, not dead, "死键=%s" % dead)
    ja = dict_block("ja")
    merge = io.open(os.path.join(I18N, "merge.ts"), encoding="utf-8",
                    errors="replace").read()
    ok("JJ3 ja 段级回落是成文设计（merge.ts 深合并 + 回落 zh-CN）",
       (ja is None or len(ja) >= 1) and "deepMergeDict" in merge
       and "回落" in merge,
       "ja 有独立 errors 段时也必须与三份齐，否则本判据要改成逐键比对")
    if tables.get("en"):
        stub = {k: v for k, v in tables["en"].items() if k != "4101"}
        miss = [c for c in codes if c not in stub]
        ok("JJ4 正控：抹掉 en 的 4101 后，判据必须只报这一枚",
           miss == ["4101"], "实际=%s" % miss[:4])


if __name__ == "__main__":
    main()
    summary()
