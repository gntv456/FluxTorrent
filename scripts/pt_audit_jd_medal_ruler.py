# -*- coding: utf-8 -*-
"""常驻闸门 JD：勋章「持有数/佩戴上限」必须与勋章墙同一把尺（P-547）。

会员视角：站方给限时勋章设了 duration_days，到期后 worker 只把 wearing
置 false 并保留行（0287 P2），因此「持有」里有过期件是常态。站内凡是
**列表**都带 `expires_at IS NULL OR expires_at > now()`（/me/medals、
勋章墙、佩戴状态判定、帖子页的佩戴勋章条），只有三处**计数**没带：
  auth_http/profile.rs      主页「勋章 N」
  auth_http/me_settings.rs  控制台概览 GET /me/overview 的「N 枚」
  community_http/medal.rs   佩戴上限判定 worn
后果两型：
  展示型——控制台写着「4 枚」，点进勋章墙只有 1 枚（数字与它自己的链接
  打架，与 P-439/P-544 同形）；
  功能型——3 枚过期仍 wearing 的行把上限（现网 medals_max_worn=3）占满，
  会员戴新勋章吃 400「已达上限」，而界面上一枚都没显示在戴。
现网存量：0 行过期持有（36 行全为长期件）⇒ 本洞在等站方配一枚限时勋章，
所以判据用夹具行现造该形状，跑完即删。

修法：三处计数接列表那把尺；后台 admin_http/user_detail.rs 的 medals
**保留不过滤**（职员面看的是「这人拿过哪些」，0287 的注释成文认可）。

判据分层：
  JD0 前提   1 枚长期 + 3 枚过期（3 枚仍 wearing）⇒ 列表本身读到 1 枚
  JD1 正控   控制台与主页的计数必须等于列表长度（旧码读到 4＝缺陷本体）
  JD2 功能   戴那枚长期勋章必须成功（旧码 400「已达上限」）
  JD3 反控   改 3 枚为长期在戴后，第 4 枚仍须被拦（防「把上限删掉」假绿）
  JD4 静态   三处计数带过期过滤；后台那只口带台账说明不过滤
收尾：删探针持有行、删探针号，断言该探针号持有残料为 0。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_jd_medal_ruler.py
"""
import io
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import call, login, ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "apps", "api", "src")
PW_OLD = "Gate#123456"
PW_NEW = "Gate#2345678"


def psql(sql):
    return psql_stdin(sql if sql.strip().endswith(";") else sql + ";")


def max_worn():
    """站方口径现网实测 3；读不到就用代码里的缺省值，不改动站点设置。"""
    v = (psql("SELECT value FROM site_settings "
              "WHERE name='medals_max_worn'") or "").strip()
    return int(v) if v.isdigit() else 3


def read_src(rel):
    with io.open(os.path.join(SRC, rel), encoding="utf-8") as f:
        return f.read()


def strip_line_comments(src):
    """整行 // 注释剥掉：本批补丁的说明行会复述旧形状，不剥就成了
    「注释洗白真洞」那一族判据噪声。"""
    return "\n".join(ln for ln in src.splitlines()
                     if not ln.strip().startswith("//"))


def count_sql(rel):
    """取该文件里所有含 user_medals 的计数 SQL 串（按字面量跨度粗切）。"""
    body = strip_line_comments(read_src(rel))
    return [s for s in body.split("SELECT")
            if "FROM user_medals" in s and "count(*)" in s]


def medal_ids(n):
    return [int(x) for x in psql(
        "SELECT id FROM medals ORDER BY id LIMIT %d" % n).split()]


def plant(uid, mid, wearing, expired_days=None):
    """expired_days=None ⇒ 长期件；否则 N 天前就到期。"""
    exp = ("NULL" if expired_days is None
           else "now() - interval '%d days'" % expired_days)
    psql("INSERT INTO user_medals(user_id, medal_id, source, wearing, "
         "granted_at, expires_at) VALUES (%d, %d, 'event', %s, now(), %s);"
         % (uid, mid, "true" if wearing else "false", exp))


def medals_of(path, token, *nest):
    """取值一律逐层 or {}：{"data": null} 时 .get("data", {}) 会拿到 None
    （pt_audit_lib 的 dig 同形），少一层就把「读不到」演成「读到 0」。"""
    s, r = call("GET", path, None, token=token)
    d = (r or {}).get("data") or {}
    for k in nest:
        d = (d.get(k) or {}) if isinstance(d, dict) else {}
    return s, (d.get("medals") if isinstance(d, dict) else None), d


