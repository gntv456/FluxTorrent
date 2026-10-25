# -*- coding: utf-8 -*-
"""闸门 SW：站长画像·运维经济面（2026-10-25 八轮）。

覆盖备份按钮全链（P-17）、gacha 发放（幽灵 uid / 幂等键，P-18）、
促销 PUT（P-19）、募资建关、分层抽查（92 档对站长口应 403）。

  P-17（P1 已修）：备份按钮默认命令 `docker exec flux-postgres pg_dump`
        在 api 容器内无 docker 二进制必炸 ENOENT 500——按钮从未成功过。
        修=默认改 pg_dump 直连 postgres:5432（PGPASSWORD 走环境变量），
        镜像补 postgresql-client-16（PGDG 源；15 连 16 服务器 mismatch）。
  P-18（P2 已修）：/admin/gacha/grant 幽灵 uid 500——users 存在性检查排在
        余额表 INSERT 之后，FK 先炸，404 守卫永远走不到。
  P-19（P1 已修）：PUT /admin/freeleech/{id} 非 category 分支 bind 错位
        （scope 占位绑了 cid 的 i32）→「cannot cast integer to
        promotion_scope」500，促销编辑整口从未成功。

  分层判据依赖探针号 sv8mod92。九轮实测：号被回收成墓碑名后，旧写法第一句
  `login()` 就让 lib 直接 SystemExit ⇒ rc=1 而红账 0 条，六条 SW14 静默蒸发
  （2026-10-25 现网跑出「12 条 PASS + 一句 login failed」就是它）。
  现改为开局查库自愈：墓碑名认领回来（不另开新号堆残料）、状态/等级/口令归一化，
  取 token 用 call 而不是 login()（失败只是 None）；探针不可用时必须落一条
  具名红，不许「跳过即绿」。
  另注：SW14 这六条本身就是**破坏性动作的权限探针**——一旦某道闸回归失效，
  这条判据不只会红，还会真的执行那次备份/促销/设置写入，红账要当事故读。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_sw_ops_economy.py
"""
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOT = login()
TAG = int(time.time()) % 100000

# ============ S1 备份链（P-17 棘轮） ============
s, r = call("GET", "/admin/backups", token=ROOT)
ok("SW1 备份列表", s == 200, str(r)[:100])
n_before = len(r.get("data", {}).get("files", []))
s, r = call("POST", "/admin/backups/run", token=ROOT, body={})
ok("SW2 备份执行 200（P-17）", s == 200, str(r)[:150])
s, r = call("GET", "/admin/backups", token=ROOT)
files = r.get("data", {}).get("files", [])
manual = [f for f in files if "manual" in str(f)]
ok("SW3 manual 备份落盘", len(manual) >= 1, str(manual[:1])[:120])

# ============ S2 gacha 发放（P-18 棘轮 + 幂等） ============
s, r = call("POST", "/admin/gacha/grant", token=ROOT,
            body={"user_id": 99999999, "amount": 1, "note": "gate ghost",
                  "idempotency_key": "swghost%08d" % TAG})
ok("SW4 幽灵 uid 404 非 500（P-18）", s == 404, "s=%s %s" % (s, str(r)[:80]))

s, r = call("POST", "/admin/gacha/grant", token=ROOT,
            body={"user_id": 1, "amount": 1, "note": "gate idem",
                  "idempotency_key": "swidem%08d" % TAG})
ok("SW5 真实 uid 发放 200", s == 200, str(r)[:110])
s, r = call("POST", "/admin/gacha/grant", token=ROOT,
            body={"user_id": 1, "amount": 1, "note": "gate idem",
                  "idempotency_key": "swidem%08d" % TAG})
ok("SW6 同幂等键重发 400 明示", s == 400, "s=%s" % s)

# ============ S3 促销（P-19 棘轮） ============
s, r = call("POST", "/admin/freeleech", token=ROOT,
            body={"kind": "free", "scope": "global", "hours": 1})
pid = r.get("data", {}).get("id") if s == 200 else None
ok("SW7 建促销", pid is not None, str(r)[:100])
if pid:
    s, r = call("PUT", "/admin/freeleech/%s" % pid, token=ROOT,
                body={"kind": "free", "scope": "global", "hours": 2})
    ok("SW8 促销整对象更新 200（P-19）", s == 200, str(r)[:110])
    s, r = call("POST", "/admin/freeleech", token=ROOT,
                body={"kind": "freeleech", "scope": "global", "hours": 1})
    ok("SW9 坏 kind 400", s == 400, "s=%s" % s)
    s, r = call("DELETE", "/admin/freeleech/%s" % pid, token=ROOT)
    ok("SW10 删促销", s == 200, str(r)[:90])

# ============ S4 募资建关 ============
s, r = call("GET", "/admin/export/users?limit=1", token=ROOT)
ok("SW11 导用户", s == 200, "s=%s" % s)
s, r = call("GET", "/admin/payment-orders", token=ROOT)
ok("SW12 订单列表", s == 200, "s=%s" % s)

