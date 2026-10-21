# -*- coding: utf-8 -*-
"""闸门 JA：喊话盒 `/free` 的促销作用域与可见性（批二百四十二 P-508）。

缺陷本体（真库事务内挂作用域级促销后实测 A/B）：
  旧取数自己写了一句
      FROM promotions p JOIN torrents t ON t.id = p.torrent_id
      WHERE p.starts_at <= now() AND p.ends_at > now() AND p.kind IN (...)
  ⇒ 只认「种子级直挂」的促销。挂上 global + category 两档作用域促销后：
      旧 SQL 回 0 张卡，同一时刻按共用层口径数得出 166 条会员可见的免费种。
  也就是全站免费周期间会员打 `/free` 得到「当前没有免费促销的种子。」，
  而列表每一行都挂着免费角标。这与后台筛选的 **P-121 是同一个洞的第二个出口**
  （`torrents/promo.rs` 的注释就写着「旧写法只数 `p.torrent_id = t.id` 直挂促销」）。

  第二条：没有任何可见性判据。`publish_http/upload_files_promo.rs` 在建种子行
  当场就挂自动促销（那时 approval_status=0），于是**未过审种的标题与链接被广播
  给全站**——会员点开只得到 1004。这与 P-439（收藏验存在不验可见）同族。

判据分层：
  JA1 真库 A/B（事务内挂夹具后 ROLLBACK）：作用域级促销下 新口径 > 旧形状；
      夹具 3 在「会员不可见」的靶子种上直挂免费促销——旧形状把它播出去，
      新口径的结果集里必须没有它（这条就是 upload 期自动挂促销的真实形状）。
  JA2 抽取器承重：从 `torrents/promo.rs` 的 matched() 里必须认得出五档作用域
      （抽取失效 ⇒ JA1 量的是一份过期口径，必须先红）；
  JA3 静态接线：读侧不得再出现内联促销匹配，必须调共用层 + 可见性口径；
  JA4 现网契约：/free 回 200 且 cards 是数组（前端 li 内纵包卡片，形状变了就崩）；
  JA5 残料：回滚后 promotions 行数等于基线。

夹具只在未提交的事务里存在，ROLLBACK 后消失；本支不写任何真实余额/流水。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ja_free_promo_scope.py
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import call, dig, login, ok, summary  # noqa: E402

PROMO = "apps/api/src/torrents/promo.rs"
READ = "apps/api/src/community_http/leak_bot/free_cards.rs"
BOT = "apps/api/src/community_http/leak_bot.rs"

# 缺陷本体的字面 SQL（提交前的旧形状，只作 A/B 的「旧」侧用）
OLD_SQL = (
    "SELECT t.id, t.approval_status FROM promotions p "
    "JOIN torrents t ON t.id = p.torrent_id "
    "WHERE p.starts_at <= now() AND p.ends_at > now() "
    "AND p.kind IN ('free','x2free') ORDER BY p.ends_at LIMIT 5"
)


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-F", "|",
         "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql 失败：%s" % (r.stderr or "").strip()[:300])
    return (r.stdout or "").strip()


def matched_clause():
    """把 promo.rs 里 matched() 的字符串字面量拼回一句 SQL 片段。

    Rust 用行尾 `\\` 续行，python 侧把 `\\`+换行+缩进吃掉即可还原同一份文本。
    """
    src = rd(PROMO)
    m = re.search(r"fn matched\(\) -> String \{\s*(.*?)\.to_string\(\)",
                  src, re.S)
    if not m:
        return ""
    body = m.group(1)
    body = re.sub(r"\\\s*\n\s*", "", body)
    return body.strip().strip('"').strip()


matched = matched_clause()
print("   [抽取] matched() 片段 %d 字符" % len(matched))

# ── JA2 抽取器承重（先于 A/B，否则量的是过期口径）──────────
need = ["p.torrent_id = t.id", "p.scope = 'global'", "p.scope = 'official'",
        "p.scope = 'non_official'", "p.scope = 'category'",
        "p.starts_at <= now()", "p.ends_at > now()"]
lost = [f for f in need if f not in matched]
ok("JA2 从 torrents/promo.rs 抽到的命中片段认得出五档作用域+时间窗",
   not lost, "缺=%s ⇒ 抽取器或共用口径搬家，JA1 的数字不算数" % lost)

# 共用口径下「会员现在能看到的免费种」（等价 exists_clause + 可见性）
NEW_EXISTS = ("EXISTS(SELECT 1 FROM promotions p WHERE %s "
              "AND p.kind::text IN ('free','x2free'))" % matched)
MEMBER_VIS = "t.approval_status = 1"

base_rows = psql("SELECT count(*) FROM promotions")
# 夹具 3 的靶子：一枚「会员不可见」（approval_status<>1）的种——先只读挑出来，
# 再用它的 id 直挂促销，这样 A/B 量的是同一枚种在两种口径下的下场。
target = psql("SELECT t.id FROM torrents t WHERE t.approval_status <> 1 "
              "ORDER BY t.id LIMIT 1")
assert target.isdigit(), "现网没有 approval_status<>1 的种，靶子挑不出来"
sql_ab = """BEGIN;
-- 夹具 1：全站免费（scope=global ⇒ torrent_id 必须为 NULL）
INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source)
VALUES ('global', NULL, 'free', now() - interval '1 hour',
        now() + interval '1 day', 'manual');
