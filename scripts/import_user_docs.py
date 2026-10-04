#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""把 docs/user/*.md 导入站内帮助中心（custom_pages 的 doc_group 分组）。

背景：wiki-plan.md 方案 A 做法 1——复用 custom_pages（0187）承载帮助中心，
0274 迁移给它加了 doc_group / doc_sort 两列。本脚本负责把仓库里的用户指南
灌进数据库，站长之后在后台「自定义页面」面板可视化编辑（DB 是权威源）。

设计要点（对齐 scripts/demo_seed.py 的既有范式）：
  - 走 HTTP API + Token 认证，不直连数据库（不依赖 psycopg、跨环境可跑）。
  - **幂等**：按 slug upsert，重复跑安全。只覆盖 title/body/doc_* 四个字段，
    绝不碰 visible —— 站长手动下线的页面不该被脚本悄悄上线。
  - **零外部依赖**：内置极简 md→HTML（标题/列表/表格/代码/粗体/链接/段落），
    够用户指南用；不引 python-markdown，避免装依赖。

用法：
  python scripts/import_user_docs.py            # 默认 127.0.0.1:8080
  python scripts/import_user_docs.py --base http://host:8080 \
      --user root --pass 'yourpass'
"""

from __future__ import annotations

import argparse
import html
import json
import os
import re
import sys
import urllib.error
import urllib.request

API = "/api/v1"
DOC_DIR = os.path.join("docs", "user")
# 分组名与目录页的展示顺序（wiki-plan.md 第五节：user 册只在站内，DB 权威）
DOC_GROUP = "guide"
# slug 前缀，避免和站长自建的自定义页 (/p/xxx) 撞名
SLUG_PREFIX = "help-"


def req(method: str, url: str, headers: dict | None = None,
        body: bytes | None = None):
    r = urllib.request.Request(url, data=body, method=method,
                               headers=headers or {})
    try:
        with urllib.request.urlopen(r, timeout=30) as resp:
            return resp.status, dict(resp.headers), resp.read()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()
    except urllib.error.URLError as e:
        print(f"✗ 连不上 {url}：{e}", file=sys.stderr)
        sys.exit(2)


def login(base: str, user: str, pwd: str) -> str:
    """登录取 flux_token cookie。

    刻意用 Cookie 而非 `/me/tokens` 的 API Token：admin 端点是
    `require_auth` + `require_perm(CUSTOMPAGES_MANAGE)`，走的是会话/权限体系，
    开放 Token 那套 scopes 不覆盖它（实测 Token 头会 401）。

    注意 actix 的响应头是小写 `set-cookie`（按 `Set-Cookie` 取永远拿空），
    这里对大小写都做一次兜底。
    """
    st, h, b = req("POST", f"{base}{API}/auth/login",
                   {"Content-Type": "application/json"},
                   json.dumps({"username": user, "password": pwd}
                              ).encode())
    if st != 200:
        sys.exit(f"✗ 登录失败（{st}）：{b[:200]!r}")
    raw = h.get("set-cookie") or h.get("Set-Cookie") or ""
    for part in raw.split(";"):
        part = part.strip()
        if part.startswith("flux_token="):
            tok = part[len("flux_token="):]
            if tok:
                return tok
    # 兜底：某些部署只在 body 里回 token
    tok = (json.loads(b).get("data") or {}).get("token")
    if tok:
        return tok
    sys.exit("✗ 登录成功但没拿到 flux_token cookie")


# ---------- 极简 md → HTML ----------

def _inline(s: str) -> str:
    """行内：代码 > 粗体 > 链接（顺序有讲究：先抽代码免得被后续规则改坏）。"""
    s = html.escape(s, quote=False)
    s = re.sub(r"`([^`]+)`", r"<code>\1</code>", s)
    s = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", s)
    # 只认站内/相对链接，外链带 relnoopener
    s = re.sub(r"\[([^\]]+)\]\((https?://[^)\s]+)\)",
               r'<a href="\2" rel="noopener">\1</a>', s)
    s = re.sub(r"\[([^\]]+)\]\((/[^)\s]*)\)", r'<a href="\1">\1</a>', s)
    return s


def md_to_html(md: str) -> str:
    """够用的 markdown 子集转换。用户指南的结构（标题/列表/表格/代码/段落）全覆盖。"""
    out: list[str] = []
    lines = md.splitlines()
    i, n = 0, len(lines)

    while i < n:
        ln = lines[i]
        s = ln.strip()

        # 代码块
        if s.startswith("```"):
            i += 1
            buf = []
            while i < n and not lines[i].strip().startswith("```"):
                buf.append(lines[i])
                i += 1
            i += 1
            out.append("<pre><code>"
                       + html.escape("\n".join(buf), quote=False)
                       + "</code></pre>")
            continue

        # 表格：当前行是 |…| 且下一行是分隔行
        if s.startswith("|") and i + 1 < n and \
                re.match(r"^\|[\s:|-]+\|$", lines[i + 1].strip()):
            def cells(row: str) -> list[str]:
                return [c.strip() for c in row.strip().strip("|").split("|")]
            head = cells(s)
            i += 2
            rows = []
            while i < n and lines[i].strip().startswith("|"):
                rows.append(cells(lines[i].strip()))
                i += 1
            t = ["<table><thead><tr>"]
            t += [f"<th>{_inline(c)}</th>" for c in head]
            t.append("</tr></thead><tbody>")
            for r in rows:
                t.append("<tr>")
                t += [f"<td>{_inline(c)}</td>" for c in r]
                t.append("</tr>")
            t.append("</tbody></table>")
            out.append("".join(t))
            continue

        # 标题 # ~######
        m = re.match(r"^(#{1,6})\s+(.*)$", s)
        if m:
            lv = len(m.group(1))
            out.append(f"<h{lv}>{_inline(m.group(2).strip())}</h{lv}>")
            i += 1
            continue

        # 水平线
        if re.match(r"^(-{3,}|\*{3,})$", s):
            out.append("<hr/>")
            i += 1
            continue

        # 引用
        if s.startswith("> "):
            buf = []
            while i < n and lines[i].strip().startswith(">"):
                buf.append(lines[i].strip().lstrip(">").strip())
                i += 1
            out.append("<blockquote>"
                       + _inline(" ".join(buf)) + "</blockquote>")
            continue

        # 列表（无序 - / 有序 1.）
        if re.match(r"^([-*+]|\d+\.)\s+", s):
            ordered = bool(re.match(r"^\d+\.", s))
            tag = "ol" if ordered else "ul"
            items = []
            while i < n and re.match(r"^([-*+]|\d+\.)\s+", lines[i].strip()):
                items.append(re.sub(r"^([-*+]|\d+\.)\s+", "",
                                    lines[i].strip()))
                i += 1
            li = "".join(f"<li>{_inline(it)}</li>" for it in items)
            out.append(f"<{tag}>{li}</{tag}>")
            continue

        # 空行
        if not s:
            i += 1
            continue

        # 段落：吃到下一个空行/块级起始
        buf = []
        while i < n and lines[i].strip() and not re.match(
                r"^(#{1,6}\s|[-*+]\s|\d+\.\s|>|\||```|-{3,}$)",
                lines[i].strip()):
            buf.append(lines[i].strip())
            i += 1
        if buf:
            out.append("<p>" + _inline(" ".join(buf)) + "</p>")

    return "\n".join(out)


def collect() -> list[tuple[str, str, str]]:
    """返回 [(slug, title, html)]，按文件名排序保证 doc_sort 稳定。"""
    if not os.path.isdir(DOC_DIR):
        sys.exit(f"✗ 找不到 {DOC_DIR}/（要在仓库根目录跑）")
    items = []
    for fn in sorted(os.listdir(DOC_DIR)):
        if not fn.endswith(".md") or fn.lower() == "readme.md":
            continue
        path = os.path.join(DOC_DIR, fn)
        with open(path, encoding="utf-8") as f:
            md = f.read()
        # 标题取首个 H1，没有就用文件名
        m = re.search(r"^#\s+(.*)$", md, re.M)
        title = m.group(1).strip() if m else os.path.splitext(fn)[0]
        # 去掉正文里的首个 H1（已作为页面大标题渲染，避免重复）
        body = re.sub(r"^#\s+.*\n+", "", md, count=1)
        slug = SLUG_PREFIX + os.path.splitext(fn)[0]
        items.append((slug, title, md_to_html(body)))
    return items


def main():
    ap = argparse.ArgumentParser(description="导入 docs/user 到帮助中心")
    ap.add_argument("--base", default="http://127.0.0.1:8080")
    ap.add_argument("--user", default="root")
    ap.add_argument("--pass", dest="pwd", default="password123")
    ap.add_argument("--dry-run", action="store_true", help="只打印不提交")
    a = ap.parse_args()

    base = a.base.rstrip("/")
    items = collect()
    if not items:
        sys.exit("✗ 没找到可导入的 md")
    print(f"→ 待导入 {len(items)} 篇（group={DOC_GROUP}）")
    for slug, title, _ in items:
        print(f"   {slug:<24} {title}")

    if a.dry_run:
        print("\n(dry-run，未提交)")
        return

    tok = login(base, a.user, a.pwd)
    h = {"Cookie": f"flux_token={tok}", "Content-Type": "application/json"}

    # 先查已有，拿到 id 决定 create 还是 update（避免盲目 POST 撞 UNIQUE）
    st, _, b = req("GET", f"{base}{API}/admin/custom-pages", h)
    if st != 200:
        sys.exit(f"✗ 拉取现有页面失败（{st}）：{b[:200]!r}")
    existing = {p["slug"]: p for p in (json.loads(b).get("data") or [])}

    ok = fail = 0
    for idx, (slug, title, body) in enumerate(items, start=1):
        payload = json.dumps({
            "slug": slug, "title": title, "body": body,
            "doc_group": DOC_GROUP, "doc_sort": idx,
        }).encode()
        old = existing.get(slug)
        if old:
            st, _, rb = req("PUT",
                            f"{base}{API}/admin/custom-pages/{old['id']}",
                            h, payload)
            verb = "更新"
        else:
            st, _, rb = req("POST",
                            f"{base}{API}/admin/custom-pages", h, payload)
            verb = "新建"
        if st in (200, 201):
            print(f"✅ {verb} {slug}")
            ok += 1
        else:
            print(f"✗ {verb}失败 {slug}（{st}）：{rb[:160]!r}")
            fail += 1

    print(f"\n完成：成功 {ok}，失败 {fail}")
    print(f"前台查看：{base.replace(':8080', ':3000')}/help")
    if fail:
        sys.exit(1)


if __name__ == "__main__":
    main()
