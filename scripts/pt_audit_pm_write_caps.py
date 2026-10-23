# -*- coding: utf-8 -*-
"""常驻闸门：写口长度上限台账——会员面写口，以及**正文会流进会员收件箱的
站方写口**（P-466 私信 / P-467 职员箱 / P-632 求种 / P-633 申诉 / P-634 受理）。

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
  求种  标题 ≤200、简介 ≤2000 —— P-632，上限与界面 maxLength 同数
  申诉  正文 ≤2000 —— 与 /appeals 界面同数，装在 require_auth 与查库之前
  受理  处理意见 ≤2000 —— P-634：站方口但正文会进会员收件箱，界面那枚
        输入框连 maxLength 都没有（现网实测 6000 字 note 落库成一条 6040
        字的站内信，绕开私信自己的 5000 字上限）

分层：
  C0/C6/C14/C15 静态：cap 在册 + 位置 + 中文整句
  C5 待收口台账（双向棘轮，两类原因分开记）：
        IN_FLIGHT = 站方正在排版的文件（论坛三口），不往里塞护栏
        CEILING   = 载体行数卡死（P-632 求种与申诉已各自拆文件收口，
                    清单已空；再出现时按同样口径登记，收口后改成硬断言）
  C1..C4/C7..C8/C10..C13 `--live`（全程零写入或当场清理）：超长必回中文
        整句；正控＝正好在上限上的请求必须放行到下一道校验；前后
        messages/staffmessages/appeals 的 count/max(id) 不变 ⇒ 「先落库再
        报错」没发生（appeals 的 max(id) 只在正控 INSERT 后前移，属序列
        语义；残留按本轮专属前缀回读）
跑法：python -u scripts/pt_audit_pm_write_caps.py [--live] [--selftest]
现网实测：2026-10-21 00:10Z（旧镜像）C1/C2 回「收件人不存在…」、C7 直接
200 落库 ⇒ 待部署证据；2026-10-23 在 p632-e650ae30（P-466/P-467 已上线）
上 C1/C2/C7 回的是上限整句但状态码是 **400**——本仓 Validation 统一 400
（errors.rs），旧断言写 422 是闸门自己的假红，已改正。同日 C10/C11 在旧
镜像上红（2001 字申诉 status=200、appeals 3|3 → 4|14），C13 收尾回读
3|3 ⇒ 缺陷本体与清场都取到证；C4 零写入（messages 2585|13934 前后不变）、
C8b 职员箱探针信按专属正文清干净（残留 0）。
"""
import io
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import call, dig, login, ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SEND = "apps/api/src/community_http/message_send.rs"
BOX = "apps/api/src/community_http/staffbox.rs"
REQ = "apps/api/src/content_http/requests/create.rs"
APL = "apps/api/src/gaps_http/appeals/create.rs"
HAN = "apps/api/src/gaps_http/appeals.rs"
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
# 载体行数卡死，护栏塞不进去（各自的原因不同，别混写成一句）。
# P-632 起两个口都已按「文件内子模块」拆出（requests/create.rs、
# appeals/create.rs），清单清空；上限本体改由 C14/C15 硬断言盯着。
# 再遇到载体顶格时按同样口径登记：写口 → 该装的列 + 为什么这轮没装。
CEILING = {}


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


def c14_req_static():
    """P-632 求种：上限与界面 maxLength 同数，且在建单 INSERT 之前。"""
    t = read(REQ)
    cap_t = "body.title.chars().count() > 200 {"
    cap_d = "d.chars().count() > 2000)"
    ok("C14 求种标题 cap 在册：title > 200", cap_t in t, "")
    ok("C14b 求种简介 cap 在册：descr > 2000", cap_d in t, "")
    i_ins = t.find("INSERT INTO requests")
    ok("C14c 两条 cap 都在建单 INSERT 之前（被拒不落库）",
       -1 < t.find(cap_t) < i_ins and -1 < t.find(cap_d) < i_ins,
       "cap@%s/%s vs insert@%s" % (t.find(cap_t), t.find(cap_d), i_ins))
    ok("C14d 求种两句上限文案是中文整句",
       '"求种标题最长 200 字"' in t and '"求种简介最长 2000 字"' in t, "")


