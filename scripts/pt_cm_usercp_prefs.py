# -*- coding: utf-8 -*-
"""CM 闸门（批七十三）：用户控制面板的列表偏好必须真的生效（P-275~276）。

现网（修前，镜像 `api:aa1021a`，2026-10-14 实测）：
  P-275 `users.incl_dead`（面板文案「显示断种/活种？」，三态 0 包括断种 /
        1 活种 / 2 断种，`<option value>` 就是这三个数）—— 能改、能存、
        能回显，列表读路径**一处都不读**：把它设成 0 后裸 `/torrents`
        仍按隐式「仅活种」筛，69 个过审种 ⇒ 恒 0 行。
  P-276 `users.torrents_per_page`（面板「每页 X 条」）同样零消费者：
        设成 5 后不带 `limit` 的请求仍回 20 行。
  ⇒ 根因形态是本仓反复出现的「写而不读」：判据活在注释与库里，执行边界
     没接（`list_noclamp_as.rs` 的缺省分支硬拼 `AND t.seeders > 0`）。
     产品默认（仅活种）本次**刻意不改**：建表默认 `incl_dead=1` 与面板
     选项文案就是设计口径，改默认等于替站长拍板。修的是接线。

本批断言里 CM1/CM2/CM5/CM6/CM8 修前必红（缺陷本体），CM3/CM4/CM7/CM9
是护栏：CM3 钉住「默认没被顺手翻转」，CM7 钉住「偏好切换不许串缓存视图」
（0289 那轮的同族安全边界），CM9 把面板其余三项未接线偏好钉成在册欠账。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_cm_usercp_prefs.py
破坏面：只动 root(uid=1) 的 incl_dead / torrents_per_page 两列，先
`CREATE TABLE … AS SELECT` 备份整行，收尾按备份整行复原并 IS DISTINCT
FROM 校验（复原不依赖被验代码），最后 DROP 备份表。
"""
import io
import os
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BAK = "_tmp_cm_bak"
KEYS = ("incl_bookmarked", "sp_state", "time_type")


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def src(f):
    return io.open(os.path.join(REPO, "apps", "api", "src", f),
                   encoding="utf-8").read()


def code(f):
    """剥掉 `//` 注释再匹配：注释里出现字段名不算消费（批六十五 CE5 同族）。"""
    return chr(10).join(l.split("//")[0] for l in src(f).splitlines())


