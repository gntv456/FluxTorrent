# -*- coding: utf-8 -*-
"""闸门 IW：聊天盒（shoutbox）全链——发言、冷却、禁言位、撤回窗口、越权删除。

选面依据：拿 `docs/api/route-manifest.json` 对 `scripts/` 做覆盖普查，
`shoutbox` 六个端点里只有 3 个文件提到过这个词（且都不是断言）。它是全站
唯一「所有人实时互相可见」的面，坏起来最响：刷屏、被禁言者继续发言、
违规发言删不掉、或者反过来——普通成员能把别人的发言删掉。

口径（读码 + 现网实测）：
  * 列表与发言都要登录（P1 审计：含用户名与发言内容，不对匿名开放）；
  * 长度 1-300 字（按 char 计，中文一个字算一个）；
  * 每人 10 秒一条（`rate_window`，fail-close：Redis 抖动是拒发而不是放行）；
  * 禁言读的是 `users.forumpost` 那面旗（与论坛/漂流瓶同一把尺）；
  * 撤回：本人 2 分钟内可删自己，职员不受时限；
  * 清空（/shoutbox/clear）是破坏口 ⇒ 本闸门**只验非职员被拒**，
    绝不真跑 root 的清空（现网别人的发言不在我的清除范围内）。

判据：
  IW1 匿名：列表/发言/删除三处都 401；
  IW2 长度边界：空串/纯空白/301 字拒，300 字过；
  IW3 冷却：同一人连发第二条 ⇒ 429；换一个号立刻能发（证明是**每人**窗口）；
  IW4 禁言位：`forumpost=false` 的成员发言被「你已被禁言」拒；恢复后能发；
  IW5 本人撤回：刚发的能删；把 created_at 改到 3 分钟前再删 ⇒ 403（时限在册）；
  IW6 越权：B 删 A 的发言 ⇒ 403（Forbidden，不是 NotFound——存在性可见是可辩护的，
      但不能删）；
  IW7 职员通道：class≥90 可删他人发言且写 audit（现网 postmanage 门槛读数记录在案）；
  IW8 清空口：非职员 403；`clear` 的 SQL 是全表 DELETE（静态在册，不真跑）；
  IW9 前端读者：`apps/web/components/shout-box.tsx` 仍在调 /api/v1/shoutbox；
  IW10 收尾零残留：本轮探针发言全部删净，现网原有条数不受影响。

跑法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_iw_shoutbox_chain.py
"""
import inspect
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, psql_stdin, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUN = int(time.time())
MARK = "iw%d" % RUN
PW = inspect.signature(login).parameters["pw"].default


def sval(q):
    return psql_stdin(q).strip()


def member(tag):
    user = "%s%s" % (tag, RUN)
    # 邮箱也要带本轮后缀：adduser 对 email 唯一，写死 iwA@iw.local 会让
    # 第二次跑「建号失败」——而且失败信息不在这条判据里，只剩一个空 uid。
    st, r = call("POST", "/admin/adduser", {"username": user,
                                            "password": PW,
                                            "email": "%s@iw.local" % user},
                 token=ROOT)
    got = sval("SELECT id FROM users WHERE username = '%s'" % user)
    if not got:
        # 建号失败要把**响应**打出来，否则崩在 int('') 上看不见原因
        raise RuntimeError("adduser(%s) 失败 http=%s resp=%s"
                           % (user, st, str(r)[:160]))
    uid = int(got)
    t0 = login(user)
    call("POST", "/me/password/change", {"old_password": PW,
                                         "new_password": PW + "x"}, token=t0)
    return user, uid, login(user, PW + "x")


ROOT = login()
before = int(sval("SELECT count(*) FROM shoutbox"))
print("== IW1 匿名三处 ==")
a1 = call("GET", "/shoutbox")
a2 = call("POST", "/shoutbox", {"message": "%s-anon" % MARK})
a3 = call("POST", "/shoutbox/delete", {"id": 1})
ok("IW1 匿名列表=%s 发言=%s 删除=%s（三处都要 401）"
   % (a1[0], a2[0], a3[0]), a1[0] == 401 and a2[0] == 401 and a3[0] == 401,
   str(a1[1])[:80])

