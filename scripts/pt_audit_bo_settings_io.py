# -*- coding: utf-8 -*-
"""闸门 BO：站点设定的导出/导入面（明文密钥的门槛、留痕与缓存语义）。

选题：`/admin/settings` 一族里批四十八没碰的三条读/交换口 ——
`GET /admin/settings/export`、`POST /admin/settings/import`、
`POST /admin/settings/smtp-test`（外加裸 `GET /admin/settings` 列表口）。

  P-191（P1）明文导出零留痕：`export.rs:39-95` 整个导出分支**一行审计都不写**
        （全文唯一的 `INSERT INTO audit_log` 在导入分支 :239，且密文掩码）。
        实测 root 连打 6 次 `?plaintext=1` 全 200 并真拿到
        `{"payment_key":"e7testkey123"}`，而 `audit_log` 里 `%export%` 动作
        **0 条** ⇒ 「谁在什么时候把全站密钥明文拉走」不可回答。
  P-192（P2）明文出口缺 no-store：导出返回裸 `ok(json)`，没有
        `Cache-Control: no-store`（本仓已有 `openapi_http::no_store:119` 的
        惯例）⇒ 一次带会话的 GET 可被中间缓存/代理日志留住全站配置。
  P-193（P1，若成立）SMTP 口令随错误正文回显：`mailer.rs:51-92` 把账密嵌进
        URL userinfo（`smtps://user:pass@host:465`），`smtp_test.rs:86` 的失败
        分支是 `Validation(format!("测试邮件发送失败：{e:#}"))` ⇒ 错误链里带出
        URL 就把口令回显给调用方。本闸门用**哨兵口令**判「正文与容器日志里
        都不许出现它」，并把五键快照/复原/复读。

**已核实、不必重查**：`settings.manage` 的 `min_class=95`（0054）并不授予
administrator —— 现网 class 95/93 打 `?plaintext=1` 都是 403/2003，root(99)
才 200；生效的是 `role_permissions` 授权表，注释「仅 sysop」行为上成立。
`import` 的形状是对的（dry-run 回 diff、`confirm=true` 才在事务里逐行
`FOR UPDATE` 落库并写掩码审计）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bo_settings_io.py
"""
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
INV = []
KEY = "anticheat_absurd_vouch_gb"
SKEYS = ["smtp_host", "smtp_port", "accountname", "accountpassword",
         "encryption"]
SENT = "bosent%d" % TAG


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


def blob(x):
    return json.dumps(x, ensure_ascii=False) if x is not None else ""


def data(x):
    if isinstance(x, tuple):
        x = x[1]
    return (x or {}).get("data") or {}


def msg(x):
    return str((x[1] or {}).get("message") or "")


