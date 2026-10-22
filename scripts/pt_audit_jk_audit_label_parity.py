# -*- coding: utf-8 -*-
"""常驻闸门 JK：审计动作 → 三语标签的完整性对账（P-556）。

职员视角：后台「最近操作 / 审计日志」那一列走
`dict.auditLabels[row.action] ?? row.action`
（apps/web/app/(main)/admin/_parts/admin-overview-panel.tsx:218、
admin-tool-panels.tsx:140）。表本身的成文纪律是「未收录显示原始 key +
未收录动作提示」——不白屏，但职员看到的还是字段名，正是这张表 2026-09-30
被建出来要治的那句反馈。P-556 实测：产线在写的动作有 90 条没标签，
`dressup.takeoff` 更是被登记成了 `dressup_takeoff`（分隔符写错，
那条文案永远读不到）。同一族先例：P-435/P-316（流水 kind 直出英文键）、
P-555（错误码缺三份方言）。

尺子来自 scripts/audit_action_labels.py（普查与闸门共用一把，别另起）：
  JK0  仪器基数：普查到的写入口 ≥300、标签表 ≥350（防判据空转假绿）
  JK1  静态字面量全集：产线写的每个动作都得有人话
  JK2  三份方言键集必须相同（只补简中＝繁中/英文用户照样看字段名）
  JK3  `format!` 拼出来的动作族：取值一律**从源码 match 臂/调用点解析**，
       不把我此刻看到的清单写死进判据（新加一个批量动作就必须补标签）
  JK4  真不可枚举的动态名（插件名）钉在承认清单里，多一条少一条都红
  JK5  正控：内存里抹掉一枚键，JK1 必须只报那一枚（不落盘改文件）
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary  # noqa: E402
import audit_action_labels as aal  # noqa: E402

ROOT = aal.ROOT
LABELS = aal.LABELS


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8", errors="replace") as f:
        return f.read()


def match_arms(rel, anchor):
    """取 `match X {` 里所有字符串臂名（到第一个 `_ =>` 为止）。

    缩进不写死：torrent_batch 的臂在 8 列、users_batch 在 12 列，
    按锚点后第一条臂的实际缩进定尺（写死一种会把另一种读成空集）。
    """
    s = read(rel)
    seg = s[s.index(anchor):]
    first = re.search(r'^(\s+)"[a-z_]+"(?:\s*\|\s*"[a-z_]+")*\s*=>',
                      seg, re.M)
    if not first:
        return []
    pad = len(first.group(1))
    stop = re.search(r'^\s{%d}_\s*=>' % pad, seg, re.M)
    if stop:
        seg = seg[:stop.start()]
    out = []
    for m in re.finditer(r'^\s{%d}"([a-z_]+)"((?:\s*\|\s*"[a-z_]+")*)\s*=>'
                         % pad, seg, re.M):
        out.append(m.group(1))
        out += re.findall(r'"([a-z_]+)"', m.group(2) or "")
    return list(dict.fromkeys(out))


def blocks():
    """三份方言块各自的键集。"""
    s = read("apps/web/i18n/audit-labels.ts")
    marks = [m.start() for m in
             re.finditer(r"export const auditLabels(Zh|Tw|En)", s)]
    assert len(marks) == 3, "标签表结构变了，闸门得跟着改读法"
    out = []
    for i, a in enumerate(marks):
        b = marks[i + 1] if i + 1 < len(marks) else len(s)
        out.append({(x or y) for x, y in
                    re.findall(r'^\s*(?:"([^"]+)"|([A-Za-z0-9_]+)):\s*"',
                               s[a:b], re.M)})
    return out


def main():
    written, skipped = aal.written_actions()
    keys = aal.label_keys()

    ok("JK0 仪器基数：写入口 %d 个（>=300）、标签 %d 条（>=350）"
       % (len(written), len(keys)),
       len(written) >= 300 and len(keys) >= 350,
       "基数掉了说明普查被改坏，后面的绿不算数")

    missing = sorted(k for k in written if k not in keys)
    ok("JK1 产线在写的每个动作都有人话标签（缺 %d）" % len(missing),
       not missing, "未收录 => 后台显示字段名：" + ", ".join(missing[:10]))

    bl = blocks()
    ok("JK2 三份方言键集相同（zh %d / tw %d / en %d）"
       % (len(bl[0]), len(bl[1]), len(bl[2])),
       bl[0] == bl[1] == bl[2],
       "只补一种语言时，另两种语言的职员仍看到字段名；差集=%s"
       % sorted(bl[0] ^ bl[1] | bl[0] ^ bl[2])[:8])

    # JK3：动态族的取值一律现从源码解析
    fams = []
    fams.append(("increment_bulk.",
                 match_arms("apps/api/src/admin_p3_http/increment_grant.rs",
                            "match body.kind.as_str()")))
    fams.append(("torrent.batch.",
                 match_arms("apps/api/src/admin_p3_http/torrent_batch.rs",
                            "match body.action.as_str()")))
    fams.append(("user.batch.",
                 match_arms("apps/api/src/admin_p3_http/users_batch.rs",
                            "match body.action.as_str()")))
    src = read("apps/api/src/games_http/pool_games.rs")
    fams.append(("game.", re.findall(
        r'pool_round\([^,]+,[^,]+,\s*"([a-z_]+)"', src)))
    st = read("apps/api/src/domain/terms/mod.rs")
    fams.append(("terms.key_", re.findall(
        r'Self::[A-Z][a-z]* => "([a-z]+)"', st)))

    for prefix, vals in fams:
        need = sorted(prefix + v for v in vals if v)
        bad = [k for k in need if k not in keys]
        ok("JK3 %s* 族取值 %d 个都有标签" % (prefix, len(need)),
           bool(need) and not bad,
           "取值集=%s；缺=%s" % (",".join(sorted(vals)), ",".join(bad)))

    # JK4：真枚举不完的动态名（插件名是外部注册的）钉在承认清单
    admit = {"plugin.{name}.sql_reject"}
    site = "apps/api/src/plugin_ctx.rs"
    still = ('format!("plugin.{name}.sql_reject")' in read(site))
    ok("JK4 不可枚举的动态名仍只有承认清单那 1 条（插件 SQL 被拒）",
       still and len(admit) == 1,
       "写点 %s；承认清单=%s" % ("在" if still else "已消失", admit))
    dyn = {m for _, arg, why in skipped if why == "动态名(format!)"
           for m in [re.search(r'format!\("([^"]+)"', arg)] if m}
    names = {re.sub(r"\{[^}]*\}|%s", "{*}", d.group(1)) for d in dyn}
    ok("JK4b 源码里 format! 拼出的动作模板都已被 JK3 覆盖或已承认",
       names <= {"increment_bulk.{*}", "torrent.batch.{*}",
                 "user.batch.{*}", "game.{*}", "terms.key_{*}"},
       "冒出新的动态族，先补取值解析再补标签：%s" % sorted(names))

    probe = "review.reject"
    one = aal.label_keys(drop={probe})
    miss2 = sorted(k for k in written if k not in one)
    ok("JK5 正控：抹掉 %s 后，JK1 必须只报它" % probe,
       miss2 == [probe], "实际报=%s" % miss2[:5])

    summary()


if __name__ == "__main__":
    main()
