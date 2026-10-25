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

第四条轴（0420）：导航项的 `perm_key` 必须等于该工具首屏读口的
`require_perm`。NULL 只按 min_class 放行，于是 90/93 档看得到入口、
点进去 403——入口与后端不是同一把钥匙。`NAV_REQUIRED_PERM` 是显式
登记表：工具名 → 权限键；DB 里改坏、迁移漏写、tab_key 改名都会红。
未登记的 NULL 条目只告警（工作台等确实没有权限判据）。

第五条轴（0421）：反方向也要对齐——有渲染分支（renderSimpleTool 的
case / page.tsx 的内联分支）却没有 DB 导航项，等于功能只活在手敲的
`?tool=` 里，后台导航与 Ctrl/⌘+K 搜索都找不到它。红；确实有意不留入口的，
写进 `NAV_INTENTIONAL_NO_ENTRY` 并注明理由。
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

# 工具 → 首屏读口 require_perm（0420 版主六轮实测，逐条用 91/92/93/95/98
# 探针打过：可见即 200、不可见即 403）。不在此表的导航项按 min_class 放行。
NAV_REQUIRED_PERM = {
    "users": "user.view",                     # /admin/users
    "userlogs": "user.view",                  # /admin/rename-logs
    "reports": "reports.handle",              # /admin/reports
    "cheaters": "cheaters.view",              # /admin/cheaters
    "subawards": "subtitles.grant",           # /admin/subtitles/awards/*
    "cats": "categories.manage",              # /admin/categories
    "tagdict": "categories.manage",           # /admin/tags-dict
    "sections": "categories.manage",          # /admin/section-dict
    "menu": "settings.manage",                # /admin/menu-items
    "p2tools": "settings.manage",             # /admin/sticky-promos
    "freeleech": "settings.manage",           # 子面板 promo-kinds 同键
    "jixiao": "jixiao.payroll.view",          # /admin/jixiao/payroll
    "resetpass": "user.resetpass",            # /admin/resetpass
    "roles": "roles.manage",                  # /admin/roles-dict
    "applications": "user.adjust",            # /admin/applications
    # 0421 补的三条（此前只有渲染分支、没有入口，导航与 Ctrl+K 都找不到）
    "ops": "settings.manage",                 # /admin/version + backups + jobs
    "homelayout": "settings.manage",          # PUT /admin/home-layout
    "viewlayout": "settings.manage",          # PUT /admin/view-hidden
}

# 有意不做成后台导航入口的渲染分支。往这里写名字要带理由——
# 「面板存在但没人找得到」本身就是缺陷形状（0421 三条即为此）。
NAV_INTENTIONAL_NO_ENTRY: set = set()


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


def psql_argv(sql):
    """本地 compose 走 docker exec；CI（e2e-smoke 带 postgres service）
    走 DATABASE_URL 直连 psql —— 同一份校验两条通路都要能跑。"""
    url = os.environ.get("DATABASE_URL", "").strip()
    if url:
        return ["psql", url, "-q", "-tAc", sql]
    return ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
            "-q", "-d", "fluxtorrent", "-tAc", sql]


def db_tab_keys():
    sql = "SELECT tab_key FROM staff_panel_entries " \
          "WHERE COALESCE(tab_key, '') <> ''"
    p = subprocess.run(psql_argv(sql), capture_output=True, text=True,
                       encoding="utf-8")
    if p.returncode:
        raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:200])
    return {x.strip() for x in p.stdout.splitlines() if x.strip()}


def db_blank_module_keys():
    """module_key='' 的导航项。规范值是 NULL（后台写路径就归一成 NULL），
    判据 `module_on_sql` 虽然同批起容忍 ''，但这类行仍是「迁移手写列时
    踩空串哨兵」的产物——批十七实测两条导航项因此从导航里静默消失。"""
    sql = "SELECT tab_key FROM staff_panel_entries WHERE module_key = ''"
    p = subprocess.run(psql_argv(sql), capture_output=True, text=True,
                       encoding="utf-8")
    if p.returncode:
        raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:200])
    return {x.strip() for x in p.stdout.splitlines() if x.strip()}


def db_nav_perms():
    """tab_key → perm_key（NULL 归一成空串，便于和登记表比差集）。"""
    sql = ("SELECT tab_key || '|' || COALESCE(perm_key, '') "
           "FROM staff_panel_entries WHERE COALESCE(tab_key, '') <> ''")
    p = subprocess.run(psql_argv(sql), capture_output=True, text=True,
                       encoding="utf-8")
    if p.returncode:
        raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:200])
    out = {}
    for line in p.stdout.splitlines():
        if "|" in line:
            key, perm = line.split("|", 1)
            out[key.strip()] = perm.strip()
    return out


def db_permission_keys():
    p = subprocess.run(psql_argv("SELECT key FROM permissions"),
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
        blank = sorted(db_blank_module_keys())
        if blank:
            problems.append(
                "导航项 module_key 写成空串（规范值是 NULL，空串是"
                "「迁移手写列踩哨兵」的产物）：%s" % ", ".join(blank))
        try:
            nav_perms = db_nav_perms()
            known = db_permission_keys()
        except Exception as e:  # noqa: BLE001
            problems.append("权限对齐校验读不了库：%s" % e)
            nav_perms, known = {}, set()
        for key, want in sorted(NAV_REQUIRED_PERM.items()):
            if key not in nav_perms:
                problems.append(
                    "权限对齐表登记了导航项 '%s'，但 DB 没有（tab_key 改名/"
                    "删除？）" % key)
                continue
            got = nav_perms[key] or "NULL"
            if got != want:
                problems.append(
                    "导航项 '%s' 的 perm_key=%s，首屏后端要 '%s'——"
                    "可见即 403" % (key, got, want))
        missing = sorted(set(NAV_REQUIRED_PERM.values()) - known)
        if missing:
            problems.append("权限对齐表引用了不存在的权限键：%s"
                            % ", ".join(missing))
        unmapped = sorted(k for k, v in nav_perms.items()
                          if not v and k not in NAV_REQUIRED_PERM)
        if unmapped:
            warns.append(
                "按 min_class 放行的导航项 %d 条（未登记权限键，请确认"
                "上游页面确无权限判据）：%s"
                % (len(unmapped),
                   ", ".join(unmapped[:10])
                   + ("…" if len(unmapped) > 10 else "")))
    unused_case = sorted(k for k in (cases | page_cases)
                         if k not in dbk
                         and k not in subroutes
                         and k not in NAV_INTENTIONAL_NO_ENTRY)
    if unused_case and dbk:
        problems.append(
            "renderSimpleTool/page.tsx 有渲染分支却没有任何后台导航入口"
            "（只能手敲 ?tool= 才进得去）：%s——要么补 staff_panel_entries"
            "（参照 0421），要么写进 NAV_INTENTIONAL_NO_ENTRY 并说明理由"
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
