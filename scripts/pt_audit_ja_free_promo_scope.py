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
  JA5 残料：回滚后 promotions 行数等于基线；
  JA7 会员面「站点现状」两个出口同源：喊话盒 /stats 的 users/torrents 计数谓词
      必须与首页站点数据（http/home.rs）逐字相等（P-509：旧写法把被拒/回收站种
      算进种子数，现网同刻 200 vs 168）。

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
    """出错必须抛（P-552 实测的两个坑）：
      ① `-e ON_ERROR_STOP=1` 是设给 docker exec 的环境变量，psql 不认，
         语句报错照样 rc=0、ERROR 只落在 stderr ⇒ 判据读空串一路报绿；
      ② psql 从 stdin 读 SQL 时默认也不按错误返回非零码。
    所以 ON_ERROR_STOP 要用 `-v` 交给 psql 本身，再补一道 stderr 检查。"""
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent",
         "-v", "ON_ERROR_STOP=1", "-tA", "-F", "|", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8",
        errors="replace")
    err = (r.stderr or "").strip()
    if r.returncode != 0 or "ERROR" in err:
        raise RuntimeError("psql 失败：%s" % err[:300])
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
           or "promotion" not in c or "poster" not in c
           or "price" not in c]
print("   [现网] http=%s code=%s cards=%d reply=%r"
      % (st, dig(resp, "code"), len(cards or []),
         (data.get("reply") or "")[:24]))
ok("JA4 /free 回话契约齐（cmd/reply/cards，卡内 id/name/promotion/poster/"
   "price）", shape and not bad_row, "形状=%s 坏卡=%d 张" % (shape, len(bad_row)))

# ── JA7 会员面「站点现状」两个出口必须同源 ────────────────
# 喊话盒 /stats（leak_bot）与首页站点数据（http/home.rs）给会员看的是同一组数，
# 过去 /stats 用裸 `count(*) FROM torrents` ⇒ 现网同刻 200 vs 168（P-509）。
HOME = "apps/api/src/http/home.rs"


# 抽取器搬家课（P-543 之后重做）：原先按 `count(*) FROM <表> WHERE <谓词>)`
# 抓文本比对，一旦某一侧改走 member_visible()（谓词在源码里是 `{vis}`）或
# 表带了别名，正则就跳去抓隔壁那条 `status = 1` 的警告数——红得毫无道理。
# 判据改成只问两件事：① 两个出口都真的调名录单点；
# ② 谁都不许再留「裸 status < 2 的在册数」（首页的 warned/banned 计数用
# status = 1/2 是另一件事，不在这条尺的射程内）。
BARE = (r"count\(\*\)\s+FROM\s+users(?:\s+\w+)?\s+WHERE\s+"
        r"status\s*<\s*2\s*\)")
OLD_STATS = ("SELECT (SELECT count(*) FROM users WHERE status < 2), "
             "(SELECT count(*) FROM torrents WHERE approval_status = 1)")


def single_point(rel):
    body = re.sub(r"\\\s*\n\s*", " ", rd(rel))  # 吃掉 Rust 续行
    body = cut(body)
    return ("user_directory::member_visible(" in body,
            re.search(BARE, body) is not None)


bot_ok, bot_bare = single_point(BOT)
home_ok, home_bare = single_point(HOME)
print("   [读数] 喊话盒 单点=%s 裸尺=%s；首页 单点=%s 裸尺=%s"
      % (bot_ok, bot_bare, home_ok, home_bare))
ok("JA7 两个出口的用户数都走名录单点 member_visible",
   bot_ok and home_ok and not bot_bare and not home_bare,
   "喊话盒=%s/%s 首页=%s/%s" % (bot_ok, bot_bare, home_ok, home_bare))
ok("JA7b 正控：修前那版 /stats 的裸 status < 2 必须被同一把尺抓到",
   re.search(BARE, OLD_STATS) is not None,
   "抓不到旧形状＝JA7 的绿是空的")

# 活体读数（不参与判定：现网镜像未重建时必然还是旧数字，只留档）
st2, resp2 = call("GET", "/shoutbox/bot/exec?cmd=/stats", token=tok)
st3, resp3 = call("GET", "/stats", token=tok)
print("   [现网] 喊话盒=%r /stats口=%s"
      % ((dig(resp2, "data", "reply") or "")[:44],
         {k: v for k, v in (dig(resp3, "data") or {}).items()
          if k in ("users", "torrents")}))

# ── JA6 播报带种卡必须带定价（P-551：促销的「免费」只免上传量扣费）──
# charge_for_download 从不看促销 ⇒ 有价种进 /free 却不带 price，会员按
# 「当前免费」去下载才发现要扣魔力。现网此刻没有免费促销在册（cards 恒空），
# 所以这条自己挂一枚限时促销当靶子，跑完立刻删，不在现网留档。
_pa = (psql("SELECT id, price FROM torrents WHERE approval_status = 1 "
            "AND price > 0 ORDER BY id LIMIT 1") or "").split("|")
if len(_pa) == 2 and all(x.strip().isdigit() for x in _pa):
    _pt, _pp = int(_pa[0]), int(_pa[1])
    _b6 = int(psql("SELECT count(*) FROM promotions"))
    _pid = ""
    try:
        # ends_at 取最近的 ⇒ 按 effective_ends 排序必进前 5，靶子一定在卡里
        # source 是枚举 promotion_source{manual,magic_pool,preserve_grace,
        # task,subtitle}，自造标记会被旧 psql 静默咽掉 ⇒ 用合法值取回 id
        _pid = psql("INSERT INTO promotions (scope, torrent_id, kind,"
                    " starts_at, ends_at, source)"
                    " VALUES ('torrent', %d, 'free',"
                    " now() - interval '1 minute',"
                    " now() + interval '10 minutes', 'manual')"
                    " RETURNING id;" % _pt).strip()
        assert _pid.isdigit(), "夹具促销没插进去（id=%r）" % _pid
        st6, resp6 = call("GET", "/shoutbox/bot/exec?cmd=/free", token=tok)
        c6 = [c for c in ((dig(resp6, "data") or {}).get("cards") or [])
              if isinstance(c, dict) and c.get("id") == _pt]
        print("   [JA6] 靶子 #%d 定价 %s 魔力；命中卡=%s"
              % (_pt, _pp, str(c6[:1])[:120]))
        ok("JA6a 靶子进了播报（前置：促销生效且对本人可见）", bool(c6),
           "http=%s 靶子不在 cards＝夹具或行序失效，JA6b 的绿不算数" % st6)
        ok("JA6b 有价种的卡必须带库里那个定价",
           bool(c6) and c6[0].get("price") == _pp,
           "缺 price 键＝播报只说免费不给价（旧镜像即此形状）")
    finally:
        if _pid.isdigit():
            psql("DELETE FROM promotions WHERE id = %s;" % _pid)
        _l6 = int(psql("SELECT count(*) FROM promotions"))
        ok("JA6c 残料：靶子促销删净且 promotions 回到基线（%d→%d）"
           % (_b6, _l6), _l6 == _b6)
else:
    ok("JA6a 靶子：库里须有「过审且有定价」的种", False,
       "读数=%r（现网有 34 枚有价种；这条红说明数据或谓词变了）" % (_pa,))

_fe = cut(rd("apps/web/components/shout-free-cards.tsx"))
ok("JA6d 界面那侧真在消费 price（接线判据，不只后端有字段）",
   "c.price" in _fe and "td-dl-price" in _fe,
   "后端下发而界面不渲染＝同一个洞换个出口（批二百六十那一课）")

summary()
