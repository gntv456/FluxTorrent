# -*- coding: utf-8 -*-
"""闸门 BN：适配器后台五口（install / list / toggle / try / delete）。

选题：`/admin/adapters` 5 条路此前**零常驻覆盖**（`grep admin/adapters
scripts/pt_audit_*.py` 命中 0），而库里只有一行 `builtin.douban`
（enabled=false）—— 这条「生态商店」链从建站到现在没被任何哨兵走过。

  P-188（P1）限流不跨调用：限流桶原先在 `call()` 里每次新建（旧
        `adapter_runtime.rs:245` `rate: Arc::new(Mutex::new((Instant::now(),
        0)))`），于是「每分钟 N 次」实际是「单次调用内 guest 可出网 N 次」。
        后台填 1，连点 100 次 `/admin/adapters/try` 就出网 100 次。
  P-189（P2）KV 缓存同理：`host_cache_*` 的表也每次新建 ⇒ 文件头承诺的
        「进程内 LRU」命中率恒 0，`CACHE_CAP` 的清理分支永不可达。
  P-190（P3）模块指纹没有读者：install 算 `checksum` 并落库，但 `GET
        /admin/adapters` 不返回它 ⇒ 装完之后无从核对「线上跑的模块」与
        「供应商公布的 SHA256」是不是同一个。

验法（不依赖外网）：装两段**手写 WAT 访客**（宿主 `Module::new` 带 wat 特性，
接受文本模块），guest 把宿主返回码原样上交 —— -2 出网越权 / -3 限流 /
-4 上游请求失败，这三句原话由 `adapter_runtime.rs` 的负码表翻译，所以在 HTTP
层就能分辨是哪一挡拦的。限流探针指向不存在的 https 域（正常恒 -4），修后第 2
次起必须变 -3；KV 探针不出网，只做 `cache_get →(命中 HIT : cache_set+MISS)`，
修后第二次必须 HIT。**每条判据各自一个探针适配器**：连打 3 次失败会触发熔断
停用，共用一个会把后面的判据全打成「未启用」（与批四十四同一课）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bn_adapters.py
"""
import atexit
import base64
import hashlib
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
LIVE = []

MOD_HEAD = """(module
  (import "host" "http_fetch"
    (func $http_fetch (param i32 i32 i32 i32) (result i32)))
  (import "host" "cache_get"
    (func $cget (param i32 i32 i32 i32) (result i32)))
  (import "host" "cache_set"
    (func $cset (param i32 i32 i32 i32 i64)))
  (memory (export "memory") 2)
  (data (i32.const 2048) "bnkey")
  (data (i32.const 2112) "{\\"success\\":true,\\"name\\":\\"HIT\\"}")
  (data (i32.const 2176) "{\\"success\\":true,\\"name\\":\\"MISS\\"}")
"""
RATE_WAT = MOD_HEAD + """
  (func (export "fetch") (param $in i32) (param $len i32) (param $out i32)
    (result i32)
    (local $rc i32)
    (local.set $rc (call $http_fetch (local.get $in) (local.get $len)
                                     (i32.const 2304) (i32.const 64)))
    (if (i32.lt_s (local.get $rc) (i32.const 0))
      (then (return (local.get $rc))))
    (memory.copy (local.get $out) (i32.const 2112) (i32.const 29))
    (return (i32.const 29)))
)
"""
KV_WAT = MOD_HEAD + """
  (func (export "fetch") (param $in i32) (param $len i32) (param $out i32)
    (result i32)
    (local $rc i32)
    (local.set $rc (call $cget (i32.const 2048) (i32.const 7)
                               (i32.const 2304) (i32.const 64)))
    (if (i32.gt_s (local.get $rc) (i32.const 0))
      (then
        (memory.copy (local.get $out) (i32.const 2112) (i32.const 29))
        (return (i32.const 29))))
    (call $cset (i32.const 2048) (i32.const 7)
                (i32.const 2048) (i32.const 7) (i64.const 600))
    (memory.copy (local.get $out) (i32.const 2176) (i32.const 30))
    (return (i32.const 30)))
)
"""


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:56], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


def cnt(t, w=None):
    return psql("SELECT count(*) FROM %s%s"
                % (t, "" if w is None else " WHERE " + w))


def blob(x):
    return json.dumps(x, ensure_ascii=False)


def data(x):
    if isinstance(x, tuple):
        x = x[1]
    return (x or {}).get("data") or {}


def msg(x):
    return str((x[1] or {}).get("message") or "")


def mk(x):
    return "http=%s msg=%s" % (x[0], msg(x)[:56])


def install(aid, wat, rate, allow, tok=None):
    body = {
        "adapter_id": aid, "kind": "metadata", "name": "BN 探针 " + aid,
        "version": "0.1.0", "http_allow": allow, "secrets_read": [],
        "rate_limit_per_min": rate,
        "wasm_b64": base64.b64encode(wat.encode()).decode(),
    }
    x = call("POST", "/admin/adapters/install", body, token=tok or ROOT)
    if x[0] == 200:
        LIVE.append(aid)
        call("POST", "/admin/adapters/toggle",
             {"adapter_id": aid, "enabled": True}, token=ROOT)
    return x


