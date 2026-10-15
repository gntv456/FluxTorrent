# -*- coding: utf-8 -*-
"""给 torrent_http 域补 #[utoipa::path] 注解（F4 分域推进，第二批）。

为什么不手写 38 条：每条注解有 8~12 行且要守 80 列，手写必超宽且
容易漏 responses 的错误码分支。本工具从 route-manifest.json 读出该域
的「方法 + 路径 + handler」，套一张人工维护的描述/参数/错误码表生成
注解文本；描述表缺项时**拒绝生成**（宁可漏一条也不写一句空描述进
spec——空描述比没有这条路径更糟，第三方会以为那就是全部语义）。

用法：python scripts/gen_torrent_openapi_ann.py [--apply]
       默认 dry-run，只打印将要写入的内容与跳过原因。
"""
import json
import os
import re
import sys

from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = "docs/api/route-manifest.json"
APPLY = "--apply" in sys.argv

# 人工描述表：(method, 相对路径) -> (tag, 描述, [(参数名, 类型, 说明)])
# 描述里的换行用 \n 占位，生成时按 80 列折行。
VERB = {"GET": "get", "POST": "post",
        "PUT": "put", "DELETE": "delete"}

DESC = {
    ("GET", "/torrents/{id}"): (
        "种子",
        "种子详情（会话鉴权）。持 view_anonymous 权限者可见匿名种子的真实发布者；staff 对暂缓/待审种也可见。",
        [("id", "i64", "种子 id")],
    ),
    ("POST", "/admin/torrents/{id}/logs/{ordinal}/adjust"): (
        "种子",
        "抓轨日志人工修正（staff；补记/更正漏抓的 announce 事件）。",
        [("id", "i64", "种子 id"), ("ordinal", "i64", "日志序号")],
    ),
    ("GET", "/admin/trumps"): (
        "种子",
        "撤种裁决总览（staff；按状态/时间分页）。",
        [("status", "Option<String>", "裁决状态筛选")],
    ),
    ("POST", "/admin/trumps/{id}/resolve"): (
        "种子",
        "裁决撤种申请（staff；维持/推翻/封禁三选一，写操作留痕）。",
        [("id", "i64", "裁决 id")],
    ),
    ("GET", "/torrents"): (
        "种子",
        "种子列表（0102 存活三态 + 多维筛选 + 二元 keyset 游标分页）。\\n"
        "默认视图只给活种；响应 hidden_dead 告知被存活谓词藏起的断种数。",
        [("category_id", "Option<Vec<i32>>", "分类多选（ANY 命中）"),
         ("search", "Option<String>", "关键字（AND 模糊，%/_ 已转义）"),
         ("alive", "Option<i16>", "存活三态：缺省=仅活种 0=全部 1=仅活种 2=仅断种"),
         ("sort", "Option<String>", "排序键 +_asc 后缀反转；asc 前缀反转方向")],
    ),
    ("GET", "/torrents/{id}/detail"): (
        "种子",
        "详情页扩展数据（简介/文件数/感谢数），与详情合并渲染。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/files"): (
        "种子",
        "文件树（按目录层级返回，付费种需已购或为发布者本人）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/thanks"): (
        "种子",
        "感谢者列表（分页）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/comments"): (
        "种子",
        "评论列表（分页，含点赞数与楼中楼计数）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/snatches"): (
        "种子",
        "抓取列表（做种/下载两种视角，按 viewer 过滤）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/nfo"): (
        "种子",
        "NFO 原文（text/plain）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/magnet"): (
        "种子",
        "磁力链接。响应体内嵌本人 passkey 的 announce——\\n"
        "与 .torrent 下载同闸门（ES36）：被停下载/挂起账号拒发，\\n"
        "付费种必须已有购买记录。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/peers"): (
        "种子",
        "当前 peer 列表（tracker 汇报口径，按 IP 聚合并做隐私截断）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/tags"): (
        "种子",
        "种子标签列表（0159 标签体系）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/related"): (
        "种子",
        "相关种子（同分类/同发布者的邻近推荐）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/file-conflicts"): (
        "种子",
        "文件冲突检查（辅种前的同种不同文件比对）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/suggest"): (
        "种子",
        "发种建议（按分类给出尚未发布的热门内容提示）。",
        [],
    ),
    ("GET", "/torrents/{id}/logs/{ordinal}"): (
        "种子",
        "抓轨日志正文（0312；ordinal 为该种日志序号）。",
        [("id", "i64", "种子 id"), ("ordinal", "i64", "日志序号")],
    ),
    ("POST", "/torrents/{id}/comments"): (
        "种子",
        "发评论。触发门槛：注册满 N 天、魔力/做种量达标（见站规）。",
        [("id", "i64", "种子 id")],
    ),
    ("DELETE", "/torrents/{id}/comments/{cid}"): (
        "种子",
        "删评论（作者本人或持 forum 删帖权）。",
        [("id", "i64", "种子 id"), ("cid", "i64", "评论 id")],
    ),
    ("POST", "/torrents/{id}/comments/{cid}/like"): (
        "种子",
        "评论点赞切换（返回切换后的点赞数与当前状态）。",
        [("id", "i64", "种子 id"), ("cid", "i64", "评论 id")],
    ),
    ("POST", "/torrents/{id}/thanks"): (
        "种子",
        "感谢（已感谢过返回 code 5002）。",
        [("id", "i64", "种子 id")],
    ),
    ("POST", "/torrents/{id}/reseed"): (
        "种子",
        "求种（挂出求种位；过期天数 <=0 走 30 天兜底，避免悬赏永久冻结）。",
        [("id", "i64", "种子 id")],
    ),
    ("PUT", "/torrents/{id}/tags"): (
        "种子",
        "覆盖写标签（0159；any/all 两种匹配口径由 tag_all 决定）。",
        [("id", "i64", "种子 id")],
    ),
    ("PUT", "/torrents/{id}/bookmark"): (
        "种子",
        "收藏切换。",
        [("id", "i64", "种子 id")],
    ),
    ("PUT", "/torrents/{id}"): (
        "种子",
        "编辑种子（发布者本人或持编辑权；改 info dict 会动 info_hash，\\n"
        "故被同种判为重复——这是刻意口径）。",
        [("id", "i64", "种子 id")],
    ),
    ("PUT", "/torrents/{id}/price"): (
        "种子",
        "设置付费价格（发布者本人）。",
        [("id", "i64", "种子 id")],
    ),
    ("POST", "/torrents/{id}/restore"): (
        "种子",
        "回收站恢复（软删种置回通过态）。",
        [("id", "i64", "种子 id")],
    ),
    ("POST", "/torrents/{id}/resubmit"): (
        "种子",
        "重新提交审核（下架/驳回种走这条路回到审核台）。",
        [("id", "i64", "种子 id")],
    ),
    ("DELETE", "/torrents/{id}"): (
        "种子",
        "软删种子（进回收站，管理员口可硬删）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/previews"): (
        "种子",
        "预览图列表。",
        [("id", "i64", "种子 id")],
    ),
    ("POST", "/torrents/{id}/previews"): (
        "种子",
        "上传预览图（发布者本人；多图按 sort 排序）。",
        [("id", "i64", "种子 id")],
    ),
    ("DELETE", "/torrents/{id}/previews"): (
        "种子",
        "删预览图（发布者本人）。",
        [("id", "i64", "种子 id")],
    ),
    ("POST", "/torrents/{id}/cue-check"): (
        "种子",
        "分卷一致性检查（比对同种各卷的 cue/文件树是否吻合）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/aggregate"): (
        "种子",
        "详情聚合口（一次返回详情+文件树+扩展，减少客户端往返）。",
        [("id", "i64", "种子 id")],
    ),
    ("POST", "/torrents/batch-download"): (
        "种子",
        "批量下载（一次下发多个 .torrent；仍逐条过下载闸门与扣费）。",
        [],
    ),
    ("POST", "/torrents/{id}/trump"): (
        "种子",
        "申请撤种裁决（Trump：认为被误判/需复核）。",
        [("id", "i64", "种子 id")],
    ),
    ("GET", "/torrents/{id}/trumps"): (
        "种子",
        "该种的撤种裁决历史。",
        [("id", "i64", "种子 id")],
    ),
}

