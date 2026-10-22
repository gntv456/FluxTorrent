# -*- coding: utf-8 -*-
"""常驻闸门 JQ：附件去重的可见性回执必须等于库里生效行（P-571）。

会员视角：同一份字节第二次上传时会被去重复用本人已有的权限行。
0294 的口径是「复用本人行 → 可见性沿用那一行」，但旧实现末尾按**本次
请求值**回显：先把图当公开图床传过（shared），再以 private 重传同一文件，
接口回 200 + `"visibility":"private"`、`deduplicated:true`，库里却仍是
shared——用户以为材料已私有，实际任何登录成员都能读。反方向（先 private
再按缺省 shared 重传）则回显 shared、他人打开 404。

判据：
  JQ1 去重时按「复用行的可见性」算生效值，响应回读它
  JQ2 响应不再走「只特判他人 shared」的旧条件
  JQ3 INSERT 绑定生效值（新行与请求值一致，复用行不写库）
  JQ4 同人同 sha 取行有 LIMIT 1；读取端仍以 shared 行聚合放行
  JQ5 正控：修前旧形状必须被同一套判据判为违例
"""
import io
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
API = os.path.join(ROOT, "apps", "api", "src", "attachment_http.rs")
READ = os.path.join(ROOT, "apps", "api", "src", "attachment_video.rs")

# 修前旧形状（仅 JQ5 正控用，不进判据正文）
OLD = """
        .bind(if visibility.is_empty() {
            "shared"
        } else {
            visibility.as_str()
        })
        "visibility": if deduped && mine.is_none() {
            "shared"
        } else if visibility.is_empty() {
            "shared"
        } else {
            &visibility
        },
"""


def body(path):
    """读源码并剥整行注释：注释里复述旧形状不许把真洞洗白。"""
    raw = io.open(path, encoding="utf-8", errors="replace").read()
    keep = []
    for line in raw.splitlines():
        s = line.strip()
        if s.startswith("//") or s.startswith("*"):
            continue
        keep.append(line)
    return "\n".join(keep)


def attach_bad(text):
    bad = []
    if "let effective = mine.as_ref()" not in text:
        bad.append("没有按复用行取生效可见性")
    if '"visibility": effective' not in text:
        bad.append("响应没回读生效值")
    if "deduped && mine.is_none()" in text:
        bad.append("仍只特判他人 shared")
    if ".bind(effective)" not in text:
        bad.append("INSERT 未绑定生效值")
    if re.search(r"\.bind\(if visibility\.is_empty\(\)", text):
        bad.append("INSERT 仍按请求值")
    if "user_id = $2 LIMIT 1" not in text:
        bad.append("同人去重取行未定序")
    return bad


def read_bad(text):
    bad = []
    if not re.search(r"\w\.visibility = 'shared'", text):
        bad.append("读取端不再按 shared 聚合")
    if "m.user_id = $1" not in text:
        bad.append("读取端不再放行本人行")
    return bad


def main():
    api = body(API)
    read = body(READ)
    b = attach_bad(api)
    ok("JQ1 去重回执 = 复用行可见性（不再回显请求值）", not b, "违例=%s" % b)
    b = read_bad(read)
    ok("JQ2 读取端仍按 shared/本人聚合判定", not b, "违例=%s" % b)
    old = attach_bad(OLD)
    ok("JQ3 正控：修前旧形状必须被判违例", bool(old),
       "旧形状=%s" % old)
    ok("JQ4 正控必须命中旧的两处（回显与 INSERT）",
       any("回读" in x or "特判" in x for x in old)
       and any("INSERT" in x for x in old), "旧形状=%s" % old)
    summary()


if __name__ == "__main__":
    main()