def try_(aid, url, tok=None):
    return call("POST", "/admin/adapters/try",
                {"adapter_id": aid, "url": url}, token=tok or ROOT)


def name_of(x):
    return str(((data(x) or {}).get("result") or {}).get("name") or "")


def finish():
    for aid in list(LIVE):
        call("POST", "/admin/adapters/delete", {"adapter_id": aid},
            token=ROOT)
        LIVE.remove(aid)


atexit.register(finish)


def mkprobe():
    codes = data(call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                      token=ROOT)).get("codes") or []
    cap = data(call("GET", "/auth/captcha", token=ROOT))
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(nums) < 2:
        return 0, None
    name = "bnmv%d" % TAG
    rg = call("POST", "/auth/register", {
        "username": name, "email": "%s@bn.local" % name,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    tok = data(rg).get("token") or data(call(
        "POST", "/auth/login", {"username": name, "password": "password123"},
        token=ROOT)).get("token")
    uid = int(psql("SELECT id FROM users WHERE username = '%s'" % name) or 0)
    return uid, tok


clear_register_rl()
BASE0 = cnt("adapters")
DOUBAN = "SELECT adapter_id||'|'||enabled::text||'|'||strikes::text||'|'" \
         "||last_error||'|'||checksum FROM adapters WHERE " \
         "adapter_id='builtin.douban'"
D0 = psql(DOUBAN)
PUID, PTOK = mkprobe()
ok("N0a 基线与探针（适配器 %s 行；成员 %s；内置行=%s）" % (
    BASE0, PUID, D0[:40]),
   BASE0.isdigit() and int(BASE0) >= 1 and PUID > 0 and PTOK and D0 != "",
   "行数=%s 成员=%s" % (BASE0, PUID))

ALLOW = ["https://bn-probe-%d.invalid/**" % TAG]
BAD = "https://bn-probe-%d.invalid/x" % TAG
OUT = "https://other-%d.invalid/y" % TAG

# ===== N1 限流必须跨调用（P-188） =====
A1 = "bn%dA" % TAG
x = install(A1, RATE_WAT, 1, ALLOW)
ok("N1a 探针适配器装上并启用（rate_limit_per_min=1）", x[0] == 200, mk(x))
x0 = try_(A1, OUT)
ok("N1b 正控：白名单外必须报「出网越权」（-2 通道在，与限流是两句不同原话）",
   x0[0] == 400 and "越权" in msg(x0), mk(x0))
x1 = try_(A1, BAD)
x2 = try_(A1, BAD)
ok("N1c 白名单内第 1 次应当放行（结果是不存在域 → 「上游请求失败」）",
   x1[0] == 400 and "上游请求失败" in msg(x1), mk(x1))
ok("N1d 第 2 次必须被「限流」挡住（P-188 主判据：桶要跨调用活着）",
   x2[0] == 400 and "限流" in msg(x2), mk(x2))
A2 = "bn%dB" % TAG
install(A2, RATE_WAT, 2, ALLOW)
seq = [try_(A2, BAD) for _ in range(3)]
txt = [msg(s) for s in seq]
ok("N1e rate=2 是「放行两次、第三次限流」而不是逢第二次就砍（计数按额度）",
   "限流" not in txt[0] and "限流" not in txt[1] and "限流" in txt[2],
   " ".join("#%d:%s" % (i + 1, t[-8:]) for i, t in enumerate(txt)))

# ===== N2 KV 缓存必须跨调用（P-189） =====
K1 = "bn%dK" % TAG
x = install(K1, KV_WAT, 30, [])
ok("N2a KV 探针装上（http_allow 为空也应装得上、不出网）",
   x[0] == 200, mk(x))
c1, c2 = try_(K1, BAD), try_(K1, BAD)
ok("N2b 第一次未命中 → MISS（探针自身通电，不是站点判据）",
   name_of(c1) == "MISS", "name=%s %s" % (name_of(c1), mk(c1)))
ok("N2c 第二次必须 HIT（P-189 主判据：表在进程里，不随 Store 丢弃）",
   name_of(c2) == "HIT", "name=%s" % name_of(c2))
K2 = "bn%dK2" % TAG
install(K2, KV_WAT, 30, [])
c3 = try_(K2, BAD)
ok("N2d 命名空间按 adapter_id 隔离：新适配器读不到别人的缓存（防串味）",
   name_of(c3) == "MISS", "name=%s" % name_of(c3))

# ===== N3 模块指纹要有读者（P-190） =====
sl = call("GET", "/admin/adapters", token=ROOT)
rows = data(sl)
rows = rows if isinstance(rows, list) else []
mine = {str(r.get("adapter_id")): r for r in rows
        if str(r.get("adapter_id", "")).startswith("bn%d" % TAG)}
CK = str((mine.get(K1) or {}).get("checksum") or "")
ok("N3a GET /admin/adapters 必须回显 checksum（修前没这个字段，装完就读不到）",
   K1 in mine and len(CK) == 64,
   "字段=%s" % sorted((mine.get(K1) or {}).keys()))
HEX = psql("SELECT encode(wasm,'hex') FROM adapters WHERE adapter_id='%s'" % K1)
CKDB = hashlib.sha256(bytes.fromhex(HEX)).hexdigest() if HEX else ""
ok("N3b 回显与库里的 checksum 都等于 wasm 字节的 sha256（不是随手写的串）",
   CKDB != "" and CK == CKDB,
   "算得=%s 回显=%s" % (CKDB[:16], CK[:16]))

# ===== N4 装入口形状与熔断 =====
B1 = {"adapter_id": "bn%dX" % TAG, "kind": "sms", "name": "x",
      "http_allow": [], "secrets_read": [],
      "wasm_b64": base64.b64encode(RATE_WAT.encode()).decode()}
x = call("POST", "/admin/adapters/install", B1, token=ROOT)
ok("N4a kind 不在 metadata/payment/indexer 里 → 400 且点名三个取值",
   x[0] == 400 and "metadata" in blob(x), mk(x))
B2 = dict(B1, kind="metadata",
          wasm_b64=base64.b64encode(b"\x00\x01bad module").decode())
x = call("POST", "/admin/adapters/install", B2, token=ROOT)
ok("N4b 编不过的模块必须在装入口就被预检拒掉（不许装一个起不来的适配器）",
   x[0] == 400 and "校验失败" in msg(x), mk(x))
B3 = dict(B1, kind="metadata", wasm_b64=base64.b64encode(b"\x00" * 8).decode())
x = call("POST", "/admin/adapters/install", B3, token=ROOT)
ok("N4c 非 wasm 魔数同样拒", x[0] == 400, mk(x))
x = call("POST", "/admin/adapters/toggle",
         {"adapter_id": "bn_no_such_%d" % TAG, "enabled": True}, token=ROOT)
ok("N4d toggle 不存在的适配器 → 400「适配器不存在」（不得静默成功）",
   x[0] == 400 and "不存在" in msg(x), mk(x))
x = call("POST", "/admin/adapters/delete",
         {"adapter_id": "bn_no_such_%d" % TAG}, token=ROOT)
ok("N4e delete 不存在的适配器 → 400",
   x[0] == 400 and "不存在" in msg(x), mk(x))
x = call("GET", "/admin/adapters", token=PTOK)
ok("N4f 成员读不到后台列表（403，settings.view 门槛在位）",
   x[0] == 403, mk(x))
x = call("POST", "/admin/adapters/install", B1, token=PTOK)
ok("N4g 成员装不了适配器（403，settings.manage 门槛在位）",
   x[0] == 403, mk(x))
A3 = "bn%dC" % TAG
install(A3, RATE_WAT, 30, ALLOW)
for _ in range(3):
    try_(A3, BAD)
st = psql("SELECT enabled::text||'|'||strikes::text FROM adapters WHERE "
          "adapter_id = '%s'" % A3)
ok("N4h 熔断在位：连续 3 次失败 → enabled=false、strikes=3（A4 设计行为）",
   st.startswith("false|3"), "库里=%s" % st)
x = try_(A3, BAD)
ok("N4i 熔断停用后不再放行出网（「未启用」而不是又打一次上游）",
   x[0] == 400 and "未启用" in msg(x), mk(x))

# ===== N9 收尾 =====
finish()
N1 = cnt("adapters")
JUNK = cnt("adapters", "adapter_id LIKE 'bn%'")
D1 = psql(DOUBAN)
ok("N9a 探针适配器全删（行数 %s→%s，bn 前缀残留 %s 行）" % (
    BASE0, N1, JUNK),
   N1 == BASE0 and JUNK == "0",
   "行数=%s→%s 残留=%s" % (BASE0, N1, JUNK))
ok("N9b 内置 builtin.douban 全程一字未动（含 checksum 逐字复读）",
   D1 == D0, "开局=%s 收口=%s" % (D0[:60], D1[:60]))
post = psql("SELECT status::text FROM users WHERE id = %d" % PUID)
if post == "0":
    call("POST", "/admin/users/status", {"user_id": PUID, "status": 2},
         token=ROOT)
    call("DELETE", "/admin/users/%d" % PUID, None, token=ROOT)
ok("N9c 探针成员已墓碑、本轮不留活跃账号",
   psql("SELECT count(*) FROM users WHERE status < 2 AND username LIKE "
        "'bnmv%%'") == "0",
   "live=%s" % psql("SELECT count(*) FROM users WHERE status < 2 AND "
                    "username LIKE 'bnmv%%'"))
AD = cnt("audit_log", "action LIKE 'adapter:%' AND created_at > now() - "
         "interval '10 minutes'")
ok("N9d 本轮四类事件都有痕（install/enable/disable/delete，审计只增不减）",
   int(AD or 0) >= 4, "近 10 分钟 adapter:* =%s 条" % AD)
summary()
