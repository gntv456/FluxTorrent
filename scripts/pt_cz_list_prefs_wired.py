# -*- coding: utf-8 -*-
"""CZ 闸门（批八十二）：控制面板「显示促销 / 显示收藏」两条偏好必须真的
改变会员看到的种列表（P-289/P-290），并且促销档位的**筛选侧**必须与
**显示侧角标**同一张表（P-291）。

现网（修前，镜像 `api:aa1025c`，2026-10-14 实测）：
  P-289 `users.sp_state`（面板「显示促销」八档 0 全部 / 1 仅普通 / 2 免费 /
        3 2X / 4 2X免 / 5 半折 / 6 2X半 / 7 30%）——能改、能存、能回显，
        列表读路径**一处都不读**（与批七十三 P-275/276 同病形第八段）。
  P-290 `users.incl_bookmarked`（「显示收藏」三档 0 全部 / 1 仅收藏 /
        2 仅未收藏）同样零消费者。
  P-291 促销筛选白名单 `free|x2|half|p30|any|none` 里**没有合成档**：
        行上角标由 `promo.rs` 的 EFFECTIVE_LABEL 合成出 `x2free`/`x2half`
        （「2xFree」「2x50%」），筛选侧却只能按「族」取——`?promo=free` 把
        2X免 一起带出来（族口径，合理），但**没有一档能只要 2X免**。
        面板第 4 档「2X免」字面就是交集，若把它折成 `free,x2` 并集就会
        把「只免费」「只 2X」的种一起端给要 2X免 的人——给错数据，
        比不接更坏。故本批同时补 `x2free`/`x2half` 两个 token。

P-292 是**记在闸门里的欠账**（CZ7f 钉桩）：兼容层 OpenAPI 的 promo 参数说明
只列了 5 档，写全要 +1 行而承载文件 `compat_http/nexusphp.rs` 行数基线是
432/432（只减不增），所以本批不改、由闸门钉住（补全的那批会看到它转红）。

判据分层：CZ1 打的是**筛选侧本身**（`?promo=`，修前 CZ1d/1e/1g 必红），
CZ2/CZ3/CZ5/CZ6 打的是**偏好接线**（修前整族全红——偏好根本没被读），
CZ4b/CZ7/CZ8 是护栏与静态对账。CZ2h（30% 档）刻意断言「必须为空**且**
不等于全站」，避免「为空也算过」的空转（批七十二 ④ 同族）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_cz_list_prefs_wired.py
写库面：① root 的四列偏好——先 `CREATE TABLE … AS SELECT` 备份整行，收尾
按备份整行复原并 IS DISTINCT FROM 校验（复原不依赖被验代码）；
② 六枚探针促销——时长取 17 分 41..45 秒这五个怪值当签名，开局与收尾都按
签名删（绝不可能命中站里真实促销）；③ root 自己的两条收藏，收尾关掉。
全程 try/finally，异常也先把签名行清掉。
"""
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BAK = "_tmp_cz_bak"
SIGS = ["interval '17 minutes %d seconds'" % n for n in (41, 42, 43, 44, 45)]
S_FREE, S_X2, S_X2FREE, S_HALF, S_X2HALF = SIGS
ANYKIND = ("'free','x2','x2free','half','x2half','p30'")


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def code(f):
    """剥掉 `//` 注释再匹配：注释里出现字段名不算消费（批六十五 CE5 同族）。"""
    p = os.path.join(REPO, "apps", "api", "src", f)
    if not os.path.exists(p):        # 修前跑红时新文件还不存在
        return ""
    txt = open(p, encoding="utf-8").read().replace("\r\n", "\n")
    return chr(10).join(l.split("//")[0] for l in txt.splitlines())


def flush():
    """清掉列表两级共享缓存（首屏 45s / 扩展 20s），换偏好后必须冷启动。"""
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


def ids(qs="", full=False):
    """返回 (http, 集合, total_estimate)。集合取行内 id 与库里期望集合比对。

    `full=True` 给 URL 侧补 `limit=100`（`MAX_LIMIT`=100 > 现网 69 个过审种）：
    不补的话一页只回 20 行，把「期望 69 个」拿来比就是闸门自己的错。
    """
    if full and qs and "limit=" not in qs:
        qs += "&limit=100"
    s, t, rows, _b = page(qs)
    return s, set(str(x.get("id")) for x in rows), t


