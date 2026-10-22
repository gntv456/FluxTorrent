# -*- coding: utf-8 -*-
"""闸门 BE：后台用户管理全族（批四十一，20 端点此前零覆盖）。

站长与版主每天在「用户管理」里改状态、改级别、调流量积分、发道具勋章、改名、
清 2FA、删号。`grep -l "admin/users" scripts/pt_audit_*.py` 此前只命中 legacy
与批三十九的字段面，整族没有一条常驻判据。判据一律「真发请求 → 真读库」，
SQL 字面量全 ASCII，且遵守批四十立下的三条：

  1. **前置条件写进判据本身**：探针号没准备好时不许出现「没有探针所以全 403」
     这种空转绿；数值断言不许 `x or 默认值`（0.0 在 Python 里是 falsy）。
  2. **清理写在被断言那一段里**，不靠 atexit。
  3. **红要能报出服务端原话**，不许被 tail 截掉。

  S1 权限阶梯   普通成员 20 个口全 403；`ensure_outranks` 的「严格高于」——
                把探针号提到与 root 同级后，伤害性操作必须被拒
  S2 数值调整    +1GiB 与流水同写、火花按实际差额入账、余额不足时 clamp 后
                **流水记实际数**；**溢出**：uploaded_delta 给 i64 极值必须拒
                （旧实现 200 落库，`uploaded` 变成 9.2E18 且流水照记）；
                **幂等键**：流量/火花/只发邀请三条路都要能查重（旧实现只发
                邀请时不写任何账 ⇒ 同键连发两次各 5 张码 = 库里 10 张）；
                note 长度、邀请 ±50 上限、回收负数按实际行数回报
  S3 状态与开关  禁言必须落到论坛侧真读的 `forumpost`；封禁联动作废未用邀请码；
                恢复正常归还发言位；`flags` 的 download_locked_by 归属不被
                ratio_watch 巡检误解；reason 长度上限
  S4 级别与详情  class 改档生效；详情接口的 passkey 只回掩码（库里全值不外泄）
  S5 列表口径    per_page 边界、未知 sort 回落、`%` 通配被转义、
                **筛选码与写接口同名异义**（列表 ?status=2 查的是 DB 的 1）
  S6 生命周期    改名护栏、清 2FA 只对真有 2FA 的号、删号必须先封禁、
                墓碑后 passkey/pass_hash/api_tokens 全部作废
  S7 收尾        探针号流水、邀请码、勋章、字段值清零；共享设定不动

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_be_admin_users.py
"""
import json, os, re, subprocess, sys, time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
CLEAN = []
GIB = 1073741824
I64MAX = 9223372036854775807


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], capture_output=True, text=True,
        encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:56], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


def cnt(t, w=None):
    return psql("SELECT count(*) FROM %s%s"
                % (t, "" if w is None else " WHERE " + w))


def sval(k):
    return psql("SELECT value FROM site_settings WHERE name = '%s'" % k)


def blob(r):
    return json.dumps(r, ensure_ascii=False) if r is not None else ""


def data(r):
    if isinstance(r, tuple):  # 也接受 (status, body)
        r = r[1]
    return (r or {}).get("data") or {}


def post(p, b=None, tok=None):
    return call("POST", p, b, token=tok or ROOT)


def put(p, b, tok=None):
    return call("PUT", p, b, token=tok or ROOT)


def get(p, tok=None):
    return call("GET", p, token=tok or ROOT)


# ---- 探针号：注册一个真实成员，全程只动它 ----
clear_register_rl()
codes = data(post("/admin/invites", {"user_id": 1, "count": 1})).get("codes") \
    or []
PU, PTOK, WHY = 0, None, ""
if not codes:
    WHY = "发码失败"
else:
    CLEAN.append(
        "UPDATE invites SET status = 2 WHERE used_by IS NULL AND id = %d"
                 % codes[0]["id"])
    cap = data(get("/auth/captcha"))
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    name = "be%d" % (TAG * 1000 + int(time.time()) % 1000)
    if len(nums) < 2:
        WHY = "验证码题面解析不出 %r" % cap
    else:
        s, r = post("/auth/register", {
            "username": name, "email": "%s@be.local" % name,
            "password": "password123", "invite_code": codes[0]["code"],
            "captcha_id": cap.get("captcha_id"),
            "captcha_answer": int(nums[0]) + int(nums[1]),
            "captcha_token": "", "fields": {}})
        PTOK = data(r).get("token")
        if not PTOK:
            PTOK = data(post("/auth/login", {"username": name,
                                             "password": "password123"})
                         ).get("token")
        if PTOK:
            PU = int(psql("SELECT id FROM users WHERE username = '%s'" % name))
            CLEAN.append("UPDATE users SET status = 2 WHERE id = %d" % PU)
        else:
            WHY = "注册/登录 http=%s %s" % (s, blob(r)[:120])