# 通用错误码说明（按方法族给一句，不逐条枚举）
ERR_AUTH = (401, "会话未登录（code 2001）")
ERR_403 = (403, "权限不足（code 2003）")
ERR_404 = (404, "种子不存在或不可见（code 1004）")
ERR_422 = (422, "参数校验失败（code 1002）")


def responses_for(method, path):
    rs = [(200, "ok")]
    rs.append(ERR_AUTH)
    if path.startswith("/torrents/{id}") or "/{id}/" in path:
        rs.append(ERR_404)
    if method in ("POST", "PUT", "DELETE"):
        rs.append(ERR_422)
    if "/torrents/{id}" in path and method in ("PUT", "POST", "DELETE"):
        rs.append(ERR_403)
    return rs


def wrap_desc(text, indent="    ", width=78):
    """把描述折成**一个** Rust 字符串字面量，每行 <=width。

    Rust 规则（踩过三次才对）：
      * 折行续符是行尾单反斜杠 `\`；
      * 整段**只在最后一行闭合引号**，中间行行尾是 `\` 而不是 `",`；
      * 续行片段不重复 `description = "` 前缀（只有第一段有）；
      * 单行放得下就一个折行符都不加。
    """
    # 1) 按显式换行 + 宽度，把描述切成物理片段
    frags = []
    for seg in [s for s in text.split("\\n") if s]:
        cur = seg
        while len(cur) > width - len(indent) - 20:
            cut = max(cur.rfind("，", 0, width - len(indent) - 24),
                      cur.rfind(" ", 0, width - len(indent) - 24))
            if cut <= 0:
                break
            frags.append(cur[:cut])
            cur = cur[cut:].lstrip("， ")
        frags.append(cur)
    # 2) 拼成字面量：每行前缀一次，只有末行闭合
    out = []
    for i, f in enumerate(frags):
        pre = '%sdescription = "' % indent if i == 0 else indent + "        "
        last = i == len(frags) - 1
        out.append(pre + f + (('",') if last else "\\"))
    return out