def c15_apl_static():
    """申诉口：上限在 require_auth 与任何 sqlx 查库之前（形状先于身份）。"""
    t = read(APL)
    cap = "if body.body.trim().chars().count() > 2000 {"
    i_cap = t.find(cap)
    i_auth = t.find("let auth = require_auth(&req, &state).await;")
    i_sql = t.find("sqlx::")
    ok("C15 申诉正文 cap 在册：body.trim() > 2000", i_cap >= 0, "")
    ok("C15b cap 在 require_auth 与首个查库之前",
       -1 < i_cap < i_auth and -1 < i_cap < i_sql,
       "cap@%s auth@%s sql@%s" % (i_cap, i_auth, i_sql))
    ok("C15c 上限文案是中文整句", '"申诉内容最长 2000 字"' in t, "")


def c16_handle_static():
    """P-634：受理申诉那句「处理意见」不只进队列详情，还进当事人收件箱。"""
    t = read(HAN)
    cap = "if body.note.trim().chars().count() > 2000 {"
    i_cap = t.find(cap)
    # 锚点取 appeal_handle 自己那条 SELECT（`FROM appeals a JOIN users u` 在
    # 队列口appeal_queue 里也有，用它会拿到更靠前的位置⇒ 假红）
    i_sel = t.find("SELECT a.user_id, u.email,")
    ok("C16 处理意见 cap 在册：note.trim() > 2000", i_cap >= 0, "")
    ok("C16b cap 在首次查库之前（先验形状）", -1 < i_cap < i_sel,
       "cap@%s vs select@%s" % (i_cap, i_sel))
    ok("C16c 上限文案中文整句", '"处理意见最长 2000 字"' in t, "")


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
       s == 400 and "主题最长 120 字" in m1,
       "详情=%s｜若详情是「收件人不存在…」＝现网二进制未含本批修复（待部署）"
       % m1[:70])
    s2, r2 = call("POST", "/messages", {"to": "__no_such_member__",
                                       "subject": "探针",
                                       "body": probe_long(5001)}, token=tok)
    m2 = (r2 or {}).get("message", "") if isinstance(r2, dict) else ""
    ok("C2 正文 5001 字 ⇒「正文最长 5000 字」（status=%s）" % s2,
       s2 == 400 and "正文最长 5000 字" in m2,
       "详情=%s｜同上：回收件人那句＝待部署" % m2[:70])
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
       s == 400 and "主题最长 100 字" in m,
       "详情=%s｜status=200 且 ok＝现网二进制未含 P-467（待部署）" % m[:60])
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


def live_c10_c13(tok):
    """通用申诉 /appeals：2001 字必拒、恰好 2000 必须放行，探针零残留。"""
    mark = "闸门申诉探针-%s" % time.strftime("%Y%m%d%H%M%S")
    before = psql_stdin("SELECT count(*)::text || '|' || "
                        "COALESCE(max(id), 0)::text FROM appeals")
    # 超长探针也带同一枚专属前缀：旧镜像会把它收下（红证本体），收尾时
    # 不能只清正控那条，否则 C13 的「零残留」自己就变成了假绿。
    s, r = call("POST", "/appeals",
                {"kind": "other",
                 "body": mark + "字" * (2001 - len(mark))}, token=tok)
    m = (r or {}).get("message", "") if isinstance(r, dict) else ""
    ok("C10 申诉正文 2001 字 ⇒「申诉内容最长 2000 字」（status=%s）" % s,
       s == 400 and "申诉内容最长 2000 字" in m,
       "详情=%s｜status=200 且 ok＝现网二进制未含本批修复（待部署）" % m[:70])
    mid = psql_stdin("SELECT count(*)::text || '|' || "
                     "COALESCE(max(id), 0)::text FROM appeals")
    ok("C11 超长申诉零写入（appeals %s → %s）" % (before, mid),
       before == mid, "")
    # 正控：正好 2000 字必须放行到下一道校验（未结上限/落库），不能回长度文案。
    # 带本轮专属前缀，落库了就按返回 id（缺失时按前缀）精确清掉再回读。
    body = mark + "字" * (2000 - len(mark))
    s2, r2 = call("POST", "/appeals", {"kind": "other", "body": body},
                  token=tok)
    m2 = (r2 or {}).get("message", "") if isinstance(r2, dict) else ""
    pid = dig(r2, "data", "id")
    ok("C12 正控：正好 2000 字必须放行（回的不是长度文案；status=%s）" % s2,
       "申诉内容最长 2000 字" not in m2,
       "message=%s｜id=%s" % (m2[:70], pid))
    if pid:
        psql_stdin("DELETE FROM appeals WHERE id = %d" % int(pid))
    psql_stdin("DELETE FROM appeals WHERE body LIKE '%s%%'" % mark)
    residue = psql_stdin("SELECT count(*)::text FROM appeals "
                         "WHERE body LIKE '%s%%'" % mark)
    after = psql_stdin("SELECT count(*)::text || '|' || "
                       "COALESCE(max(id), 0)::text FROM appeals")
    ok("C13 本轮探针申诉零残留（残留=%s；appeals %s → %s，max(id) 前移"
       "属序列语义）" % (residue, before, after), residue == "0", "")


