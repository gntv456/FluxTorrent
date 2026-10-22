# -*- coding: utf-8 -*-
"""P-583/P-585 组订阅闸门。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_qg_group_sub.py

判据：
  1. 静态三节接线：组详情口回 subscribed；详情块把它传给按钮；
     按钮用它初始化 state。
  2. 承重：三节各自改回旧形状，静态判据必须变红。
  3. P-585：单版本组（items=1）详情页仍渲染组区块，订阅入口不依赖第二版。
  4. 活体：匿名 false → root 订阅后 true → 退订后 false；
     只清理本轮新增的订阅行（进门前已有的不动）。
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import call, login, ok, summary  # noqa: E402

API = "apps/api/src/publish_http/group.rs"
BLOCKS = "apps/web/components/torrent-detail-blocks.tsx"
BUTTON = "apps/web/components/group-subscribe-button.tsx"
PAGE = "apps/web/app/(main)/torrent/[id]/page.tsx"

PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def src(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8").read()


def wiring_ok(api, blocks, btn):
    return ('"subscribed": subscribed' in api
            and "initialSubscribed={group.subscribed ?? false}" in blocks
            and "useState(initialSubscribed)" in btn)


def group_block_visible(page):
    return bool(re.search(r"group\.items\.length\s*>\s*0", page))


def main():
    api, blocks, btn = src(API), src(BLOCKS), src(BUTTON)
    ok("组详情口回 subscribed + 详情块传参 + 按钮按初始态渲染",
       wiring_ok(api, blocks, btn),
       "三节：后端字段 / 详情块 prop / 按钮 useState")
    ok("承重：后端不回 subscribed 被抓",
       not wiring_ok(api.replace('"subscribed": subscribed,', ""),
                     blocks, btn), "合成旧码")
    ok("承重：详情块不传初始态被抓",
       not wiring_ok(api, blocks.replace(
           "initialSubscribed={group.subscribed ?? false}", ""), btn),
       "合成旧码")
    ok("承重：按钮忽略初始态被抓",
       not wiring_ok(api, blocks, btn.replace(
           "useState(initialSubscribed)", "useState(false)")),
       "合成旧码")

    page = src(PAGE)
    ok("单版本组也渲染组区块（订阅入口不依赖第二版）",
       group_block_visible(page), "详情页门禁应为 items.length > 0")
    ok("承重：门禁退回只认多版本被抓",
       not group_block_visible(page.replace(
           "group.items.length > 0", "group.items.length > 1")),
       "合成旧码")

    tok = login()
    uid = psql("SELECT id FROM users WHERE username='root'")
    # 避开 root 已订阅的组：探针只碰「进门未订阅」的那一个，收尾才可断言归零。
    row = psql(
        "SELECT t.id, t.group_id FROM torrents t "
        "WHERE t.group_id IS NOT NULL AND t.approval_status=1 "
        "AND NOT EXISTS (SELECT 1 FROM group_subscriptions gs "
        "                WHERE gs.group_id=t.group_id AND gs.user_id=%s) "
        "ORDER BY t.id LIMIT 1" % uid)
    if not row:
        ok("活体：站内存在可测且未订阅的组种", False,
           "没有 group_id 非空的过审种，活体未验")
        return
    tid, gid = (int(x) for x in row.split("|"))
    inserted = False
    try:
        s, r = call("GET", "/torrents/%d/group" % tid)
        anon = (r.get("data") or {}).get("subscribed")
        ok("匿名读组详情 subscribed=false", s == 200 and anon is False,
           (s, anon))

        s, r = call("POST", "/torrents/groups/%d/subscribe" % gid, None,
                    token=tok)
        ok("root 订阅探针组", s == 200 and r.get("code") == 0, r)
        inserted = True
        s, r = call("GET", "/torrents/%d/group" % tid, token=tok)
        sub = (r.get("data") or {}).get("subscribed")
        ok("订阅后组详情 subscribed=true", s == 200 and sub is True,
           (s, sub))

        s, r = call("POST", "/torrents/groups/%d/unsubscribe" % gid, None,
                    token=tok)
        ok("root 退订探针组", s == 200 and r.get("code") == 0, r)
        inserted = False
        s, r = call("GET", "/torrents/%d/group" % tid, token=tok)
        sub = (r.get("data") or {}).get("subscribed")
        ok("退订后组详情 subscribed=false", s == 200 and sub is False,
           (s, sub))
    finally:
        if inserted:
            call("POST", "/torrents/groups/%d/unsubscribe" % gid, None,
                 token=tok)
        left = psql("SELECT count(*) FROM group_subscriptions "
                    "WHERE group_id=%d AND user_id=%s" % (gid, uid))
        ok("收尾订阅行归零（本轮只碰未订阅组）", left == "0", left)


if __name__ == "__main__":
    main()
    summary()