ok("S0 探针成员已就绪（uid=%s）%s" % (PU, WHY), PU > 0 and PTOK is not None,
   WHY or "uid=%s" % PU)
def a4w():
    """四类写口的留痕数。断言一律用增量：audit_log 是累计表，
    「总量 >= 4」开闸一轮之后恒真＝哑断言（P-301 的第二段）。"""
    acts = ("user.adjust", "user.delete", "user.set_status",
            "user.flags")
    return dict((a, int(cnt("audit_log", "action = '" + a + "'")
                            or -1)) for a in acts)


BASE = {"users": cnt("users"), "invites": cnt("invites"),
        "root_class": psql("SELECT class_id FROM users WHERE id = 1"),
        "class": psql("SELECT class_id FROM users WHERE id = %d" % PU),
        "announce": sval("announce_interval"),
        "audit4": a4w()}
print("基线：%s" % BASE)
CLEAN += ["DELETE FROM traffic_ledger WHERE user_id = %d" % PU,
          "DELETE FROM spark_ledger WHERE user_id = %d" % PU,
          "DELETE FROM invites WHERE inviter_id = %d" % PU,
          "DELETE FROM user_medals WHERE user_id = %d" % PU,
          "DELETE FROM user_field_values WHERE user_id = %d" % PU,
          "UPDATE users SET uploaded = 0, downloaded = 0, spark_balance = 0 "
          "WHERE id = %d" % PU]


def wipe():
    for stmt in CLEAN:
        psql(stmt)


def adjust(body):
    return post("/admin/users/adjust", dict(body, user_id=PU))


# ============ S1 权限阶梯 ============
PATHS = [
    ("GET", "/admin/users?page=1&per_page=20", None),
    ("GET", "/admin/users/%d" % PU, None),
    ("GET", "/admin/users/%d/snatches" % PU, None),
    ("GET", "/admin/users/%d/fields" % PU, None),
    ("PUT", "/admin/users/%d/fields" % PU, {"values": {}}),
    ("POST", "/admin/users/adjust", {"user_id": PU, "spark_delta": 1}),
    ("POST", "/admin/users/status", {"user_id": PU, "status": 1}),
    ("POST", "/admin/users/class", {"user_id": PU, "class_id": 2}),
    ("PUT", "/admin/users/flags", {"user_id": PU, "suspended": True}),
    ("POST", "/admin/users/batch",
     {"action": "class", "ids": [PU], "value": 2}),
    ("POST", "/admin/users/passkey/reveal", {"user_id": PU}),
    ("POST", "/admin/users/passkey/reset", {"user_id": PU}),
    ("POST", "/admin/users/%d/rename" % PU, {"new_name": "bex"}),
    ("POST", "/admin/users/%d/jixiao" % PU, {"type_id": 1}),
    ("POST", "/admin/users/%d/grant-item/1" % PU, {}),
    ("POST", "/admin/users/%d/medal/1" % PU, {}),
    ("POST", "/admin/users/%d/revoke-item/1" % PU, {}),
    ("POST", "/admin/users/%d/medal/1/redate" % PU, {"days": 1}),
    ("POST", "/admin/users/%d/2fa/clear" % PU, None),
    ("DELETE", "/admin/users/%d" % PU, None),
]
leak = []
if PTOK:
    for m, p, b in PATHS:
        st = call(m, p, b, token=PTOK)[0]
        if st != 403:
            leak.append("%s %s=%s" % (m, p, st))
ok("S1a 普通成员 20 个后台用户端点全部 403（%d 个口）" % len(PATHS),
   PTOK is not None and not leak, blob(leak)[:200])

