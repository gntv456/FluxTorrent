# -*- coding: utf-8 -*-
"""DA 闸门（批八十三）：控制面板「隐私等级」三档必须真的决定别人在你主页上
能看到什么（P-293）。

现网（修前，容器 `api:latest`，2026-10-14 实测）：
  P-293 `users.privacy` 三档（面板原文：普通 / 低(邮箱地址公开) /
        **强(个人信息全部隐藏)**）能改、能存、能在自己面板回显，
        公开主页 `GET /users/{id}` 一处都不读 ⇒ 任何登录成员都能拿到
        选了这个会员「全部隐藏」的性别 / 简介 / 签名 / 上下行带宽 /
        BT 客户端 / 邀请人 / 自定义字段。这是「存了 + 回显了，但没有
        执行路径读它」那一族的**隐私版**，后果比列表偏好重一个量级。
  修前红证（本闸门 DA1）：目标号设 strong 后，普通成员打过去仍拿到
        「男/简介/签名/带宽/邀请人」全量。

只接 `strong` 的理由（不是偷懒）：`low` 的括注讲的是**邮箱**，而
`/users/{id}` 与 `/users/{id}/torrentlist` 都不下发邮箱（DA6 直接断言
响应里没有 email 键）⇒ 这一档在本接口没有可执行语义。按既有纪律：
语义没对上的档宁可不接、钉桩 + 报告「面板文案与实现不符」，
不硬套最近的一档做假收口。

分层：DA1 遮蔽本体（修前必红）、DA2 反向（不许永远隐藏）、DA3 本人视角、
DA4 职员豁免（`user.view`，不新增权限键）、DA5 误伤护栏、DA6 low 钉桩、
DA7 静态对账、DA8 收尾。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_da_privacy_wired.py
写库面：两个探针成员（AU 闸门同款注册配方：邀请码 + 算术验证码），
收尾按 `status=2` 墓碑、邀请码当场删；目标的个人信息列用 psql 填样张，
探针号本就是一次性账号，不动任何真人数据。
"""
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PW = "password123"
TAG = int(time.time()) % 100000
TUSER = "dat%d" % TAG          # 目标（隐私档由本闸门设）
VUSER = "dav%d" % TAG          # 观众（普通成员）
KEYS = ["gender", "info", "signature", "upload_speed", "download_speed",
        "client_agent", "inviter_id", "inviter_name", "user_fields"]
CLEAN = []
CIDS = []


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def code(f):
    p = os.path.join(REPO, "apps", "api", "src", f)
    if not os.path.exists(p):
        return ""
    txt = io_open(p).replace("\r\n", "\n")
    return chr(10).join(l.split("//")[0] for l in txt.splitlines())


def io_open(p):
    return open(p, encoding="utf-8").read()


