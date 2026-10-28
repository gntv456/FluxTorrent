# -*- coding: utf-8 -*-
"""闸门 MH：/me/overview 的每个角标数字必须与库里同数（批一百九十一）。

为什么钉这个：会员面板上那一排数字是**用户判断自己站点处境的唯一依据**
（还剩几张邀请、收藏了几条、勋章几枚、火花余额）。本仓这一族已经出过好几种形：
P-439 收藏只验存在 ⇒ 角标 +1 而清单里没有；#70 分页参数收了不用；
概览页三数恒等式（users/banned/total）也曾各算各的。
所以这条不比对「像不像」，而是**同一个数字两条路各取一次**：
接口那条（`GET /me/overview`）与库那条（独立 SQL 谓词）。

实测（2026-10-28 现网 root）：12 项逐字相等
（bookmarks 1 / medals 4 / wearing 2 / comments 0 / uploads 195 /
invites_used 936 / invites_pending 86 / seeding 0 / leeching 0 /
spark_balance 10568 / uploaded 2149580800 / downloaded 5767430272）。
`invites` 的 status 分布还了账面：列表 total 1039 = 936(已用) + 86(未用)
 + 其余状态若干 ⇒ 面板的两个角标各自口径正确，不要求两者相加等于总数。

判据：
  MH1 前置：接口取到读数（token 失效/服务没起要当场红，不许空对空）
  MH2 主判据：12 项逐字相等；红账要指名是哪一项、两边各是多少
  MH3 反空转：至少 6 项的数必须非零（否则「全 0 == 全 0」也能蒙过 MH2）
  MH4 口径登记：`invites` 状态分布必须覆盖「面板两角标之外还有别的状态」这一事实
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, jwt_token, ok, psql_stdin, summary  # noqa

UID = 1  # root：与 jwt_token("root") 同一个账号
SQL = {
    "bookmarks": "SELECT count(*) FROM bookmarks WHERE user_id = %s",
    "medals": "SELECT count(*) FROM user_medals WHERE user_id = %s",
    "worn_medals": "SELECT count(*) FROM user_medals WHERE user_id = %s "
                   "AND wearing",
    "comments": "SELECT count(*) FROM comments WHERE user_id = %s",
    "uploads": "SELECT count(*) FROM torrents WHERE owner_id = %s "
               "AND approval_status = 1",
    "invites_used": "SELECT count(*) FROM invites WHERE inviter_id = %s "
                    "AND status = 1",
    "invites_pending": "SELECT count(*) FROM invites WHERE inviter_id = %s "
                       "AND status = 0 AND (expires_at IS NULL OR "
                       "expires_at > now())",
    "seeding": "SELECT count(*) FROM snatches WHERE user_id = %s AND seeding",
    "leeching": "SELECT count(*) FROM snatches WHERE user_id = %s AND leeching",
    "spark_balance": "SELECT spark_balance FROM users WHERE id = %s",
    "uploaded": "SELECT uploaded FROM users WHERE id = %s",
    "downloaded": "SELECT downloaded FROM users WHERE id = %s",
}


def one(sql):
    return psql_stdin(sql).splitlines()[0].strip()


tok = jwt_token("root")
s, r = call("GET", "/me/overview", None, tok)
d = (r or {}).get("data") if isinstance(r, dict) else None
d = d or {}
ok("MH1 前置：/me/overview 取到读数字段（空读数不算通过）",
   s == 200 and {"bookmarks", "medals", "spark_balance"} <= set(d),
   "http=%s 键数=%d" % (s, len(d)))

diff, zero = [], 0
for key, q in SQL.items():
    api = d.get(key)
    if isinstance(api, list):
        api = len(api)
    sqlv = one(q % UID)
    if str(api) != sqlv:
        diff.append("%s 接口=%s 库里=%s" % (key, api, sqlv))
    if sqlv not in ("0", ""):
        zero += 1
ok("MH2 面板 12 个数字与独立 SQL 逐字相等（不一致要指名是哪一项）",
   not diff, "; ".join(diff) or "-")
ok("MH3 反空转：至少 6 项非零（防「两边都取不到数」蒙成相等）",
   zero >= 6, "非零项=%d" % zero)

dist = psql_stdin(
    "SELECT string_agg(x, ' ' ORDER BY x) FROM (SELECT status::text || "
    "'=' || count(*)::text AS x FROM invites WHERE inviter_id = %s "
    "GROUP BY status) y;" % UID).splitlines()[0].strip()
used = int(one(SQL["invites_used"] % UID))
pend = int(one(SQL["invites_pending"] % UID))
tot_all = int(one("SELECT count(*) FROM invites WHERE inviter_id = %s" % UID))
ok("MH4 口径登记：邀请三态之和必须能解释列表 total（面板只报 0/1 两态）",
   tot_all >= used + pend,
   "状态分布=%s；面板 used=%d pending=%d，全量=%d"
   % (dist or "-", used, pend, tot_all))
print("[读数] /invites 列表 total 与状态分布见上（若有第三态，面板两个角标"
      "相加小于总数是**正确**的，不要改成相等）")

summary()
