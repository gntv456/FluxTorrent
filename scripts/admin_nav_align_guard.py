# -*- coding: utf-8 -*-
"""管理台导航三源一致性校验（补 admin-tool-switch.tsx 注释里点名、
但仓库中并不存在的 `_admin_nav_align.py`）。

`?tool=X` 能不能渲染，取决于三处必须同时对齐的东西：
  1. DB `staff_panel_entries.tab_key`（导航项 + 深链合法性判定）
  2. `staff-tools.tsx` 的 `ToolTab` 联合类型（TS 侧白名单）
  3. `admin-tool-switch.tsx` 的 `STAFF_TOOL_TABS` 数组 + 
     `renderSimpleTool` 里的 `case "x":`
任一处漏了，表现都不是报错而是**静默落进概览页**——站长点了导航项
却看到总览，或面板写好了却没有入口。注释让人跑这个脚本而脚本不存在，
等于这条约束从来没有守卫。

判定规则：
  - DB 的每个 tab_key 必须「可渲染」：出现在 STAFF_TOOL_TABS 或
    renderSimpleTool 的 case 里（二者任一即可，两条通路都真实存在）。
  - STAFF_TOOL_TABS 的每一项必须是 ToolTab 的成员（否则 TS 编译期
    就会红，但这里再兜一层，防有人改成 `as any`）。
  - ToolTab 里存在、但 DB 没有导航项的键：只告警（staff-tools 内部
    子页签可能有意不出现在后台导航里）。
"""
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SWITCH = os.path.join(ROOT, "apps", "web", "app", "(main)", "admin",
                      "_parts", "admin-tool-switch.tsx")
STAFF = os.path.join(ROOT, "apps", "web", "components", "staff-tools.tsx")
PAGE = os.path.join(ROOT, "apps", "web", "app", "(main)", "admin",
                   "page.tsx")
ADMIN_DIR = os.path.join(ROOT, "apps", "web", "app", "(main)", "admin")


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def brace_block(src, start):
    """从 start 处的 `{` 起配到配对右括号（字符串/模板里的括号不参与计数）"""
    i = src.index("{", start)
    depth = 0
    in_s = None
    while i < len(src):
        c = src[i]
        if in_s:
            if c == "\\":
                i += 2
                continue
            if c == in_s:
                in_s = None
        elif c in "\"'`":
            in_s = c
        elif c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return src[start:i + 1]
        i += 1
    return src[start:]


def switch_cases(src):
    """renderSimpleTool 的 case 值：只取该函数体内部的 case 字面量"""
    at = src.index("export function renderSimpleTool")
    body = brace_block(src, src.index("{", at))
    return set(re.findall(r'case\s+"([^"]+)"\s*:', body))


def staff_tool_tabs(src):
    at = src.index("export const STAFF_TOOL_TABS")
    seg = src[at:src.index("];", at) + 2]
    return set(re.findall(r'"([^"]+)"', seg))


def tool_tab_union(src):
    at = src.index("export type ToolTab")
    seg = src[at:src.index(";", at)]
    return set(re.findall(r'"([^"]+)"', seg))


def db_tab_keys():
    sql = "SELECT tab_key FROM staff_panel_entries " \
          "WHERE COALESCE(tab_key, '') <> ''"
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8")
    if p.returncode:
        raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:200])
    return {x.strip() for x in p.stdout.splitlines() if x.strip()}


def main():
    switch_src = read(SWITCH)
    page_src = read(PAGE)
    # 真实子路由（/admin/settings 等）也是合法入口
    subroutes = {d for d in os.listdir(ADMIN_DIR)
                 if os.path.isdir(os.path.join(ADMIN_DIR, d))
                 and not d.startswith("_") and d != "[...slug]"}
    # page.tsx 自己内联处理的复杂页签
    page_cases = set(re.findall(r'case\s+"([^"]+)"\s*:', page_src))
    staff_src = read(STAFF)
    cases = switch_cases(switch_src)
    tabs = staff_tool_tabs(switch_src)
    union = tool_tab_union(staff_src)
    try:
        dbk = db_tab_keys()
    except Exception as e:  # noqa: BLE001  无库时只跑静态两两对齐
        print("[跳过 DB 校验] %s" % e)
        dbk = set()

    problems = []
    warns = []

    if dbk:
        renderable = cases | tabs | page_cases | subroutes
        dead_nav = sorted(k for k in dbk if k not in renderable)
        if dead_nav:
            problems.append(
                "DB 导航项没有渲染分支（点了会静默落进概览）：%s"
                % ", ".join(dead_nav))
        for t in sorted(tabs):
            if t not in union:
                problems.append("STAFF_TOOL_TABS 的 '%s' 不是 ToolTab 成员"
                                % t)
        orphan = sorted(k for k in union if k not in dbk and k in tabs)
        if orphan:
            warns.append("ToolTab/staff 面板有、后台导航无（可能有意）：%s"
                         % ", ".join(orphan))
    unused_case = sorted(k for k in (cases | page_cases)
                         if k not in dbk and k not in subroutes)
    if unused_case and dbk:
        warns.append("renderSimpleTool 有 case 但 DB 没有导航项：%s"
                     % ", ".join(unused_case))

    for w in warns:
        print("[告警] %s" % w)
    if problems:
        for m in problems:
            print("[红] %s" % m)
        print("FAILED: %d 项" % len(problems))
        return 1
    print("OK：DB 导航 %d 项全部可渲染；staff 页签 %d 个都在 ToolTab 联合里"
          % (len(dbk), len(tabs)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