# ============ S5 分层抽查：92 档对站长口 403 ============
import subprocess  # noqa: E402

PROBE = "sv8mod92"


def psql(sql):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-Atc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if p.returncode:
        raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:200])
    lines = [x for x in p.stdout.splitlines() if x.strip()]
    return lines[0].strip() if lines else ""


def probe_token():
    """备好一枚 class 92 探针号，返回 (token, uid)。

    旧写法第一句就是 `login("sv8mod92", …)`，而 lib 的 login 在**非 429
    失败时直接 SystemExit**：号被回收成墓碑名后，本来该走自愈的分支根本到不了，
    表现是 rc=1 而红账 0 条——日志只剩 12 条 PASS 加一句 login failed，
    六条「92 档对站长口应 403」静默蒸发。
    探活也不许靠打错密码：连续失败第 3 次起要 pow_token，会把后续所有登录
    一起卡住。现在先查库定身分（墓碑名认领回来，不另开新号堆残料），
    再用 call 直接登录取 token——失败只是一个 None，不是整闸门死亡。
    """
    uid = psql("SELECT id FROM users WHERE username = '%s'" % PROBE)
    if not uid:
        uid = psql("SELECT id FROM users WHERE username LIKE 'deleted-%%-%s'"
                   " ORDER BY id DESC LIMIT 1" % PROBE)
        if uid:
            psql("UPDATE users SET username='%s' WHERE id=%s" % (PROBE, uid))
    if uid:
        psql("UPDATE users SET status=0, class_id=92, pass_hash="
             "(SELECT pass_hash FROM users WHERE id=1) WHERE id=%s" % uid)
    else:
        s, r = call("POST", "/admin/adduser", token=ROOT,
                    body={"username": PROBE,
                          "email": "%s@probe.local" % PROBE,
                          "password": "password123"})
        if s != 200:
            return None, None
        uid = str((r.get("data") or {}).get("user_id") or "")
        if not uid.isdigit():
            return None, None
        call("POST", "/admin/users/class", token=ROOT,
             body={"user_id": int(uid), "class_id": 92})
        psql("UPDATE users SET pass_hash=(SELECT pass_hash FROM users"
             " WHERE id=1) WHERE id=%s" % uid)
    s, r = call("POST", "/auth/login",
                body={"username": PROBE, "password": "password123"})
    tok = (r.get("data") or {}).get("token") if s == 200 else None
    return tok, uid


MOD, PROBE_UID = probe_token()
ready = bool(MOD) and bool(PROBE_UID) and psql(
    "SELECT class_id FROM users WHERE id=%s" % PROBE_UID) == "92"
ok("SW13 92 探针号就绪（开局自愈；不可用即红，不许静默跳过）", ready,
   "uid=%s" % PROBE_UID)
if ready:
    for tag, method, path, body in [
        ("备份 run", "POST", "/admin/backups/run", {}),
        ("gacha grant", "POST", "/admin/gacha/grant",
         {"user_id": 1, "amount": 1, "note": "x",
          "idempotency_key": "swlayer%08d" % TAG}),
        ("freeleech set", "POST", "/admin/freeleech",
         {"kind": "free", "scope": "global", "hours": 1}),
        ("terms keys", "POST", "/admin/terms/keys",
         {"key": "sw.x", "display_zh": "x"}),
        ("staffpanel add", "POST", "/admin/staffpanel-entries",
         {"section": "ops", "name": "x", "url": "/x", "info": ""}),
        ("settings 写", "PUT", "/admin/settings", {"name": "x", "value": "y"}),
    ]:
        s, r = call(method, path, token=MOD, body=body)
        ok("SW14 %s 92→403" % tag, s == 403, "s=%s" % s)
else:
    ok("SW14 分层抽查未执行（探针号不可用）", False,
       "六条「92→403」判据一条都没打出去——红在仪器，不在现网")

# ============ S6 备份盘存量台账（只记读数，不判红）============
# 现网事实：POST /admin/backups/run 每次落一枚 ~5MB 的 manual dump，
# GET /admin/backups 只读目录、没有任何保留上限，也没有删除入口
# （`admin_p3_http/ops.rs` 里只有 list/run/version 三口）。站长本人正在改这个
# 文件（P-17 本轮已在码里），审站方不代拍保留策略——这里只把读数钉在账面上，
# 哪天加了轮转或删除入口，就把这段换成判据。
s, r = call("GET", "/admin/backups", token=ROOT)
files = ((r.get("data") or {}).get("files")) or []
total_mb = sum(int(f[1]) for f in files if isinstance(f, list)
               and len(f) > 1) // (1024 * 1024)
manual = [f for f in files if isinstance(f, list)
          and "manual" in str(f[0])]
print("[WARN] SW15 备份目录读数：%d 枚（manual %d 枚）/ %d MB，"
      "无保留上限、无删除入口（待站长裁）" % (len(files), len(manual),
                                            total_mb))

summary()