# 等级阶梯：把探针号提到与 root 同级，伤害性操作必须被拒
s, r = post("/admin/users/class", {"user_id": PU, "class_id": 99})
ok("S1b 结构性禁止：不能把别人提成第二个站长（403，且不落库）",
   s == 403 and psql("SELECT class_id FROM users WHERE id = %d" % PU)
   == BASE["class"], "%s now=%s" % (s, psql("SELECT class_id FROM users "
                                            "WHERE id = %d" % PU)))
s, r = post("/admin/users/class", {"user_id": PU, "class_id": 98})
ok("S1c 封顶=操作者-1：提到 98 可以（正控，证明 S1b 不是一刀切）",
   s == 200 and psql("SELECT class_id FROM users WHERE id = %d" % PU) == "98",
   "%s now=%s %s" % (s, psql("SELECT class_id FROM users WHERE id = %d" % PU),
                     blob(r)[:80]))
bad = [x[0] for x in (("adjust", call("POST", "/admin/users/adjust",
                                      {"user_id": PU, "spark_delta": 1},
                                      token=PTOK)),
                      ("status", post("/admin/users/status",
                                      {"user_id": PU, "status": 1}, PTOK)),
                      ("flags", put("/admin/users/flags",
                                    {"user_id": PU, "suspended": True},
                                    PTOK)))
       if x[1][0] != 403]
ok("S1d 同级/对自己下手一律拒：98 档探针号自己动自己 adjust/status/flags",
   PTOK is not None and not bad, "%s %s" % (blob(bad),
                                           psql("SELECT class_id FROM users "
                                                "WHERE id = %d" % PU)))
s, r = post("/admin/users/class", {"user_id": PU,
                                   "class_id": int(BASE["class"] or 0)})
ok("S1e 级别已还原（class=%s）" % psql("SELECT class_id FROM users "
                                      "WHERE id = %d" % PU),
   s == 200 and psql("SELECT class_id FROM users WHERE id = %d" % PU)
   == BASE["class"])

# ============ S2 数值调整 ============
s, r = adjust({"uploaded_delta": GIB, "note": "be-giB"})
ok("S2a +1GiB 与流水同写（快照与 SUM(ledger) 必须一致）",
   s == 200 and psql("SELECT uploaded FROM users WHERE id = %d" % PU)
   == str(GIB) and cnt("traffic_ledger", "user_id = %d AND delta_up = %d"
                       % (PU, GIB)) == "1",
   "%s up=%s %s" % (s, psql("SELECT uploaded FROM users WHERE id = %d" % PU),
                    blob(r)[:90]))
psql("UPDATE users SET spark_balance = 0 WHERE id = %d" % PU)
s, r = adjust({"spark_delta": 100})
ok("S2b 火花入账：balance 与 spark_ledger 都记 100",
   s == 200 and psql("SELECT spark_balance FROM users WHERE id = %d" % PU)
   == "100" and psql("SELECT amount FROM spark_ledger WHERE user_id = %d "
                     "ORDER BY id DESC LIMIT 1" % PU) == "100",
   "%s bal=%s %s" % (s, psql("SELECT spark_balance FROM users WHERE id = %d"
                              % PU), blob(r)[:90]))
s, r = adjust({"spark_delta": -999})
bal = psql("SELECT spark_balance FROM users WHERE id = %d" % PU)
last = psql("SELECT amount FROM spark_ledger WHERE user_id = %d "
            "ORDER BY id DESC LIMIT 1" % PU)
ok("S2c 余额不足时按**实际差额**入账（clamp 到 0，流水记 -100 不是 -999）",
   s == 200 and bal == "0" and last == "-100", "bal=%s ledger=%s" % (bal,
                                                                     last))
psql("DELETE FROM traffic_ledger WHERE user_id = %d" % PU)
psql("DELETE FROM spark_ledger WHERE user_id = %d" % PU)
psql("UPDATE users SET uploaded = 0, spark_balance = 0 WHERE id = %d" % PU)
s, r = adjust({"uploaded_delta": I64MAX})
up1 = psql("SELECT uploaded FROM users WHERE id = %d" % PU)
ok("S2d 溢出必须被拒：uploaded_delta=i64::MAX → 400 且库里没脏值",
   s == 400 and "上限" in blob(r) and up1 == "0",
   "%s up=%s %s" % (s, up1, blob(r)[:150]))
ok("S2e 被拒的溢出请求没留下流水",
   cnt("traffic_ledger", "user_id = %d" % PU) == "0",
   cnt("traffic_ledger", "user_id = %d" % PU))
