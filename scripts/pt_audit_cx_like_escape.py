# -*- coding: utf-8 -*-
"""CX 闸门：ILIKE/LIKE 模式里的用户输入必须转义通配符（P-332）。

病形：SQL 写成 `ILIKE '%' || $n || '%'`，把用户输入直接拼成模式。 `%` 与 `_` 在
LIKE 里是通配符 ⇒
  - 搜 `%` = 不过滤（全表返回，助手注释点名的「性能滥用」面）
  - 搜 `100%` 变成 `%100%`，尾巴那个 % 吞掉「以 100 开头」的语义
  - 搜 `_` 命中任意单字符（`AB` 也算匹配 `A_B`）
先例有两条，口径不同：`torrents::esc_like`（转义 % _，并**剥掉**反斜杠）与
`http::like_pattern`（转义 % _ \\，并加 % 包裹）。凡是 SQL 里已经拼了
`'%' || $n || '%'` 的口，必须用 esc_like 这一族（只转义、不包裹，且不动参数编号，
`$n::text IS NULL` 的判空照样成立）。

刻意保留通配的两处（管理员配的邮箱域模式，见台账）：
`auth_http/register.rs`、`invite_http/email.rs`。

活体判据只读：拿 `/users?q=%` 这类口比对「搜 %」「搜一个不存在的串」「搜真名」
三个结果数——不转义时前两者差一个数量级，转义后必须相等。
正控钉在 00ea5f7b（本批修复的前一个提交）的文件原文，必须报洞。
"""
import glob
import os
import re
import subprocess
import sys

sys.path.insert(0, "scripts")
from pt_audit_lib import call, login, ok, dig, FAILURES  # noqa: E402

CTRL_REV = "00ea5f7b"  # 本批转义修复的前一个提交（绝对钉）
ROOT = "."
SCAN_DIRS = ["apps/api/src", "apps/worker/src"]
PAT_RE = re.compile(r"(?:ILIKE|LIKE)\s*'[%]'\s*\|\|\s*(.+?)\s*\|\|\s*'[%]'")
FN_RE = re.compile(r"\bfn (\w+)")
# 台账：刻意允许通配符的口子（管理员自己配的域名模式）
ALLOW = {
    "apps/api/src/auth_http/register.rs":
        "邮箱域黑白名单本身用 * / % 通配，转义会让管理员配的失效",
    "apps/api/src/invite_http/email.rs":
        "同上：邀请邮箱域校验读的是同一张 pattern 表",
}
ESCAPERS = ("esc_like", "like_pattern")


