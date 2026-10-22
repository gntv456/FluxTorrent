# -*- coding: utf-8 -*-
"""常驻闸门：会员目录「最近上线」排序不得把从没上线的号排在最前（P-458）。

形状：`users.last_seen_at` 可空；`/users` 的 `desc` 缺省 true；PG 的 `DESC`
缺省是 **NULLS FIRST**。前端 FilterLinks 明确给了「最近上线」这一档
（`?sort=last_seen`），于是点它之后第一页全是 last_seen_at IS NULL 的号
（现网实测 20/20，56 个可见会员里 50 个从没上线），真正最近活跃的人被推到
第三页之后——这一档排出来的是它承诺的反面。

修法（1d2dac68）：两支 ord 一律 `NULLS LAST`（未知值沉底）。同仓已有这个写法
（`openapi_http/me_torrents.rs`、`staff_http/site_db.rs`），不是新机制。

判据分层：
  M0 正控   FIX^ 的 ord 两支没有 NULLS，且现网这一列确实可空（尺的前提）
  M1        静态：两支 ord 都带 NULLS LAST
  M2        PG A/B：同一 WHERE + LIMIT 20 只换 ORDER BY，旧页满屏 NULL、
            新页把有值的排到最前（数字直接来自现网库，不是断言常量）
  M3        界面可达：/users 页确实把「最近上线」做成了一档（点得到才算洞）
  M4        白名单棘轮：从源码抓 order_col 的全部排序键，逐个查可空性——
            只要有**任何**一个可空键，M1 就必须为真（以后有人往白名单里加
            一个可空列而没写 NULLS，这里会红）
  M5        反例在册：`snatches.last_seen_at` 是 NOT NULL——这正是本批
            **没有**把 NULLS LAST 扩到 profile.rs:189 / user_torrentlist.rs:97
            的理由；哪天它变成可空，那条理由就失效，这条判据负责提醒重看
跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_member_sort_nulls.py
      负控：同加 --selftest
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
UD = "apps/api/src/repo/community/user_directory.rs"
PAGE = "apps/web/app/(main)/users/page.tsx"
FIX = "1d2dac68"
VISIBLE = ("u.status < 2 AND NOT u.parked AND NOT u.archived "
           "AND u.username NOT LIKE 'deleted-%'")
OLD_ORD = "DESC"          # PG 缺省 = DESC NULLS FIRST
PSQL = ["docker", "exec", "-i", "flux-postgres",
        "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-v",
         "ON_ERROR_STOP=1", "-F", "|",
        "-f", "-"]


def read(rel, ref=None):
    if ref:
        return subprocess.run(["git", "show", "%s:%s" % (ref, rel)],
                              capture_output=True, text=True,
                              encoding="utf-8", errors="replace").stdout
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def psql(sql):
    p = subprocess.run(PSQL, input=sql, capture_output=True, text=True,
                       encoding="utf-8", errors="replace", cwd=ROOT)
    if p.returncode != 0:
        raise RuntimeError("psql 失败：%s" % ((p.stderr or "")[:240]))
    return (p.stdout or "").strip()


def ord_pair(src):
    """抓 `let ord = if desc { "…" } else { "…" }`（含跨行写法）两支字符串。"""
    m = re.search(
        r"let ord = if desc \{\s*\"([^\"]+)\"\s*\} else \{\s*\"([^\"]+)\"",
        src)
    if not m:                       # 单行写法
        m = re.search(r"let ord = if desc \{ \"([^\"]+)\" \} else \{ "
                      r"\"([^\"]+)\" \};", src)
    return (m.group(1), m.group(2)) if m else (None, None)


def whitelist_cols(src):
    """order_col 白名单里 match 臂右侧的列表达式（去表名限定）。"""
    body = src[src.index("pub fn order_col"):]
    return sorted({m.group(1).split(".")[-1] for m in re.finditer(
        r'=>\s*"([a-z_]+(?:\.[a-z_]+)?)"', body)})


def nullable(col):
    return psql("SELECT COALESCE(string_agg(is_nullable, ','), '?') FROM "
                "information_schema.columns WHERE table_schema='public' "
                "AND table_name='users' AND column_name='%s';" % col)


def main():
    cur = read(UD)
    old = read(UD, FIX + "^")
    a, b = ord_pair(cur)
    oa, ob = ord_pair(old)
    # ---------- M0 正控 ----------
    ok("M0 正控：FIX^ 的两支 ord 是裸 %r / %r（无 NULLS），且 users."
       "last_seen_at 现网确实可空" % (oa, ob),
       oa == "DESC" and ob == "ASC" and nullable("last_seen_at") == "YES",
       "前提没了＝这条尺看的已经不是这批修复")
    # ---------- M1 静态 ----------
    ok("M1 两支 ord 都带 NULLS LAST（实测 %r / %r）" % (a, b),
       all("NULLS LAST" in (s or "") for s in (a, b)),
       "退回裸 DESC＝PG 又把 NULL 排到最前")
    # ---------- M2 PG A/B ----------
    n_null = int(psql("SELECT count(*) FROM users u WHERE %s "
                      "AND u.last_seen_at IS NULL;" % VISIBLE))
    n_set = int(psql("SELECT count(*) FROM users u WHERE %s "
                     "AND u.last_seen_at IS NOT NULL;" % VISIBLE))
    ab = psql("""