# 红线版会把 i64::MAX 真写进去：这里立刻复位，避免下游断言变成级联假红
psql("UPDATE users SET uploaded = 0 WHERE id = %d" % PU)
KEY = "bekey%d" % (TAG * 7)
ok("S2f 短幂等键先被拒（8~120）", adjust({"spark_delta": 1,
                                        "idempotency_key": "short"})[0]
   == 400)
s, r = adjust({"uploaded_delta": GIB, "idempotency_key": KEY})
s2, r2 = adjust({"uploaded_delta": GIB, "idempotency_key": KEY})
ok("S2g 流量调整同键二次被拒（库里只多 1GiB）",
   s == 200 and s2 == 400 and "已使用" in blob(r2)
   and psql("SELECT uploaded FROM users WHERE id = %d" % PU) == str(GIB),
   "%s %s up=%s" % (s, s2, psql("SELECT uploaded FROM users WHERE id = %d"
                                 % PU)))
psql("DELETE FROM traffic_ledger WHERE user_id = %d" % PU)
psql("UPDATE users SET uploaded = 0 WHERE id = %d" % PU)
k2 = "bekeyinv%d" % TAG
s, r = adjust({"invite_grant": 5, "idempotency_key": k2})
g1 = (data(r) or {}).get("invite_granted")
s2, r2 = adjust({"invite_grant": 5, "idempotency_key": k2})
live = cnt("invites", "inviter_id = %d AND status = 0" % PU)
ok("S2h 只发邀请也吃幂等键：同键第二次必须被拒，库里恰好 5 张码（不是 10）",
   s == 200 and g1 == 5 and s2 == 400 and "idempotency_key" in blob(r2)
   and live == "5",
   "第一次=%s 第二次=%s 库中=%s %s" % (g1, blob(r2)[:60], live, blob(r)[:60]))
ok("S2i 邀请单次 ±50：要 100 张当场被拒（不静默截断）",
   adjust({"invite_grant": 100})[0] == 400
   and "50" in blob(adjust({"invite_grant": 100})[1]))
s, r = adjust({"invite_grant": -3})
ok("S2j 回收按实际行数回报（要 3 张，库里真少 3 张）",
   s == 200 and (data(r) or {}).get("invite_revoked") == 3
   and cnt("invites", "inviter_id = %d AND status = 0" % PU) == "2",
   "%s %s live=%s" % (s, blob(r)[:90],
                      cnt("invites", "inviter_id = %d AND status = 0" % PU)))
psql("DELETE FROM invites WHERE inviter_id = %d" % PU)
psql("DELETE FROM user_modify_logs WHERE uid = %d" % PU)
s, r = adjust({"uploaded_delta": 1024, "note": "N" * 2000})
rl = psql("SELECT length(reason) FROM traffic_ledger WHERE user_id = %d "
          "ORDER BY id DESC LIMIT 1" % PU)
ok("S2k note 有长度上限（2000 字要么拒要么截，不许原样进流水）",
   (s == 400 and "note" in blob(r)) or (s == 200 and rl.isdigit()
                                       and int(rl) <= 300),
   "%s reason_len=%s %s" % (s, rl, blob(r)[:110]))
psql("DELETE FROM traffic_ledger WHERE user_id = %d" % PU)
psql("DELETE FROM spark_ledger WHERE user_id = %d" % PU)
psql("UPDATE users SET spark_balance = 0 WHERE id = %d" % PU)
ok("S2l 什么都不给调时直接拒（不许空转成功）",
   adjust({})[0] == 400 and adjust({"note": "x"})[0] == 400)

# ============ S3 状态与开关 ============
s, r = post("/admin/users/status", {"user_id": PU, "status": 3})
ok("S3a status 只接 0/1/2：3 当场拒", s == 400 and "0/1/2" in blob(r),
   "%s %s" % (s, blob(r)[:90]))
s, r = post("/admin/users/status", {"user_id": PU, "status": 1,
                                    "reason": "刷屏"})
fp = psql("SELECT forumpost FROM users WHERE id = %d" % PU)
st = psql("SELECT status FROM users WHERE id = %d" % PU)
ok("S3b 禁言落到论坛侧真读的那面旗子（forumpost=false 且 status=1）",
   s == 200 and fp == "f" and st == "1", "fp=%s st=%s" % (fp, st))