def build_ann(method, path, tag, desc, params, handler):
    verb = {"GET": "get", "POST": "post",
            "PUT": "put", "DELETE": "delete"}[method]
    lines = ["#[utoipa::path(", "    %s," % verb,
             '    path = "/api/v1%s",' % path,
             '    tag = "%s",' % tag]
    lines += wrap_desc(desc)
    if params:
        lines.append("    params(")
        for nm, ty, d in params:
            lines.append(
                '        ("%s" = %s, description = "%s"),'
                % (nm, ty, d))
        lines.append("    ),")
    lines.append("    responses(")
    for st, d in responses_for(method, path):
        lines.append('        (status = %d, description = "%s"),' % (st, d))
    lines.append("    )")
    lines.append(")]")
    return lines


def main():
    mf = os.path.join(ROOT, MANIFEST)
    routes = json.load(open(mf, encoding="utf-8"))["routes"]
    pre = "apps/api/src/torrent_http/"
    th = [r for r in routes if r["file"].startswith(pre)]
    print("== 种子域 utoipa 注解生成（%d 条路由）==" % len(th))
    todo = skipped = 0
    for r in th:
        key = (r["method"], r["path"])
        if key not in DESC:
            print("  [跳过-无描述表] %-6s %s" % (r["method"], r["path"]))
            skipped += 1
            continue
        todo += 1
    print("有描述表 %d 条 / 跳过 %d 条" % (todo, skipped))
    ok("GEN1 描述表覆盖种子域全部路由", skipped == 0,
       "缺 %d 条描述表（补齐后重跑）" % skipped)
    if not APPLY:
        print("\n(dry-run；加 --apply 才写入)")
        summary()
        return

    # 实际写入：按「行索引」定位，不靠跨行正则（宏与 async fn 紧邻，
    # 跨行正则在有 pub()/属性宏时会漏匹配——踩过一次）
    byfile = {}
    for r in th:
        byfile.setdefault(r["file"], []).append(r)
    wrote = already = missed = 0
    for rel, rs in sorted(byfile.items()):
        full = os.path.join(ROOT, rel)
        src = open(full, encoding="utf-8", newline="").read()
        # 行尾以工作区文件为准：core.autocrlf=true 时 checkout 出来
        # 是 CRLF 而 `git show` 是 LF，用后者判定会把整文件行尾改写
        # （踩过一次：aggregate.rs 的 SQL 续行反斜杠全被破坏）
        crlf = "\r\n" in src
        lines = src.split("\r\n") if crlf else src.split("\n")
        nl = "\r\n" if crlf else "\n"
        # 一次遍历收集所有 actix 宏行下标（从后往前插，避免下标漂移）
        targets = []
        for r in rs:
            mac = {"GET": "get", "POST": "post",
                   "PUT": "put", "DELETE": "delete"}[r["method"]]
            want = '#[%s("%s")]' % (mac, r["path"])
            for i, l in enumerate(lines):
                if l.strip() != want:
                    continue
                # 下一非空行应是 async fn（允许中间有 pub(...)）
                j = i + 1
                while j < len(lines) and not lines[j].strip():
                    j += 1
                if j >= len(lines) or "async fn" not in lines[j]:
                    continue
                # 已带 utoipa 注解？宏行往上找最近的非空行
                k = i - 1
                while k >= 0 and not lines[k].strip():
                    k -= 1
                if k >= 0 and lines[k].strip() == ")]":
                    already += 1
                    break
                targets.append((i, r, lines[j]))
                break
            else:
                print("  [跳过-宏未命中] %-6s %s" % (r["method"], r["path"]))
                missed += 1
        for i, r, fnline in sorted(targets, reverse=True):
            tag, desc, params = DESC[(r["method"], r["path"])]
            ann = build_ann(r["method"], r["path"], tag, desc,
                            params, r["handler"])
            if not re.search(r'\bpub\b', fnline):
                fnline = fnline.replace("async fn", "pub(crate) async fn", 1)
            lines[i] = nl.join(ann) + nl + lines[i]
            lines[i + 1] = fnline
            wrote += 1
        if targets:
            open(full, "w", encoding="utf-8", newline="").write(nl.join(lines))
    print("写入 %d 条（已存在 %d / 未命中 %d）" % (wrote, already, missed))
    ok("GEN2 全部路由均已带注解", wrote + already == len(th),
       "写了 %d / 已有 %d / 未命中 %d / 应为 %d"
       % (wrote, already, missed, len(th)))
    summary()


if __name__ == "__main__":
    main()