def db_ids(pred):
    out = psql("SELECT string_agg(t.id::text, ',' ORDER BY t.id) FROM "
               "torrents t WHERE t.approval_status=1 AND %s" % pred)
    return set(x for x in out.split(",") if x)


def has(kinds):
    ks = ",".join("'%s'" % k for k in kinds)
    return ("EXISTS(SELECT 1 FROM promotions p WHERE p.torrent_id=t.id "
            "AND p.starts_at<=now() AND p.ends_at>now() AND p.kind::text "
            "IN (%s))" % ks)


def setprefs(**kw):
    cols = {"incl_dead": "incl_dead", "per_page": "torrents_per_page",
            "sp": "sp_state", "bm": "incl_bookmarked"}
    for k, v in kw.items():
        psql("UPDATE users SET %s=%d WHERE id=1" % (cols[k], v))
    flush()


clear_register_rl()
psql("DROP TABLE IF EXISTS %s" % BAK)
psql("CREATE TABLE %s AS SELECT id, incl_dead, torrents_per_page, sp_state, "
     "incl_bookmarked FROM users WHERE id=1" % BAK)
BASE = psql("SELECT row_to_json(t) FROM %s t" % BAK)
PREF0 = psql("SELECT incl_dead || '/' || torrents_per_page || '/' || "
             "sp_state || '/' || incl_bookmarked FROM users WHERE id=1")
BM0 = int(psql("SELECT count(*) FROM bookmarks WHERE user_id=1"))
BMK = []
DIRTY0 = int(psql("SELECT count(*) FROM users WHERE incl_dead<>1 "
                  "OR torrents_per_page<>0 OR sp_state<>0 "
                  "OR incl_bookmarked<>0"))
PROBE0 = psql("SELECT count(*) FROM promotions WHERE ends_at - starts_at "
              "IN (%s)" % ",".join(SIGS))
APPR = int(psql("SELECT count(*) FROM torrents WHERE approval_status=1"))
PROMO_BASE = int(psql("SELECT count(*) FROM promotions"))

# 探针：6 个过审种上挂 5 档促销（P4 用 free+x2 两行合成，没有 x2free 行）
PB = psql("SELECT string_agg(id::text, ',' ORDER BY id) FROM ("
          "SELECT id FROM torrents WHERE approval_status=1 ORDER BY id "
          "LIMIT 6) x").split(",")
PS = [x for x in PB if x]