s, r = post("/admin/users/%d/rename" % PU, {"new_name": "bez%d" % TAG})
ok("S3c 禁言账号仍可改名（护栏只看等级，不被状态误伤）",
   s == 200 and psql("SELECT username FROM users WHERE id = %d" % PU)
   == "bez%d" % TAG, "%s now=%s %s" % (s, psql("SELECT username FROM users "
                                               "WHERE id = %d" % PU),
                                       blob(r)[:80]))
s, r = put("/admin/users/flags", {"user_id": PU})
ok("S3d flags 至少要一个开关", s == 400, "%s %s" % (s, blob(r)[:80]))
s, r = put("/admin/users/flags", {"user_id": PU,
                                  "download_enabled": False})
lock = psql("SELECT download_locked_by FROM users WHERE id = %d" % PU)
ok("S3e 手动冻结把归属写成 staff（ratio_watch 巡检不得误解）",
   s == 200 and lock == "staff", "%s lock=%s" % (s, lock))
s, r = put("/admin/users/flags", {"user_id": PU, "download_enabled": True})
ok("S3f 解冻同时清掉 download_locked_by",
   s == 200 and psql("SELECT coalesce(download_locked_by,'-') FROM users "
                     "WHERE id = %d" % PU) == "-",
   psql("SELECT coalesce(download_locked_by,'-') FROM users WHERE id = %d"
        % PU))
s, r = post("/admin/users/status", {"user_id": PU, "status": 1,
                                    "reason": "R" * 4000})
ml = psql("SELECT max(length(content)) FROM user_modify_logs "
          "WHERE uid = %d" % PU)
ok("S3g reason 长度有上限（4000 字不许原样进留痕/PM/webhook）",
   (s == 400 and "reason" in blob(r)) or (not ml.isdigit())
   or int(ml) <= 300, "http=%s 留痕长度=%s %s" % (s, ml, blob(r)[:90]))
if s == 200:
    psql("DELETE FROM user_modify_logs WHERE uid = %d" % PU)
s, r = adjust({"invite_grant": 3})
inv_live = cnt("invites", "inviter_id = %d AND status = 0" % PU)
s2, r2 = post("/admin/users/status", {"user_id": PU, "status": 2,
                                      "reason": "违规刷屏"})
void = cnt("invites", "inviter_id = %d AND status = 2" % PU)
ok("S3h 封禁联动作废其未用邀请码（造 3 张 → 3 张作废，不是 0 张的空转）",
   inv_live == "3" and s2 == 200 and void == "3",
   "造=%s 作废=%s http=%s" % (inv_live, void, s2))
s, r = post("/admin/users/status", {"user_id": PU, "status": 0})
ok("S3i 恢复正常归还发言位（forumpost=true 且 status=0）",
   s == 200 and psql("SELECT forumpost FROM users WHERE id = %d" % PU) == "t"
   and psql("SELECT status FROM users WHERE id = %d" % PU) == "0",
   "%s fp=%s" % (s, psql("SELECT forumpost FROM users WHERE id = %d" % PU)))
pm = cnt("messages", "receiver_id = %d" % PU)
ok("S3j 状态变更当事人看得见（PM ≥3 条：禁言/超长理由禁言/封禁/恢复）",
   pm.isdigit() and int(pm) >= 3, "pm=%s" % pm)
psql("DELETE FROM messages WHERE receiver_id = %d" % PU)
psql("DELETE FROM user_modify_logs WHERE uid = %d" % PU)
psql("DELETE FROM invites WHERE inviter_id = %d" % PU)

# ============ S4 详情与脱敏 ============
s, r = get("/admin/users/%d" % PU)
d = data(r)
pk = psql("SELECT passkey FROM users WHERE id = %d" % PU)
shown = blob(d)
ok("S4a 详情 200 且回关键字段（spark/invites/warned）", s == 200
   and "spark_balance" in shown and "invites_unused" in shown,
   "%s %s" % (s, shown[:100]))
ok("S4b passkey 只回掩码：库里全值 32 位绝不出现在响应里",
   len(pk) == 32 and pk not in shown and "****" in shown,
   "len=%s in_body=%s" % (len(pk), pk in shown))
s, r = get("/admin/users/%d/snatches" % PU)
ok("S4c 无做种记录时返回空列表而不是 500（可空列读非 Option 的老病族）",
   s == 200, "%s %s" % (s, blob(r)[:90]))
