# -*- coding: utf-8 -*-
"""CN 闸门（批七十四）：`?promo=` 促销筛选参数——非法值静默当「不筛」+ 库里
在用的 p30 档在列表里筛不出来（P-277~278）。

现网（修前，镜像 `api:aa1022a`，2026-10-14 实测）：
  P-277 `norm_promo` 对不认识的值返回 `None`，谓词退化成 `$p IS NULL`
        = **不加条件** ⇒ `?alive=0&promo=zzz` 拿到全站 69 个种。同族的分类
        参数（`category_id`）早先已改成「给了值却解析不出来 ⇒ 400」，注释
        原话就是「静默丢掉等于返回全站，比 400 危险得多」——促销参数漏修。
        多选中混非法同理：`?promo=free,zzz` 静默只剩 free 一档。
  P-278 显示侧认 p30（`torrents/promo.rs` 的 DOWN_FACTOR / EFFECTIVE_LABEL /
        `label()` 三处都有 `p30` ⇒ 行上挂「30%」角标），筛选侧白名单里**没有
        p30**（库里 10 条 p30 促销真实存在）⇒ 正是该模块开头写的要防止的
        「徽标显示免费、但按免费筛不出来」那类漂移，p30 撞上了。
  旧单元测试 `promo_unknown_dropped` 断言 `norm_promo("weird") == None`
        ⇒ 把洞固化成回归护栏（病形第四段），本批连同判据一起改。

修后契约：空值=不筛；给了值就必须全部落在白名单
`free|x2|half|p30|any|none` 内，任一非法 ⇒ 400 站内信封。
CN1/CN2/CN3/CN5/CN5b/CN6b/CN7b/CN8 修前必红；CN0a/CN0c/CN4/CN6/CN10 是
正控与「别把自家界面打进 400」的护栏。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_cn_promo_param.py
写库面：只在**一个已过审的探针种**上挂一枚 p30 促销（时长取 20 分 13 秒这个
怪值当签名，收尾与开局都按签名删，绝不可能命中站里真实促销）；令牌临时签、
收尾按 name 删；全程 try/finally，异常也先把签名行清掉（上一版没有 finally，
中途崩过一次，留下 1 枚生效促销把下一轮的阳性对照污染成假红——已修）。
"""
import io
import os
import subprocess
import sys
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOKEN_NAME = "cn74_probe_token"
SIG = "interval '20 minutes 13 seconds'"
HAS_LIVE = ("EXISTS(SELECT 1 FROM promotions p WHERE p.torrent_id = t.id "
            "AND p.starts_at <= now() AND p.ends_at > now())")


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def src(f):
    p = os.path.join(REPO, "apps", "api", "src", f)
    if not os.path.exists(p):  # 本批新建的文件在「修前跑红」时还不存在
        return ""
    return io.open(p, encoding="utf-8").read()


def code(f):
    return chr(10).join(l.split("//")[0] for l in src(f).splitlines())


