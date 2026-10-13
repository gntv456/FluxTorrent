# -*- coding: utf-8 -*-
"""闸门 BL：站点设定的写入口（单项 PUT / 批量 PUT / 预检 / 历史）。

选题：`/admin/settings` 一族 8 条路径（9 个「方法+路径」），修前常驻闸门
只打过 **1 条**只读口 `GET /admin/settings/schema`（`pt_audit_g_logcheck.py:446`），
其余 8 个组合零覆盖。计数教训：拿 `grep "/admin/settings"` 数命中会把
`/admin/settings/schema` 那一行也算成「裸口覆盖」（子串误匹配）——必须按
「方法+完整路径」逐条比。这一族的要害是
**同一个字段的规则被两处实现**：预检端点 `POST /admin/settings/validate`
用 `settings_http::engine::validate_field`（number / 整数 / min / max /
必填 / 正则 / 长度），批量写口 `PUT /admin/settings/groups` 也用它 ——
唯独单项写口 `PUT /admin/settings`（面板不调它、`docs/api/route-manifest.json`
里它是对外契约的一条，站长脚本与第三方工具走的就是它）只判「非空 +
长度 ≤4096 + 不是 site_type」。

  P-186（P1）单项写口不接那把尺：给 number 型设定项写 "abc"、写低于
        min、写高于 max 都 200 落库；而读端普遍是 `.parse().unwrap_or(
        默认值)` ⇒ 脏值被静默吞成默认，站长看到的是「改了没生效」，
        库里却留着一个永不生效的值。（trim 归一化两处都有，不是缺口，L1g 当
        正控钉住。）
  P-187（P1）单项写口的留痕与读者对不上：`/admin/settings/history` 读
        `action='setting:update' AND ref->>'setting' = name`（批量口正是
        这么写的），单项口写的却是 `site_setting_update` 且 `ref` 为空
        ⇒ 走单项编辑的改动在历史面板里永远看不到，也读不出「谁把哪个
        键从什么改成了什么」。

顺带核实并**不立案**的一条：批量口入口只判 `settings.view`（class ≥ 90），
比单项口的 `settings.manage`（≥ 99）松 —— 但 `settings_meta.min_class`
是 489/489 全量填写的，批量口逐字段按 `write_min` 卡（`announce_interval`
= 99），所以它不比单项口松。这条由 L3d/L3e/L3f 钉住（403 是「整批没有一个
可写字段」，混批时越权字段进 400 字段级错误、整批不落库）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bl_settings_put.py
"""
import atexit
import json
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
TAG = int(time.time()) % 100000
NOW0 = ""
INV = []


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:56], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


def sval(k):
    return psql("SELECT value FROM site_settings WHERE name = '%s'" % k)


def cnt(t, w=None):
    return psql("SELECT count(*) FROM %s%s"
                % (t, "" if w is None else " WHERE " + w))


def blob(r):
    return json.dumps(r, ensure_ascii=False) if r is not None else ""


def data(r):
    if isinstance(r, tuple):
        r = r[1]
    return (r or {}).get("data") or {}


def msg(r):
    return str((r[1] or {}).get("message") or "")


def post(p, b=None, tok=None):
    return call("POST", p, b, token=tok or ROOT)


def put(p, b, tok=None):
    return call("PUT", p, b, token=tok or ROOT)


