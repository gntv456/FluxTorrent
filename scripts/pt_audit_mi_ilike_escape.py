# -*- coding: utf-8 -*-
"""闸门 MI：`ILIKE` 的用户输入必须经过转义器（批一百九十三，2026-10-28）。

来历：`torrents/list.rs` 的注释记着旧站口径——「裸 `%`/`_` 进 ILIKE 会变成通配」，
`/torrents` 精确模式曾经把用户串原样喂给 ILIKE，搜 `%` 直接命中全站
（P-332 一族：`invites` 邮箱搜索、`artists`、`networks`、`sport_matches`、
`ban_log` 都补过）。本轮把整条 `ILIKE` 面重扫一遍，逐口确认真的没有漏网的：

  · `http/auth_infra.rs:24 like_pattern()` ＝ `%` + `\`→`\\`、`%`→`\%`、`_`→`\_`
    ——`admin_http/records.rs`（5 处）、`user_list.rs`、`audit.rs`、
    `shop_orders_admin.rs`、`templates.rs` 全走它；
  · `torrents/list.rs:13 esc_like()` + SQL 侧 `ESCAPE chr(92)` ——`/torrents`、
    `syn_search.rs`、`count_bind.rs`；
  · `community_http/forums.rs:78` 自带同口径三行 replace；
  · `invites.rs:76` / `artists.rs:78` / `networks.rs` 用 `ILIKE '%' || $n || '%'`
    包配，绑定值仍过 `esc_like`。

⇒ 结论：**当前零处裸绑**。这条判据因此钉的是「别再有新的」：
按文件扫 `ILIKE`，认不得出转义器的就是违例；违例集合必须等于台账（今天为空）。
尺子自己也要能红——MI1 用一段合成裸绑代码做正控。
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "apps", "api", "src")

ILIKE = re.compile(r"\bILIKE\b", re.I)
# 认这四类「已经转义」的形状。**按标识符认，不按调用形状认**：
# `.bind(kw.map(crate::torrents::esc_like))` 里 esc_like 后面跟的是 `)` 不是 `(`，
# 第一版按 `esc_like(` 匹配就把 7 个「其实转义了」的文件全判成违例（红在尺子上）。
# 代价是同文件只 import 不用也会被放过——所以 MI3 每次运行都把文件列表打出来，
# 让人逐轮复核，而不是让它当一个无人查看的绿章。
ESCAPERS = re.compile(r"esc_like|like_pattern|esc_kw|"
                      r"ESCAPE\s+chr\(92\)")
COMMENT = re.compile(r"^\s*(//|/\*)")

# 台账：实测「本文件里有 ILIKE、但转义器住在调用方」的两个文件。
# 不是借口——MI4 会去调用方把那件事查出来，查不到就当轮红。
LEDGER = {
    "torrent_http/works_sql.rs": "模式由 works.rs::like_of → esc_like 生成，"
                                 "本文件只拼 SQL 文本",
    "torrents/syn_search.rs": "ESCAPE chr(92) 由 list_noclamp_as.rs:76 传入，"
                              "本文件写的是 {esc} 占位",
}


def strip_block(txt):
    txt = re.sub(r"/\*.*?\*/", " ", txt, flags=re.S)
    return "\n".join(ln for ln in txt.splitlines() if not COMMENT.match(ln))


def offenders(txt):
    """有 ILIKE、但整文件找不到任何转义形状 ⇒ 违例。"""
    if not ILIKE.search(txt):
        return False
    return not ESCAPERS.search(txt)


def read(rel):
    p = os.path.join(SRC, rel.replace("/", os.sep))
    return strip_block(open(p, encoding="utf-8", errors="replace").read())


files, flagged = 0, set()
for dp, _d, ns in os.walk(SRC):
    for n in ns:
        if not n.endswith(".rs"):
            continue
        files += 1
        p = os.path.join(dp, n)
        t = strip_block(open(p, encoding="utf-8", errors="replace").read())
        rel = os.path.relpath(p, SRC).replace(os.sep, "/")
        if offenders(t):
            flagged.add(rel)

ok("MI1 正控：合成裸绑片段必须被判为违例（尺子认得『ILIKE 但没转义』）",
   offenders("SELECT 1 FROM t WHERE t.name ILIKE $1"),
   "合成片段判违例=%s" % offenders("SELECT 1 FROM t WHERE t.name ILIKE $1"))
ok("MI1b 正控反向：走过 like_pattern 的片段不得被误判（防尺子恒红）",
   not offenders("let p = like_pattern(&q);\nSELECT 1 FROM t WHERE "
                 "t.name ILIKE $1"),
   "反向片段判违例=%s" % offenders("let p = like_pattern(&q);\nSELECT 1 "
                                   "FROM t WHERE t.name ILIKE $1"))
ok("MI2 基数：真的扫到了 .rs（数量掉下限＝ROOT 指错，读数不作数）",
   files > 500, "文件=%d" % files)

new = flagged - set(LEDGER)
gone = set(LEDGER) - flagged
ok("MI3 违例集合 = 台账（新增=又有一个搜索框裸绑用户串；减少=补了转义要销号）",
   not new and not gone,
   "新增=%s 已消失=%s 现网 .rs=%d 已复核"
   % (sorted(new), sorted(gone), files))

# MI4 台账不能只是「我说的」：转义器既然住在调用方，就去调用方把它查出来。
w = read("torrent_http/works.rs")
s = read("torrents/list_noclamp_as.rs")
checks = {
    "works.rs 里有 like_of→esc_like": ("esc_like" in w and "like_of" in w),
    "list_noclamp_as.rs 传 ESCAPE chr(92)": ("ESCAPE chr(92)" in s),
}
bad4 = {k: v for k, v in checks.items() if not v}
ok("MI4 台账可查证：两个「转义在调用方」的说法必须在调用方真找得到",
   not bad4, "查不到=%s" % (list(bad4) or "-"))

summary()
