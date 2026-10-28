# -*- coding: utf-8 -*-
"""闸门 MK：正则类站点设定必须在保存/预检那一刻被拒（P-443 的活体钉）。

背景：`upload_title_pattern` 的出厂说明原先教「Java 正则」，而本站引擎是
Rust `regex` crate——它**不支持后查与反向引用**且不报错：站长照说明写的规范名
会编译失败或静默永不匹配，之后每一个真实发种都被「站点命名正则非法，请联系站长」
（publish_http/upload_precheck.rs 的 check_quality）挡住，而设置页一切正常。
修法是 `settings_meta.options.rule` = "regex" 时，`validate_field` 当场编译一次
并拒（复用批四十八 P-186 那把唯一的尺：预检口与写入口同一个函数）。

为什么必须有这条（不是重复 Rust 单测）：
  · 单测只证「函数会拒」，**不证这台键的 options 里真写着 rule=regex**——
    少那一行整段校验根本不进入，面板与单测都看不出，线上仍是旧行为 ⇒ MK4；
  · 也不证「没配 rule 的文本键没被顺手拦死」⇒ MK5 现取一把无 rule 的 text 键
    喂同一个坏正则，必须放行（防过度收紧把全站文本设定拦死）；
  · MK6 钉面板文案：不能再出现「Java」这种教错语法的字面。

跑法：python scripts/pt_audit_mk_regex_setting_live.py（纯预检口，零写入）
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, jwt_token, ok, psql_stdin, summary

KEY = "upload_title_pattern"
BS = chr(92)                      # 单个反斜杠，避免各处转义写法互相骗
SAMPLE = "^[^.]+[" + BS + ".]{4}[" + BS + ".](1080p|2160p)"
BAD = [("后查", "(?<=x)y"), ("反向引用", "^(a)" + BS + "1$"),
       ("未闭合字符类", "^[" + BS + "d")]
tk = jwt_token("root")


def post(name, value):
    return call("POST", "/admin/settings/validate",
                {"name": name, "value": value}, tk)


def field_err(r):
    d = (r or {}).get("data")
    if isinstance(d, list) and d:
        return str(d[0].get("error") or "")
    return ""


s_ok, r_ok = post(KEY, SAMPLE)
ok("MK1 前置：预检口可达且出厂示例放行（读不到数＝token/服务问题，别当产品洞）",
   s_ok == 200 and (r_ok or {}).get("code") == 0,
   "http=%s code=%s msg=%s" % (s_ok, (r_ok or {}).get("code"),
                               str((r_ok or {}).get("message"))[:40]))

hits = []
for label, bad in BAD:
    s, r = post(KEY, bad)
    hits.append((label, s, "须为合法正则表达式" in field_err(r)))
ok("MK2 三类坏形状（后查/反向引用/未闭合）必须 400 且点名原因",
   all(h[1] == 400 and h[2] for h in hits),
   "实测=%s" % (hits,))

s_empty, r_empty = post(KEY, "")
ok("MK3 空值仍是「不校验」的既有语义（必须放行）",
   s_empty == 200 and (r_empty or {}).get("code") == 0, "http=%s" % s_empty)

rule = psql_stdin("SELECT coalesce(options->>'rule', '(无)') FROM "
                  "settings_meta WHERE name = '%s';"
                  % KEY).splitlines()[0].strip()
ok("MK4 这台键真的挂着 rule=regex（少这行 MK2 测的是不存在的分支＝假绿）",
   rule == "regex", "options->>'rule'=%s" % rule)

# 对照键必须挑**根本没有 rule 的 text 键**：第一版只排除 `rule=regex`，
# 结果挑中 `BASEURL`（它带 `rule:"url"`，被拒是**正确行为**）——
# 判据的筛选条件少覆盖一种取值，就把别人的正当拒绝读成自己的洞。
other = psql_stdin(
    "SELECT name FROM settings_meta WHERE type IN ('text','textarea') "
    "AND coalesce(options::text, '') NOT LIKE '%rule%' ORDER BY name "
    "LIMIT 1;").splitlines()
other_name = other[0].strip() if other and other[0].strip() else ""
if other_name:
    s_o, r_o = post(other_name, "(?<=x)y")
    ok("MK5 反向钉：没配任何 rule 的文本键不得被拦（防把全站文本设定拦死）",
       s_o == 200 and (r_o or {}).get("code") == 0,
       "对照键=%s http=%s code=%s err=%s"
       % (other_name, s_o, (r_o or {}).get("code"), field_err(r_o)[:60]))
else:
    ok("MK5 反向钉：取不到对照的无 rule 文本键 ⇒ 这条没跑（不算通过）",
       False, "对照键=无")

hint = psql_stdin("SELECT coalesce(hint, '') FROM settings_meta "
                  "WHERE name = '%s';" % KEY).splitlines()[0]
ok("MK6 面板提示不能再教 Java 写法（教错＝站长写出永不匹配的正则）",
   "Java" not in hint, "hint=%s" % re.sub(r"\s+", " ", hint)[:80])

summary()