def mkstaff(cls):
    codes = data(call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                      token=ROOT)).get("codes") or []
    cap = data(call("GET", "/auth/captcha", token=ROOT))
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(nums) < 2:
        return 0, None, "发码/验证码没准备好"
    INV.append(codes[0]["id"])
    name = "bo%d_%d" % (cls, TAG)
    rg = call("POST", "/auth/register", {
        "username": name, "email": "%s@bo.local" % name,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    tok = data(rg).get("token") or data(call(
        "POST", "/auth/login", {"username": name, "password": "password123"},
        token=ROOT)).get("token")
    uid = int(psql("SELECT id FROM users WHERE username = '%s'" % name) or 0)
    if cls:
        call("POST", "/admin/users/class", {"user_id": uid, "class_id": cls},
             token=ROOT)
        tok = data(call("POST", "/auth/login", {"username": name,
                     "password": "password123"},
                     token=ROOT)).get("token") or tok
    if not tok:
        return uid, None, "注册/登录 http=%s" % rg[0]
    return uid, tok, ""


DONE = [False]


def finalize():
    """收尾：五键 + 测试键复原，探针墓碑。崩在半路也要跑。"""
    for k, v in SNAP.items():
        psql("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
             % (v.replace("'", "''"), k))
    if sval(KEY) != K0:
        psql("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
             % (K0, KEY))
    if DONE[0]:
        return
    for u in (P95, P93):
        live = u and psql(
            "SELECT status::text FROM users WHERE id = %d" % u) != "2"
        if live:
            call("POST", "/admin/users/status", {"user_id": u, "status": 2},
                 token=ROOT)
            call("DELETE", "/admin/users/%d" % u, None, token=ROOT)
    if INV:
        psql("UPDATE invites SET status = 2 WHERE id IN (%s)"
             % ",".join(str(int(i)) for i in INV))


import atexit  # noqa: E402

atexit.register(finalize)

clear_register_rl()
K0 = sval(KEY)
SNAP = {k: sval(k) for k in SKEYS}
P95, T95, E95 = mkstaff(95)
P93, T93, E93 = mkstaff(93)
ok("O0a 探针就绪（class95=%s class93=%s）%s" % (P95, P93, E95 + E93),
   P95 > 0 and T95 and P93 > 0 and T93, "95=%s 93=%s" % (P95, P93))
SECKEYS = psql("SELECT string_agg(m.name, ',') FROM settings_meta m "
               "JOIN site_settings s ON s.name = m.name WHERE m.secret")
NSEC = len([x for x in SECKEYS.split(",") if x])
MASKED = psql("SELECT count(*) FROM settings_meta m JOIN site_settings s "
              "ON s.name = m.name WHERE m.secret AND btrim(s.value) <> ''")
ok("O0b 现网密文键 %s 个（其中非空 %s 个），测试键 %s=%s"
   % (NSEC, MASKED, KEY, K0), NSEC >= 1 and K0.isdigit(),
   "密文=%s 非空=%s 测试键=%s" % (NSEC, MASKED, K0))

# ===== O1 明文导出的真实门槛（正控：别按 min_class 猜） =====
s95, b95 = call("GET", "/admin/settings/export?plaintext=1", None, token=T95)
s93, b93 = call("GET", "/admin/settings/export?plaintext=1", None, token=T93)
ok("O1a 明文导出对 95/93 级必须 403（生效门槛来自 role_permissions，"
   "不是 permissions.min_class=95）",
   s95 == 403 and s93 == 403 and (b95 or {}).get("code") == 2003,
   "95→http=%s code=%s，93→http=%s" % (s95, (b95 or {}).get("code"), s93))
sr, br = call("GET", "/admin/settings/export?plaintext=1", None, token=ROOT)
VALS = data((sr, br)).get("settings") or {}
PKEY = str(VALS.get("payment_key") or "")
DBPK = sval("payment_key")
ok("O1b 正控：sysop 确实拿得到明文（否则后面几条判据都是空转）",
   sr == 200 and (DBPK == "" or PKEY == DBPK),
   "http=%s 回显长度=%d 库里长度=%d" % (sr, len(PKEY), len(DBPK)))

# ===== O2 掩码导出必须是「真的掩码」 =====
sm, bm = call("GET", "/admin/settings/export", None, token=T93)
D2 = data((sm, bm))
V2 = D2.get("settings") or {}
MS = D2.get("masked_secrets") or []
leak = [k for k in MS if str(V2.get(k) or "") not in ("",)]
ok("O2a 默认导出让 93 级也能读全站配置，但密文值必须是空串（%s 个密文键，"
   "回显非空 %s 个）" % (len(MS), len(leak)),
   sm == 200 and len(MS) == NSEC and not leak,
   "masked=%s 泄漏=%s" % (len(MS), json.dumps(
       {k: str(V2.get(k))[:12] for k in leak[:3]}, ensure_ascii=False)))
ok("O2b 掩码导出要自报 plaintext=false，前端据此决定密文空值语义",
   D2.get("plaintext") is False, "plaintext=%s" % D2.get("plaintext"))

# ===== O3 明文导出必须留痕（P-191） =====
A0 = cnt("audit_log")
E0 = cnt("audit_log", "action = 'setting:export'")
call("GET", "/admin/settings/export?plaintext=1", None, token=ROOT)
call("GET", "/admin/settings/export?plaintext=1", None, token=ROOT)
E1 = cnt("audit_log", "action = 'setting:export'")
ok("O3a 明文导出要留下可读的动作行（本轮打了 2 次，%s → %s 条）"
   % (E0, E1), int(E1 or 0) - int(E0 or 0) >= 2,
   "前后=%s/%s" % (E0, E1))
ROW = psql("SELECT ref FROM audit_log WHERE action = 'setting:export' "
           "ORDER BY id DESC LIMIT 1")
RJ = json.loads(ROW) if ROW.startswith("{") else {}
ok("O3b 留痕要能回答「谁、明文还是掩码、几条」，但绝不把密钥值写进审计",
   RJ.get("plaintext") is True and str(RJ.get("count") or "").isdigit()
   and (DBPK == "" or DBPK not in ROW) and SENT not in ROW,
   "ref=%s  actor 有无=%s" % (ROW[:90], "actor" in ROW))

# ===== O4 明文/掩码出口都要 no-store（P-192） =====
import urllib.error  # noqa: E402
import urllib.request  # noqa: E402
from pt_audit_lib import BASE  # noqa: E402


def cache_control(path, tok):
    """直读响应头（`call()` 只回状态码与 JSON，看不到头）。"""
    req = urllib.request.Request(BASE + path)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            return str(r.headers.get("Cache-Control") or ""), r.status
    except urllib.error.HTTPError as e:
        return str((e.headers or {}).get("Cache-Control") or ""), e.code
    except Exception as e:  # noqa: BLE001
        return "读取失败:%s" % e, 0


CC1, ST1 = cache_control("/admin/settings/export?plaintext=1", ROOT)
CC2, ST2 = cache_control("/admin/settings/export", T93)
ok("O4a 明文导出必须带 Cache-Control: no-store（带会话的 GET 把全站密钥"
   "交给中间缓存/代理日志是不能接受的）",
   ST1 == 200 and "no-store" in CC1, "http=%s Cache-Control=%r" % (ST1, CC1))
ok("O4b 掩码导出同样要 no-store（它给的是全站配置与密文键清单）",
   ST2 == 200 and "no-store" in CC2, "http=%s Cache-Control=%r" % (ST2, CC2))

# ===== O5 导入口的形状（不写站点设定，只跑 dry-run） =====
s, b = call("POST", "/admin/settings/import", {"settings": {}}, token=ROOT)
ok("O5a 空导入必须 400（不得当成「什么都没做」的成功）",
   s == 400 and "空" in msg((s, b)), "http=%s msg=%s" % (s, msg((s, b))[:50]))
s, b = call("POST", "/admin/settings/import",
            {"settings": {"bo_no_such_%d" % TAG: "1"}}, token=ROOT)
ok("O5b 未知键要在 unknown 里点名，且不落库",
   s == 200 and "bo_no_such_%d" % TAG in blob((s, b))
   and cnt("site_settings", "name = 'bo_no_such_%d'" % TAG) == "0",
   "http=%s body=%s" % (s, blob(b)[:70]))
s, b = call("POST", "/admin/settings/import",
            {"settings": {KEY: "abc"}, "confirm": False}, token=ROOT)
ok("O5c dry-run 要把坏值逐条点名（导入接的是同一把尺）",
   s == 400 and KEY in blob((s, b)), "http=%s body=%s" % (s, blob(b)[:80]))
K1V = sval(KEY)
s, b = call("POST", "/admin/settings/import",
            {"settings": {KEY: "7"}, "confirm": False}, token=ROOT)
D5 = data((s, b))
ok("O5d confirm=false 只回 diff、一个字都不改（本轮 200=%s，库里=%s）"
   % (s == 200, sval(KEY)),
   s == 200 and D5.get("dry_run") is True and sval(KEY) == K1V,
   "http=%s will_change=%s 库里=%s" % (s, D5.get("will_change"), sval(KEY)))
s9, b9 = call("POST", "/admin/settings/import",
              {"settings": {KEY: "7"}, "confirm": True}, token=T95)
ok("O5e 95 级职员不能落库导入（写侧要 settings.manage）",
   s9 == 403 and sval(KEY) == K1V, "http=%s 库里=%s" % (s9, sval(KEY)))

# ===== O6 smtp-test 不许把口令回显（P-193） =====
psql("UPDATE site_settings SET value='127.0.0.1' WHERE name='smtp_host'")
psql("UPDATE site_settings SET value='2525' WHERE name='smtp_port'")
psql("UPDATE site_settings SET value='bo-probe' WHERE name='accountname'")
psql("UPDATE site_settings SET value='%s' WHERE name='accountpassword'" % SENT)
psql("UPDATE site_settings SET value='none' WHERE name='encryption'")
ok("O6a 探针 SMTP 配置写入并复读（host=%s port=%s 口令哨兵=%s）"
   % (sval("smtp_host"), sval("smtp_port"), SENT),
   sval("smtp_host") == "127.0.0.1" and sval("accountpassword") == SENT,
   "host=%s" % sval("smtp_host"))
ssm, bsm = call("POST", "/admin/settings/smtp-test", {"to": "bo@bo.local"},
                token=ROOT)
LEAK = SENT in blob((ssm, bsm)) or SENT in msg((ssm, bsm))
LOG = subprocess.run(
    ["docker", "logs", "--since", "2m", "flux-api"], stdout=subprocess.PIPE,
    stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
LEAKLOG = SENT in ((LOG.stdout or "") + (LOG.stderr or ""))
ok("O6b smtp-test 的响应正文里绝不许出现哨兵口令（http=%s）" % ssm,
   not LEAK, "body=%s" % blob(bsm)[:110])
ok("O6c 容器日志里也不许出现哨兵口令", not LEAKLOG,
   "日志命中=%s" % LEAKLOG)
for k, v in SNAP.items():
    psql("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
         % (v.replace("'", "''"), k))
ok("O6d 五键已复原并逐键复读一致（%s）"
   % ",".join("%s=%s" % (k, sval(k) or "(空)") for k in SKEYS),
   all(sval(k) == SNAP[k] for k in SKEYS),
   "现值=%s" % json.dumps({k: sval(k)[:14] for k in SKEYS},
                          ensure_ascii=False))

# ===== O7 裸列表口也必须掩码（正控：这条一直是对的，钉住别漂） =====
sl, bl = call("GET", "/admin/settings", None, token=T93)
rows = data((sl, bl))
rows = rows if isinstance(rows, list) else (rows.get("settings")
                                           if isinstance(rows, dict) else [])
if isinstance(rows, dict):
    rows = [{"name": k, "value": v} for k, v in rows.items()]
sec_names = [x for x in SECKEYS.split(",") if x]
bad = [r for r in rows
       if str((r or {}).get("name")) in sec_names
       and str((r or {}).get("value") or "") != ""]
ok("O7a GET /admin/settings 对 93 级可读，但 %s 个密文键的值必须是空串"
   % len(sec_names), sl == 200 and len(rows) > 0 and not bad,
   "http=%s 行数=%d 非空密文=%s" % (sl, len(rows),
                                    [r.get("name") for r in bad][:3]))

# ===== O9 收尾 =====
if sval(KEY) != K0:
    psql("UPDATE site_settings SET value = '%s' WHERE name = '%s'" % (K0, KEY))
ok("O9a 测试键复原（%s → %s，复读=%s）" % (K1V, K0, sval(KEY)),
   sval(KEY) == K0, "现值=%s" % sval(KEY))
for u in (P95, P93):
    if u and psql("SELECT status::text FROM users WHERE id = %d" % u) != "2":
        call("POST", "/admin/users/status", {"user_id": u, "status": 2},
             token=ROOT)
        call("DELETE", "/admin/users/%d" % u, None, token=ROOT)
if INV:
    psql("UPDATE invites SET status = 2 WHERE id IN (%s)"
         % ",".join(str(int(i)) for i in INV))
DONE[0] = True
ok("O9b 探针已墓碑、本轮不留活跃账号",
   psql("SELECT count(*) FROM users WHERE status < 2 AND username LIKE "
        "'bo%%'") == "0",
   "live=%s" % psql("SELECT count(*) FROM users WHERE status < 2 AND "
                    "username LIKE 'bo%%'"))
ok("O9c 没有野键落库（bo_ 前缀 %s 行）、审计只增不减（%s → %s）"
   % (cnt("site_settings", "name LIKE 'bo\\_%'"), A0, cnt("audit_log")),
   cnt("site_settings", "name LIKE 'bo\\_%'") == "0"
   and int(cnt("audit_log") or 0) >= int(A0 or 0),
   "野键=%s 审计=%s→%s" % (cnt("site_settings", "name LIKE 'bo\\_%'"),
                            A0, cnt("audit_log")))
summary()
