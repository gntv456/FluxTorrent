# -*- coding: utf-8 -*-
"""P-37 守卫：通知偏好的「会发」与「可关」必须同一套键。

三处的键各自维护、无共同来源（P-37 的根因）：
  ① 后端白名单 `community_http/notice.rs` 的 `const KEYS`（存储层受理集）；
  ② 前端偏好页渲染源 `i18n/*.ts` 的 `noticePrefs.keys`（用户能看到并拨动的集）；
  ③ 实际会发的 kind：`u_notice_enabled(_,'<kind>')` 的字面量。
判据（在册 ≠ 可见 的镜像问题：会发 ≠ 可关）：
  · UI 三语键集必须一致；
  · ② ⊆ ①（UI 能拨的必须存得下）；
  · ③ ⊆ ①（会发的必须存得下）；
  · ③ ⊆ ② ∪ push_*（会发的事件类必须可关；push_* 走 users.notice_prefs
    另一段 JSON、不属站内信事件类，由 push_notify.rs 单独消费，豁免）。
只拦新增：历史遗留如果哪天要清，先改脚本常量再谈。
"""
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
from pt_audit_lib import ok, summary  # noqa: E402


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as f:
        return f.read()


def ui_keys(rel):
    s = read(rel)
    # 两式：zh-CN 是主对象内的属性 `noticePrefs: {`；en/zh-TW 是
    # `const noticePrefsXx: Dict[...] = {`。取第一处带 keys 块的定义。
    pat = r"noticePrefs\w*: (?:Dict\[\"noticePrefs\"\] = )?\{"
    for m in re.finditer(pat, s):
        j = s.find("as Record<string, string>", m.start())
        if j < 0:
            continue
        ki = s[m.start():j].find("keys: {")
        if ki < 0:
            continue
        block = s[m.start():j][ki:]
        ks = re.findall(r"^\s*([a-z_][a-z0-9_]*):\s", block, re.M)
        got = {k for k in ks if k != "keys"}
        if got:
            return got
    raise AssertionError(rel)


def whitelist():
    s = read("apps/api/src/community_http/notice.rs")
    i = s.find("const KEYS")
    assert i > 0
    j = s.find("];", i)
    return set(re.findall(r'"([a-z_]+)"', s[i:j]))


NOTICE_KINDS = {
    "hr_prewarn", "hr_violation", "wishlist", "group_new_version",
    "network_new_release", "subgroup_new_release", "resurrection",
    "class_promo", "gift", "comment_reply", "system", "donate_tier",
    "request_fulfilled",
}


def sent_kinds():
    """扫 api+worker 全部 u_notice_enabled 调用附近出现的 kind 字面量。

    三种形态：① 直接字面量 `u_notice_enabled($1,'x')`；② 跨行直接量；
    ③ CASE 表达式里的多个字面量（见 review_side_effects.rs 的 0332/0334）。
    ③ 无法从调用点精确定位，改为在调用点后 220 字符窗内收集所有
    `'<小写+下划线>'` 串，再用 NOTICE_KINDS allowlist 滤掉 SQL 维度
    值（'network'/'subtitle_group' 等）——窗内出现的都必须是白名单键。
    """
    kinds = set()
    for base in ("apps/api/src", "apps/worker/src"):
        for dirpath, _, files in os.walk(os.path.join(ROOT, base)):
            for fn in files:
                if not fn.endswith(".rs"):
                    continue
                text = read(os.path.relpath(os.path.join(dirpath, fn), ROOT))
                for m in re.finditer(r"u_notice_enabled\(", text):
                    seg = text[m.end():m.end() + 220]
                    for s in re.findall(r"'([a-z][a-z0-9_]{2,})'", seg):
                        if s in NOTICE_KINDS:
                            kinds.add(s)
    return kinds


ui = {f: ui_keys("apps/web/i18n/%s" % f)
      for f in ("zh-CN.ts", "en.ts", "zh-TW.ts")}
print("== P-37 通知偏好键对账 ==")
base = ui["zh-CN.ts"]
for f, ks in ui.items():
    ok("%s 有 noticePrefs.keys 且与 zh-CN 同集" % f, ks == base,
       "差集=%s" % sorted(ks ^ base) if ks != base else "%d 键" % len(ks))

wl = whitelist()
ok("白名单 const KEYS 存在", len(wl) > 0, "%d 键" % len(wl))
missing_ui = base - wl
ok("UI 可拨的键白名单都受理（UI ⊆ 存量）", not missing_ui,
   "白名单缺=%s" % sorted(missing_ui) if missing_ui else "")

sent = sent_kinds()
ok("成功扫到会发 kind（前置：非空才有意义）", len(sent) > 0,
   "%s" % sorted(sent))
not_stored = sent - wl
ok("会发的 kind 白名单都存得下（会发 ⊆ 存量）", not not_stored,
   "白名单缺=%s" % sorted(not_stored) if not_stored else "")
mutable = base | {k for k in wl if k.startswith("push_")}
unmutable = sent - mutable
ok("会发的事件类都有关闭开关（push_* 另段豁免）", not unmutable,
   "无法关=%s" % sorted(unmutable) if unmutable else "可关=%s" % sorted(sent))
summary()