def flush():
    """清掉列表两级共享缓存（首屏 45s / 扩展 20s），换偏好后必须冷启动。"""
    r = subprocess.run(
        ["docker", "exec", "flux-redis", "sh", "-c",
         "redis-cli --scan --pattern 'cache:tlist*' | xargs -r redis-cli DEL"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        universal_newlines=True, encoding="utf-8")
    return len([x for x in (r.stdout or "").splitlines() if x.strip().isdigit()])


def page(qs=""):
    s, b = call("GET", "/torrents" + qs, None, token=ROOT)
    d = b.get("data") if isinstance(b, dict) and "data" in b else b
    if not isinstance(d, dict):
        return s, None, []
    return s, d.get("total_estimate"), (d.get("items") or [])


def total(qs=""):
    return page(qs)[1]


def setprefs(incl_dead=None, per_page=None):
    if incl_dead is not None:
        psql("UPDATE users SET incl_dead=%d WHERE id=1" % incl_dead)
    if per_page is not None:
        psql("UPDATE users SET torrents_per_page=%d WHERE id=1" % per_page)
    flush()


clear_register_rl()

# ===== 库情与阳性对照 =====
APPR = int(psql("SELECT count(*) FROM torrents WHERE approval_status=1"))
ALIVE = int(psql("SELECT count(*) FROM torrents WHERE approval_status=1 "
                 "AND seeders>0"))
DEAD = int(psql("SELECT count(*) FROM torrents WHERE approval_status=1 "
                "AND seeders=0"))
ok("CM0a 库里有 %d 个过审种（活种 %d / 断种 %d）—— 断言分母先站住"
   % (APPR, ALIVE, DEAD), APPR > 0, "过审=%d 活=%d 断=%d" % (APPR, ALIVE, DEAD))
s0, t0, r0 = page("?alive=0")
ok("CM0b 阳性对照：`?alive=0` 必须读得到种（http=%s total=%s rows=%s），"
   "读到 0 说明站里真没种，本闸门作废" % (s0, t0, len(r0)),
   t0 is not None and int(t0) > 0, "total=%s" % t0)

psql("DROP TABLE IF EXISTS %s" % BAK)
psql("CREATE TABLE %s AS SELECT id, incl_dead, torrents_per_page FROM users "
     "WHERE id=1" % BAK)
BASE = psql("SELECT row_to_json(t) FROM %s t" % BAK)

try:
    # ===== CM1 偏好 0 =「包括断种」⇒ 裸 URL 必须看到全部过审种 =====
    setprefs(incl_dead=0)
    t_bare0 = total()
    ok("CM1 P-275：incl_dead=0（面板文案「包括断种」）后裸 /torrents 必须"
       "放行断种：total 应=%d，实测=%s（修前恒 0）" % (APPR, t_bare0),
       int(t_bare0) == APPR, "total=%s 期望=%d" % (t_bare0, APPR))

    # ===== CM2 偏好 2 =「断种」⇒ 只回断种，且行内容也得对上 =====
    setprefs(incl_dead=2)
    s2, t_bare2, items2 = page()
    nz = [it.get("seeders") for it in items2 if (it.get("seeders") or 0) != 0]
    ok("CM2 P-275：incl_dead=2（面板文案「断种」）后裸 /torrents total 应"
       "=%d（断种数），实测=%s" % (DEAD, t_bare2),
       int(t_bare2) == DEAD, "total=%s 期望=%d" % (t_bare2, DEAD))
    ok("CM2b 光看计数不够：返回 %d 行里做种数非 0 的行必须为 0（实际 %d 行"
       "带做种 ⇒ 口径没真落到行上）" % (len(items2), len(nz)),
       not nz, "非 0 做种行=%d" % len(nz))

    # ===== CM3 偏好 1（建表默认）⇒ 产品默认口径不许被顺手翻转 =====
    setprefs(incl_dead=1)
    t_bare1 = total()
    ok("CM3 护栏：incl_dead=1（`users.incl_dead` 建表默认、全站 1347 个账号"
       "的现值）时裸 /torrents 必须仍是「仅活种」= %d，实测=%s —— "
       "改默认是站长的产品决策，本批只接线不翻默认" % (ALIVE, t_bare1),
       int(t_bare1) == ALIVE, "total=%s 期望=%d" % (t_bare1, ALIVE))

    # ===== CM4 显式 ?alive= 压在偏好之上 =====
    setprefs(incl_dead=0)
    t_o1 = total("?alive=1")
    setprefs(incl_dead=2)
    t_o0 = total("?alive=0")
    ok("CM4 优先级：偏好 0 时 `?alive=1` 必须回到活种（应=%d 实测=%s）；"
       "偏好 2 时 `?alive=0` 必须放行全部（应=%d 实测=%s）"
       % (ALIVE, t_o1, APPR, t_o0),
       int(t_o1) == ALIVE and int(t_o0) == APPR,
       "alive=1→%s / alive=0→%s" % (t_o1, t_o0))

    # ===== CM5 旧站参数 include_dead 的显式值同样压过偏好 =====
    setprefs(incl_dead=0)
    t_i0 = total("?include_dead=0")
    t_i1 = total("?include_dead=1")
    ok("CM5 P-275：偏好=包括断种时，旧站口径 `?include_dead=0`（只要活种）"
       "必须仍回活种数 %d（实测 %s），`=1` 必须回 %d（实测 %s）—— "
       "第三方刷流工具靠这个参数" % (ALIVE, t_i0, APPR, t_i1),
       int(t_i0) == ALIVE and int(t_i1) == APPR,
       "=0→%s / =1→%s" % (t_i0, t_i1))

    # ===== CM6 每页条数偏好 =====
    setprefs(incl_dead=0, per_page=5)
    _, _, p5 = page("?alive=0")
    setprefs(per_page=0)
    _, _, p20 = page("?alive=0")
    setprefs(per_page=5)
    _, _, p2 = page("?alive=0&limit=2")
    ok("CM6 P-276：torrents_per_page=5 ⇒ 不带 limit 的列表必须回 5 行"
       "（实测 %d 行；修前恒 20）；偏好=0（未设置）⇒ 20 行（实测 %d）；"
       "`?limit=2` 必须压过偏好（实测 %d）"
       % (len(p5), len(p20), len(p2)),
       len(p5) == 5 and len(p20) == 20 and len(p2) == 2,
       "偏好 5→%d 行 / 未设置→%d 行 / limit=2→%d 行"
       % (len(p5), len(p20), len(p2)))

    # ===== CM7 偏好切换不许串共享缓存（0289 同族安全边界）=====
    setprefs(incl_dead=0)
    t_warm = total()          # 偏好 0 先打一次，写进共享缓存
    psql("UPDATE users SET incl_dead=1 WHERE id=1")   # 故意不 flush
    t_cross = total()
    ok("CM7 护栏：偏好 0 的请求先写共享键，同一裸 URL 立刻切成偏好 1 —— "
       "后者必须按自己的口径重算（应=%d），不能吃到上一轮的 total=%s "
       "（0289 首屏串用户同族）" % (ALIVE, t_warm),
       int(t_cross) == ALIVE, "写手=%s 读手=%s" % (t_warm, t_cross))
    flush()

    # ===== CM11 真实链路：偏好必须由面板接口写进去才生效（不是 psql 造的）=====
    s_put, _b_put = call("PUT", "/me/settings",
                         {"incl_dead": 2, "torrents_per_page": 7},
                         token=ROOT)
    read_back = psql("SELECT incl_dead || '/' || torrents_per_page FROM users "
                     "WHERE id=1")
    flush()
    s_p, t_p, r_p = page()
    ok("CM11 链路闭环：PUT /me/settings {incl_dead:2, torrents_per_page:7} "
       "→ http=%s、库内回读=%s、裸 /torrents total=%s（应=%d）且行数≤7"
       "（实测 %d）—— 会员在控制面板里点的两下必须真的改变他看到的列表"
       % (s_put, read_back, t_p, DEAD, len(r_p)),
       s_put == 200 and read_back == "2/7" and int(t_p) == DEAD
       and 0 < len(r_p) <= 7,
       "http=%s 回读=%s total=%s rows=%d" % (s_put, read_back, t_p, len(r_p)))
    setprefs(incl_dead=1, per_page=0)

    # ===== CM8 静态对账：偏好字段必须真的被列表路径消费 =====
    prefs = code("torrent_http/list_prefs.rs")
    ent = code("torrent_http/list.rs")
    sel_ok = ("incl_dead" in prefs) and ("torrents_per_page" in prefs)
    ok("CM8a P-275 implemented 对账：list_prefs.rs 的 SQL 必须同时读两列 "
       "（incl_dead=%s / torrents_per_page=%s），只读不 SELECT 就是假接线"
       % ("incl_dead" in prefs, "torrents_per_page" in prefs), sel_ok)
    n_fold = ent.count("alive_from_pref") + ent.count("limit_from_pref")
    ok("CM8b 入口必须把两项折算接上：list.rs 里 alive_from_pref + "
       "limit_from_pref 共 %d 处（各 1 才算两条都接）" % n_fold,
       ent.count("alive_from_pref") == 1
       and ent.count("limit_from_pref") == 1, "合计=%d" % n_fold)
    reg = code("torrent_http/mod.rs").count("mod list_prefs")
    ok("CM8c 拆出的 `list_prefs.rs` 必须在 mod.rs 注册（注册了才编译得进来，"
       "这条是给「拆完忘注册」留的红）：命中=%d" % reg, reg == 1,
       "命中=%d" % reg)
    wired = ("q.alive.or(alive_pref)" in ent) and (
        "q.limit.or(pref_limit)" in ent)
    ok("CM8d 接线形状：URL 参数必须**压在偏好之上**（`q.alive.or(alive_pref)"
       "` 与 `q.limit.or(pref_limit)` 两处都在=%s）——反过来就变成偏好覆盖"
       "显式参数" % wired, wired)


    # ===== CM9 钉桩：面板其余三项偏好仍未接线，不许静默漂成「已处理」=====
    hits = []
    for root, _dirs, files in os.walk(os.path.join(REPO, "apps", "api", "src")):
        for fn in files:
            if not fn.endswith(".rs"):
                continue
            rel = os.path.relpath(os.path.join(root, fn), REPO).replace(
                "\\", "/")
            if "me_settings_kv.rs" in rel or "dto.rs" in rel:
                continue
            body = chr(10).join(
                l.split("//")[0]
                for l in io.open(os.path.join(root, fn),
                                 encoding="utf-8").read().splitlines())
            for key in KEYS:
                if key in body:
                    hits.append((key, rel))
    for key in KEYS:
        n = len([h for h in hits if h[0] == key])
        ok("CM9 钉桩：面板项 `%s` 在 me_settings_kv 读写处之外命中 %d 处"
           "—— 必须仍是 0（有消费者了就删掉本桩）。站长欠账：收藏三态/"
           "促销八态/时间口径映射要不要接、接成什么，是产品决策不猜"
           % (key, n), n == 0, "命中=%d" % n)
finally:
    # ===== CM10 复原与残留 =====
    psql("UPDATE users u SET incl_dead=b.incl_dead, "
         "torrents_per_page=b.torrents_per_page FROM %s b WHERE u.id=b.id"
         % BAK)
    bad = psql("SELECT count(*) FROM users u JOIN %s b USING (id) WHERE "
               "u.incl_dead IS DISTINCT FROM b.incl_dead OR "
               "u.torrents_per_page IS DISTINCT FROM b.torrents_per_page"
               % BAK)
    resid = psql("SELECT count(*) FROM users WHERE incl_dead<>1 "
                 "OR torrents_per_page<>0")
    ok("CM10a 复原：root 两列按备份整行回原位（备份=%s，不一致行数=%s）"
       % (BASE, bad), bad == "0", "不一致=%s" % bad)
    psql("DROP TABLE IF EXISTS %s" % BAK)
    left = psql("SELECT count(*) FROM information_schema.tables WHERE "
                "table_name='%s'" % BAK)
    ok("CM10b 备份表收尾：`%s` 仍存在行数=%s" % (BAK, left), left == "0")
    ok("CM10c 残留：库内非默认偏好（incl_dead<>1 或 tpp<>0）的用户数=%s"
       % resid, resid == "0", "残留=%s" % resid)
    flush()

summary()
