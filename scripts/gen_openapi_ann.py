# -*- coding: utf-8 -*-
"""按域批量补 #[utoipa::path] 注解（F4 分域推进通用引擎）。

每条注解 8~12 行且要守 80 列，手写必然超宽、也容易漏 responses 的
错误码分支。本引擎从 route-manifest.json 读「方法 + 路径 + handler」，
套 scripts/openapi_ann_desc.json 里人工维护的描述表生成注解文本。

**描述表缺项时拒绝生成**（GEN1 判据）：宁可漏一条，也不写一句空描述
进 spec——空描述比没有这条路径更糟，第三方会以为那就是全部语义。

用法：
    python scripts/gen_openapi_ann.py --domain community_http [--apply]
    python scripts/gen_openapi_ann.py --domain all --apply
默认 dry-run，只打印将要写入的内容与跳过原因。

生成器本身踩过三个坑（都在下面注释里写死，别改回去）：
  1. 行尾判定用 `git show`（LF）而工作区是 CRLF（core.autocrlf=true）
     ⇒ 整文件行尾被改写，SQL 续行反斜杠全被破坏。**以工作区为准**。
  2. 跨行正则匹配 handler 在有 pub()/属性宏时漏匹配 ⇒ 用行索引。
  3. 折行时反斜杠无条件加 / 每段重复 `description = "` / 每段都闭合
     引号 ⇒ 生成的字面量不是合法 Rust。单行放得下不加折行符；只有
     首段带前缀；只在末段闭合。
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = "docs/api/route-manifest.json"
DESCF = "scripts/openapi_ann_desc.json"
APPLY = "--apply" in sys.argv

VERB = {"GET": "get", "POST": "post",
        "PUT": "put", "DELETE": "delete"}
ERR_AUTH = (401, "会话未登录（code 2001）")
ERR_403 = (403, "权限不足（code 2003）")
ERR_404 = (404, "资源不存在或不可见（code 1004）")
ERR_422 = (422, "参数校验失败（code 1002）")


def argval(flag, default=None):
    if flag in sys.argv:
        i = sys.argv.index(flag)
        if i + 1 < len(sys.argv):
            return sys.argv[i + 1]
    return default


def wrap_desc(text, indent="    ", width=78):
    """把描述折成**一个** Rust 字符串字面量，每行 <=width。

    Rust 规则（踩过三次才对）：
      * 折行续符是行尾单反斜杠；
      * 整段**只在最后一行闭合引号**；
      * 续行片段不重复 `description = "` 前缀；
      * 单行放得下就一个折行符都不加。
    """
    frags = []
    for seg in [s for s in text.split("\\n") if s]:
        cur = seg
        while len(cur) > width - len(indent) - 20:
            room = width - len(indent) - 24
            cut = max(cur.rfind("，", 0, room), cur.rfind(" ", 0, room))
            if cut <= 0:
                break
            frags.append(cur[:cut])
            cur = cur[cut:].lstrip("， ")
        frags.append(cur)
    out = []
    for i, f in enumerate(frags):
        pre = '%sdescription = "' % indent if i == 0 else indent + "        "
        last = i == len(frags) - 1
        out.append(pre + f + (('",') if last else "\\"))
    return out


def responses_for(method, path, extra):
    rs = [(200, "ok"), ERR_AUTH]
    rs += list(extra)
    if method in ("POST", "PUT", "DELETE"):
        rs.append(ERR_422)
    return rs


def build_ann(method, path, tag, desc, params, extra):
    lines = ["#[utoipa::path(", "    %s," % VERB[method],
             '    path = "/api/v1%s",' % path,
             '    tag = "%s",' % tag]
    lines += wrap_desc(desc)
    if params:
        lines.append("    params(")
        for nm, ty, d in params:
            lines.append(
                '        ("%s" = %s, description = "%s"),' % (nm, ty, d))
        lines.append("    ),")
    # responses 尽量压到一行（省 4 行/条，5 个社区文件才留得住 300 行
    # 门禁），压不进 78 列就逐条折行。utoipa 的简写是
    # `(status, description = "...")` —— 括号里必须带 `description =`，
    # 写成 `(200, "ok")` 展开后语义不对（踩过一次）。
    items = [('status = %d, description = "%s"' % (st, d))
             for st, d in responses_for(method, path, extra)]
    head = "    responses("
    one = head + ", ".join("(%s)" % it for it in items) + "))]"
    if len(one) <= 78:
        lines.append(one)
        return lines
    lines.append(head)
    for k, it in enumerate(items):
        pre = "        "
        body = "(%s)" % it
        # 末项：body 的 `)` 闭了 item，还要一个 `)` 闭 responses(，
        # 最后 `]` 闭 #[utoipa::path( ⇒ 追加 `))]`
        tail = "," if k < len(items) - 1 else "))]"
        if len(pre + body + tail) <= 78:
            lines.append(pre + body + tail)
        else:
            head2, ds = it.split(", description = ", 1)
            lines.append("        (%s," % head2)
            lines.append('            description = %s%s' % (ds, tail))
    return lines
    lines.append(head)
    for k, it in enumerate(items):
        suf = "," if k < len(items) - 1 else ")]"
        pre = "        "
        if len(pre + it + suf) > 78:
            # 单条就超宽：拆成 (status, description) 两行
            st, ds = it[len("("):-1].split(", description = ", 1)
            lines.append("        (status = %s," % st)
            lines.append("            %s%s" % (ds, suf))
        else:
            # it 自带闭括号，suf 的 `)` 会多一个 —— 只补逗号
            lines.append(pre + it + ("," if suf == "," else ""))
    return lines


def apply_domain(domain, entries, routes):
    prefix = "apps/api/src/%s/" % domain
    th = [r for r in routes if r["file"].startswith(prefix)]
    table = {(e["method"], e["path"]): e for e in entries}
    missing = [(r["method"], r["path"]) for r in th
               if (r["method"], r["path"]) not in table]
    print("== %s：路由 %d 条 / 描述表 %d 条 ==" % (domain, len(th), len(table)))
    for m, p in missing:
        print("  [跳过-无描述表] %-6s %s" % (m, p))
    ok("GEN1 %s 描述表覆盖全部路由" % domain, not missing,
       "缺 %d 条（补进 openapi_ann_desc.json 后重跑）" % len(missing))
    if not APPLY:
        print("(dry-run；加 --apply 才写入)")
        return 0, 0, len(missing)
    byfile = {}
    for r in th:
        if (r["method"], r["path"]) in table:
            byfile.setdefault(r["file"], []).append(r)
    wrote = already = missed = 0
    for rel, rs in sorted(byfile.items()):
        full = os.path.join(ROOT, rel)
        src = open(full, encoding="utf-8", newline="").read()
        crlf = "\r\n" in src
        lines = src.split("\r\n") if crlf else src.split("\n")
        nl = "\r\n" if crlf else "\n"
        targets = []
        for r in rs:
            e = table[(r["method"], r["path"])]
            want = '#[%s("%s")]' % (VERB[r["method"]], r["path"])
            for i, l in enumerate(lines):
                if l.strip() != want:
                    continue
                j = i + 1
                while j < len(lines) and not lines[j].strip():
                    j += 1
                if j >= len(lines) or "async fn" not in lines[j]:
                    continue
                k = i - 1
                while k >= 0 and not lines[k].strip():
                    k -= 1
                if k >= 0 and lines[k].strip() == ")]":
                    already += 1
                    break
                targets.append((i, r, e, lines[j]))
                break
            else:
                print("  [跳过-宏未命中] %-6s %s" % (r["method"], r["path"]))
                missed += 1
        for i, r, e, fnline in sorted(targets, reverse=True):
            ann = build_ann(r["method"], r["path"], e["tag"], e["desc"],
                            e["params"], e.get("errors", ()))
            if not re.search(r"\bpub\b", fnline):
                fnline = fnline.replace("async fn", "pub(crate) async fn", 1)
            lines[i] = nl.join(ann) + nl + lines[i]
            lines[i + 1] = fnline
            wrote += 1
        if targets:
            open(full, "w", encoding="utf-8", newline="").write(nl.join(lines))
    print("写入 %d 条（已存在 %d / 未命中 %d）" % (wrote, already, missed))
    ok("GEN2 %s 全部路由均已带注解" % domain,
       wrote + already == len(th) - len(missing),
       "写了 %d / 已有 %d / 应为 %d" % (wrote, already, len(th) - len(missing)))
    return wrote, already, len(missing)


def main():
    dom = argval("--domain", "torrent_http")
    mf = os.path.join(ROOT, MANIFEST)
    routes = json.load(open(mf, encoding="utf-8"))["routes"]
    df = os.path.join(ROOT, DESCF)
    data = json.load(open(df, encoding="utf-8"))
    if dom == "all":
        for k in sorted(data):
            apply_domain(k, data[k], routes)
    else:
        if dom not in data:
            print("openapi_ann_desc.json 无域 '%s'（现有: %s）"
                  % (dom, ", ".join(sorted(data))))
            FAILURES.append("域 %s 无描述表" % dom)
        else:
            apply_domain(dom, data[dom], routes)
    summary()


if __name__ == "__main__":
    main()