-- 夹具 2：一枚作用域级分类免费（挑会员可见种最多的分类）
INSERT INTO promotions (scope, kind, starts_at, ends_at, source, category_id)
SELECT 'category', 'free', now() - interval '1 hour',
       now() + interval '1 day', 'manual', x.category_id
FROM (SELECT t.category_id, count(*) c FROM torrents t
      WHERE t.approval_status = 1 GROUP BY t.category_id
      ORDER BY c DESC LIMIT 1) x;
-- 夹具 3：把一枚免费促销直挂在靶子种上（它就是 upload 期自动挂促销的形状）
INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source)
VALUES ('torrent', %s, 'free', now() - interval '1 hour',
        now() + interval '1 day', 'manual');
SELECT 'A_old' k, count(*) n,
       count(*) FILTER (WHERE z.approval_status <> 1) bad,
       count(*) FILTER (WHERE z.id = %s) hit FROM (%s) z;
-- B 侧不加 LIMIT 5：这里量的是「口径」本身，presentation 上限由 Rust 单测钉。
-- 带 LIMIT 会让判据被行序掩护——靶子 id 排在 5 名之外时，恒真的可见性谓词
-- 也不会让 JA1b 变红，判据就白给了。
SELECT 'B_new' k, count(*) n, 0 bad,
       count(*) FILTER (WHERE t.id = %s) hit
  FROM torrents t WHERE %s AND %s;
ROLLBACK;
SELECT 'POST' k, count(*) n, 0 bad, 0 hit FROM promotions;
""" % (target, target, OLD_SQL, target, NEW_EXISTS, MEMBER_VIS)

ab = {}
for line in psql(sql_ab).split("\n"):
    if not line.strip():
        continue
    parts = line.split("|")
    if len(parts) == 4:
        ab[parts[0]] = tuple(int(x) for x in parts[1:])
print("   [A/B] 靶子种 #%s（会员不可见）。夹具齐时：旧形状 %s 张卡，其中"
      "会员打不开 %s 张、含靶子 %s 张；新口径会员可见 %s 条、含靶子 %s 条；"
      "回滚后流水 %s 行"
      % (target, ab.get("A_old", (0, 0))[0], ab.get("A_old", (0, 0))[1],
         ab.get("A_old", (0, 0))[2], ab.get("B_new", (0, 0))[0],
         ab.get("B_new", (0, 0))[2], ab.get("POST", (0, 0))[0]))
ok("JA1a 作用域级促销期间 新口径必须比旧形状报得多（旧口漏 global/category）",
   ab.get("B_new", (0, 0))[0] > ab.get("A_old", (0, 0))[0],
   "新 %s vs 旧 %s" % (ab.get("B_new"), ab.get("A_old")))
ok("JA1b 同一枚不可见种：旧形状广播出去（含靶子≥1），新口径必须不含它",
   ab.get("A_old", (0, 0))[2] == 1 and ab.get("B_new", (0, 0))[2] == 0
   and ab.get("A_old", (0, 0))[1] >= 1,
   "旧=%s 新=%s（n/bad/hit）" % (ab.get("A_old"), ab.get("B_new")))
ok("JA5 残料：事务回滚后 promotions 行数等于基线",
   ab.get("POST", (0, 0))[0] == int(base_rows),
   "基线 %s 行，跑完 %s 行" % (base_rows, ab.get("POST")))

# ── JA3 静态接线（读侧不许自己写促销匹配）─────────────────
def cut(src):
    """剥 // 与 /// 行注释、/* */ 块注释：本模块的说明里就写着旧形状，
    不剥注释会把「讲缺陷」当成「有缺陷」（批一百五十的同一课）。"""
    s = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


read_src = cut(rd(READ))
bot_src = cut(rd(BOT))
inline = re.findall(r"FROM\s+promotions\s+p\s+JOIN\s+torrents", bot_src
                    + read_src)
wired = ("promo::exists_clause" in read_src
         and "promo::effective_kind" in read_src
         and "visibility_sql" in read_src)
ok("JA3 读侧走共用层（exists_clause/effective_kind/visibility_sql），"
   "且没有内联促销匹配", not inline and wired,
   "内联命中=%d；三件齐=%s" % (len(inline), wired))

# ── JA4 现网契约（前端 li 内纵包卡片，回话形状变了就整块不渲染）──
tok = login("root", "password123")
st, resp = call("GET", "/shoutbox/bot/exec?cmd=/free", token=tok)
data = dig(resp, "data") or {}
cards = data.get("cards")
shape = (st == 200 and dig(resp, "code") == 0 and isinstance(cards, list)
         and isinstance(data.get("reply"), str)
         and data.get("cmd") == "/free")
bad_row = [c for c in (cards or []) if not isinstance(c, dict)
           or not isinstance(c.get("id"), int)
           or not isinstance(c.get("name"), str)
           or "promotion" not in c or "poster" not in c]
print("   [现网] http=%s code=%s cards=%d reply=%r"
      % (st, dig(resp, "code"), len(cards or []),
         (data.get("reply") or "")[:24]))
ok("JA4 /free 回话契约齐（cmd/reply/cards，卡内 id/name/promotion/poster）",
   shape and not bad_row, "形状=%s 坏卡=%d 张" % (shape, len(bad_row)))

summary()