s, r = get("/admin/users/99999999")
ok("S4d 不存在的账号 404/400 且不崩", s in (400, 404), "%s" % s)

# ============ S5 列表口径 ============
s, r = get("/admin/users?page=1&per_page=0")
ok("S5a per_page 边界（0 与 101 都拒）", s == 400
   and get("/admin/users?page=1&per_page=101")[0] == 400, "%s" % s)
s, r = get("/admin/users?page=1&per_page=5&sort=nosebleed")
ok("S5b 未知 sort 回落 id（不拼进 SQL）", s == 200, "%s %s" % (s,
                                                              blob(r)[:80]))
s, r = get("/admin/users?q=%25&page=1&per_page=5")
tot = (data(r) or {}).get("total")
ok("S5c 搜索词里的 %% 被转义（不许把通配当「全表」）",
   s == 200 and isinstance(tot, int) and tot < int(BASE["users"]),
   "total=%s users=%s" % (tot, BASE["users"]))
s, r = get("/admin/users?status=2&page=1&per_page=20")
rows = data(r).get("rows") or []
stset = set(str(x.get("status")) for x in rows)
ok("S5d 列表 ?status=2 查的是 DB 的 1（禁言）——与写接口 2=封禁同名异义",
   s == 200 and (not rows or stset == {"1"}), "statuses=%s" % sorted(stset))
s, r = get("/admin/users?id=%d&page=1&per_page=20" % PU)
rows = data(r).get("rows") or []
ok("S5e 按 id 精确筛选只回探针号（占位符没串位）",
   s == 200 and len(rows) == 1 and str(rows[0]["id"]) == str(PU),
   "%s rows=%s" % (s, blob(rows)[:90]))
s, r = get("/admin/users?class_id=%d&page=1&per_page=5"
           % int(BASE["class"] or 0))
ok("S5f 按等级筛选：total 必须等于该档在库人数（命中数与基数一起写）",
   s == 200 and data(r).get("total") == int(
       cnt("users", "class_id = %s" % BASE["class"])),
   "api_total=%s db=%s" % (data(r).get("total"),
                           cnt("users", "class_id = %s" % BASE["class"])))

# ============ S6 生命周期 ============
s, r = post("/admin/users/%d/rename" % PU, {"new_name": ""})
ok("S6a 改名不许空名/超长", s == 400 and post(
    "/admin/users/%d/rename" % PU, {"new_name": "x" * 40})[0] == 400, "%s" % s)
s, r = post("/admin/users/%d/2fa/clear" % PU)
ok("S6b 没开 2FA 的号清 2FA：400 且点名「未开启」（不许假装清了）",
   s == 400 and "未开启" in blob(r), "%s %s" % (s, blob(r)[:110]))
s, r = call("DELETE", "/admin/users/%d" % PU, None, token=ROOT)
# 话术同尺（站方 7b773433）：拒单句改成「仅已处置的账号（停用/墓碑/封禁）可删除」。
# 本条原来断言旧句子「先封禁」，产品改口径后它就**永远不可能红得对**——
# 现在按现口径断言，并加一条承重：活跃号确实没被删掉（状态仍 <2、行还在）。
ok("S6c 删号必须先处置（活跃账号防误删，且账号确实还在）",
   s == 400 and "仅已处置的账号" in blob(r)
   and cnt("users", "id = %d AND status < 2" % PU) == "1",
   "%s %s" % (s, blob(r)[:110]))
# 造证据：删号**之前**探针号名下有两张未使用邀请码
pre_inv = adjust({"invite_grant": 2})
ok("S6c2 前置条件：探针号名下确有 2 张未用码（否则 S6g 是空判）",
   pre_inv[0] == 200 and cnt("invites", "inviter_id = %d AND status = 0"
                             % PU) == "2", blob(pre_inv[1])[:90])
post("/admin/users/status", {"user_id": PU, "status": 2})
s, r = call("DELETE", "/admin/users/%d" % PU, None, token=ROOT)
u = psql("SELECT username || '|' || pass_hash || '|' || coalesce(passkey,'-') "
         "FROM users WHERE id = %d" % PU)
ok("S6d 已封禁账号可删（200）", s == 200, "%s %s" % (s, blob(r)[:140]))
ok("S6e 墓碑三件套：改名 deleted-*、pass_hash 置不可登录、passkey 作废",
   u.split("|")[0].startswith("deleted-") and u.split("|")[1] == "!"
   and u.split("|")[2].startswith("deleted-"), u[:110])