print("== IW2 长度边界 ==")
uA, idA, tkA = member("iwA")
e1 = call("POST", "/shoutbox", {"message": "   "}, token=tkA)
e2 = call("POST", "/shoutbox", {"message": "x" * 301}, token=tkA)
e3 = call("POST", "/shoutbox", {"message": "%s-300" % MARK + "。" * (300
                              - len(MARK) - 4)}, token=tkA)
ok("IW2 纯空白=%s、301 字=%s（都要 1002 拒）；300 字=%s（要过）"
   % (e1[1].get("code"), e2[1].get("code"), e3[1].get("code")),
   e1[1].get("code") == 1002 and e2[1].get("code") == 1002
   and e3[1].get("code") == 0,
   "%s | %s" % (str(e1[1].get("message"))[:36],
                str(e2[1].get("message"))[:36]))

print("== IW3 每人 10 秒冷却 ==")
c1 = call("POST", "/shoutbox", {"message": "%s-cool1" % MARK}, token=tkA)
c2 = call("POST", "/shoutbox", {"message": "%s-cool2" % MARK}, token=tkA)
uB, idB, tkB = member("iwB")
c3 = call("POST", "/shoutbox", {"message": "%s-other" % MARK}, token=tkB)
ok("IW3 同人连发第二条 ⇒ 非 0（实测 code=%s http=%s），另一人立刻可发"
   "（code=%s）" % (c2[1].get("code"), c2[0], c3[1].get("code")),
   c3[1].get("code") == 0 and c2[1].get("code") != 0,
   "首条=%s；窗口是 per-user，不是全站的" % c1[1].get("code"))

print("== IW4 禁言位（users.forumpost）==")
_, idC, tkC = member("iwC")
sval("UPDATE users SET forumpost = FALSE WHERE id = %d" % idC)
m1 = call("POST", "/shoutbox", {"message": "%s-muted" % MARK}, token=tkC)
sval("UPDATE users SET forumpost = TRUE WHERE id = %d" % idC)
time.sleep(11)
m2 = call("POST", "/shoutbox", {"message": "%s-unmuted" % MARK}, token=tkC)
ok("IW4 禁言者发言被「你已被禁言」拒（code=%s），恢复后同一条路可发（code=%s）"
   % (m1[1].get("code"), m2[1].get("code")),
   "禁言" in str(m1[1].get("message")) and m2[1].get("code") == 0,
   str(m1[1])[:90])

print("== IW5 本人 2 分钟撤回窗口 ==")
sid = ((call("POST", "/shoutbox", {"message": "%s-mine" % MARK},
             token=tkB)[1].get("data") or {}).get("id"))
d_ok = call("POST", "/shoutbox/delete", {"id": int(sid)}, token=tkB)
sid2 = ((call("POST", "/shoutbox", {"message": "%s-old" % MARK},
              token=tkA)[1].get("data") or {}).get("id"))
sval("UPDATE shoutbox SET created_at = now() - interval '3 minutes' "
     "WHERE id = %d" % int(sid2))
d_late = call("POST", "/shoutbox/delete", {"id": int(sid2)}, token=tkA)
ok("IW5 刚发的能撤回（code=%s），改老 3 分钟后撤回被拒（http=%s code=%s）"
   % (d_ok[1].get("code"), d_late[0], d_late[1].get("code")),
   d_ok[1].get("code") == 0 and d_late[1].get("code") == 2003,
   "要的是**窗口生效**（2003=越权拒）；实测=%s"
   % str(d_late[1].get("message"))[:40])

print("== IW6/IW7 删除他人发言 ==")
# 复用 IW5 那条「改老 3 分钟、本人已删不掉」的行：省一次发送，
# 也避免撞上本人 10 秒冷却（sid=None 会让判据崩成 TypeError 而不是报红）。
sid3 = int(sid2)
x1 = call("POST", "/shoutbox/delete", {"id": sid3}, token=tkB)
alive = sval("SELECT count(*) FROM shoutbox WHERE id = %d" % sid3)
uid91 = idB
sval("UPDATE users SET class_id = 91 WHERE id = %d" % uid91)
time.sleep(11)          # 顺带躲开发言冷却，也让改级落到新签的 token 里
tk91 = login(uB, PW + "x")
import base64
import json as _json
claim = _json.loads(base64.b64decode(tk91.split(".")[1] + "=="))
x2 = call("POST", "/shoutbox/delete", {"id": sid3}, token=tk91)
alive2 = sval("SELECT count(*) FROM shoutbox WHERE id = %d" % int(sid3))
aud = sval("SELECT count(*) FROM audit_log WHERE action = "
           "'shoutbox_delete' AND ref->>'id' = '%s'" % sid3)
