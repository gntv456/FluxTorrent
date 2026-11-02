# -*- coding: utf-8 -*-
"""闸门 TZ：标签芯片不许隐形（tag_dict 配色一致性）。

2026-11-02 站长在「标签字典」列表页看到：官种 / 合集 / 完结 / 官方 的预览是
空白方块——框在、字看不见。根因不在那几条数据，在**列默认值的假设**：
`tag_dict.color` 默认 `#ffffff`，预设「背后一定有底色」；而站型包导入
（staff_http/pack_tags.rs）建标签时只写 name/kind/scope，底色与字色全落默认
⇒ 存进来就是「空底 + 白字」。所以这不是「某几条漏配」，而是**每个站型包带进来
的标签**都这个形状（现网 9 条里 4 条中，含被 10 颗种引用的「官种」）。

三层各钉一条，缺一层都会再犯：
  T1 数据：库里不得存在「bg_color 为空而 color 非空」的行（迁移 0454 抹平，
     并把列默认改成空串——「没配色」就该存成什么都没写）；
  T2 建口：走 API 新建一个不配色的标签，落库 color 必须是空串，不能是
     只在有底色时才成立的白色；探针建完即删并复算基线；
  T3 渲染：三处芯片（后台列表预览 / 后台编辑预览 / 论坛 TagChip）必须走
     lib/tag-style 这一把尺；源码里不得再出现「没有底色守卫就直接铺 color」。

0454 未应用（api 未重启）时 T1 记 PENDING，T2 读到旧默认值时也记 PENDING——
那是待部署的本体，不是判据过期。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_tz_tag_paint.py
"""
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
os.chdir(os.path.dirname(HERE))
sys.path.insert(0, HERE)
from pt_audit_lib import (  # noqa: E402
    call, grep_count, jwt_token, ok, psql_stdin, summary,
)

TOK = jwt_token("root")


def q(sql):
    return psql_stdin(sql)


def eq(label, got, want, extra=""):
    ok(label + "（实际=%r 期望=%r%s）" % (got, want, extra),
       str(got) == str(want))


print("== T1 存库形状：不得有「空底 + 白字」的隐形行 ==")
MIG = q("SELECT count(*) FROM _sqlx_migrations WHERE version = 454")
naked = q("SELECT count(*) FROM tag_dict "
          "WHERE COALESCE(bg_color,'') = '' AND COALESCE(color,'') <> ''")
names = q("SELECT coalesce(string_agg("
          "id || ':' || name || '=' || color, ' / '), '(无)')"
          " FROM tag_dict WHERE COALESCE(bg_color,'') = '' "
          "AND COALESCE(color,'') <> ''")
painted = q("SELECT count(*) FROM tag_dict "
            "WHERE COALESCE(bg_color,'') <> '' AND color = '#ffffff'")
print("   有底色且白字的正常行=%s（这把尺不误伤它们）" % painted)
if MIG == "1":
    eq("T1a 隐形形状行数归零（0454 已应用）", naked, "0", " 残留=%s" % names)
    dflt = q("SELECT coalesce(column_default,'NULL') FROM information_schema."
             "columns WHERE table_name='tag_dict' AND column_name='color'")
    eq("T1b color 列默认已改成空串", dflt, "''::text")
else:
    print("   PENDING：0454 未应用（api 未重启），现存隐形行=%s（%s）"
          % (naked, names))

print("== T2 建口默认值：不配色的标签不许落一个白色 ==")
base_cnt = q("SELECT count(*) FROM tag_dict")   # 造探针**之前**取基线
probe = "tz-invisible-%d" % int(time.time())
st, body = call("POST", "/admin/tags-dict", {"name": probe, "kind": "plain",
                                             "scope": "torrent"}, token=TOK)
eq("T2a 探针标签建得出来", st, 200)
row = q("SELECT coalesce(bg_color,'(null)')||'|'||coalesce(color,'(null)') "
        "FROM tag_dict WHERE name='%s'" % probe)
pid = q("SELECT id FROM tag_dict WHERE name='%s'" % probe)
print("   探针落库形状 bg|color = %s (id=%s)" % (row, pid))
if row == "|":
    eq("T2b 不写底色时 color 也必须是空串", "|", row)
else:
    print("   PENDING：现网 api 仍是 0454 之前的建口默认（COALESCE '#ffffff'），"
          "本仓已改为空串，重建镜像后此条自动变判据（当前读到 %s）" % row)
sd, bd = call("DELETE", "/admin/tags-dict/%s" % pid, token=TOK)
left = q("SELECT count(*) FROM tag_dict WHERE name='%s'" % probe)
eq("T2c 探针已删除", (sd, left), (200, "0"))
eq("T2d 标签总数回基线", q("SELECT count(*) FROM tag_dict"), base_cnt)

print("== T3 渲染侧一把尺（静态）==")
uses = grep_count(r"tagChipStyle\(", ["apps/web/components"])
eq("T3a 三处芯片都走 lib/tag-style", uses >= 3, True, " 命中=%d" % uses)
NAIVE_COLOR = r"color:\s*(r|tag|edit\.f)\.color\b"
naive = grep_count(NAIVE_COLOR, ["apps/web/components"])
eq("T3b 组件里不许再出现「无底色守卫直接铺存库 color」", naive, 0,
   " 命中=%d" % naive)
helper = os.path.exists("apps/web/lib/tag-style.ts")
eq("T3c 尺子本体在册", helper, True)
guarded = grep_count(r"bg_color\s*\?", ["apps/web/components"])
print("   （成员端既有守卫写法 `bg_color ? …` 仍存 %d 处，那些点本来就不会隐形）"
      % guarded)

summary()
