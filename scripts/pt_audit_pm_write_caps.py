# -*- coding: utf-8 -*-
"""常驻闸门：私信写口的长度上限（P-466），与论坛四口的待收口台账。

缺陷本体：`POST /messages` 的主题/正文**服务端零上限**——只有浏览器
`maxLength`（120 / 5000）。绕开界面直接 POST 就是一封几十 MB 的信，还会原样
进收件箱、web-push 与邮件正文（本仓同类先例：P-133 举报备注「5000 字整段塞进
一条私信撑满收件箱」、P-331「限长把 UTF-8 字节当字判」）。站内成文口径是
`x.chars().count() > N` + 中文整句（版块名最长 64 字 / 版块规则最长 1000 字）。

修法：两条上限装在**收件人查询之前**（先验形状再查库，也才谈得上零写入探针）。

分层：
  C0 静态（默认跑）：两条 cap 在册，且位置早于 target 查询
  C5 待收口台账（默认跑）：论坛四个写口（topics.title / topics.body /
     posts.body 两处）此刻仍无服务端上限 ⇒ 钉在清单里；站方把那三个文件
     （topic_create.rs / post_write.rs / post_edit.rs，均为其人在途件）收口后，
     本条必须改成「必须已装 cap」——清单少一条也红（双向棘轮）
  C1..C4 活体（`--live` 才跑，全程零写入）：
     C1 主题 121 字 ⇒ 回「主题最长 120 字」
     C2 正文 5001 字 ⇒ 回「正文最长 5000 字」
     C3 正控：主题正好 120 字 + 正文正好 5000 字 ⇒ 必须继续往下走，回的是
        「收件人不存在或已注销」（证明 cap 没把合法载荷一起拒掉）
     C4 三条探针前后 messages 的 count/max(id) 不变 ⇒ 「先落库再报错」没发生
跑法：python -u scripts/pt_audit_pm_write_caps.py [--live] [--selftest]
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import call, login, ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SEND = os.path.join(ROOT, "apps", "api", "src", "community_http",
                    "message_send.rs")
CAP_SUBJECT = "if body.subject.chars().count() > 120 {"
CAP_BODY = "if body.body.chars().count() > 5000 {"
ANCHOR = "let target: Option<(i64, String, i32)>"

# 论坛侧待收口清单（写口 → 该装的列）：这三个文件此刻是站方在途件，
# 本批不把护栏塞进去（编辑别人正在排版的文件＝覆盖他们的在制品）。
PENDING_PORTS = {
    "apps/api/src/community_http/topic_create.rs": ["title", "body"],
    "apps/api/src/community_http/post_write.rs": ["body"],
    "apps/api/src/community_http/post_edit.rs": ["body"],
}


def read(path):
    raw = io.open(path, encoding="utf-8", newline="").read()
    return raw.replace("\r\n", "\n")


def c0_static():
    t = read(SEND)
    ok("C0a 主题上限在册（120 字，与界面同一枚数）", CAP_SUBJECT in t,
       t[t.find("if body.subject.chars"):][:60] if "chars" in t else "找不到")
    ok("C0b 正文上限在册（5000 字，与界面同一枚数）", CAP_BODY in t, "")
    i_sub = t.find(CAP_SUBJECT)
    i_anc = t.find(ANCHOR)
    ok("C0c 两条 cap 都在收件人查询之前（先验形状再查库）",
       -1 < i_sub < i_anc and t.find(CAP_BODY) < i_anc,
       "cap@%s vs query@%s" % (i_sub, i_anc))
    ok("C0d 上限文案仍是中文整句（会员看得懂，且没落进 format! 动态串）",
       '"主题最长 120 字"' in t and '"正文最长 5000 字"' in t, "")


def pending_have_caps():
    """返回论坛侧「已经装了 cap」的写口（用于双向棘轮对账）。"""
    capped = {}
    for rel, cols in PENDING_PORTS.items():
        t = read(os.path.join(ROOT, rel))
        capped[rel] = [c for c in cols
                       if re.search(r"body\.%s\.chars\(\)\.count\(\)\s*>\s*\d+"
                                    % re.escape(c), t)]
    return capped


def c5_ledger():
    capped = pending_have_caps()
    done = [r for r, v in capped.items() if v]
    ok("C5 论坛三口仍是『未收口』状态（%d/%d 已装 ⇒ 装完要把本条改成硬断言）"
       % (len(done), len(PENDING_PORTS)), len(done) == 0,
       "已装=%s" % done)
    for r, v in capped.items():
        print("   [C5] %-46s 已装上限=%s" % (r, v or "无"))


def msg_len(n):
    return "字" * n


def c1_c4_live():
    tok = login("root")
    before = psql_stdin("SELECT count(*)::text || '|' || "
                       "COALESCE(max(id), 0)::text FROM messages")
    s, r = call("POST", "/messages", {"to": "__no_such_member__",
                                      "subject": msg_len(121),
                                      "body": "探针"}, token=tok)
    m1 = (r or {}).get("message", "") if isinstance(r, dict) else ""
    ok("C1 主题 121 字 ⇒ 回「主题最长 120 字」（status=%s）" % s,
       s == 422 and "主题最长 120 字" in m1, m1[:90])
    s2, r2 = call("POST", "/messages", {"to": "__no_such_member__",
                                       "subject": "探针",
                                       "body": msg_len(5001)}, token=tok)
    m2 = (r2 or {}).get("message", "") if isinstance(r2, dict) else ""
    ok("C2 正文 5001 字 ⇒ 回「正文最长 5000 字」（status=%s）" % s2,
       s2 == 422 and "正文最长 5000 字" in m2, m2[:90])
    s3, r3 = call("POST", "/messages", {"to": "__no_such_member__",
                                       "subject": msg_len(120),
                                       "body": msg_len(5000)}, token=tok)
    m3 = (r3 or {}).get("message", "") if isinstance(r3, dict) else ""
    ok("C3 正控：正好 120/5000 必须放行到下一道校验（回的是收件人那句）",
       "收件人不存在" in m3, "status=%s message=%s" % (s3, m3[:80]))
    after = psql_stdin("SELECT count(*)::text || '|' || "
                       "COALESCE(max(id), 0)::text FROM messages")
    ok("C4 三条探针零写入（messages 前后=%s → %s）" % (before, after),
       before == after, "")


def selftest():
    src = read(SEND)
    if CAP_SUBJECT not in src or CAP_BODY not in src:
        print("SELFTEST 前置失败：cap 不在源码里（本闸门测的就是它）")
        return 2
    flipped = src.replace(CAP_SUBJECT, "if false {")
    flipped = flipped.replace(CAP_BODY, "if false {")
    io.open(SEND, "w", encoding="utf-8", newline="").write(
        flipped.replace("\n", "\r\n"))
    try:
        p = subprocess.run([sys.executable, os.path.abspath(__file__)],
                           capture_output=True, encoding="utf-8",
                           errors="replace")
        out = p.stdout or ""
        reds = [ln for ln in out.splitlines() if "[FAIL]" in ln]
        print("SELFTEST rc=%d 红条=%d" % (p.returncode, len(reds)))
        for ln in reds:
            print("   ", ln.strip()[:110])
        joined = " ".join(reds)
        if p.returncode == 0 or "C0a" not in joined or "C0b" not in joined:
            print("SELFTEST FAILED：摘掉两条 cap 应让 C0a/C0b 同时红")
            return 1
        return 0
    finally:
        io.open(SEND, "w", encoding="utf-8", newline="").write(src)
        same = read(SEND) == src
        print("SELFTEST 还原字节一致=%s" % same)


def main():
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    c0_static()
    c5_ledger()
    if "--live" in sys.argv:
        c1_c4_live()
    else:
        print("   [C1..C4] 跳过（加 --live 才发请求；三条探针全程零写入）")
    summary()


if __name__ == "__main__":
    main()