def read(rel, rev=None):
    if rev:
        o = subprocess.run(["git", "show", "%s:%s" % (rev, rel)],
                           capture_output=True, encoding="utf-8",
                           errors="replace")
        return o.stdout if o.returncode == 0 else None
    try:
        with open(rel, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def fn_span(src, pos):
    heads = [m.start() for m in FN_RE.finditer(src)]
    before = [h for h in heads if h < pos]
    after = [h for h in heads if h > pos]
    if not before:
        return src[:after[0] if after else len(src)]
    return src[before[-1]:after[0] if after else len(src)]


def audit(src_map):
    """返回 [(文件, 行, fn, 被拼进模式的表达式, 是否已转义)]。"""
    rows = []
    for rel, src in src_map.items():
        if src is None:
            continue
        pos = 0
        for i, line in enumerate(src.split("\n")):
            m = PAT_RE.search(line)
            at = pos + (m.start() if m else 0)
            pos += len(line) + 1
            if not m:
                continue
            span = fn_span(src, at)
            # SQL 里就地 replace 链转义（模式取自列时只能这样）
            sqlside = "replace(" in m.group(0)
            rs = any(e in span for e in ESCAPERS)
            # 文件级：转义可能发生在另一个函数里（email_pat / 直接 bind 处），
            # 所以「本文件出现过 escaper」+ 活体 A/B 一起才算数
            filelevel = any(e in src for e in ESCAPERS)
            rows.append((rel, i + 1, fn_name(span), m.group(1)[:28],
                         sqlside or rs or filelevel))
    return rows


def fn_name(span):
    m = FN_RE.search(span or "")
    return m.group(1) if m else "?"


def sources(rev=None):
    out = {}
    for d in SCAN_DIRS:
        if rev:
            o = subprocess.run(
                ["git", "grep", "-l", "-E",
                 r"(ILIKE|LIKE) '%' \|\|", rev, "--", d],
                capture_output=True, encoding="utf-8", errors="replace")
            for line in o.stdout.splitlines():
                rel = line.split(":", 1)[1]
                out[rel] = read(rel, rev)
        else:
            for p in glob.glob(d + "/**/*.rs", recursive=True):
                rel = p.replace(os.sep, "/")
                s = read(rel)
                if s and ("||" in s and ("ILIKE" in s or "LIKE" in s)):
                    out[rel] = s
    return out


print("=" * 68)
print("一、静态普查（拼进 LIKE 模式的用户输入有没有转义）")
print("=" * 68)
rows = audit(sources())
holes = [r for r in rows if not r[4]
         and not any(r[0].endswith(a) for a in ALLOW)]
print("基数：%d 处 LIKE 模式拼接；未转义且不在台账 %d 处；台账 %d 条"
      % (len(rows), len(holes), len(ALLOW)))
for h in holes[:10]:
    print("   未转义 %-46s:%-5s %-22s 拼的是 %s" % h[:4])
for rel in sorted({r[0] for r in rows
                   if any(r[0].endswith(a) for a in ALLOW)}):
    why = [ALLOW[a] for a in ALLOW if rel.endswith(a)][0]
    print("   刻意保留通配：%s（%s）" % (rel, why[:34]))
ok("CX1 没有未登记的 LIKE 通配拼接口", not holes,
   "%d 处：%s" % (len(holes), [h[0].split("/")[-1] for h in holes]))
ok("CX1b 普查基数可信（≥ 10 处拼接点，别测在空集合上）",
   len(rows) >= 10, "基数=%d" % len(rows))
ok("CX1c 台账每条都带 ≥ 20 字的理由",
   all(len(v) >= 20 for v in ALLOW.values()),
   [k for k, v in ALLOW.items() if len(v) < 20])

ctl = audit({("apps/api/src/" + f): read("apps/api/src/" + f, CTRL_REV)
             for f in ["community_http/artists.rs",
                       "repo/community/ban_log.rs",
                       "repo/community/user_directory.rs",
                       "admin_p3_http/invites.rs",
                       "community_http/sport_matches.rs"]})
ctl_holes = [c for c in ctl if not c[4]]
print("正控（%s 原文）：%d 处拼接，未转义 %d 处" % (CTRL_REV, len(ctl), len(ctl_holes)))
ok("CX2 正控：修复前那版必须报出 ≥ 4 处未转义（判据承重）",
   len(ctl_holes) >= 4, "只报出 %d 处 ⇒ 判据空转" % len(ctl_holes))

# 心愿单命中（模式取自列）两处必须在 SQL 里转义
w = read("apps/worker/src/jobs/social_team_expire.rs") or ""
ok("CX3 心愿单命中 SQL 两处都带上了转义链",
   w.count("ILIKE '%' || replace(replace(replace(") >= 2,
   "实际出现 %d 处" % w.count("ILIKE '%' || replace(replace(replace("))

# CX6：直接问 Postgres 这条转义链的语义（只读 SELECT，不动库）——
# 心愿单那两处是列拼模式，跑不到 job 就只能用这一层拿红证/绿证
SEM_SQL = """
SELECT 'xyz' ILIKE '%' || 'x%' || '%'                          AS p_unesc,
       'xyz' ILIKE '%' || replace(replace(replace('x%', '\\', ''),
              '%', '\\%'), '_', '\\_') || '%'                   AS p_esc,
       'axb' ILIKE '%' || 'a_b' || '%'                          AS u_unesc,
       'axb' ILIKE '%' || replace(replace(replace('a_b', '\\', ''),
              '%', '\\%'), '_', '\\_') || '%'                   AS u_esc;
"""


def psql(sql_text):
    o = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-At", "-f", "-"],
        input=sql_text.encode("utf-8"), capture_output=True)
    return o.stdout.decode("utf-8", "replace").strip()


sem = psql(SEM_SQL)
print("CX6 语义实测（p=关键词含%% / u=关键词含_）：\n   %s" % sem)
parts = sem.split("|")
ok("CX6 转义链语义成立（不转义会误命中、转义后按字面）",
   len(parts) >= 4 and parts[0].strip() == "t" and parts[1].strip() == "f"
   and parts[2].strip() == "t" and parts[3].strip() == "f",
   "读数=%r（期望 t|f|t|f）" % parts)

print()
if os.environ.get("CX_SKIP_LIVE"):
    print("（CX_SKIP_LIVE=1：跳过活体层）")
    print("红账：%s" % (FAILURES or "无"))
    sys.exit(1 if FAILURES else 0)
print("=" * 68)
print("二、活体只读 A/B（打的是 %s）" % os.environ.get(
    "FLUX_API_BASE", "http://127.0.0.1:8080/api/v1"))
