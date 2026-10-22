# -*- coding: utf-8 -*-
"""P-587 审核裁决副作用「不得静默失败」闸门（纯静态，不起服务）。

背景：decide_one 的副作用清单（review_decide.rs 头注释）共五项。其中
「连击/被拒计数 + 发布信用分」此前写成 `let _ = sqlx::query(...)`：
SQL 一旦失败（列改名/类型漂移），裁决照常落库、日志零线索；而
publish_credits 是免审线判据（publish_http/upload_credits.rs）——
用户连过五审却永远拿不到免审。P-323 已在 review_side_effects.rs
立过同款规矩（失败必须 WARN），本闸门把两处一起钉住。

判据：
  1. 三支裁决文件不得出现「丢弃 sqlx 结果」的 `let _ =` 形状；
  2. 计数/信用分 UPDATE 必须被 if let Err(e) 包住且 WARN 点名；
  3. 过审三条副作用必须逐条 if let Err(e)（P-323 回归钉）；
  4. 承重：换回旧形状 / 漏一条守卫，判据必须变红。
"""
import io
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

DECIDE = "apps/api/src/services/review_decide.rs"
SIDE = "apps/api/src/admin_http/review_side_effects.rs"
BATCH = "apps/api/src/admin_http/review_batch.rs"

SILENT = re.compile(r"let\s+_\s*=\s*sqlx::query")
COUNTER = re.compile(
    r"if\s+let\s+Err\(e\)\s*=\s*sqlx::query\(\s*"
    r"\"UPDATE users u SET approve_streak[\s\S]{0,1200}?"
    r"tracing::warn!\([\s\S]{0,300}?审核计数")
SIDE_QUERY = re.compile(r"let\s+side\s*=\s*sqlx::query")
SIDE_GUARD = re.compile(r"if\s+let\s+Err\(e\)\s*=\s*side")


def src(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8").read()


def strip_comments(text):
    """去掉 // 行注释；字符串字面量里的 // 保留（按双引号状态机走）。"""
    out = []
    i, n, in_str = 0, len(text), False
    while i < n:
        c = text[i]
        if in_str:
            out.append(c)
            if c == "\\" and i + 1 < n:
                out.append(text[i + 1])
                i += 2
                continue
            if c == '"':
                in_str = False
            i += 1
            continue
        if c == '"':
            in_str = True
            out.append(c)
            i += 1
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            while i < n and text[i] != "\n":
                i += 1
            continue
        out.append(c)
        i += 1
    return "".join(out)


def silent_count(code):
    return len(SILENT.findall(code))


def guard_pairs(code):
    return len(SIDE_QUERY.findall(code)), len(SIDE_GUARD.findall(code))


def main():
    decide = src(DECIDE)
    code = strip_comments(decide)
    side_code = strip_comments(src(SIDE))
    batch_code = strip_comments(src(BATCH))
    total_silent = (
        silent_count(code)
        + silent_count(side_code)
        + silent_count(batch_code)
    )
    ok("三支裁决文件无「丢弃 sqlx 结果」形状", total_silent == 0,
       "命中 %d 处" % total_silent)
    ok("计数/信用分 UPDATE 被 if let Err 包住并 WARN 点名",
       bool(COUNTER.search(code)), DECIDE)
    # 承重：只把守卫换回旧形状（字符串/换行原样），判据必须变红
    old = code.replace(
        "if let Err(e) = sqlx::query(\n"
        "        \"UPDATE users u SET approve_streak",
        "let _ = sqlx::query(\n"
        "        \"UPDATE users u SET approve_streak")
    ok("承重：换回 let _ = 旧形状被抓",
       old != code and silent_count(old) == 1
       and not COUNTER.search(old), "合成旧码")
    ok("计数/信用分 UPDATE 仍在（不得靠删副过闸）",
       "UPDATE users u SET approve_streak" in decide, DECIDE)
    q, g = guard_pairs(side_code)
    ok("过审三条副作用逐条 if let Err（查询 %d / 守卫 %d）"
       % (q, g), q == 3 and g == 3, SIDE)
    broken = side_code.replace("if let Err(e) = side", "if let Ok(_) = side", 1)
    q2, g2 = guard_pairs(broken)
    ok("承重：漏一条副作用守卫被抓", q2 == 3 and g2 == 2, "合成旧码")


if __name__ == "__main__":
    main()
    summary()
