# -*- coding: utf-8 -*-
"""常驻闸门：会员面写口的长度上限台账（P-466 私信 / P-467 职员箱）。

缺陷本体：`POST /messages`（主题/正文）与 `POST /contactstaff`（主题）唯一的
长度约束是浏览器 `maxLength`，写口本身零上限——绕开界面直接 POST 就是一封几十
MB 的信，而且这封信会原样进收件箱、web-push 载荷与邮件正文。本仓对这一族早定过
性：P-133「举报备注没有上限 ⇒ 5000 字整段塞进一条私信撑满收件箱」，成文口径是
`x.chars().count() > N` + 中文整句（版块名最长 64 字 / 版块规则最长 1000 字），
按「字」判不按 UTF-8 字节（P-128/P-331，否则中文用户被掐掉三分之二）。

已装（硬断言在册）：
  私信  主题 ≤120、正文 ≤5000 —— 装在**收件人查询之前**（先验形状再查库，
        也才谈得上零写入探针）
  职员箱 主题 ≤100（与界面同一枚数；正文界面没给上限，不替站长拍数）

分层：
  C0/C6 静态：cap 在册 + 位置 + 中文整句
  C5 待收口台账（双向棘轮，两类原因分开记）：
        IN_FLIGHT = 站方正在排版的文件（论坛三口），不往里塞护栏
        FILE_FULL = 载体已顶在 300 行门禁上（requests.rs 净增 1 行即红、
                    appeals.rs 已在基线 315），要先拆文件才装得下
  C1..C4/C7..C8 `--live`（全程零写入）：超长必回中文整句；正控＝正好在上限上
        的请求必须放行到下一道校验；前后 messages/staffmessages 的 count/max(id)
        不变 ⇒ 「先落库再报错」没发生
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
SEND = "apps/api/src/community_http/message_send.rs"
BOX = "apps/api/src/community_http/staffbox.rs"
CAPS = {
    SEND: ["if body.subject.chars().count() > 120 {",
           "if body.body.chars().count() > 5000 {"],
    BOX: ["if body.subject.chars().count() > 100 {"],
}
ANCHOR = "let target: Option<(i64, String, i32)>"

# 待收口清单：写口 → 该装的列 + 为什么这轮没装
IN_FLIGHT = {  # 站方正在同一文件里排版/改口径
    "apps/api/src/community_http/topic_create.rs": ["title", "body"],
    "apps/api/src/community_http/post_write.rs": ["body"],
    "apps/api/src/community_http/post_edit.rs": ["body"],
}
# 载体行数卡死，护栏塞不进去（各自的原因不同，别混写成一句）：
#   requests.rs 300 行＝正好顶在 300 上限，净增一行即「新超限」；
#   appeals.rs 315 行＝存量超限、基线钉在 315，「只许瘦不许胖」，净增即红。
# 两者都要先拆文件才装得下 cap ⇒ 交回站长（拆文件不是本批的活）。
CEILING = {
    "apps/api/src/content_http/requests.rs": ["title(200)", "descr(2000)"],
    "apps/api/src/gaps_http/appeals.rs": ["body(2000，现只有 ≥10 字下限)"],
}


def read(rel):
    raw = io.open(os.path.join(ROOT, rel), encoding="utf-8", newline="").read()
    return raw.replace("\r\n", "\n")


def c0_static():
    t = read(SEND)
    for cap in CAPS[SEND]:
        ok("C0 私信 cap 在册：%s" % cap.replace("if ", "").replace(" {", ""),
           cap in t, "")
    i_sub = t.find(CAPS[SEND][0])
    i_anc = t.find(ANCHOR)
    ok("C0c 两条 cap 都在收件人查询之前（先验形状再查库）",
       -1 < i_sub < i_anc and t.find(CAPS[SEND][1]) < i_anc,
       "cap@%s vs query@%s" % (i_sub, i_anc))
    ok("C0d 上限文案是中文整句（会员看得懂，不是 format! 动态串）",
       '"主题最长 120 字"' in t and '"正文最长 5000 字"' in t, "")


def c6_static():
    t = read(BOX)
    for cap in CAPS[BOX]:
        ok("C6 职员箱 cap 在册：%s" % cap.replace("if ", "").replace(" {", ""),
           cap in t, "")
    ok("C6b 主题上限文案中文整句", '"主题最长 100 字"' in t, "")


def c5_ledger():
    """未收口的写口必须仍在清单里；哪天装了，本条会红，逼着把清单改成硬断言。"""
    rows = []
    for group, table in (("站方在途件", IN_FLIGHT),
                       ("载体行数卡死", CEILING)):
        for rel, cols in table.items():
            t = read(rel)
            got = [c for c in cols
                   if re.search(r"\b%s\w*\.chars\(\)\.count\(\)\s*>"
                                % re.escape(c.split("(")[0]), t)]
            rows.append((group, rel, cols, got))
    installed = [r for r in rows if r[3]]
    ok("C5 待收口清单 %d 个口仍无服务端上限（已装=%d ⇒ 装完要把清单改成硬断言）"
       % (len(rows), len(installed)), not installed,
       "、".join("%s:%s" % (r[1].split("/")[-1], r[3]) for r in installed))
    for group, rel, cols, _g in rows:
        print("   [C5] %-14s %-46s 待装=%s" % (group, rel, cols))


def probe_long(n):
    return "字" * n


def live_c1_c4(tok):
    before = psql_stdin("SELECT count(*)::text || '|' || "
                        "COALESCE(max(id), 0)::text FROM messages")
    s, r = call("POST", "/messages", {"to": "__no_such_member__",
                                      "subject": probe_long(121),
                                      "body": "探针"}, token=tok)
    m1 = (r or {}).get("message", "") if isinstance(r, dict) else ""
    ok("C1 主题 121 字 ⇒「主题最长 120 字」（status=%s）" % s,
       s == 422 and "主题最长 120 字" in m1, m1[:90])
    s2, r2 = call("POST", "/messages", {"to": "__no_such_member__",
                                       "subject": "探针",
                                       "body": probe_long(5001)}, token=tok)
    m2 = (r2 or {}).get("message", "") if isinstance(r2, dict) else ""
    ok("C2 正文 5001 字 ⇒「正文最长 5000 字」（status=%s）" % s2,
       s2 == 422 and "正文最长 5000 字" in m2, m2[:90])
    s3, r3 = call("POST", "/messages", {"to": "__no_such_member__",
                                       "subject": probe_long(120),
                                       "body": probe_long(5000)}, token=tok)
    m3 = (r3 or {}).get("message", "") if isinstance(r3, dict) else ""
    ok("C3 正控：正好 120/5000 必须放行到下一道校验（回收件人那句）",
       "收件人不存在" in m3, "status=%s message=%s" % (s3, m3[:80]))
    after = psql_stdin("SELECT count(*)::text || '|' || "
                       "COALESCE(max(id), 0)::text FROM messages")
    ok("C4 三条探针零写入（messages %s → %s）" % (before, after),
       before == after, "")


def live_c7_c8(tok):
    # root 是管理组（class_id>=90），跳过 60s 防刷，探针直接撞到主题上限
    before = psql_stdin("SELECT count(*)::text || '|' || "
                        "COALESCE(max(id), 0)::text FROM staffmessages")
    s, r = call("POST", "/contactstaff", {"subject": probe_long(101),
                                          "body": "闸门探针"}, token=tok)
    m = (r or {}).get("message", "") if isinstance(r, dict) else ""
    ok("C7 职员箱主题 101 字 ⇒「主题最长 100 字」（status=%s）" % s,
       s == 422 and "主题最长 100 字" in m, m[:90])
    s2, r2 = call("POST", "/contactstaff", {"subject": probe_long(100),
                                           "body": "闸门探针"}, token=tok)
    m2 = (r2 or {}).get("message", "") if isinstance(r2, dict) else ""
    ok("C8 正控：主题正好 100 字必须放行（回的不是那句长度文案；status=%s）" % s2,
       "主题最长 100 字" not in m2, m2[:80])
    after = psql_stdin("SELECT count(*)::text || '|' || "
                       "COALESCE(max(id), 0)::text FROM staffmessages")
    # C8 是合法长度的正控，职员箱那条口会真落一封信：按本轮专属正文精确清掉，
    # 再断言零残留（不是「恒真的哑断言」）。
    psql_stdin("DELETE FROM staffmessages WHERE body = '闸门探针'")
    residue = psql_stdin("SELECT count(*)::text FROM staffmessages "
                         "WHERE body = '闸门探针'")
    ok("C8b 本轮探针信零残留（残留=%s；staffmessages %s → %s）"
       % (residue, before, after), residue == "0", "")


def selftest():
    src = read(SEND)
    missing = [c for c in CAPS[SEND] if c not in src]
    if missing:
        print("SELFTEST 前置失败：cap 不在源码里 %s" % missing)
        return 2
    flipped = src
    for c in CAPS[SEND]:
        flipped = flipped.replace(c, "if false {")
    io.open(os.path.join(ROOT, SEND), "w", encoding="utf-8",
            newline="").write(flipped.replace("\n", "\r\n"))
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
        if p.returncode == 0 or "120" not in joined or "5000" not in joined:
            print("SELFTEST FAILED：摘掉两条 cap 应让两条 C0 断言同时红")
            return 1
        return 0
    finally:
        io.open(os.path.join(ROOT, SEND), "w", encoding="utf-8",
                newline="").write(src)
        same = read(SEND) == src
        print("SELFTEST 还原字节一致=%s" % same)


def main():
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    c0_static()
    c6_static()
    c5_ledger()
    if "--live" in sys.argv:
        tok = login("root")
        live_c1_c4(tok)
        live_c7_c8(tok)
    else:
        print("   [C1..C8] 跳过（加 --live 才发请求；探针全程零写入或当场清理）")
    summary()


if __name__ == "__main__":
    main()
