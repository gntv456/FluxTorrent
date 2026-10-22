# -*- coding: utf-8 -*-
"""常驻闸门 JF：同一个会员的同名读数，跨出口必须给同一个数（P-549/550 的仪器）。

这轮抓到的三条（P-548 上传榜做种中、P-549 后台发布数、P-550 概览收藏夹
恒 0）都不是靠读代码发现的，而是把「同名字段在几个出口各读一次」摆在一起
对账发现的：HTTP 全 200、code 全 0，单看任何一个出口都发现不了。
本闸门把这把尺常驻下来，防下一轮新增出口再漂。

比对口径分两层（这是仪器自己的前提，写错就全是假红）：
  自助层  以 root 本人登录，比 GET /me、GET /me/overview、
          GET /users/1（data.profile）、GET /admin/users/1（平铺）。
          四个口都是「root 自己」，字段名相同就必须值相同。
  他人体  以 root 身份比 GET /users/{uid} 与 GET /admin/users/{uid}。
          **不许把 /me 系列混进来**：/me 永远是调用者自己，
          混进他人体对账会得到「每个 uid 都反常」的一屏假红。

允许的差异只有两条，其余同名字段一律要求相等（JF3 把这张例外表钉死，
新增例外必须同时给出成文理由，否则红）：
  uploads：public 主页按口径排除匿名发布（`NOT t.anonymous`），
           后台与自己面板都含 ⇒ 主页 ≤ 后台 是设计，不算漂。
  medals： 主页/控制台只数有效件，后台「持有」含过期件（0287 的成文口径，
           见 JD 闸门）。库里一旦出现过期件，这两个口就该分叉。
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import call, login, ok, summary  # noqa: E402

KEYS = ["uploaded", "downloaded", "seeding", "leeching", "uploads",
        "comments", "medals", "bookmarks", "unread_messages",
        "spark_balance", "class_id", "donor", "parked", "invites_pending"]
# 允许相等的差异字段：(字段, 原因)
EXCEPT_SELF = {}
EXCEPT_OTHER = {"uploads": "public 主页按口径排除匿名发布",
                "medals": "后台看持有（含过期），主页只数有效件"}
SELF_EXITS = ["/me", "/me/overview", "/users/1", "/admin/users/1"]


def grab(path, token):
    s, r = call("GET", path, None, token=token)
    d = (r or {}).get("data") or {}
    return s, (d.get("profile") if "profile" in d else d) or {}


def diff_of(vals):
    """vals: {exit: {field: value}} ⇒ 返回「同名字段取了多个不同值」的表。"""
    out = {}
    for k in KEYS:
        seen = {e: v[k] for e, v in vals.items() if k in v and v[k] is not None}
        if len(set(seen.values())) > 1:
            out[k] = seen
    return out


def main():
    root = login()
    vals = {}
    codes = []
    for path in SELF_EXITS:
        s, d = grab(path, root)
        codes.append((path, s))
        vals[path] = d
    ok("JF0 四个自助出口都回 200（读不到就谈不上对账）",
       all(c == 200 for _, c in codes), str(codes))
    bad = {k: v for k, v in diff_of(vals).items() if k not in EXCEPT_SELF}
    ok("JF1 本人四层同名读数一致（比对 %d 个字段）"
       % sum(1 for k in KEYS if any(k in v for v in vals.values())),
       not bad, str(bad)[:220])

    uids = (os.environ.get("JF_UIDS", "1,52,6785,7008,7330")).split(",")
    drift = []
    checked = 0
    for u in uids:
        s1, prof = grab("/users/%s" % u, root)
        s2, ad = grab("/admin/users/%s" % u, root)
        if s1 != 200 or s2 != 200:
            continue  # 名录尺子会挡掉被处置/归档号，不是这条判据的范围
        checked += 1
        for k, v in diff_of({"/users": prof, "/admin": ad}).items():
            if k not in EXCEPT_OTHER:
                drift.append("uid=%s %s %s" % (u, k, v))
    ok("JF2 他人体两出口同名读数一致（核了 %d 名）" % checked,
       not drift, "; ".join(drift[:3])[:220])
    ok("JF3 例外表仍是这两条（uploads 匿名、medals 过期）",
       set(EXCEPT_OTHER) == {"uploads", "medals"},
       "新增例外必须带成文理由，否则就是拿例外表掩盖漂移")
    pc = diff_of({"a": {"medals": 4, "uploads": 9, "bookmarks": 1},
                  "b": {"medals": 4, "uploads": 9, "bookmarks": 0}})
    ok("JF4 正控：造一处不一致，对账必须只报它（抓到 %s）"
       % sorted(pc), list(pc) == ["bookmarks"],
       "探测器失灵＝这本闸门是空的")


if __name__ == "__main__":
    main()
    summary()
