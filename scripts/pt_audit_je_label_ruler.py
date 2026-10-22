# -*- coding: utf-8 -*-
"""常驻闸门 JE：后台上传榜的「做种中」列不许换定义（P-548）。

会员/职员视角：站点工具 → 上传者（/admin/uploaders）那一列的表头是
`t.stSeeding`＝「做种中」，而这个键在站内别的屏（用户列表、用户详情、
/me、PT++）一律指「该会员此刻在做种多少条目」。上传榜的 SQL 却写成：

    count(*) FROM snatches s JOIN torrents t2 ON t2.id = s.torrent_id
    WHERE s.user_id = u.id AND s.seeding AND t2.owner_id = u.id

多挂的 `t2.owner_id = u.id` 把它变成「他在回种自己发布的种」——几乎恒 0。
后果：职员在这张表看到某会员「做种中 0」，而同一行点进用户详情是「8」；
两个屏同一个标签两个数（P-542/P-543/P-544 同族）。
修法：摘掉 JOIN 与 owner 谓词，接站内那把尺（行内替换，site.rs 是
存量超限文件，行数与超宽行数都不许动）。

证据分层（本轮点名未验的半边）：
  JE1 正控   把旧形状喂给同一把尺，必须被抓出来（判据自己先证明能红）
  JE2 普查   全仓凡「用户维度 × seeding/leeching」的 snatches 计数都不许
             挂 JOIN/owner 谓词；命中数与基数一起报（当前 22 条 / 违例 0）
  JE3 活体   /admin/uploaders 的每行 seeding 必须等于 /admin/users/{id}
             的 seeding（同一会员、同一标签、两个出口）
  JE4 分辨力 现网此刻 seeding 行数＝0 ⇒ JE3 只是「不矛盾」，不是「抓到过」。
             库里出现真实做种后这条才带牙齿，故只 WARN 不冒充绿证。
             之所以不插 snatches 夹具行：那会被分钟级 worker tick 真结算
             （做种积分/HR 检查），代价大于这条判据的价值。
  JE4 普查   同一把尺推广到「发布数」族（P-549）：全仓凡用户维度的
             AS uploads 必须带 approval_status = 1。后台用户详情此前写
             `<> 3`（今天 P-544 的残留），把待审/驳回也算进发布数，与
             会员 /me、控制台、上传榜又是两个数（现网有 12 行是 0/2/4 档）。
             JE4b 拿旧尺 `<> 3` 的字面量当正控，证明这条普查抓得到。
  JE5 活体   挑一名下有未过审种的会员：后台 uploads 必须等于过审数。
             旧镜像读到旧尺＝这条红＝部署账，不是新洞。

窗口口径一条自我纠正：AS uploads 的标签在右括号**之外**，用 `[^)]*`
取子查询会整族漏抓（本批第一版就这么漏过一遍），所以 JE4 的窗口一律
取到 AS uploads 为止；基数一起报（命中 5 条）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_je_label_ruler.py
"""
import io
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import call, login, ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "apps", "api", "src")
SITE = "staff_http/site.rs"
# 修前的字面量（624aaa6e 落地时多挂的 JOIN + owner 谓词）
OLD_SHAPE = ("SELECT count(*) FROM snatches s JOIN torrents t2 "
             "ON t2.id = s.torrent_id WHERE s.user_id = u.id "
             "AND s.seeding AND t2.owner_id = u.id)")
PAT = re.compile(r"SELECT count\(\*\) FROM snatches[^)]*\)", re.S)


def strip_comments(src):
    """整行 // 注释先剥掉：判据名的说明文字会复述旧形状，
    不剥就是「注释洗白真洞」那一族。"""
    return "\n".join(l for l in src.splitlines()
                     if not l.strip().startswith("//"))


def offending(text):
    """返回把「用户维度的做种/下载中计数」换了定义的字面量。"""
    out = []
    for m in PAT.finditer(strip_comments(text)):
        s = " ".join(m.group(0).split())
        if "seeding" not in s and "leeching" not in s:
            continue
        if "JOIN" in s.upper() or "owner_id" in s:
            out.append(s)
    return out


def scan(rel):
    with io.open(os.path.join(SRC, rel), encoding="utf-8") as f:
        return f.read()


def census():
    hits = []
    bad = []
    for dirpath, _d, names in os.walk(SRC):
        for n in names:
            if not n.endswith(".rs"):
                continue
            rel = os.path.relpath(os.path.join(dirpath, n), SRC)
            rel = rel.replace("\\", "/")
            body = scan(rel)
            for m in PAT.finditer(strip_comments(body)):
                s = " ".join(m.group(0).split())
                if "seeding" in s or "leeching" in s:
                    hits.append(rel)
                    if "JOIN" in s.upper() or "owner_id" in s:
                        bad.append("%s :: %s" % (rel, s[:90]))
    return hits, bad


# 「发布数」那一族：窗口要一直取到 AS uploads（标签在右括号之外，
# 用 [^)]* 会整族漏抓——本批普查先踩过一次）
UP_PAT = re.compile(r"SELECT count\(\*\) FROM torrents.{0,180}?AS uploads",
                    re.S)


def uploads_all(text):
    return [" ".join(m.group(0).split())
            for m in UP_PAT.finditer(strip_comments(text))]