try:
    # ===== CZ0 前置与阳性对照 =====
    ok("CZ0a 库里有 %d 个过审种、探针取到 %d 个（%s）——断言分母先站住"
       % (APPR, len(PS), PS), APPR > 0 and len(PS) == 6,
       "过审=%d 探针=%s" % (APPR, len(PS)))
    psql("DELETE FROM promotions WHERE ends_at - starts_at IN (%s)"
         % ",".join(SIGS))
    live_other = int(psql("SELECT count(*) FROM promotions WHERE "
                          "starts_at<=now() AND ends_at>now()"))
    ok("CZ0b 挂探针前库内生效促销=%d 必须为 0——否则集合不完全由探针决定，"
       "下面的等式断言全不可信" % live_other, live_other == 0,
       "生效=%d" % live_other)

    def ins(tid, kind, sig):
        return psql("INSERT INTO promotions (scope, torrent_id, kind, "
                    "starts_at, ends_at, created_by) VALUES ('torrent', "
                    "%s, '%s', now(), now() + %s, 1) RETURNING id"
                    % (tid, kind, sig))

    probe_ids = []
    probe_ids += [ins(PS[0], "free", S_FREE)]
    probe_ids += [ins(PS[1], "x2", S_X2)]
    probe_ids += [ins(PS[2], "x2free", S_X2FREE)]
    probe_ids += [ins(PS[3], "free", S_FREE), ins(PS[3], "x2", S_X2)]
    probe_ids += [ins(PS[4], "half", S_HALF)]
    probe_ids += [ins(PS[5], "x2half", S_X2HALF)]
    flush()

    E_ALL = db_ids("true")
    E_FREE = db_ids(has(["free", "x2free"]))
    E_X2 = db_ids(has(["x2", "x2free", "x2half"]))
    E_HALF = db_ids(has(["half", "x2half"]))
    E_P30 = db_ids(has(["p30"]))
    E_X2FREE = E_FREE & E_X2
    E_X2HALF = E_HALF & E_X2
    E_NONE = E_ALL - (E_FREE | E_X2 | E_HALF | E_P30)
    print("期望集合：全部=%d free族=%s x2族=%s half族=%s 2X免=%s 2X半=%s "
          "无促销=%d" % (len(E_ALL), sorted(E_FREE), sorted(E_X2),
                         sorted(E_HALF), sorted(E_X2FREE), sorted(E_X2HALF),
                         len(E_NONE)))
    ok("CZ0c 探针形状站住：2X免=2 个（直挂档 1 + 合成档 1）、2X半=1 个、"
       "三族非空——否则 CZ1 的交集断言是空转（实测 %s / %s）"
       % (sorted(E_X2FREE), sorted(E_X2HALF)),
       len(E_X2FREE) == 2 and len(E_X2HALF) == 1 and E_FREE and E_X2
       and E_HALF, "x2free=%s x2half=%s" % (sorted(E_X2FREE),
                                            sorted(E_X2HALF)))

    # ===== CZ1 筛选侧：族档 / 合成档语义（P-291）=====
    s1a, r1a, _ = ids("?alive=0&promo=free", full=True)
    ok("CZ1a `?promo=free`（族口径）必须=免费族 %s，实测 http=%s %s"
       % (sorted(E_FREE), s1a, sorted(r1a)), r1a == E_FREE,
       "实测=%s 期望=%s" % (sorted(r1a), sorted(E_FREE)))
    s1b, r1b, _ = ids("?alive=0&promo=x2", full=True)
    ok("CZ1b `?promo=x2` 必须=上传翻倍族 %s，实测 %s"
       % (sorted(E_X2), sorted(r1b)), r1b == E_X2, "实测=%s" % sorted(r1b))
    s1c, r1c, _ = ids("?alive=0&promo=half", full=True)
    ok("CZ1c `?promo=half` 必须=半折族 %s，实测 %s"
       % (sorted(E_HALF), sorted(r1c)), r1c == E_HALF, "实测=%s" % sorted(r1c))
    s1d, _t1d, rows1d, b1d = page("?alive=0&promo=x2free&limit=100")
    # `page()` 回的是**原始行**（dict 列表），`ids()` 才折成 id 集合；
    # 这里要按集合比，所以自己折一次（上一版直接 sorted(rows) ⇒
    # TypeError: '<' not supported between instances of 'dict' and 'dict'）。
    r1d = set(str(x.get("id")) for x in rows1d)
    ok("CZ1d P-291：`?promo=x2free`（2X 且免）必须=交集 %s，实测 http=%s "
       "%s msg=%s —— 修前白名单里没这一档 ⇒ 400，而面板第 4 档「2X免」"
       "只有并集可折" % (sorted(E_X2FREE), s1d, sorted(r1d) if s1d == 200
                        else "-", (b1d or {}).get("message")),
       s1d == 200 and r1d == E_X2FREE, "http=%s 实测=%s 期望=%s"
       % (s1d, sorted(r1d), sorted(E_X2FREE)))
    s1e, r1e, _ = ids("?alive=0&promo=x2half")
    ok("CZ1e `?promo=x2half` 必须=交集 %s，实测 http=%s %s"
       % (sorted(E_X2HALF), s1e, sorted(r1e)), s1e == 200
       and r1e == E_X2HALF, "http=%s 实测=%s" % (s1e, sorted(r1e)))
    _s1f, _t1f, r1f, _b1f = page("?alive=0&promo=any&limit=100")
    badge = dict((str(x.get("id")), x.get("promotion")) for x in r1f)
    ok("CZ1f 筛的就是显示的那一列：探针 P4=%s 只有 free+x2 两行（没有 "
       "x2free 行），其角标必须=%s（合成）且被 `?promo=x2free` 命中=%s "
       "——角标与筛选两处实现漂移就是 P-278 那一族"
       % (PS[3], badge.get(PS[3]), PS[3] in E_X2FREE),
       badge.get(PS[3]) == "x2free" and PS[3] in r1d,
       "角标=%s 命中=%s" % (badge.get(PS[3]), PS[3] in r1d))
    _, r1g, _ = ids("?alive=0&promo=free,x2", full=True)
    ok("CZ1g 并集≠交集（这条就是「面板 4 档不能折成 free,x2」的证据）："
       "`?promo=free,x2`=%d 个 %s 必须严格多于 `?promo=x2free`=%d 个 %s"
       % (len(r1g), sorted(r1g), len(r1d) if s1d == 200 else -1,
          sorted(r1d)),
       len(r1g) > (len(r1d) if s1d == 200 else 0),
       "并集=%d 交集=%d" % (len(r1g), len(r1d) if s1d == 200 else -1))
    s1h, _r1h, _t1h, b1h = page("?alive=0&promo=zzz")
    msg1h = (b1h or {}).get("message") or ""
    hit8 = [t for t in ("free", "x2", "half", "p30", "x2free", "x2half",
                        "any", "none") if t in msg1h]
    ok("CZ1h 400 的文案就是用户能学到的接口：`?promo=zzz` 必须 400，且「可用」"
       "清单要把 8 档全列出来（实测命中 %d/8，msg=%s）——加了合成档而文案"
       "不跟着走，界面与第三方工具永远学不到新档"
       % (len(hit8), msg1h[:64]), s1h == 400 and len(hit8) == 8,
       "http=%s 命中=%d/8" % (s1h, len(hit8)))

    # ===== CZ2 偏好折算：bare /torrents 必须按面板档位筛（P-289）=====
    # 现网全部 69 个过审种 seeders=0，裸 URL 的隐式缺省是「仅活种」⇒ 恒 0 行，
    # 读不到任何差异。故先把 incl_dead 设成 0（批七十三已接线的那一维），
    # 让 bare 视图等于「全部过审种」；再把「每页条数」偏好设成 100
    # （同样已接线，`MAX_LIMIT`=100 > 69），一页才装得下整份期望集合，
    # 否则裸 URL 只回 20 行，比对必红——那是闸门自己的错不是产品的错。
    setprefs(incl_dead=0, per_page=100, sp=0)
    s2, bare_all, _ = ids()
    ok("CZ2a 护栏：sp_state=0（全部）时裸 /torrents 必须=全部过审种 %d 个，"
       "实测 http=%s %d 个" % (len(E_ALL), s2, len(bare_all)),
       bare_all == E_ALL, "实测=%d 期望=%d" % (len(bare_all), len(E_ALL)))
    FOLD = {1: ("none", E_NONE), 2: ("free", E_FREE), 3: ("x2", E_X2),
            4: ("x2free", E_X2FREE), 5: ("half", E_HALF),
            6: ("x2half", E_X2HALF), 7: ("p30", E_P30)}
    for st in sorted(FOLD):
        tok, exp = FOLD[st]
        setprefs(sp=st)
        s3, got, tot = ids()
        set3, got_u, _ = ids("?alive=0&promo=" + tok, full=True)
        ok("CZ2%d P-289：面板第 %d 档（=%s）折进读路径后，裸 /torrents 必须"
           "与显式 `?promo=%s` 同一批行（期望 %d 个=%s，实测偏好侧 http=%s "
           "%d 个=%s、URL 侧 http=%s %d 个）"
           % (st, st, tok, tok, len(exp), sorted(exp), s3, len(got),
              sorted(got), set3, len(got_u)),
           got == exp and got_u == exp,
           "偏好=%s URL=%s 期望=%s" % (sorted(got), sorted(got_u),
                                       sorted(exp)))
        if st == 7:
            # `total_estimate` 的**合法 0**不能被 `tot or -1` 这类写法吃掉
            # （falsy 陷阱：上一版就是这么把一条绿判据读成红的）。
            ok("CZ2h 30%% 档必须**真的被筛空**而不是「为空也算过」：实测 "
               "%d 个（库内 p30 生效种=%d），且 total=%s 不等于全站 %d"
               % (len(got), len(E_P30), tot, len(E_ALL)),
               len(got) == 0 and tot is not None and int(tot) == 0,
               "rows=%d total=%s" % (len(got), tot))
    setprefs(sp=99)
    s4, dirty, _ = ids()
    ok("CZ2x 脏值口径：sp_state=99（面板给不出这值，写口界是 -1..200）必须"
       "按「无偏好=全部」处理，实测 http=%s %d 个（全站 %d）——脏值不该"
       "把列表打空，也不该报错" % (s4, len(dirty), len(E_ALL)),
       dirty == E_ALL, "实测=%d" % len(dirty))

    # ===== CZ3 显式 URL 压在偏好之上 =====
    setprefs(sp=4)
    s5, got3a, _ = ids("?alive=0&promo=none", full=True)
    ok("CZ3a 优先级：偏好=2X免 时 `?promo=none` 必须回到无促销集 %d 个"
       "（实测 http=%s %d 个）——偏好是「我的默认视图」不是「强制视图」"
       % (len(E_NONE), s5, len(got3a)), got3a == E_NONE,
       "实测=%d" % len(got3a))
    _, got3b, _ = ids("?alive=0&promo=free,x2", full=True)
    ok("CZ3b 偏好=2X免 时 `?promo=free,x2` 必须按 URL 的并集 %d 个（实测 "
       "%d 个），不能被偏好收窄成交集" % (len(r1g), len(got3b)),
       got3b == r1g, "实测=%s" % sorted(got3b))
    setprefs(sp=2)
    _, got3c, _ = ids("?alive=0&promo=x2free", full=True)
    ok("CZ3c 偏好=免费 时 `?promo=x2free` 必须=交集 %s（实测 %s）"
       % (sorted(E_X2FREE), sorted(got3c)), got3c == E_X2FREE,
       "实测=%s" % sorted(got3c))

    # ===== CZ4 收藏偏好（P-290）=====
    for tid in (PS[0], PS[2]):
        s_bm, _b_bm = call("PUT", "/torrents/%s/bookmark" % tid,
                           {"on": True}, token=ROOT)
        BMK.append(tid)
        ok("CZ4x 探针收藏写入必须 200（PUT /torrents/%s/bookmark 实测 "
           "%s）——否则下面的收藏断言是空转" % (tid, s_bm), s_bm == 200,
           "http=%s" % s_bm)
    E_BM = set([PS[0], PS[2]])
    flush()
    setprefs(sp=0, bm=1)
    s6, got4, _ = ids()
    ok("CZ4a P-290：incl_bookmarked=1（面板「仅收藏」）时裸 /torrents 必须"
       "=root 的收藏 %s，实测 http=%s %s（修前=全站 %d 个）"
       % (sorted(E_BM), s6, sorted(got4), len(E_ALL)), got4 == E_BM,
       "实测=%s" % sorted(got4))
    setprefs(bm=0)
    _, got4b, _ = ids()
    ok("CZ4b 护栏：bm=0（全部）必须=全站 %d 个，实测 %d 个"
       % (len(E_ALL), len(got4b)), got4b == E_ALL, "实测=%d" % len(got4b))
    setprefs(bm=2)
    s7, got4c, _ = ids()
    ok("CZ4c 钉桩：bm=2（面板「仅未收藏」）**刻意未接**——TorrentFilter 只有"
       "「只看收藏」一维，折成 false 会演成「全部」＝给错数据。此档必须是"
       "欠账而不是假收口：实测 http=%s %d 个（=全部 %d，符合「未接」）"
       % (s7, len(got4c), len(E_ALL)), got4c == E_ALL,
       "实测=%d" % len(got4c))
    bml = code("torrent_http/list_prefs.rs")
    ok("CZ4d 静态：`bookmarked_from_pref` 必须是 `incl_bookmarked == 1` 形状"
       "（把 2 折成 true 就是假收口）=%s" % ("incl_bookmarked == 1" in bml),
       "incl_bookmarked == 1" in bml)

    # ===== CZ5 真链路：PUT /me/settings（面板走的就是这个口）=====
    s8, _b8 = call("PUT", "/me/settings",
                   {"incl_dead": 0, "sp_state": 4, "incl_bookmarked": 1},
                   token=ROOT)
    rb = psql("SELECT incl_dead || '/' || sp_state || '/' || "
              "incl_bookmarked FROM users WHERE id=1")
    flush()
    s9, got5, _ = ids()
    ok("CZ5 链路闭环：PUT /me/settings{sp_state:4,incl_bookmarked:1} → "
       "http=%s、库内回读=%s、裸 /torrents 必须=「2X免 ∩ 我的收藏」=%s "
       "实测 http=%s %s"
       % (s8, rb, sorted(E_X2FREE & E_BM), s9, sorted(got5)),
       s8 == 200 and got5 == (E_X2FREE & E_BM) and rb == "0/4/1",
       "http=%s/%s 回读=%s 实测=%s" % (s8, s9, rb, sorted(got5)))

    # ===== CZ6 偏好切换不许串共享缓存（0289/CM7 同族安全边界）=====
    setprefs(incl_dead=0, per_page=100, sp=0, bm=0)
    _, warm_all, _ = ids()          # 先以「全部」写进首屏共享键
    psql("UPDATE users SET sp_state=2 WHERE id=1")   # 故意不 flush
    s10, crossed, tot10 = ids()
    ok("CZ6 护栏：偏好 0 的请求先写共享键，同一裸 URL 立刻切成偏好 2 —— "
       "后者必须按自己的口径重算（应=免费族 %d 个），不能吃到上一轮的 "
       "%d 个（实测 http=%s %d 个，写手=%d 读手=%d）"
       % (len(E_FREE), len(warm_all), s10, len(crossed), len(warm_all),
          len(crossed)), crossed == E_FREE,
       "写手=%d 读手=%d" % (len(warm_all), len(crossed)))

    # ===== CZ7 静态对账：折算必须真的接在进入 filter 的那一步 =====
    ent = code("torrent_http/list.rs")
    ok("CZ7a 入口必须两处折算都在：list.rs 里 promo_from_pref=%d 处、"
       "bookmarked_from_pref=%d 处（各 1 才算两条都接）"
       % (ent.count("promo_from_pref"), ent.count("bookmarked_from_pref")),
       ent.count("promo_from_pref") == 1
       and ent.count("bookmarked_from_pref") == 1,
       "promo=%d bm=%d" % (ent.count("promo_from_pref"),
                           ent.count("bookmarked_from_pref")))
    ok("CZ7b 折算必须在 URL 之后（`.or_else(` 形状）：URL 给了 promo 就不该"
       "再读偏好，实测 or_else=%d 处" % ent.count("or_else"),
       ent.count("or_else") >= 2, "or_else=%d" % ent.count("or_else"))
    allsrc = ""
    for root_, _d, fs in os.walk(os.path.join(REPO, "apps", "api", "src")):
        for fn in fs:
            if fn.endswith(".rs"):
                allsrc += code(os.path.relpath(
                    os.path.join(root_, fn),
                    os.path.join(REPO, "apps", "api", "src")).replace(
                        "\\", "/"))
    sel = ("sp_state" in allsrc and "incl_bookmarked" in allsrc)
    ok("CZ7c SELECT 面：必须有一条 SQL 把四列一起读出来（含 sp_state/"
       "incl_bookmarked=%s）——只写折算函数不 SELECT 就是假接线" % sel, sel)
    pn = code("torrent_http/query/promo_norm.rs")
    ok("CZ7d 白名单必须含两个合成档：`x2free`=%s `x2half`=%s"
       % ("\"x2free\"" in pn, "\"x2half\"" in pn),
       '"x2free"' in pn and '"x2half"' in pn)
    ex = code("torrents/list.rs")
    ok("CZ7e 谓词面必须有两臂（与白名单同批，否则 token 合法却无匹配臂）："
       "x2free=%s x2half=%s" % ("promo_x2free" in ex, "promo_x2half" in ex),
       "promo_x2free" in ex and "promo_x2half" in ex)
    # P-292 钉桩（本批**刻意不改**）：兼容层 OpenAPI 里 promo 参数的说明
    # 只列了 5 档，批七十四补的 p30 与本批补的 x2free/x2half 都没写进去——
    # 第三方工具照文档传 ?promo=p30 会被 400 拒掉，属于「契约说小了」。
    # 不改的理由要说清：承载文件 compat_http/nexusphp.rs 行数基线 432/432
    # （只减不增），把八档写全需要 +1 行 ⇒ 必撞门禁；正确改法是站方先把
    # 该文件拆小。断言取「仍未写全」为真——谁补全了它，这条就转红提醒删桩
    # （与批六十九 CI6 / 批八十二 CZ4c 同一套钉桩口径）。
    doc = code("compat_http/nexusphp.rs")
    m = re.search(r'"promo" = String, Query, description = "([^"]*)"', doc)
    dtext = m.group(1) if m else ""
    listed = [t for t in ("free", "x2", "half", "p30", "x2free", "x2half",
                          "any", "none") if t in dtext]
    ok("CZ7f 钉桩 P-292：兼容层 OpenAPI 的 promo 说明=%r 只列到 %d/8 档，"
       "必须仍是「没写全」（在册欠账）；写全的那一批请回来删掉本桩"
       % (dtext[:44], len(listed)), m is not None and len(listed) < 8,
       "已列=%d/8" % len(listed))