SELECT 'OLDNULL|' || count(*) FILTER (WHERE x.l IS NULL)::text
  FROM (SELECT u.last_seen_at AS l FROM users u WHERE %s
        ORDER BY u.last_seen_at %s, u.id %s LIMIT 20) x;
SELECT 'NEWNULL|' || count(*) FILTER (WHERE x.l IS NULL)::text
  FROM (SELECT u.last_seen_at AS l FROM users u WHERE %s
        ORDER BY u.last_seen_at DESC NULLS LAST, u.id DESC LIMIT 20) x;
SELECT 'NEWTOP1|' || COALESCE(max(x.l)::text, 'NULL')
  FROM (SELECT u.last_seen_at AS l FROM users u WHERE %s
        ORDER BY u.last_seen_at DESC NULLS LAST, u.id DESC LIMIT 1) x;
""" % (VISIBLE, OLD_ORD, OLD_ORD, VISIBLE, VISIBLE))
    cells = dict(ln.split("|", 1) for ln in ab.split("\n") if "|" in ln)
    old_null = int(cells.get("OLDNULL", "-1"))
    new_null = int(cells.get("NEWNULL", "-1"))
    top1 = cells.get("NEWTOP1", "?")
    ok("M2 现网 A/B（可见会员 %d 个：有上线记录 %d / 无 %d）：旧页 20 行里 "
       "NULL=%s，新页 NULL=%s，新页第一行 last_seen=%s"
       % (n_null + n_set, n_set, n_null, old_null, new_null, top1),
       n_null > 0 and old_null == min(20, n_null)
       and new_null == max(0, 20 - n_set) and top1 != "NULL",
       "旧页没满屏 NULL＝这一列已被填满了（结论要重新定性）；"
       "新页还排 NULL＝NULLS LAST 没生效")
    # ---------- M3 界面可达 ----------
    page = read(PAGE)
    ok("M3 界面可达：/users 页把 last_seen 做成了一档可点的排序",
       'value: "last_seen"' in page and "sortLastSeen" in page,
       "控件撤了＝这条洞不再有用户可达路径，判据改由 M1 兜住")
    # ---------- M4 白名单棘轮 ----------
    cols = whitelist_cols(cur)
    nulls = [c for c in cols if nullable(c) == "YES"]
    ok("M4 排序白名单 %s 里可空的列有 %s；只要非空，两支 ord 都必须 "
       "NULLS LAST" % (cols, nulls or "无"),
       bool(cols) and ((not nulls) or ("NULLS LAST" in (a or "")
                                       and "NULLS LAST" in (b or ""))),
       "新加的可空排序键没配 NULLS LAST＝同一形状又长回来了")
    # ---------- M5 反例在册 ----------
    sn = psql("SELECT is_nullable FROM information_schema.columns WHERE "
              "table_schema='public' AND table_name='snatches' AND "
              "column_name='last_seen_at';")
    ok("M5 反例在册：snatches.last_seen_at=%s（NOT NULL 是本批不把 NULLS "
       "LAST 扩到 profile.rs:189 / user_torrentlist.rs:97 的理由；变成 "
       "YES 就得重看那两条）" % sn, sn == "NO",
       "可空性变了，那两条按 last_seen DESC 的口要重新定性")
    # ---------- M6 在册谓词只有一个来源，邻居出口必须引它 ----------
    # P-529/P-530：荣誉榜与勋章墙各写一份 WHERE，两处都漏了本模块规则 3
    # （封号/封存/归档/墓碑不进名录）。修法是把那四项收成 `member_visible(alias)`
    # 单点，两个邻居改成调它；这条判据钉的就是"单点 + 邻居引用"，
    # 以后谁再手抄一份字面量就会在这里红。
    ud = read(UD)
    four = ["status < 2", "parked", "archived", "deleted-%"]
    body = ud.split("fn member_visible(")[-1].split("pub ")[0]
    ok("M6 在册四条件收在 user_directory::member_visible 单点（缺 %d 项）"
       % len([c for c in four if c not in body]),
       "pub(crate) fn member_visible(" in ud
       and all(c in body for c in four) and "const VISIBLE" not in ud,
       "fn 体里缺=%s；const VISIBLE 残留=%s"
       % ([c for c in four if c not in body], "const VISIBLE" in ud))
    neighbors = {
        "community_http/notice.rs(荣誉榜)":
            "apps/api/src/community_http/notice.rs",
        "http/plugins.rs(勋章墙)": "apps/api/src/http/plugins.rs",
        "http/home.rs(首页在册数)": "apps/api/src/http/home.rs",
    }
    nofetch = [k for k, v in neighbors.items()
               if "user_directory::member_visible" not in read(v)]
    ok("M6b 名录的两个邻居出口都引同一单点（漏接 %d）" % len(nofetch),
       not nofetch, "、".join(nofetch) or "-")
    # ---- M7 首页「在册会员」与名录同一把尺（P-543）----
    # /home 的 site_data 段整个缓存在 Redis（cache:home:shared:v2）：不清掉
    # 就是在读上一次渲染的旧数，两个基线会给出同一个读数，判据当场失效。
    subprocess.run(["docker", "exec", "flux-redis", "sh", "-c",
                    'redis-cli -a "$REDIS_PASSWORD" --no-auth-warning '
                    "DEL cache:home:shared:v2"], capture_output=True,
                    text=True)
    tok = login("root")
    s1, h = call("GET", "/home", token=tok)
    site = ((h or {}).get("data") or {}).get("site_data") or {}
    s2, u = call("GET", "/users?per_page=1", token=tok)
    tot = ((u or {}).get("data") or {}).get("total")
    a = int(psql("SELECT count(*) FROM users WHERE status < 2"))
    b = int(psql("SELECT count(*) FROM users WHERE status < 2 AND NOT "
                 "parked AND NOT archived AND username NOT LIKE "
                 "'deleted-%'"))
    ok("M7 前置：现网确有「status<2 但名字仍是 deleted-」的行（%d vs %d）"
       % (a, b), a > b, "旧口径=%d 名录口径=%d" % (a, b))
    ok("M7b 首页 site.users == 名录 total == 名录口径（三处一致）",
       s1 == 200 and s2 == 200 and site.get("users") == tot == b,
       "首页=%s 名录=%s 名录口径=%s" % (site.get("users"), tot, b))

    if "--selftest" in sys.argv:
        selftest()
    summary()


def selftest():
    p = os.path.join(ROOT, UD)
    snap = io.open(p, "rb").read()
    txt = snap.decode("utf-8")
    a = '        let ord = if desc {\n            "DESC NULLS LAST"\n' \
        '        } else {\n            "ASC NULLS LAST"\n        };'
    b = '        let ord = if desc { "DESC" } else { "ASC" };'
    if txt.count(a) != 1:
        ok("S0 负控锚点唯一（命中 %d）" % txt.count(a), False,
           "ord 写法搬家了，先去看源码现在的形状")
        return
    try:
        io.open(p, "wb").write(txt.replace(a, b, 1).encode("utf-8"))
        cur = read(UD)
        x, y = ord_pair(cur)
        nulls_ok = "NULLS LAST" in (x or "") and "NULLS LAST" in (y or "")
        ab = psql("SELECT 'NULLCOUNT|' || count(*) FILTER (WHERE x.l IS NULL)"
                  "::text FROM (SELECT u.last_seen_at AS l FROM users u "
                  "WHERE %s ORDER BY u.last_seen_at %s, u.id %s LIMIT 20) x;"
                  % (VISIBLE, OLD_ORD, OLD_ORD))
        nc = ab.split("|")[-1].strip()
        ok("S1 负控：翻回裸 DESC 后 M1 抓到（NULLS 在册=%s），且旧形状在"
           "现网第一页 NULL=%s 条" % (nulls_ok, nc),
           not nulls_ok and nc == "20", "翻红了没抓到＝判据不承重")
    finally:
        io.open(p, "wb").write(snap)
    back = io.open(p, "rb").read() == snap
    x, y = ord_pair(read(UD))
    ok("S2 负控还原：按字节回到翻转前（%s）且正向复跑两支都带 NULLS LAST"
       % back, back and "NULLS LAST" in (x or "") and "NULLS LAST" in (y or ""),
       "还原不干净＝把翻转态留在了工作树")


if __name__ == "__main__":
    main()