def uploads_offending(text):
    return [s for s in uploads_all(text) if "approval_status = 1" not in s]


def uploads_census():
    rows, bad = [], []
    for dirpath, _d, names in os.walk(SRC):
        for n in names:
            if not n.endswith(".rs"):
                continue
            rel = os.path.relpath(os.path.join(dirpath, n), SRC)
            rel = rel.replace("\\", "/")
            for s in uploads_all(scan(rel)):
                rows.append(rel)
                if "approval_status = 1" not in s:
                    bad.append("%s :: %s" % (rel, s[:88]))
    return rows, bad


def main():
    ok("JE1 正控：旧形状必须被这把尺抓到（抓到 %d 条）"
       % len(offending(OLD_SHAPE)), len(offending(OLD_SHAPE)) == 1,
       "判据抓不到已知洞＝这条普查是空的")

    hits, bad = census()
    ok("JE2 普查：用户维度 seeding/leeching 计数不挂 JOIN/owner"
       "（命中 %d 条、违例 %d 条）" % (len(hits), len(bad)),
       not bad and len(hits) >= 20,
       "; ".join(bad[:3]) or "基数掉到 20 以下说明尺子失效")
    ok("JE3a 上传榜的 seeding 子查询已是站内尺",
       not offending(scan(SITE)),
       offending(scan(SITE))[:1])

    # P-549：后台「发布数」与会员控制台同尺——只数过审（1 档）。
    # 0/2/4 只有作者与职员可读、3 档谁都不可读，两种都不算「已发布」。
    up_hits, up_bad = uploads_census()
    ok("JE4 普查：用户维度 AS uploads 必须带 approval_status = 1"
       "（命中 %d 条、违例 %d 条）" % (len(up_hits), len(up_bad)),
       not up_bad and len(up_hits) >= 5, "; ".join(up_bad[:3]))
    ok("JE4b 正控：旧尺 <> 3 的形状必须被同一把尺抓到",
       len(uploads_offending("SELECT count(*) FROM torrents t WHERE "
                             "t.owner_id = u.id AND t.approval_status <> 3) "
                             "AS uploads")) == 1)

    root = login()
    s, r = call("GET", "/admin/uploaders", None, token=root)
    rows = (r or {}).get("data") or []
    mism = []
    for row in rows[:12]:
        s3, one = call("GET", "/admin/users/%d" % row["id"], None, token=root)
        d = (one or {}).get("data") or {}   # 详情口是平铺结构，没有 data.user
        if d.get("seeding") != row["seeding"]:
            mism.append("uid=%s 榜=%s 详情=%s"
                        % (row["id"], row["seeding"], d.get("seeding")))
    ok("JE3b 同一会员在两个出口的「做种中」必须相等（比对 %d 行）"
       % min(len(rows), 12), s == 200 and bool(rows) and not mism,
       "; ".join(mism[:3]) or "榜里读不到行（前置未满足）")
    n_seed = psql_stdin("SELECT count(*) FROM snatches WHERE seeding")
    print("[WARN] 现网此刻 seeding 行数=%s；0 行时 JE3b 只能证明"
          "「不矛盾」，出现真实做种后这条才有牙齿（不插夹具行：snatches "
          "会被分钟级 tick 真结算）" % (n_seed or "?"))

    u9 = (psql_stdin("SELECT owner_id FROM torrents WHERE approval_status "
                    "IN (0,2,4) GROUP BY owner_id ORDER BY count(*) DESC"
                    " LIMIT 1") or "").strip()
    if u9:
        uid9 = int(u9)
        pub = int(psql_stdin("SELECT count(*) FROM torrents WHERE owner_id=%d"
                             " AND approval_status = 1" % uid9) or 0)
        not3 = int(psql_stdin("SELECT count(*) FROM torrents WHERE owner_id=%d"
                              " AND approval_status <> 3" % uid9) or 0)
        s9, d9 = call("GET", "/admin/users/%d" % uid9, None, token=root)
        api9 = ((d9 or {}).get("data") or {}).get("uploads")
        ok("JE5 后台发布数不数待审/驳回（uid=%d 过审 %d、旧尺 %d、api=%s）"
           % (uid9, pub, not3, api9), not3 > pub and api9 == pub,
           "读成 %s＝与会员 /me 的 uploads 两个数；旧镜像没带上本批时"
           "这条红＝部署账" % api9)
    # P-550：控制台概览的「收藏夹」格此前被硬写成 0（payload 有键没人算）
    s10, me10 = call("GET", "/me", None, token=root)
    bm_me = ((me10 or {}).get("data") or {}).get("bookmarks")
    s11, ov11 = call("GET", "/me/overview", None, token=root)
    bm_ov = ((ov11 or {}).get("data") or {}).get("bookmarks")
    ok("JE6 概览收藏夹数等于 /me（/me=%s 概览=%s）" % (bm_me, bm_ov),
       bm_me == bm_ov and (bm_me or 0) > 0,
       "两个口读同一个「我的收藏」，恒 0 那格是编出来的装饰值")
    print("[INFO] 上传榜首行读数样例：%s" % (rows[0] if rows else {}))


if __name__ == "__main__":
    main()
    summary()
