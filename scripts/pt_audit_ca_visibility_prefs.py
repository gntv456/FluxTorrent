# -*- coding: utf-8 -*-
"""闸门 CA：论坛可见性两档偏好（P-339）——「看头像 / 看签名」曾是有写无读的假开关。

实测红线（批一百五十四）：`view_avatars` / `view_signatures` 两个字段**全仓只出现在
`me_settings_kv` 的写侧**（UPDATE 与 bind），没有任何读路径引用它们——用户能在
用户中心勾掉「不看头像」，存得进去，而读路径照旧 `SELECT u.avatar_url` 照常返回：

    view_avatars=false → GET /forums/topics/{id} 仍带 avatar_url

签名更糟：帖子读路径压根不返回 `u.signature`，于是「显示签名」那档开关连
「显示什么」都没有；而 profile 路径把签名查了**两次**——主查询算过一次、
`extra` 语句又原样取一次并覆盖掉结果，开关等于没接线。

本闸钉九件事（全部真跑，读的是接口真实响应）：
  S0 正控：写口 PUT /me/settings 改得动这两个开关（否则下游全是空判）。
  S1 头像档：view_avatars=false ⇒ 帖子楼层 avatar_url 变 null。
  S2 头像档：view_avatars=true ⇒ 头像照常返回（反证 CASE 写反了）。
  S3 签名档：view_signatures=false ⇒ 帖子楼层与 profile 两处都不带签名。
  S4 签名档：view_signatures=true ⇒ 两处都带签名。
  S5 profile 头像档与帖子档同口径（不许一处执法一处不执法）。
  S6 profile 顶层 signature 与 profile.signature 同值（不许顶层绕过开关）。
  S7 自己的档对本人也生效（界面上「我看别人的」与「别人看我」共用同一列，
     本闸只钉前者：自己的偏好确实被读路径尊重）。
  S8 零残留：头像/签名/两档开关全部回到跑前值。

收尾纪律：只改 root 的 avatar_url / signature / 两档开关，逐项回读复原。
"""
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, login, ok, summary  # noqa: E402

# BASE 一律取自 pt_audit_lib（单一来源）：此前这里另起一份默认 8100，
# 与主栈 8080 不同步，且与同一脚本里 login() 用的 base 不是一回事——
# 闸门直接连不上（WinError 10061），整条判据根本跑不起来。
UID = 1
MARK = "ca339%d" % (int(time.time()) % 100000)
AV = "https://example.invalid/avatar-probe.png"
SG = "CA339 签名档探针"
TID = []


def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        r = urllib.request.urlopen(req, data, timeout=25)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:  # noqa: BLE001
            return e.code, {}


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
         "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (r.stdout or "").strip()


def set_pref(view_avatars, view_signatures):
    psql("UPDATE users SET view_avatars = %s, view_signatures = %s WHERE id = %d"
         % ("true" if view_avatars else "false",
            "true" if view_signatures else "false", UID))


def profile_of(token):
    return (call("GET", "/users/%d" % UID, token=token)[1].get("data") or {})


def first_post(token, tid):
    d = call("GET", "/forums/topics/%d" % tid, token=token)[1].get("data") or {}
    return (d.get("posts") or [{}])[0]


ROOT = login()
ORIG = psql("SELECT COALESCE(avatar_url,''), COALESCE(signature,''), "
            "view_avatars, view_signatures FROM users WHERE id = %d" % UID)
print("== S0 地基：素材 + 写口 + 跑前快照（%s）==" % ORIG)

psql("UPDATE users SET avatar_url = '%s', signature = '%s' WHERE id = %d"
     % (AV, SG, UID))
s, r = call("POST", "/forums/topics",
            {"forum_id": 1, "title": MARK, "body": MARK}, token=ROOT)
tid = (r.get("data") or {}).get("topic_id")
if tid:
    TID.append(tid)
ok("S0 素材就绪（造一条楼，取它的作者字段）", bool(tid), "tid=%s" % tid)

# S0 正控：走产品自己的写口改开关，改得动才谈执法
st, rw = call("PUT", "/me/settings",
              {"view_avatars": False, "view_signatures": False}, token=ROOT)
back = psql("SELECT view_avatars, view_signatures FROM users WHERE id = %d" % UID)
ok("S0 正控：PUT /me/settings 改得动这两档（红线：写口都没有就无从执法）",
   back.startswith("f|"), "写回=%s http=%s" % (back, st))