def make_member(name):
    """AU 闸门同款配方：root 签邀请码 → 算术验证码 → 注册 → 登录取 token"""
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return None, None
    cid, cval = codes[0]["id"], codes[0]["code"]
    CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
    CIDS.append(cid)
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    s, rr = call("POST", "/auth/register", {
        "username": name, "email": "%s@da.local" % name, "password": PW,
        "invite_code": cval, "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    uid = (rr.get("data") or {}).get("user_id")
    tok = None
    for _ in range(8):
        tok = (call("POST", "/auth/login",
                    {"username": name, "password": PW})[1].get("data")
               or {}).get("token")
        if tok:
            break
        time.sleep(7)
    return uid, tok


clear_register_rl()
time.sleep(6)
print("== DA0 前置：两个探针成员 ==")
TID, TT = make_member(TUSER)
VID, VT = make_member(VUSER)
ok("DA0 探针成员可准备（目标 uid=%s / 观众 uid=%s）——登录限流下必须两个都拿到 token"
   % (TID, VID), bool(TT) and bool(VT) and TID and VID and TID != VID,
   "T=%s V=%s" % (TID, VID))
if not (TT and VT):
    ok("DA0b 探针准备失败，本闸门作废（不判产品）", False)
    summary()
    sys.exit(1)

try:
    psql("UPDATE users SET privacy='strong', gender=2, info='隐私样张-简介', "
         "signature='隐私样张-签名', upload_speed=12345, download_speed=6789 "
         "WHERE id=%s" % TID)
    body = (call("GET", "/users/%s" % TID, None, token=TT)[1].get("data")
            or {})
    ok("DA0c 样张数据确实落在目标账号上（修前观众能直接读到）",
       str(body.get("gender")) == "女" and body.get("info") == "隐私样张-简介"
       and body.get("inviter_name") == "root",
       "gender=%s info=%s 邀请人=%s" % (body.get("gender"),
                                        body.get("info"),
                                        body.get("inviter_name")))

    # P-575：造一条「已下架种 + 评论者本人的评论」夹具，验证主页评论动态
    # 是否越过种子可见性（旧口径按 user_id 直取）。
    cat = psql("SELECT id FROM categories LIMIT 1")
    htid = psql(
        "INSERT INTO torrents (info_hash,name,category_id,size,owner_id,"
        "approval_status) VALUES (md5('da-hide-%d'),'DA隐藏种',%s,1,%s,3) "
        "RETURNING id" % (TAG, cat, TID))
    ok("DA1d0 P-575 夹具落库（已下架种 id=%s）" % htid, bool(htid))
    if htid:
        psql("INSERT INTO comments (torrent_id,user_id,body) VALUES "
             "(%s,%s,'DAHIDE%d')" % (htid, TID, TAG))
        CLEAN.append("DELETE FROM comments WHERE torrent_id = %s" % htid)
        CLEAN.append("DELETE FROM torrents WHERE id = %s" % htid)

    def recent_bodies(tok):
        d = (call("GET", "/users/%s" % TID, None, token=tok)[1].get("data")
             or {})
        return [c.get("body") for c in (d.get("recent_comments") or [])]

    hmark = "DAHIDE%d" % TAG
    seen_v = recent_bodies(VT)
    ok("DA1e P-575：已下架种上的评论不得从评论者主页下发（成员看到=%s）"
       % seen_v, hmark not in seen_v)
    psql("UPDATE torrents SET approval_status = 1 WHERE id = %s" % htid)
    seen_on = recent_bodies(VT)
    ok("DA1f 对照：同一条评论在种子上架后必须出现（防止一律空表）",
       hmark in seen_on, "看到=%s" % seen_on)
    psql("UPDATE torrents SET approval_status = 3 WHERE id = %s" % htid)

    # P-576 钉桩：/me/torrentlist 的 seeding/completed 两档按 snatches.user_id
    # 直取，不接 visibility_sql ⇒ 本人「已完成」仍列着点进去 404 的种。
    # 载体 misc_handlers.rs 正被全仓排版（工作树未提交），本轮只钉桩。
    if htid:
        psql("INSERT INTO snatches (user_id,torrent_id,completed_at) VALUES "
             "(%s,%s,now()) ON CONFLICT DO NOTHING" % (VID, htid))
        CLEAN.append("DELETE FROM snatches WHERE user_id=%s AND torrent_id=%s"
                     % (VID, htid))
        nrow = psql("SELECT count(*) FROM snatches WHERE user_id=%s AND "
                    "torrent_id=%s" % (VID, htid))
        s_l, b_l = call("GET", "/me/torrentlist?kind=completed&per=100",
                        None, token=VT)
        lst = (b_l or {}).get("data") or {}
        ids = [str(r.get("torrent_id")) for r in (lst.get("rows") or [])]
        sd, _ = call("GET", "/torrents/%s" % htid, None, token=VT)
        ok("DA1g P-576 钉桩（改好删本条）：本人已完成列表仍列着已下架种 "
           "id=%s（列表命中=%s），而详情 http=%s ⇒ 点名字即 404"
           % (htid, htid in ids, sd), htid in ids and sd == 404,
           "夹具行=%s 列表 http=%s total=%s 详情=%s"
           % (nrow, s_l, lst.get("total"), sd))
        # P-577（已修）：公开主页的完成档也必须与详情同尺。
        s_p, b_p = call("GET", "/users/%s/torrentlist?limit=100" % VID,
                        None, token=VT)
        p_lst = (b_p or {}).get("data") or {}
        p_ids = [str(r.get("torrent_id"))
                 for r in (p_lst.get("completed") or [])]
        ok("DA1h P-577：主页完成档不得列出不可见种（详情 http=%s）" % sd,
           s_p == 200 and htid not in p_ids,
           "夹具行=%s completed 条数=%s 列表 http=%s"
           % (nrow, len(p_ids), s_p))

    # ===== DA1 遮蔽本体（修前必红）=====
    s1, b1 = call("GET", "/users/%s" % TID, None, token=VT)
    d1 = (b1 or {}).get("data") or {}
    leaked = [k for k in KEYS
              if not (k == "user_fields" and d1.get(k) in (None, []))
              and d1.get(k) is not None]
    ok("DA1 P-293：目标 privacy=strong 时，普通成员打 /users/{id} 必须拿不到"
       "个人信息 9 键，实测泄漏=%s（http=%s；修前泄漏 %s）"
       % (leaked, s1, KEYS[:3]), s1 == 200 and not leaked,
       "泄漏=%s" % leaked)
    ok("DA1b 遮蔽要落到值上：gender 应 null（实测 %s）、info 应 null（实测 "
       "%s）、signature 应 null（实测 %s）、inviter_name 应 null（实测 %s）"
       % (d1.get("gender"), d1.get("info"), d1.get("signature"),
          d1.get("inviter_name")),
       d1.get("gender") is None and d1.get("info") is None
       and d1.get("signature") is None and d1.get("inviter_name") is None)
    ok("DA1c 带宽两列也必须 null（实测 up=%s down=%s）"
       % (d1.get("upload_speed"), d1.get("download_speed")),
       d1.get("upload_speed") is None
       and d1.get("download_speed") is None)

    # P-573：响应里 profile 子对象还带一份 signature（现网红证：顶层 null
    # 而 data.profile.signature 仍是明文）。
    nested_sig = (d1.get("profile") or {}).get("signature")
    ok("DA1d P-573：嵌套 profile.signature 也必须 null（实测 %r）——"
       "只遮顶层时普通成员照样拿到签名档" % nested_sig,
       d1.get("signature") is None and nested_sig is None)

    # ===== DA2 反向：normal 不许被一起藏掉 =====
    psql("UPDATE users SET privacy='normal' WHERE id=%s" % TID)
    d2 = (call("GET", "/users/%s" % TID, None, token=VT)[1].get("data") or {})
    ok("DA2 护栏：privacy=normal（面板「普通」）时同一观众必须照旧看得到——"
       "gender=%s info=%s（把 strong 的实现写成「永远隐藏」在这里就红）"
       % (d2.get("gender"), d2.get("info")),
       d2.get("gender") == "女" and d2.get("info") == "隐私样张-简介")

    # ===== DA3 本人视角 =====
    d3 = (call("GET", "/users/%s" % TID, None, token=TT)[1].get("data") or {})
    psql("UPDATE users SET privacy='strong' WHERE id=%s" % TID)
    d3b = (call("GET", "/users/%s" % TID, None, token=TT)[1].get("data") or {})
    ok("DA3 自己看自己的主页不该被自己藏的开关遮掉：strong 时本人仍读到 "
       "gender=%s info=%s（普通档=%s）" % (d3b.get("gender"),
                                          d3b.get("info"), d3.get("gender")),
       d3b.get("gender") == "女" and d3b.get("info") == "隐私样张-简介")

    # ===== DA4 职员豁免走已有键 user.view =====
    d4 = (call("GET", "/users/%s" % TID, None, token=ROOT)[1].get("data") or {})
    ok("DA4 职员（有 user.view）打他人主页必须仍全量——gender=%s info=%s "
       "邀请人=%s（这一条是给「别把管理面也一并瞎掉」兜底）"
       % (d4.get("gender"), d4.get("info"), d4.get("inviter_name")),
       d4.get("gender") == "女" and d4.get("info") == "隐私样张-简介"
       and d4.get("inviter_name") == "root")

    # ===== DA5 误伤护栏：公开身份/统计列不许被藏 =====
    prof = d1.get("profile") or {}
    ok("DA5 遮蔽只动清单：观众仍拿到 username=%s、uploads=%s、"
       "hr_limit=%s、recent_uploads=%d 条、subtitle_count 键在=%s"
       % (prof.get("username"), prof.get("uploads"), d1.get("hr_limit"),
          len(d1.get("recent_uploads") or []), "subtitle_count" in d1),
       prof.get("username") == TUSER and "hr_limit" in d1
       and "subtitle_count" in d1 and "spark_balance" in d1,
       "username=%s" % prof.get("username"))

    # ===== DA6 钉桩：low 档没有可执行语义（文案说的是邮箱）=====
    psql("UPDATE users SET privacy='low' WHERE id=%s" % TID)
    d6 = (call("GET", "/users/%s" % TID, None, token=VT)[1].get("data") or {})
    has_email = [k for k in d6.keys() if "mail" in k.lower()]
    ok("DA6 钉桩：面板第 2 档写「低(邮箱地址公开)」，但本接口**从来不下发邮箱**"
       "（响应里的邮箱类键=%s）⇒ low 档没有可执行语义，本批刻意不接。"
       "谁哪天把邮箱放进来，这条就该转红提醒接上"
       % has_email, not has_email and d6.get("gender") == "女",
       "邮箱键=%s gender=%s" % (has_email, d6.get("gender")))

    # ===== DA7 静态对账 =====
    hv = code("auth_http/profile_privacy.rs")
    pr = code("auth_http/profile.rs")
    listed = []
    m = re.search(r"PERSONAL_KEYS: \[&str; \d+\] = \[(.*?)\];", hv, re.S)
    if m:
        listed = re.findall(r'"([a-z_]+)"', m.group(1))
    ok("DA7a 键清单必须与断言口径一致：helper 里 PERSONAL_KEYS=%s（9 键，"
       "少一个就是漏遮）" % listed, sorted(listed) == sorted(KEYS),
       "清单=%s" % listed)
    ok("DA7b 入口必须真的挂上遮蔽（profile.rs 里 mask_personal=%d 处、"
       "helper 里判定 hides_personal=%d 处）——只写函数不接就是假接线"
       % (pr.count("mask_personal"), hv.count("hides_personal")),
       pr.count("mask_personal") == 1 and hv.count("hides_personal") >= 2)
    ok("DA7c 判定形状：只有 strong 隐藏、本人早退、职员走 user.view"
       "（personal_hidden=%s / auth.id == target=%s / USER_VIEW=%s）"
       % ('privacy.unwrap_or("normal") == "strong"' in hv,
          "auth.id == target" in hv, "USER_VIEW" in hv),
       'privacy.unwrap_or("normal") == "strong"' in hv
       and "auth.id == target" in hv and "USER_VIEW" in hv)
    reg = code("auth_http/mod.rs").count("mod profile_privacy")
    ok("DA7d 承载文件必须在 mod 注册（没注册就编译不进来）：%d 处" % reg,
       reg == 1)
    # P-573：单测与 DA1d 之外再加一道静态钉——防止有人「优化」掉嵌套遮蔽。
    nest = hv.count('map.get_mut("profile")')
    ins = hv.count('p.insert("signature"')
    ok("DA7e P-573：apply 必须同步清嵌套 profile.signature（"
       "get_mut(profile)=%d 处、insert(signature)=%d 处）" % (nest, ins),
       nest == 1 and ins == 1)
    ph = code("auth_http/profile_helpers.rs")
    direct = pr.count("FROM comments WHERE user_id")
    vis = ph.count("visibility_sql(Some(viewer))")
    ok("DA7f P-575：评论动态必须按观看者可见性过滤（profile.rs 直取=%d、"
       "helper visibility_sql=%d）" % (direct, vis), direct == 0 and vis == 1)
finally:
    # ===== DA8 收尾 =====
    for stmt in CLEAN:
        psql(stmt)
    for uid in [x for x in (TID, VID) if x]:
        psql("UPDATE users SET status = 2 WHERE id = %s" % uid)
    left = int(psql("SELECT count(*) FROM users WHERE (username LIKE "
                   "'dat%%' OR username LIKE 'dav%%') AND status <> 2"))
    live_invite = int(psql("SELECT count(*) FROM invites WHERE id IN (%s)"
                           % (",".join(str(x) for x in CIDS) or "0")))
    ok("DA8a 探针号已墓碑（本轮两个）：残留可登录同名账号=%d（应 0）" % left,
       left == 0, "残留=%d" % left)
    ok("DA8b 本轮签的 %d 枚邀请码当场删净（按 id 回查残留=%d）"
       % (len(CIDS), live_invite), live_invite == 0,
       "残留=%d/%d" % (live_invite, len(CIDS)))

summary()