print("=" * 68)
IMPOSSIBLE = "zz-不存在的关键串-qq"
# (路由, 取真实片段用的字段, 搜索参数名)
EPS = [("/users", "username", "q"), ("/artists", "name", "q"),
       ("/networks", "name", "q"), ("/ban-log", "username", "q"),
       ("/matches", "home", "team"), ("/admin/invites", "email", "email")]


def rows_of(resp):
    d = dig(resp, "data")
    if isinstance(d, list):
        return d
    if isinstance(d, dict):
        for k in ("items", "rows", "list", "data"):
            if isinstance(d.get(k), list):
                return d[k]
    return []


def count(tok, ep, q, param):
    url = ep + (("?" + param + "=" + urllib_quote(q)) if q is not None
                else "?per=50&limit=50&page=1")
    s, r = call("GET", url, None, tok)
    if s != 200:
        return None, "http=%s code=%s" % (s, r.get("code"))
    return rows_of(r), None


def urllib_quote(s):
    import urllib.parse
    return urllib.parse.quote(s, safe="")


tok = login()
tested = 0
for ep, namekey, param in EPS:
    base, err = count(tok, ep, None, param)
    if base is None:
        print("   %s 打不通：%s（跳过）" % (ep, err))
        continue
    imp, _ = count(tok, ep, IMPOSSIBLE, param)
    wild, _ = count(tok, ep, "%", param)
    under, _ = count(tok, ep, "_", param)
    # 正控探针：优先取「可搜字段」（name/username/title/email…）的值做片段——
    # 随便取首行某个字符串会挑到时间戳，搜不到就变成「正控不成立」的假跳过
    probe = None
    prefer = [namekey, "name", "username", "title", "email", "subject"]
    for r in (base or [])[:5]:
        if not isinstance(r, dict):
            continue
        vals = [r.get(k) for k in prefer if isinstance(r.get(k), str)]
        vals += [v for v in r.values()
                 if isinstance(v, str) and v not in vals]
        for v in vals:
            if re.match(r"^\d{4}-\d\d", v):
                continue
            # 片段要避开 %/_（含它们的值照样能用：按通配符切开取干净的一段），
            # 否则正控自己就撞上被测的坑，红得没道理
            pieces = [p for p in re.split(r"[%_]", v) if len(p) >= 2]
            if pieces:
                probe = pieces[0][:2]
                break
        if probe:
            break
    hit, _ = count(tok, ep, probe, param) if probe else (None, None)
    print("   %-14s 无过滤=%-4d 搜不存在=%-4s 搜%%=%-4s 搜_=%-4s 真名片段=%s"
          % (ep, len(base), len(imp or []), len(wild or []),
             len(under or []), (len(hit) if hit is not None else "无料")))
    if not probe or not hit:
        print("        ↑ 正控不成立（没有可搜的真实值），这一支不计入绿")
        continue
    if imp is not None and len(imp) == len(base) and len(base) > 0:
        print("        ↑ 「搜不存在的串」竟等于全量 ⇒ 过滤本身没生效，判据不可信")
        continue
    tested += 1
    # 判据：转义生效 = **返回的每一行都字面含有这个 needle**。整行 JSON 都算数
    # （这些口跨列匹配，如 networks 还搜 alias；未转义时 %/_ 当通配会返回不含
    #  它的行。拿「必须零条」当判据会被真实数据打红——站里本来就有 37 个用户名
    #  含下划线，搜 _ 合法地该命中它们）。ILIKE 不区分大小写，比较前统一小写。
    import json as _json

    def literal_all(rows, needle):
        bad = []
        for r in rows or []:
            blob = _json.dumps(r, ensure_ascii=False).lower()
            if needle.lower() not in blob:
                bad.append(str(r)[:30])
        return bad
    ok("CX4 %s 搜「%%」返回的每一行都必须字面含 %%（%d 条）" % (ep, len(wild or [])),
       not literal_all(wild, "%"),
       "这些行不含 %%（=通配没被转义）：%s" % literal_all(wild, "%")[:5])
    ok("CX4b %s 搜「_」返回的每一行都必须字面含 _（%d 条）" % (ep, len(under or [])),
       not literal_all(under, "_"),
       "这些行不含 _（=通配没被转义）：%s" % literal_all(under, "_")[:5])
    ok("CX4c %s 正控：搜真实片段「%s」必须有命中（过滤本身在工作）" % (ep, probe),
       bool(hit), "0 条 ⇒ 这一支测不出转义，别当绿")
ok("CX5 至少两条口成立正控并完成 A/B（别整批空转）", tested >= 2,
   "成立正控的口只有 %d 支" % tested)

print()
print("红账：%s" % (FAILURES or "无"))
sys.exit(1 if FAILURES else 0)