if tid:
    # 先按写口的值（false/false）验关闭态
    p = first_post(ROOT, tid)
    pr = profile_of(ROOT)
    ok("S1 头像档关：帖子楼层不再带头像（红线：仍返回 avatar_url）",
       p.get("avatar_url") is None, "avatar=%r" % p.get("avatar_url"))
    ok("S3 签名档关：帖子楼层不带签名",
       p.get("signature") is None, "sig=%r" % p.get("signature"))
    ok("S3 签名档关：profile 子对象也不带签名",
       (pr.get("profile") or {}).get("signature") is None,
       "sig=%r" % (pr.get("profile") or {}).get("signature"))
    ok("S5 profile 头像档关：与帖子同口径（同为 null，不得一处执法一处不执法）",
       (pr.get("profile") or {}).get("avatar_url") is None,
       "avatar=%r" % (pr.get("profile") or {}).get("avatar_url"))
    ok("S6 签名档关：顶层 signature 也不带（红线：extra 语句二次取值绕过开关）",
       pr.get("signature") is None, "top_sig=%r" % pr.get("signature"))

    # 反证：开回来，两处都要有
    set_pref(True, True)
    p2 = first_post(ROOT, tid)
    pr2 = profile_of(ROOT)
    ok("S2 头像档开：帖子楼层照常带头像（反证 CASE 没写反）",
       p2.get("avatar_url") == AV, "avatar=%r" % p2.get("avatar_url"))
    ok("S4 签名档开：帖子楼层带签名", p2.get("signature") == SG,
       "sig=%r" % p2.get("signature"))
    ok("S4 签名档开：profile 带签名",
       (pr2.get("profile") or {}).get("signature") == SG,
       "sig=%r" % (pr2.get("profile") or {}).get("signature"))
    ok("S5 profile 头像档开：与帖子同口径",
       (pr2.get("profile") or {}).get("avatar_url") == AV,
       "avatar=%r" % (pr2.get("profile") or {}).get("avatar_url"))
    ok("S6 顶层与子对象同值（顶层不得绕过开关）",
       pr2.get("signature") == (pr2.get("profile") or {}).get("signature"),
       "top=%r sub=%r" % (pr2.get("signature"),
                          (pr2.get("profile") or {}).get("signature")))

    # S7：只切一档，另一档不得被误伤（两个开关必须各自独立）
    set_pref(False, True)
    p3 = first_post(ROOT, tid)
    ok("S7 两档互不干扰：关头像档时签名仍应可见",
       p3.get("avatar_url") is None and p3.get("signature") == SG,
       "avatar=%r sig=%r" % (p3.get("avatar_url"), p3.get("signature")))
    set_pref(True, False)
    p4 = first_post(ROOT, tid)
    ok("S7 两档互不干扰：关签名档时头像仍应可见",
       p4.get("avatar_url") == AV and p4.get("signature") is None,
       "avatar=%r sig=%r" % (p4.get("avatar_url"), p4.get("signature")))


def cleanup():
    if TID:
        n = psql("DELETE FROM posts WHERE body LIKE '%s%%'" % MARK)
        print("   [cleanup] 探针楼已删 %s 条" % n)
    # 原值里可能带单引号（用户自己写的签名档），一律走 dollar-quoting，
    # 否则 '%s' 直接把 SQL 打断——实测清理因此没生效，头像与签名留在库里。
    # 两个坑，都是实测踩出来的：
    #  ① 用户自己写的签名档可能含单引号 → 走 dollar-quoting，别用 '%s'。
    #  ② psql -tAc 的布尔列回读是 `t`/`f`，但**写**的时候 `t` 会被当成列名
    #     （报 `column "t" does not exist`），必须写 true/false。第一版直接把
    #     回读到的 `t` 填回 UPDATE，于是清理静默失败，头像与签名留在库里。
    av, sg, va, vs = (ORIG.split("|") + ["", "", "true", "true"])[:4]
    # 跑前快照里若**已经是**探针值，说明上一轮清理没生效（或进程被打断）：
    # 「恢复成跑前值」只会把污染原样留下——探针头像指向永不可解析的
    # example.invalid，站长头像就一直是坏图，登录态每个页面都在报
    # net::ERR_NAME_NOT_RESOLVED。认探针值 = 脏，这里直接清空，
    # 让脚本跑一次就把历史污染洗掉（自愈），而不是一轮轮继承下去。
    healed = av == AV or sg == SG
    if av == AV:
        av = ""
    if sg == SG:
        sg = ""
    av_lit = "NULL" if av == "" else "$q$%s$q$" % av
    sg_lit = "NULL" if sg == "" else "$q$%s$q$" % sg
    psql("UPDATE users SET avatar_url = %s, signature = %s, "
         "view_avatars = %s, view_signatures = %s WHERE id = %d"
         % (av_lit, sg_lit,
            "true" if va in ("t", "true") else "false",
            "true" if vs in ("t", "true") else "false", UID))
    # 比对口径要与跑前一致：跑前用 COALESCE(...,'')取空串，
    # 这里也用同一口径，否则 NULL 与空串会被判成不一致（第一版就
    # 因此报了一条假红）。
    now = psql("SELECT COALESCE(avatar_url,''), COALESCE(signature,''), "
               "view_avatars, view_signatures FROM users WHERE id = %d" % UID)
    # 口径同上：洗过脏就按洗后的期望值比，否则会把「自愈」判成假红
    expect = "%s|%s|%s|%s" % (av, sg, va, vs)
    ok("S8 零残留：头像/签名/两档开关回到跑前值", now == expect,
       "现=%s 期望=%s%s" % (now, expect,
          "（跑前快照已是探针值 → 已洗空）" if healed else ""))


try:
    summary()
finally:
    cleanup()