finally:
    # ===== CZ8 复原与残留 =====
    psql("DELETE FROM promotions WHERE ends_at - starts_at IN (%s)"
         % ",".join(SIGS))
    for tid in BMK:
        call("PUT", "/torrents/%s/bookmark" % tid, {"on": False},
             token=ROOT)
    psql("UPDATE users u SET incl_dead=b.incl_dead, "
         "torrents_per_page=b.torrents_per_page, sp_state=b.sp_state, "
         "incl_bookmarked=b.incl_bookmarked FROM %s b WHERE u.id=b.id" % BAK)
    bad = psql("SELECT count(*) FROM users u JOIN %s b USING (id) WHERE "
               "u.incl_dead IS DISTINCT FROM b.incl_dead OR "
               "u.torrents_per_page IS DISTINCT FROM b.torrents_per_page OR "
               "u.sp_state IS DISTINCT FROM b.sp_state OR "
               "u.incl_bookmarked IS DISTINCT FROM b.incl_bookmarked" % BAK)
    flush()
    after = int(psql("SELECT count(*) FROM promotions"))
    live_left = int(psql("SELECT count(*) FROM promotions WHERE "
                         "starts_at<=now() AND ends_at>now()"))
    bm_left = int(psql("SELECT count(*) FROM bookmarks WHERE user_id=1"))
    dirty_pref = int(psql("SELECT count(*) FROM users WHERE incl_dead<>1 "
                          "OR torrents_per_page<>0 OR sp_state<>0 "
                          "OR incl_bookmarked<>0"))
    ok("CZ8a root 四列按备份整行复原（备份=%s，不一致行数=%s，起点值=%s）"
       % (BASE, bad, PREF0), bad == "0", "不一致=%s" % bad)
    ok("CZ8b 探针促销删净：`promotions` %d → %d，生效残留=%d"
       % (PROMO_BASE, after, live_left),
       after == PROMO_BASE and live_left == 0,
       "after=%d 残留=%d" % (after, live_left))
    ok("CZ8c 收藏回原位：root 书签 %d → %d" % (BM0, bm_left),
       bm_left == BM0, "bm=%d" % bm_left)
    ok("CZ8d 非默认偏好的用户数 本轮起点=%d → 收尾=%d 必须相等（本批只动 "
       "root；写死 ==0 的判据在别人正常用过这些偏好之后永远不可能绿）"
       % (DIRTY0, dirty_pref), dirty_pref == DIRTY0,
       "收尾=%d 起点=%d" % (dirty_pref, DIRTY0))
    psql("DROP TABLE IF EXISTS %s" % BAK)
    left = psql("SELECT count(*) FROM information_schema.tables WHERE "
                "table_name='%s'" % BAK)
    ok("CZ8e 备份表收尾：`%s` 仍存在行数=%s" % (BAK, left), left == "0")
    ok("CZ8f 探针签名行本就应为 0（开局 %s，若收尾删不掉说明签名被复用）"
       % PROBE0, psql("SELECT count(*) FROM promotions WHERE "
                      "ends_at - starts_at IN (%s)" % ",".join(SIGS)) == "0")

summary()