def live_c17_c19(tok):
    """受理口探针：用一个不存在的申诉号，只验形状不碰任何真实队列项。"""
    before_a = psql_stdin("SELECT count(*)::text || '|' || "
                          "COALESCE(max(id), 0)::text FROM appeals")
    before_m = psql_stdin("SELECT count(*)::text || '|' || "
                          "COALESCE(max(id), 0)::text FROM messages")
    s, r = call("POST", "/admin/appeals/handle",
                {"appeal_id": 999999999, "accept": True,
                 "note": probe_long(2001), "unban": False}, token=tok)
    m = (r or {}).get("message", "") if isinstance(r, dict) else ""
    ok("C17 处理意见 2001 字 ⇒「处理意见最长 2000 字」（status=%s）" % s,
       s == 400 and "处理意见最长 2000 字" in m,
       "详情=%s｜回「申诉不存在或已处理」＝现网二进制未含 P-634（待部署）"
       % m[:70])
    s2, r2 = call("POST", "/admin/appeals/handle",
                  {"appeal_id": 999999999, "accept": True,
                   "note": probe_long(2000), "unban": False}, token=tok)
    m2 = (r2 or {}).get("message", "") if isinstance(r2, dict) else ""
    ok("C18 正控：正好 2000 字必须放行到存在性校验（回收「不存在」那句）",
       "申诉不存在或已处理" in m2, "status=%s message=%s" % (s2, m2[:80]))
    after_a = psql_stdin("SELECT count(*)::text || '|' || "
                         "COALESCE(max(id), 0)::text FROM appeals")
    after_m = psql_stdin("SELECT count(*)::text || '|' || "
                         "COALESCE(max(id), 0)::text FROM messages")
    ok("C19 两条受理探针零写入（appeals %s → %s；messages %s → %s）"
       % (before_a, after_a, before_m, after_m),
       before_a == after_a and before_m == after_m, "")


def c9_translations():
    """本闸门盯着的那几句必须仍在译文表里（三列齐）。

    踩过的坑：新增校验文案只登记进源码就算完 ⇒ `validation_i18n_guard`
    当场报「新增未译的校验串」，而 en/zh-TW 界面里那句是中文孤岛。
    """
    tsv = os.path.join(ROOT, "apps", "api", "i18n", "validation_details.tsv")
    raw = io.open(tsv, encoding="utf-8", newline="").read()
    keys = {}
    for line in raw.replace("\r\n", "\n").split("\n"):
        if not line or line.startswith("#"):
            continue
        cols = line.split("\t")
        keys[cols[0]] = cols
    need = ["主题最长 120 字", "正文最长 5000 字", "主题最长 100 字",
            "求种标题最长 200 字", "求种简介最长 2000 字",
            "申诉内容最长 2000 字", "处理意见最长 2000 字"]
    for k in need:
        cols = keys.get(k)
        ok("C9 校验文案已进译文表三语齐：%s" % k,
           bool(cols) and len(cols) == 3 and all(c.strip() for c in cols),
           str(cols)[:110])


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
    c14_req_static()
    c15_apl_static()
    c16_handle_static()
    c5_ledger()
    c9_translations()
    if "--live" in sys.argv:
        tok = login("root")
        live_c1_c4(tok)
        live_c7_c8(tok)
        live_c10_c13(tok)
        live_c17_c19(tok)
    else:
        print("   [C1..C19] 跳过（加 --live 才发请求；探针全程零写入或当场清理）")
    summary()


if __name__ == "__main__":
    main()