def main():
    root = login()
    cap = max_worn()
    user = "jd%d" % (int(time.time()) % 100000)
    mids = medal_ids(cap + 1)
    if len(mids) < cap + 1:
        raise SystemExit("勋章目录不足 %d 枚，造不出上限场景" % (cap + 1))
    live_mid, exp_mids = mids[0], mids[1:cap + 1]

    s, r = call("POST", "/admin/adduser", {"username": user,
                                           "password": PW_OLD,
                                           "email": "%s@jd.local" % user},
                token=root)
    if s != 200:
        raise SystemExit("建探针号失败 http=%s %s" % (s, str(r)[:160]))
    uid = int(psql("SELECT id FROM users WHERE username='%s'" % user) or 0)
    try:
        # 临时密码账号只放行 /me 与改密（auth_infra.rs），先改密再测写口
        pw = login(user, PW_OLD)
        s, r = call("POST", "/me/password/change",
                    {"old_password": PW_OLD, "new_password": PW_NEW},
                    token=pw)
        ok("JD0a 探针改密（写口前提）", s == 200,
           "http=%s %s" % (s, str(r)[:160]))
        mem = login(user, PW_NEW)

        plant(uid, live_mid, False)
        for m in exp_mids:
            plant(uid, m, True, 2)

        s, mine, md = medals_of("/me/medals", mem)
        items = md.get("items")
        listed = len(items) if isinstance(items, list) else len(
            [x for x in (md.get("medals") or []) if isinstance(x, dict)])
        ok("JD0b 勋章墙读到 1 枚（前提：列表本来就筛过期）", listed == 1,
           "http=%s data=%s" % (s, str(md)[:180]))

        s, panel, _ = medals_of("/me/overview", mem)
        ok("JD1a 控制台枚数须等于勋章墙（读到 %s，墙 1）" % panel,
           panel == listed,
           "多出来的是过期件：面板数字与它自己的链接打架"
           "（夹具共 %d 枚持有、%d 枚已过期）" % (cap + 1, cap))
        s, prof, _ = medals_of("/users/%d" % uid, root, "profile")
        ok("JD1b 主页枚数须等于勋章墙（读到 %s，墙 1）" % prof,
           prof == listed,
           "public 主页把过期件算进持有（夹具 %d 枚持有、%d 枚过期）"
           % (cap + 1, cap))

        s, w = call("PUT", "/medals/wear", {"medal_id": live_mid,
                                            "wear": True}, token=mem)
        ok("JD2 过期件不得占佩戴上限（http=%s）" % s, s == 200,
           "界面显示 0 枚在戴却报「已达上限」＝把会员挡在功能外；"
           "旧镜像这条红（现网 medals_max_worn=%d 被 3 枚过期占满）" % cap)

        # 反控：把 3 枚改为长期在戴（清掉 live 的佩戴态），第 4 枚仍须被拦
        call("PUT", "/medals/wear", {"medal_id": None}, token=mem)
        for m in exp_mids:
            psql("UPDATE user_medals SET expires_at = NULL, wearing = true, "
                 "source = 'shop' WHERE user_id = %d AND medal_id = %d;"
                 % (uid, m))
        worn = psql("SELECT count(*) FROM user_medals WHERE user_id = %d "
                    "AND wearing AND (expires_at IS NULL "
                    "OR expires_at > now())" % uid)
        ok("JD3a 夹具改为 %d 枚长期在戴（读到 %s）" % (cap, worn),
           worn == str(cap))
        s, w2 = call("PUT", "/medals/wear", {"medal_id": live_mid,
                                             "wear": True}, token=mem)
        ok("JD3b 戴满 %d 枚长期件后第 4 枚仍须拦（http=%s）" % (cap, s),
           s == 400, "这条红说明上限被一起删掉了，不是尺子修好了")
    finally:
        psql("DELETE FROM user_medals WHERE user_id = %d;" % uid)
        call("DELETE", "/admin/users/%d" % uid, None, token=root)
        left = psql("SELECT count(*) FROM user_medals WHERE user_id = %d"
                    % uid)
        ok("JD9 残料归零（探针持有行 %s）" % left, left == "0")

    for rel in ("auth_http/profile.rs", "auth_http/me_settings.rs",
                "community_http/medal.rs"):
        hits = count_sql(rel)
        bad = [h for h in hits if "expires_at" not in h]
        ok("JD4a %s 的 user_medals 计数带过期过滤（抓到 %d 条）"
           % (rel, len(hits)), bool(hits) and not bad,
           "违例 %d 条" % len(bad))
    admin = count_sql("admin_http/user_detail.rs")
    ok("JD4b 后台持有数仍不过滤（台账：职员面看「拿过哪些」）",
       bool(admin) and all("expires_at" not in h for h in admin),
       "抓到 %d 条；若改成过滤，请连同台账一起更新本判据" % len(admin))


if __name__ == "__main__":
    main()
    summary()