ok("S6f 删号后 api_tokens 清零",
   cnt("api_tokens", "user_id = %d" % PU) == "0")
# 旧实现 `DELETE FROM invites WHERE inviter_id = $1 OR used_by = $1` 会连
# 「谁请进来的 / 他请了谁」一起抹掉；账号只是墓碑化，FK 并不要求删行
ok("S6g 删号保留邀请留痕：两张码行仍在且被作废（不是被 DELETE 掉）",
   cnt("invites", "inviter_id = %d" % PU) == "2"
   and cnt("invites", "inviter_id = %d AND status = 2" % PU) == "2",
   "行=%s 作废=%s" % (cnt("invites", "inviter_id = %d" % PU),
                      cnt("invites", "inviter_id = %d AND status = 2" % PU)))
u2 = psql("SELECT username FROM users WHERE id = %d" % PU)
s, r = call("DELETE", "/admin/users/%d" % PU, None, token=ROOT)
u3 = psql("SELECT username FROM users WHERE id = %d" % PU)
ok("S6h 重复删除幂等：不报 500，且**不再二次改名**（旧实现放行 status>=2，"
   "同一账号留下两个墓碑名）", s in (200, 400, 404) and u2 == u3 and bool(u3),
   "%s 第一次=%s 第二次=%s" % (s, u2, u3))
s, r = call("DELETE", "/admin/users/1", None, token=ROOT)
ok("S6i root 不能删自己", s == 400 and "自己" in blob(r), "%s" % s)

# ============ S7 收尾 ============
wipe()
CLEAN.append("DELETE FROM invites WHERE inviter_id = %d" % PU)
wipe()
NOW = {"users": cnt("users"), "invites": cnt("invites")}
ok("S7a 邀请码与流水回到基线，探针号已墓碑",
   NOW["invites"] == BASE["invites"]
   and psql("SELECT status FROM users WHERE id = %d" % PU) == "2"
   and cnt("traffic_ledger", "user_id = %d" % PU) == "0"
   and cnt("spark_ledger", "user_id = %d" % PU) == "0", str(NOW))
ok("S7b 共享设定一个字没动（announce_interval=%s）" % sval("announce_interval"),
   sval("announce_interval") == BASE["announce"])
_s7c = int(cnt("audit_log", "action IN ('user.adjust','user.delete','user."
                "set_status','user.flags')") or -1)
_n4 = a4w()
_g = dict((k, _n4[k] - BASE["audit4"][k]) for k in _n4)
ok("S7c 留痕只增不删（增量口径）：四类写口各自都要留痕，增量 "
   "%s（开闸时 %s）" % (_g, BASE["audit4"]),
   all(v >= 1 for v in _g.values())
   and sum(_g.values()) >= 4 and -1 not in _n4.values(),
   "增量=%s 现值=%s" % (_g, _n4))
ok("S7c2 累计量不许被删：四类总量 %s 不得低于开闸时的 %s"
   % (sum(_n4.values()), sum(BASE["audit4"].values())),
   sum(_n4.values()) >= sum(BASE["audit4"].values()),
   "现值=%s" % sum(_n4.values()))

# ---- S8 后台用户详情的「发布数」不含 3 档软删种（P-544）----
_s8 = int(psql("SELECT owner_id FROM torrents WHERE approval_status = 3 "
               "GROUP BY owner_id ORDER BY count(*) DESC LIMIT 1"))
_s8all = int(cnt("torrents", "owner_id = %d" % _s8))
_s8vis = int(cnt("torrents", "owner_id = %d AND "
                 "approval_status = 1" % _s8))
_s8del = int(cnt("torrents", "owner_id = %d AND "
                 "approval_status = 3" % _s8))
ok("S8 前置：该用户名下确有「谁都不可读」的软删种（%d 行里 %d 行是 3 档）"
   % (_s8all, _s8del), _s8del > 0 and _s8all > _s8vis, "uid=%d" % _s8)
_d8 = data(get("/admin/users/%d" % _s8))
ok("S8 用户详情 uploads == 过审行数（P-549 与 /me 同尺，待审/驳回/软删都不算）",
   _d8.get("uploads") == _s8vis,
   "api=%s 应为=%s 全量=%s" % (_d8.get("uploads"), _s8vis, _s8all))

summary()
