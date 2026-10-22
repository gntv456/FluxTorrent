# -*- coding: utf-8 -*-
"""普查：英文字典里没翻过的中文值（P-555/P-435 同族，只是这次缺的是翻译）。

会员视角：站点语言能切到 en（`site_settings.default_language` / 用户
`site_language`，config.ts 的 `siteLangToLocale` 是唯一换算口），语言切成
English 后**界面本该全是英文**；一条没翻的中文值就会让英文用户在一屏英文里
撞见一句简中——和 P-555（错误码缺英文标签）、P-435（流水 kind 直出英文键）
是同一族「界面语言不纯」的缺陷，方向相反而已。

判据口径：
  * 值取自**运行时字典**（`en: Dict` 的叶子），不是正则扫 ts：行级正则会漏
    「值换行写」的条目（P-558 那批就这么错过）。
  * 只认 CJK 表意文字（U+4E00-U+9FFF）；中日共写的汉字（如「確認」「登録」）
    在日文里也合法，所以本普查只看 en，不看 ja。
  * 键集对齐不用查：`en` 被标成 `Dict`，缺键编译不过（tsc rc=0 即证）。
  * 正控：同一把尺量 zh-CN，命中数必须接近全量（>2000）；否则说明
    正则/走树是空的，en 的「没几条」就不能当结论。

用法：python scripts/audit_lang_cjk.py
"""
import io
import json
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEAVES = os.path.join(ROOT, "_tmp_dict_leaves.json")
CJK = re.compile(u"[一-鿿]")
# 有意的中文：示例路径、货币默认名、站内梗（列出来人工看，不静默放行）
ALLOW_NOTE = {
    "common.currencyDefault": "货币名默认值本身就是中文词",
}


def scan(map_name, flat):
    hits = [(k, v) for k, v in flat.items() if CJK.search(v)]
    return hits


def main():
    if not os.path.exists(LEAVES):
        raise SystemExit("缺 _tmp_dict_leaves.json：先在 apps/web 跑 "
                         "`npx vitest run tests/i18n-magic-leaves.test.ts`")
    d = json.load(io.open(LEAVES, encoding="utf-8"))
    zh = scan("flat", d["flat"])
    en = scan("en", d.get("en") or {})
    print("zh-CN 中文叶子 %d（基数自证：应≈全量 %d）" % (len(zh), d["total"]))
    if len(zh) < d["total"] * 0.6:
        print("  ↑ 尺子可疑：zh-CN 命中数远低于全量，别信 en 那一条的结论")
        sys.exit(1)
    print("en 里夹着中文的叶子 = %d 条 / 共 %d 条英文值"
          % (len(en), len(d.get("en") or {})))
    grp = {}
    for k, _v in en:
        top = k.split(".")[0]
        grp[top] = grp.get(top, 0) + 1
    print("按段分布（前 12）：%s" % sorted(grp.items(),
                                          key=lambda x: -x[1])[:12])
    for k, v in en[:40]:
        print("  %-44s %s" % (k[:44], v[:64]))
    if len(en) > 40:
        print("  …另有 %d 条" % (len(en) - 40))


if __name__ == "__main__":
    main()