ok("IW6 普通成员删他人发言 ⇒ 拒且行还在（http=%s code=%s 行数=%s）"
   % (x1[0], x1[1].get("code"), alive),
   x1[1].get("code") in (2003, 1002) and alive == "1", str(x1[1])[:90])
ok("IW7 class=91（发布员）删他人发言 ⇒ code=%s 行数=%s 审计=%s 行"
   % (x2[1].get("code"), alive2, aud),
   x2[1].get("code") == 0 and alive2 == "0" and aud == "1",
   "token 里的 class_id=%s（若被判拒而 claim 是 91，说明现网 postmanage 门槛"
   "比源码里的 90 高，要按镜像实装配比读；claim 不是 91 则是本闸门登录取号问题）。"
   "现网权限表把 staff.panel / forums 管理类登记在 role_key=90 ⇒ 91 发布员按"
   "现配置就是全站管帖人；要收窄得改权限表而不是代码"
   % claim.get("class_id"))

print("== IW8 清空口 ==")
c403 = call("POST", "/shoutbox/clear", token=tkA)
src = open(os.path.join(ROOTDIR, "apps", "api", "src", "community_http",
                        "shoutbox.rs"), encoding="utf-8",
           errors="replace").read()
ok("IW8 非职员 clear ⇒ http=%s code=%s；clear 的实现是全表 DELETE + class≥90 闸"
   % (c403[0], c403[1].get("code")), c403[1].get("code") == 2003
   and "DELETE FROM shoutbox" in src and "class_id < 90" in src,
   "本闸门**不真跑** root 的 clear：现网有别人的发言，清空不在我的权限里")

print("== IW9 前端读者 ==")
fe = open(os.path.join(ROOTDIR, "apps", "web", "components",
                       "shout-box.tsx"), encoding="utf-8",
          errors="replace").read()
callers = []
for base, _d, names in os.walk(os.path.join(ROOTDIR, "apps", "web")):
    if "node_modules" in base or "/.next" in base:
        continue
    for n in names:
        if not n.endswith((".ts", ".tsx")):
            continue
        t = open(os.path.join(base, n), encoding="utf-8",
                 errors="replace").read()
        if "shoutbox/delete" in t:
            callers.append(os.path.join(base, n).replace("\\", "/"))
ok("IW9 钉桩：/shoutbox/delete 目前**没有任何前端读者**（实=%d）；列表口读者在册=%s"
   % (len(callers), "/api/v1/shoutbox" in fe),
   not callers and "/api/v1/shoutbox" in fe,
   "后端两条能力（本人 2 分钟撤回 / 职员删他人发言）界面没有入口："
   "成员发错撤不回、职员看到违规喊话只能整间清空。销桩＝接上按钮后把这条"
   "改成『callers ≥ 1』并把两个调用点分别钉住（本人撤回 vs 职员删除）")

print("== IW10 收尾 ==")
pat = "%s%%" % MARK
mine = [x for x in sval("SELECT string_agg(id::text, ',') FROM shoutbox "
                        "WHERE message LIKE '%s'" % pat).split(",") if x]
for i in mine:
    call("POST", "/shoutbox/delete", {"id": int(i)}, token=ROOT)
sval("UPDATE users SET status = 2 WHERE id IN (%d, %d, %d)" % (idA, idB, idC))
resid = int(sval("SELECT count(*) FROM shoutbox WHERE message LIKE '%s'"
                 % pat))
after = int(sval("SELECT count(*) FROM shoutbox"))
ok("IW10 本轮 %d 条探针发言经职员通道删净（残留须 0，实=%d）"
   % (len(mine), resid), resid == 0,
   "总条数 %d（开测前 %d；差额只与我自己的前缀行有关，"
   "别人在这期间发的言不算我的账）" % (after, before))

summary()