def flush():
    subprocess.run(
        ["docker", "exec", "flux-redis", "sh", "-c",
         "redis-cli --scan --pattern 'cache:tlist*' | xargs -r redis-cli DEL"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        universal_newlines=True, encoding="utf-8")


def page(qs=""):
    s, b = call("GET", "/torrents" + qs, None, token=ROOT)
    d = b.get("data") if isinstance(b, dict) and "data" in b else b
    if not isinstance(d, dict):
        return s, None, [], (b or {})
    return s, d.get("total_estimate"), (d.get("items") or []), b


def total(qs=""):
    return page(qs)[1]


def n_total(qs):
    v = total(qs)
    return -1 if v is None else int(v)


def compat(qs, plain):
    req = urllib.request.Request(BASE + "/compat/nexusphp/torrents.json" + qs)
    req.add_header("Authorization", "Token " + (plain or ""))
    try:
        with urllib.request.urlopen(req, timeout=25) as r:
            return r.status
    except urllib.error.HTTPError as e:
        return e.code
    except Exception:
        return 0


clear_register_rl()
swept = len([x for x in psql("DELETE FROM promotions WHERE kind='p30' "
                             "AND scope='torrent' AND ends_at - starts_at "
                             "= %s RETURNING id" % SIG).splitlines()
             if x.strip().isdigit()])
APPR = int(psql("SELECT count(*) FROM torrents WHERE approval_status=1"))
ALIVE = int(psql("SELECT count(*) FROM torrents WHERE approval_status=1 "
                 "AND seeders>0"))
PROMO_BASE = int(psql("SELECT count(*) FROM promotions"))
# 申领表：这张表不属于本批的写口，但**别人（站长/其它闸门）会正常用它**，
# 所以收尾只能判「本批有没有动它」，取起点条数做差，不能写死 == 0。
INTENTS_BASE = int(psql("SELECT count(*) FROM promo_intents"))
LIVE_ANY = int(psql("SELECT count(*) FROM torrents t WHERE "
                    "t.approval_status=1 AND %s" % HAS_LIVE))
NONE_ALL = APPR - LIVE_ANY
NONE_ALIVE = int(psql("SELECT count(*) FROM torrents t WHERE "
                      "t.approval_status=1 AND t.seeders>0 AND NOT (%s)"
                      % HAS_LIVE))
print("过审种=%d 其中活种=%d 促销行=%d（开局清掉上轮遗留签名行=%d）"
      " 有生效促销的过审种=%d" % (APPR, ALIVE, PROMO_BASE, swept, LIVE_ANY))

# ===== CN0 正控：促销维的两个补集口径都要站住 =====
t_none_alive_dim = n_total("?promo=none")
t_none = n_total("?alive=0&promo=none")
ok("CN0a 阳性对照：无生效促销=%d 个种，`?alive=0&promo=none` 必须=%d"
   "（实测 %d）—— 对不上说明参数或促销谓词坏了" % (NONE_ALL, NONE_ALL, t_none),
   t_none == NONE_ALL and NONE_ALL > 0, "total=%d 期望=%d" % (t_none,
                                                             NONE_ALL))
ok("CN0c 先钉住「默认视图隐式仅活种」那一维：不带 alive 的 promo=none 实测 "
   "%d 必须等于「活种且无促销」=%d（库内活种=%d）——本闸门所有促销断言都走 "
   "alive=0，否则会踩 2026-10-06 那个「空列表被当成参数坏了」的假红"
   % (t_none_alive_dim, NONE_ALIVE, ALIVE),
   t_none_alive_dim == NONE_ALIVE, "裸=%d 期望=%d" % (t_none_alive_dim,
                                                      NONE_ALIVE))

PLAIN = ""
PROMO = ""
try:
    # ===== 探针：在一个过审种上挂一枚签名 p30 促销 =====
    TID = psql("SELECT id FROM torrents WHERE approval_status=1 "
               "ORDER BY id DESC LIMIT 1")
    PROMO = psql("INSERT INTO promotions (scope, torrent_id, kind, starts_at,"
                 " ends_at, created_by) VALUES ('torrent', %s, 'p30', "
                 "now(), now() + %s, 1) RETURNING id" % (TID, SIG))
    flush()
    P30 = int(psql("SELECT count(DISTINCT t.id) FROM torrents t JOIN "
                   "promotions p ON p.torrent_id = t.id WHERE p.kind='p30' "
                   "AND p.starts_at<=now() AND p.ends_at>now() AND "
                   "t.approval_status=1"))

    LIVE_NOW = int(psql("SELECT count(*) FROM torrents t WHERE "
                       "t.approval_status=1 AND %s" % HAS_LIVE))
    # ===== CN1 非法单值必须 400（修前：静默按「不筛」回全站）=====
    s1, t1, _, b1 = page("?alive=0&promo=zzz")
    ok("CN1 P-277：`?promo=zzz` 必须 400 并回站内信封，实测 http=%s "
       "total=%s msg=%s —— 修前是 200 + **全站 %d 行**（要一档拿到全部，"
       "答非所问还不报错）" % (s1, t1, (b1 or {}).get("message"), APPR),
       s1 == 400 and t1 is None, "http=%s total=%s" % (s1, t1))

    # ===== CN2/CN3 p30 必须是可筛的档，且真的落到行上 =====
    s2, t2, r2, _ = page("?alive=0&promo=p30")
    ids2 = [str(x.get("id")) for x in r2]
    ok("CN2 P-278：`?promo=p30` 必须是合法档（修前被白名单丢掉 ⇒ 静默回全站 "
       "%d 行）—— 实测 http=%s total=%s，库内生效 p30 的过审种=%d"
       % (APPR, s2, t2, P30),
       s2 == 200 and int(t2) == P30, "http=%s total=%s 应为=%d"
       % (s2, t2, P30))
    ok("CN3 光对总数不对内容不算过：按 p30 筛必须**恰好命中探针种 %s**，"
       "实测命中 %s 行（=生效 p30 数 %d）" % (TID, ids2, P30),
       ids2 == [TID] and len(r2) == P30, "命中=%s" % ids2)

    # ===== CN4 显示侧本来就认 p30（漂移的另一半）=====
    _, _, r4, _ = page("?alive=0&promo=any")
    badge = [x.get("promotion") for x in r4 if str(x.get("id")) == TID]
    ok("CN4 同一种数据两条实现要对齐：`promo=any` 取回探针种后角标=%s 必须是 "
       "p30 —— 角标认得、筛选档里没有，正是 promo.rs 开头声明要防的那类漂移"
       % badge, badge == ["p30"], "角标=%s" % badge)

    # ===== CN5 多选：合法组合要两档任一，混非法要报错 =====
    _, _, r5, _ = page("?alive=0&promo=free,p30")
    ok("CN5 多选合法组合 `?promo=free,p30` 必须两档任一命中（修前静默丢掉 "
       "p30 只剩 free 档 ⇒ 实测 %d 行，应含探针种 %s）"
       % (len(r5), TID), str(TID) in [str(x.get("id")) for x in r5],
       "rows=%d" % len(r5))
    s5, _, _, b5 = page("?alive=0&promo=free,zzz")
    ok("CN5b 多选中混非法必须 400（修前静默按 free 筛，用户少要了一档却看不"
       "出来）：实测 http=%s msg=%s" % (s5, (b5 or {}).get("message")),
       s5 == 400, "http=%s" % s5)

    # ===== CN6 六档逐个不许被打断 =====
    codes = [n_total("?alive=0&promo=" + v) for v in
             ("free", "x2", "half", "any", "none", "p30")]
    ok("CN6 六档（含新增 p30）必须全部走到业务层并回非负计数（%s）—— "
       "收紧只针对非法值；any=%d 应等于有生效促销的种数 %d、p30=%d"
       % (codes, codes[3] if codes[3] >= 0 else -1, LIVE_NOW, codes[5]),
       all(c >= 0 for c in codes) and codes[3] == LIVE_NOW
       and codes[5] == P30, "codes=%s 期望 any=%d p30=%d" % (codes, LIVE_NOW,
                                                             P30))

    # ===== CN6b 兼容层共用同一个归一器（工具侧不能各判一套）=====
    s_j1, _ = call("GET", "/compat/nexusphp/torrents.json?promo=free", None,
                   token=ROOT)
    s_mk, b_mk = call("POST", "/me/tokens",
                      {"name": TOKEN_NAME, "scopes": ["read"],
                       "password": "password123"}, token=ROOT)
    dd = (b_mk or {}).get("data") or {}
    PLAIN = dd.get("token") or dd.get("plaintext") or ""
    c_ok = compat("?promo=free&rows=1", PLAIN) if PLAIN else 0
    c_bad = compat("?promo=zzz&rows=1", PLAIN) if PLAIN else 0
    ok("CN6b 兼容层（API Token 直打，签令牌 http=%s）：`promo=free` 必须 200"
       "（实测 %s）、`promo=zzz` 必须 400（实测 %s，修前 200 静默全站）；"
       "会话 JWT 打这里按设计是 %s 不进" % (s_mk, c_ok, c_bad, s_j1),
       c_ok == 200 and c_bad == 400, "free=%s zzz=%s" % (c_ok, c_bad))

    # ===== CN7/CN8 静态对账：显示侧与筛选侧同一张档位表、判据不许回退 =====
    disp = code("torrents/promo.rs")
    filt = code("torrents/list.rs")
    pn = code("torrent_http/query.rs") + code(
        "torrent_http/query/promo_norm.rs")
    ok("CN7a 显示侧 `torrents/promo.rs` 认 p30（倍率/角标/标签三处之一）=%s"
       % ("'p30'" in disp), "'p30'" in disp)
    ok("CN7b 筛选侧必须也有 p30 桶：修前 `torrents/list.rs` 的 extra_preds "
       "只有 free/x2/half 三臂 + any/none，p30 命中=%s ⇒ 两处实现缺半边"
       % ("'p30'" in filt), "'p30'" in filt and "exists_clause" in filt)
    ok("CN8 判据不许回退成静默：`norm_promo` 必须返回 Result（修前 Option，"
       "未知值=None=不筛），且旧断言 `norm_promo(Some(\"weird\"...)), None`"
       "必须已从源码里消失：签名=%s 旧断言残留=%s"
       % ("Result<Option<String>" in pn,
          'norm_promo(Some("weird".into())), None' in pn),
       "Result<Option<String>" in pn
       and 'norm_promo(Some("weird".into())), None' not in pn)

    # ===== CN10 收紧别把自家界面打进 400 =====
    adv = io.open(os.path.join(REPO, "apps", "web", "app", "(main)",
                               "torrents", "_parts",
                               "torrents-adv-groups.tsx"),
                  encoding="utf-8").read()
    sent = [v for v in ("free", "x2", "half", "any", "none", "p30")
            if ('["%s",' % v) in adv]
    ok("CN10 界面促销 chip 实际会发出的档位=%s 必须都在白名单内（白名单将来"
       "再收窄，这里就是 400 的来源）" % sent, len(sent) >= 3,
       "档位=%s" % sent)
finally:
    # ===== CN9 复原与残留（异常也必须先清签名行与临时令牌）=====
    if PROMO:
        psql("DELETE FROM promotions WHERE id=%s" % PROMO)
    gone = len([x for x in psql("DELETE FROM promotions WHERE kind='p30' "
                                "AND scope='torrent' AND ends_at - starts_at "
                                "= %s RETURNING id" % SIG).splitlines()
                if x.strip().isdigit()])
    psql("DELETE FROM api_tokens WHERE name='%s'" % TOKEN_NAME)
    flush()
    # 判据不能用全表绝对计数（PROMO_BASE）：并跑时别的闸门也在造
    # 促销行（实测：bf2/cn 并跑后 CN9a 报 after=1，那一行是别人的），
    # ——合格时又要求全表回到跑前的数字，属于闸门之间互相污染，
    # 不是产品回归。改成按**本闸门的唯一签名**（SIG）判残留。
    after = int(psql("SELECT count(*) FROM promotions"))
    live_left = int(psql("SELECT count(*) FROM promotions WHERE "
                         "starts_at<=now() AND ends_at>now()"))
    mine_left = int(psql("SELECT count(*) FROM promotions WHERE scope='torrent' "
                         "AND ends_at - starts_at = %s" % SIG))
    tk_ghost = int(psql("SELECT count(*) FROM api_tokens WHERE name='%s'"
                        % TOKEN_NAME))
    intents = int(psql("SELECT count(*) FROM promo_intents"))
    ok("CN9a 探针促销删净：本闸门签名行残留=%d（全表 %d → %d）"
       "——判据身份而非全表计数，不被并跑污染"
       % (mine_left, PROMO_BASE, after),
       mine_left == 0,
       "本闸门残留=%d after=%d 生效残留=%d" % (mine_left, after, live_left))
    ok("CN9a2 临时令牌清干净：按 name 残留=%d" % tk_ghost, tk_ghost == 0)
    ok("CN9b 未牵动申领表：`promo_intents` 本轮增删=%d（起点 %d → 终点 %d，"
       "本批不写它；判据取**差值**而不是绝对零——这张表被别人正常用过之后，"
       "== 0 的写法就永远不可能绿）" % (intents - INTENTS_BASE, INTENTS_BASE,
                                        intents),
       intents == INTENTS_BASE)

summary()