def mkprobe(prefix, cls):
    codes = data(post("/admin/invites", {"user_id": 1, "count": 1})
                 ).get("codes") or []
    cap = data(call("GET", "/auth/captcha", token=ROOT))
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(nums) < 2:
        return 0, None, "发码/验证码没准备好"
    INV.append(codes[0]["id"])
    name = "%s%d" % (prefix, TAG * 100 + int(time.time()) % 100)
    rg = post("/auth/register", {
        "username": name, "email": "%s@bl.local" % name,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    tok = data(rg).get("token") or data(
        post("/auth/login", {"username": name,
                             "password": "password123"})).get("token")
    if not tok:
        return 0, None, "注册/登录 http=%s %s" % (rg[0], blob(rg[1])[:90])
    uid = int(psql("SELECT id FROM users WHERE username = '%s'" % name))
    if cls:
        post("/admin/users/class", {"user_id": uid, "class_id": cls})
        tok = data(post("/auth/login", {"username": name,
                                        "password": "password123"})).get(
            "token") or tok
    return uid, tok, ""


clear_register_rl()
NOW0 = psql("SELECT now()::text")
MV, MTV, E1 = mkprobe("blmv", None)
S92, T92, E2 = mkprobe("bl92", 92)
ok("L0a 探针就绪（成员 %s / 职员 %s class=%s）%s"
   % (MV, S92, psql("SELECT class_id::text FROM users WHERE id = %d" % S92),
      E1 + E2), MV > 0 and S92 > 0 and T92)
# 测试键：number 型、带 min/max、不参与核心行为（快照 + 收尾复原 + 复读）
KEY = "anticheat_absurd_vouch_gb"
SEC = "tmdb_api_key"
K0 = sval(KEY)
S0 = sval(SEC)
NSET0 = cnt("site_settings")
ok("L0b 测试键 %s 现值=%s（库里 min/max=%s/%s，键总数=%s）"
   % (KEY, K0,
      psql("SELECT min::text FROM settings_meta WHERE name = '%s'" % KEY),
      psql("SELECT max::text FROM settings_meta WHERE name = '%s'" % KEY),
      NSET0),
   K0.isdigit() and int(K0) >= 1 and NSET0.isdigit() and int(NSET0) > 100,
   "现值=%s 键总数=%s" % (K0, NSET0))
ok("L0c 密文键 %s 开局为空（secret=%s），本轮只借用它验掩码"
   % (SEC, psql("SELECT secret::text FROM settings_meta WHERE name='%s'"
                % SEC)),
   S0 == "", "现值长度=%d" % len(S0))
H0 = int(cnt("audit_log", "action = 'setting:update' AND ref->>'setting' = "
             "'%s'" % KEY))


def hist_total(k):
    """历史面板 total：读不到键返回 -999（不许把合法的 0 当成读失败）。"""
    d = data(call("GET", "/admin/settings/history?name=%s" % k, token=ROOT))
    v = d.get("total")
    return -999 if v is None else int(v)


def put_one(name, value, tok=None):
    return put("/admin/settings", {"name": name, "value": value}, tok)


DONE = False


def finalize():
    """收尾：设定复原 + 探针墓碑。崩溃/atexit 也要走这里，不留脏账。"""
    if "K0" not in globals():
        return
    if sval(KEY) != K0:
        put_one(KEY, K0)
    if sval(SEC) != S0:
        psql("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
             % (S0, SEC))
    if DONE:
        return
    for u in (MV, S92):
        if u <= 0:
            continue
        if psql("SELECT status::text FROM users WHERE id = %d" % u) == "2":
            continue
        post("/admin/users/status", {"user_id": u, "status": 2})
        call("DELETE", "/admin/users/%d" % u, None, token=ROOT)
        psql("DELETE FROM spark_ledger WHERE user_id = %d" % u)


atexit.register(finalize)


def hist_row(k):
    return psql("SELECT ref FROM audit_log WHERE action = 'setting:update' "
                "AND ref->>'setting' = '%s' ORDER BY id DESC LIMIT 1" % k)


# ===== L1 预检与写入必须是同一把尺（P-186） =====
sv, rv = post("/admin/settings/validate", {"name": KEY, "value": "abc"})
ok("L1a 预检端点对 number 键写 \"abc\" 判非法（这把尺本来就对，字段级错误"
   "随 data 回传）", sv == 400 and "数字" in blob(rv),
   "http=%s body=%s" % (sv, blob(rv)[:90]))
sp, rp = put_one(KEY, "abc")
ok("L1b 单项写口对同一个值必须同样拒（不得比预检松）、库里不得留脏",
   sp == 400 and sval(KEY) == K0 and hist_total(KEY) == H0,
   "http=%s msg=%s 库里=%s total=%s/%s"
   % (sp, msg((sp, rp))[:40], sval(KEY), hist_total(KEY), H0))
s2, r2 = put_one(KEY, "0")
ok("L1c 低于 min 的数必须拒（且库里没被写脏）",
   s2 == 400 and sval(KEY) == K0,
   "http=%s msg=%s 现值=%s" % (s2, msg((s2, r2))[:40], sval(KEY)))
s3, r3 = put_one(KEY, "99999")
ok("L1d 高于 max 的数必须拒", s3 == 400 and sval(KEY) == K0,
   "http=%s msg=%s 现值=%s" % (s3, msg((s3, r3))[:40], sval(KEY)))
s4, r4 = put_one(KEY, "12")
ok("L1e 正例：范围内的数写得进、面板读得到（且只记一条历史）",
   s4 == 200 and sval(KEY) == "12" and hist_total(KEY) == H0 + 1,
   "http=%s 现值=%s total=%s(H0=%s)" % (s4, sval(KEY), hist_total(KEY), H0))
u_at = psql("SELECT updated_at::text FROM site_settings WHERE name = '%s'"
            % KEY)
s5, r5 = put_one(KEY, "12")
u_at2 = psql("SELECT updated_at::text FROM site_settings WHERE name = '%s'"
             % KEY)
ok("L1f 同值重复提交是幂等：不刷 updated_at、不新增历史（total 仍 %s）"
   % (H0 + 1),
   s5 == 200 and u_at == u_at2 and hist_total(KEY) == H0 + 1,
   "http=%s at=%s/%s total=%s" % (s5, u_at, u_at2, hist_total(KEY)))
put_one(KEY, " 13 ")
ok("L1g 正控：写入口的 trim 归一化本来就对（提交 \" 13 \" 落库 \"13\"，"
   "与预检 normalized 同口径）", sval(KEY) == "13", "现值=%s" % sval(KEY))

# ===== L2 留痕必须对得上历史面板（P-187） =====
H1 = hist_total(KEY)
ok("L2a 单项写口的改动要在历史里读得到（本轮改了 2 次有效值，total 从 "
   "%s 涨到 %s）" % (H0, H1), H1 == H0 + 2,
   "H0=%s H1=%s（预想 +2：12、13）" % (H0, H1))
row = hist_row(KEY)
RJ = json.loads(row) if row.startswith("{") else {}
ok("L2b 历史行里要有「哪个键、旧值→新值」（修前这里一行都读不到）",
   RJ.get("new") == "13" and RJ.get("old") == "12"
   and RJ.get("setting") == KEY,
   "ref=%s" % row[:110])
ok("L2c 写一次只记一条（不是写一次记三条）", H1 - H0 == 2,
   "delta=%s" % (H1 - H0))
GRP = psql("SELECT m.group_key FROM settings_meta m WHERE m.name = '%s'" % KEY)
s6, r6 = put("/admin/settings/groups", {"group": GRP,
                                        "values": {KEY: "14"}}, ROOT)
ok("L2d 正控：批量口写的值同样进历史（它一直是对的，本轮只是把单项口对齐）",
   s6 == 200 and sval(KEY) == "14" and hist_total(KEY) == H1 + 1,
   "http=%s 现值=%s total=%s/%s" % (s6, sval(KEY), hist_total(KEY), H1))
bad = cnt("audit_log", "action = 'setting:update' AND ref ? 'via' AND "
          "(ref->>'setting' IS NULL OR NOT (ref ? 'old') OR NOT (ref ? 'new'))")
ok("L2e 单项口的历史行一个字都不能缺（via=single 的 0 行缺 setting/old/new）",
   bad == "0", "缺键行=%s" % bad)

# ===== L3 权限与既有护栏分层 =====
sm, rm = put_one(KEY, "20", MTV)
ok("L3a 成员打不开单项写口（403），库里没动",
   sm == 403 and sval(KEY) == "14", "http=%s 现值=%s" % (sm, sval(KEY)))
s7, r7 = put_one("site_type", "nexusphp")
ok("L3b site_type 仍被引导走站型包（三审 B-4 回归）",
   s7 == 400 and "站点类型" in msg((s7, r7)),
   "http=%s msg=%s" % (s7, msg((s7, r7))[:60]))
s8, r8 = put_one("bl_no_such_key_%d" % TAG, "1")
ok("L3c 未知键必须 400（不得 INSERT 造新键）",
   s8 == 400 and cnt("site_settings", "name = 'bl_no_such_key_%d'" % TAG)
   == "0", "http=%s" % s8)
s9, r9 = put("/admin/settings/groups", {"group": "tracker",
                                        "values": {"announce_interval": "61"}},
             T92)
ok("L3d 92 级职员提交「整批都不可写」→ 403（groups.rs:151 无任何可写字段），"
   "且 announce_interval 没被改",
   s9 == 403 and sval("announce_interval") == "1800",
   "http=%s msg=%s 现值=%s" % (s9, msg((s9, r9))[:40],
                               sval("announce_interval")))
MINI = "bank_min_deposit"
V0 = sval(MINI)
s10, r10 = put("/admin/settings/groups", {"group": "economy",
                                          "values": {MINI: V0}}, T92)
ok("L3e 正控：同口同职员写一个 60 级字段（%s，现值原样回填）→ 200 saved=0"
   " ⇒ L3d 的 403 是字段门槛，不是整口对 92 级封死" % MINI,
   s10 == 200 and int((data((s10, r10)) or {}).get("saved", -1)) == 0
   and sval(MINI) == V0,
   "http=%s saved=%s 现值=%s" % (s10,
                                 (data((s10, r10)) or {}).get("saved"),
                                 sval(MINI)))
s11, r11 = put("/admin/settings/groups", {"group": "economy",
                                          "values": {MINI: V0,
                                                     "announce_interval":
                                                         "61"}}, T92)
ok("L3f 混批里的越权字段走 400 字段级错误、整批不落库（可写字段也不趁势写入）",
   s11 == 400 and "无权" in blob((s11, r11))
   and sval("announce_interval") == "1800" and sval(MINI) == V0,
   "http=%s body=%s ann=%s" % (s11, blob(r11)[:90], sval(
       "announce_interval")))

# ===== L4 密文键的留痕不落明文 =====
PROBE = "bl-sec-%d" % TAG
sq, rq = put_one(SEC, PROBE)
rj = json.loads(hist_row(SEC) or "{}") if hist_row(SEC).startswith("{") \
    else {}
ok("L4a 密文键经单项口写入成功且历史读得到（键 %s）" % SEC,
   sq == 200 and sval(SEC) == PROBE and hist_total(SEC) >= 1,
   "http=%s 现值=%s total=%s" % (sq, sval(SEC), hist_total(SEC)))
ok("L4b 历史里只记「(空)/已更新」，明文绝不出现在审计（也不出现在接口回显）",
   rj.get("old") == "(空)" and rj.get("new") == "已更新"
   and PROBE not in json.dumps(rj, ensure_ascii=False)
   and PROBE not in blob(rq)
   and cnt("audit_log", "ref::text LIKE '%%%s%%'" % PROBE) == "0",
   "ref=%s 全库命中=%s" % (json.dumps(rj, ensure_ascii=False)[:90],
                           cnt("audit_log", "ref::text LIKE '%%%s%%'"
                               % PROBE)))
psql("UPDATE site_settings SET value = '' WHERE name = '%s'" % SEC)
ok("L4c 密文探针已用库层复原（不依赖被验代码），复读=%s" % sval(SEC),
   sval(SEC) == S0, "现值长度=%d" % len(sval(SEC)))

# ===== L9 收尾：设定复原 + 复读 + 不留活跃账号 =====
sr, rr = put_one(KEY, K0)
ok("L9a 测试键经写入口复原（值=%s、复读一致）" % K0,
   sr == 200 and sval(KEY) == K0,
   "http=%s 现值=%s" % (sr, sval(KEY)))
psql("DELETE FROM user_modify_logs WHERE uid IN (%d, %d)" % (MV, S92))
for u in (MV, S92):
    psql("DELETE FROM spark_ledger WHERE user_id = %d" % u)
    psql("DELETE FROM traffic_ledger WHERE user_id = %d" % u)
    psql("DELETE FROM messages WHERE receiver_id = %d OR sender_id = %d"
         % (u, u))
    post("/admin/users/status", {"user_id": u, "status": 2})
    call("DELETE", "/admin/users/%d" % u, None, token=ROOT)
    psql("DELETE FROM spark_ledger WHERE user_id = %d" % u)
DONE = True
if INV:
    psql("UPDATE invites SET status = 2 WHERE id IN (%s)"
         % ",".join(str(int(i)) for i in INV))
NSET1 = cnt("site_settings")
JUNK = cnt("site_settings", "name LIKE 'bl\\_%'")
ok("L9b 除测试键外一个字没动：键总数 %s→%s、announce_interval=%s、"
   "本轮造的野键 %s 个（应为 0）" % (NSET0, NSET1, sval(
       "announce_interval"), JUNK),
   NSET1 == NSET0 and sval("announce_interval") == "1800" and JUNK == "0",
   "键总数=%s→%s ann=%s 野键=%s" % (NSET0, NSET1,
                                    sval("announce_interval"), JUNK))
ok("L9c 探针已墓碑、本轮不留活跃账号",
   psql("SELECT count(*) FROM users WHERE id IN (%d, %d) AND status >= 2 "
        "AND passkey LIKE 'deleted-%%'" % (MV, S92)) == "2"
   and psql("SELECT count(*) FROM users WHERE status < 2 AND (username LIKE "
            "'blmv%%' OR username LIKE 'bl92%%')") == "0",
   "live=%s" % psql("SELECT count(*) FROM users WHERE status < 2 AND "
                    "username LIKE 'bl%%'"))
OLD = cnt("audit_log", "action = 'site_setting_update' AND created_at > '%s'"
          % NOW0)
ok("L9d 旧的空留痕动作 site_setting_update 本轮 0 条（老代码路径已断根），"
   "审计只增不减（setting:update total=%s ≥ %s）" % (
       hist_total(KEY), H0),
   OLD == "0" and hist_total(KEY) >= H0,
   "本轮旧动作=%s total=%s H0=%s" % (OLD, hist_total(KEY), H0))
summary()
