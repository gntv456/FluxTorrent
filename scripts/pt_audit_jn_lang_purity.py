# -*- coding: utf-8 -*-
"""常驻闸门 JN：英文字典里不许夹未翻译的中文（P-555 的反方向）。

会员视角：语言能切到 English（`site_settings.default_language` /
users.site_language，config.ts 的 `siteLangToLocale` 是唯一换算口）。切成英文后
一屏里蹦出一句简中，就是「界面语言不纯」——P-555 是错误码缺英文标签，
这条量的是整本字典的值。

键集对齐不用查：`en: Dict` 由 tsc 强制（缺键编译不过）；本闸门只管**没翻过的值**。
值取自运行时字典（apps/web/tests/i18n-magic-leaves.test.ts 导的平铺表），
不是正则扫 ts —— 行级正则会漏「值换行写」的条目（P-558 那批错过一次）。

2026-10-22 实测：en 7108 条值里只有 3 条含汉字，且都是语言名
（简体中文／繁體中文／日本語 —— 语言列表按母语显示是惯例），
所以本批是净结论；JN 是防以后加文案时漏翻的护栏。

用法：python scripts/pt_audit_jn_lang_purity.py
"""
import io
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEAVES = os.path.join(ROOT, "_tmp_dict_leaves.json")
CJK = re.compile(u"[一-鿿]")
# 承认清单：语言名按母语显示（endonym）是刻意的，不是漏翻
ALLOW = {"subtitles.langChs", "subtitles.langCht", "subtitles.langJpn"}


def load():
    if not os.path.exists(LEAVES):
        raise SystemExit("缺 _tmp_dict_leaves.json：先在 apps/web 跑 "
                         "`npx vitest run tests/i18n-magic-leaves.test.ts`")
    return json.load(io.open(LEAVES, encoding="utf-8"))


def main():
    d = load()
    zh = [k for k, v in d["flat"].items() if CJK.search(v)]
    ok("JN0 尺子有牙：zh-CN %d/%d 条值含汉字（基数不自证＝走树空转）"
       % (len(zh), d["total"]),
       len(zh) >= d["total"] * 0.6 and d["total"] > 2000,
       "命中数远低于全量说明 walk/正则坏了，下面的绿不算数")

    en = {k: v for k, v in d["en"].items() if CJK.search(v)}
    extra = sorted(set(en) - ALLOW)
    gone = sorted(ALLOW - set(en))
    ok("JN1 英文值里的汉字只剩语言名 3 条（多一条＝漏翻，少一条＝清单过期）",
       not extra and not gone,
       "新增漏翻=%s；清单里已消失=%s" % (extra[:6], gone[:6]))
    for k in extra[:10]:
        print("   [漏翻] %-40s %s" % (k[:40], en[k][:64]))

    # 正控：内存里把一条正常英文值改成中文，判据必须抓到它
    probe = next((k for k, v in d["en"].items()
                  if not CJK.search(v) and len(v) > 3), None)
    if probe:
        fake = dict(d["en"])
        fake[probe] = u"这条没翻过"
        hit = [k for k, v in fake.items() if CJK.search(v)]
        ok("JN2 正控：把 %s 换成中文后必须落进命中集" % probe,
           probe in hit and probe not in ALLOW, "命中=%d 条" % len(hit))
    else:
        ok("JN2 正控：拿不到干净的英文样本，判据未验过", False)

    summary()


if __name__ == "__main__":
    main()
