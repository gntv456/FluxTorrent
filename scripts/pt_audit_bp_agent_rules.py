# -*- coding: utf-8 -*-
"""闸门 BP：UA 客户端黑白名单后台（/admin/agentrules 五条路）。

选题：这一族在 `scripts/pt_audit_*.py` 里**零字面量命中**，而它是 tracker
侧真正执行的防护姿态（allow/deny + 版本区间 + priority）。红线四条：

  P-193（P0）list 与 export 对任何人都 500 —— 0351 加 version_min/version_max/
        priority 三列时，SELECT 写成裸 `COALESCE(...)`（输出列名 = `coalesce`），
        而 sqlx 的 `FromRow` 按**列名**取值 ⇒ `no column found for name:
        version_min`。面板里这一页只在「写完之后」刷新列表
        （`staff-tools-sys-agentrules.tsx:54/146`），所以症状是：进页面永远
        空列表 + 每点一次新增/删除都弹「内部错误」，而**那次写入已经落库**。
  P-194（P1）import 无事务：中途一条撞库错（备注含 U+0000 → PG
        `invalid byte sequence`）整批 500，但前面已插入的规则留在库里。
  P-195（P2）import 把同一把尺降级成静默跳过：mode 写错连 invalid 都不计，
        正文不点名哪条为什么没进去（对照 add 的直接 400 原话）。
  P-196（P3）note 无长度上限（实测 2003 字入库）、pattern 长度按**字节**判
        （一条三十几字的中文 UA 规则被「1-100」自拒）。

**未复现、按纵深记**：库里没有 UNIQUE(mode,pattern)（只有 pkey），去重靠
`WHERE NOT EXISTS` ⇒ 结构上有并发双插窗口，但两发并发实测只落 1 行。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bp_agent_rules.py
"""
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
NEW = "bpprobe%d" % TAG
NULNOTE = "bad" + chr(0) + "note"
PFX = "bp%d" % TAG
UID = [0]


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:56], (r.stderr or "").strip()[:80]))
    return (r.stdout or "").strip()


def cnt_rules():
    return psql("SELECT count(*) FROM agent_rules")


def probe_rows():
    return psql("SELECT count(*) FROM agent_rules WHERE pattern LIKE '%s' "
                "OR note LIKE '%s'" % (NEW + "%", PFX + "%"))


def blob(x):
    return json.dumps(x, ensure_ascii=False) if x is not None else ""


def data(x):
    if isinstance(x, tuple):
        x = x[1]
    return (x or {}).get("data")


def msg(x):
    return str(((x[1] if isinstance(x, tuple) else x) or {}).get("message")
               or "")[:56]


def finalize():
    ids = psql("SELECT string_agg(id::text, ',') FROM agent_rules WHERE "
               "pattern LIKE '%s' OR note LIKE '%s'" % (NEW + "%", PFX + "%"))
    if ids:
        psql("DELETE FROM agent_rules WHERE id IN (%s)" % ids)
    for u in UID:
        if u and psql("SELECT status::text FROM users WHERE id = %d" % u) \
                != "2":
            call("POST", "/admin/users/status", {"user_id": u, "status": 2},
                 token=ROOT)
            call("DELETE", "/admin/users/%d" % u, None, token=ROOT)


import atexit  # noqa: E402

atexit.register(finalize)


def add(mode, pattern, note="", tok=None, **kw):
    body = {"mode": mode, "pattern": pattern, "note": note}
    body.update(kw)
    return call("POST", "/admin/agentrules", body, token=tok or ROOT)


def imp(rules, tok=None):
    return call("POST", "/admin/agentrules/import", {"rules": rules},
                token=tok or ROOT)


def head(path, tok):
    req = urllib.request.Request(BASE + path)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            return r.status, str(r.headers.get("Cache-Control") or "")
    except urllib.error.HTTPError as e:
        return e.code, str((e.headers or {}).get("Cache-Control") or "")
    except Exception as e:  # noqa: BLE001
        return 0, "读取失败:%s" % e


clear_register_rl()
BASE0 = int(cnt_rules() or 0)
codes = (data(call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                  token=ROOT)) or {}).get("codes") or []
cap = data(call("GET", "/auth/captcha", token=ROOT))
nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
T92 = None
if codes and len(nums) >= 2:
    nm = "bp92_%d" % TAG
    call("POST", "/auth/register", {
        "username": nm, "email": "%s@bp.local" % nm,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}}, token=ROOT)
    u92 = int(psql("SELECT id FROM users WHERE username = '%s'" % nm) or 0)
    UID.append(u92)
    call("POST", "/admin/users/class", {"user_id": u92, "class_id": 92},
         token=ROOT)
    T92 = (data(call("POST", "/auth/login", {"username": nm,
            "password": "password123"}, token=ROOT)) or {}).get("token")
ok("Q0a 探针就绪（92 级=%s）、规则基线 %s 行" % (bool(T92), BASE0),
   bool(T92) and BASE0 > 0, "基线=%s 行" % BASE0)

# ===== Q1 读口必须能用（P-193） =====
s1, b1 = call("GET", "/admin/agentrules", None, token=ROOT)
rows = data((s1, b1)) or []
r0 = rows[0] if isinstance(rows, list) and rows else {}
ok("Q1a GET /admin/agentrules 必须 200（修前：任何人 500，日志 "
   "`no column found for name: version_min`）",
   s1 == 200, "http=%s msg=%s" % (s1, msg((s1, b1))))
ok("Q1b 列表每行要带得出版本区间与优先级（就是这三列把查询打崩的）",
   isinstance(rows, list) and len(rows) > 0
   and all(k in r0 for k in ("version_min", "version_max", "priority")),
   "行数=%s 字段=%s" % (len(rows) if isinstance(rows, list) else -1,
                        sorted(r0.keys())[:8]))
s2, b2 = call("GET", "/admin/agentrules/export", None, token=ROOT)
D2 = data((s2, b2)) or {}
ok("Q1c export 同样必须 200 且带 rules 数组（同一份裸 COALESCE）",
   s2 == 200 and isinstance(D2.get("rules"), list)
   and int(D2.get("count") or 0) == BASE0,
   "http=%s count=%s" % (s2, D2.get("count")))
h1, c1 = head("/admin/agentrules", ROOT)
h2, c2 = head("/admin/agentrules/export", ROOT)
ok("Q1d 两个读口都要 no-store（UA 黑白名单=防护姿态，不该留在中间缓存）",
   "no-store" in c1 and "no-store" in c2,
   "list=%r export=%r" % (c1[:24], c2[:24]))

# ===== Q2 权限与门槛 =====
sm, bm = call("GET", "/admin/agentrules", None, token=T92)
sa, ba = add("deny", "%s-perm" % NEW, PFX, tok=T92)
ok("Q2a 写侧要 settings.manage：92 级新增被拒（403）、库里没多行",
   sa == 403 and probe_rows() == "0", "http=%s 探针行=%s"
   % (sa, probe_rows()))
# 口径变更（2026-10-20）：本条原判「读侧 92 级可读」，那正是 P-338 修掉的
# **权限倒挂**（读比写松：class 91 能读全表、写却被 403）。产品已把读口与
# 写口对齐到同一把权限键 SETTINGS_MANAGE（class 99 起），故判据随之改成
# 「92 级读同样被拒」——写侧拒（Q2a）、读侧也拒，读写同档才叫对齐。
ok("Q2b 读侧与写侧同档（SETTINGS_MANAGE）：92 级读也被拒（403）",
   sm == 403, "92 级 list http=%s（旧行为 200＝读比写松，P-338 已修）" % sm)

# ===== Q3 导入必须逐条点名（P-195） =====
rules = [
    {"mode": "deny", "pattern": "%s-ok" % NEW, "note": "%s-ok" % PFX},
    {"mode": "maybe", "pattern": "%s-badmode" % NEW, "note": PFX},
    {"mode": "allow", "pattern": "([unclosed", "note": "%s-badregex" % PFX},
    {"mode": "allow", "pattern": "x" * 120, "note": "%s-badlen" % PFX},
]
s3, b3 = imp(rules)
D3 = data((s3, b3)) or {}
rej = D3.get("rejected") or []
ok("Q3a 1 合法 + 3 非法：invalid 必须是 3（修前 mode 写错都不计，只回 2）",
   s3 == 200 and int(D3.get("invalid") or -1) == 3,
   "http=%s 回包=%s" % (s3, json.dumps(
       {k: D3.get(k) for k in ("added", "invalid", "skipped", "duplicates")},
       ensure_ascii=False)))
ok("Q3b 要逐条点名「第几条、为什么」，而不是只给一个计数",
   len(rej) == 3 and all(("index" in r and "reason" in r) for r in rej)
   and any("mode" in str(r.get("reason")) for r in rej)
   and any("正则" in str(r.get("reason")) for r in rej),
   "rejected=%s" % json.dumps(rej, ensure_ascii=False)[:120])
ok("Q3c 只有合法那条落库（探针行=1）", probe_rows() == "1",
   "探针行=%s" % probe_rows())

# ===== Q4 导入不许留半包（P-194） =====
before = cnt_rules()
s4, b4 = imp([
    {"mode": "deny", "pattern": "%s-tx1" % NEW, "note": "%s-tx1" % PFX},
    {"mode": "deny", "pattern": "%s-tx2" % NEW, "note": NULNOTE},
    {"mode": "deny", "pattern": "%s-tx3" % NEW, "note": "%s-tx3" % PFX},
])
D4 = data((s4, b4)) or {}
nul = psql("SELECT count(*) FROM agent_rules WHERE pattern = '%s-tx2'" % NEW)
ok("Q4a 带 U+0000 的那条要被点名拒掉、整批不 500（修前：http=500 且 tx1 已落库、tx3 根本没跑=半包）",
   s4 == 200 and nul == "0"
   and any("空字节" in str(r.get("reason")) for r in (D4.get("rejected") or [])),
   "http=%s msg=%s NUL行=%s" % (s4, msg((s4, b4)), nul))
ok("Q4b 同批两条合法规则该进就进（原子性由事务保证，这里验「不误伤好条目」）",
   psql("SELECT count(*) FROM agent_rules WHERE pattern IN "
        "('%s-tx1','%s-tx3')" % (NEW, NEW)) == "2",
   "合法两条=%s 行数 %s→%s" % (psql(
       "SELECT count(*) FROM agent_rules WHERE pattern IN "
       "('%s-tx1','%s-tx3')" % (NEW, NEW)), before, cnt_rules()))

# ===== Q5 备注与 pattern 的形状（P-196） =====
s5, b5 = add("deny", "%s-longnote" % NEW, "y" * 2003)
ok("Q5a 备注超过 200 字必须拒（修前 2003 字原样入库）",
   s5 == 400 and "200" in msg((s5, b5)),
   "http=%s 库里长度=%s msg=%s" % (
       s5, psql("SELECT length(note)::text FROM agent_rules WHERE pattern = "
                "'%s-longnote'" % NEW) or "-", msg((s5, b5))))
CN = "迅雷" * 40  # 40 个汉字 = 120 字节，但只有 40 字
s6, b6 = add("deny", CN, "%s-cn" % PFX)
ok("Q6 pattern 长度要按**字符**判：40 字中文 UA 规则合法（修前按字节 120 "
   ">100 被自拒）", s6 == 200, "http=%s msg=%s" % (s6, msg((s6, b6))))

# ===== Q7 delete 形状 =====
s7, b7 = call("POST", "/admin/agentrules/delete", {"id": 99999999},
              token=ROOT)
ok("Q7a 删不存在的 id → 404（不是 500、也不是静默 200）",
   s7 == 404, "http=%s msg=%s" % (s7, msg((s7, b7))))
sid = psql("SELECT min(id)::text FROM agent_rules WHERE pattern LIKE '%s'"
           % (NEW + "%"))
s8, b8 = call("POST", "/admin/agentrules/delete", {"id": int(sid or 0)},
              token=ROOT)
ok("Q7b 删自己的探针规则 → 200 且行数少 1",
   s8 == 200 and sid.isdigit(), "http=%s id=%s" % (s8, sid))

# ===== Q9 收尾 =====
finalize()
BASE1 = int(cnt_rules() or 0)
ok("Q9a 规则表回到基线（%s→%s，探针残留 %s 行）" % (BASE0, BASE1,
                                                   probe_rows()),
   BASE1 == BASE0 and probe_rows() == "0",
   "行数=%s→%s 残留=%s" % (BASE0, BASE1, probe_rows()))
ok("Q9b 探针号已墓碑、审计只增不减",
   psql("SELECT count(*) FROM users WHERE status < 2 AND username LIKE "
        "'bp%%'") == "0",
   "live=%s" % psql("SELECT count(*) FROM users WHERE status < 2 AND "
                    "username LIKE 'bp%%'"))
AD = psql("SELECT string_agg(action, ',') FROM (SELECT DISTINCT action FROM "
          "audit_log WHERE action LIKE 'agentrule.%' ORDER BY action) t")
ok("Q9c 三类动作都要真的落到 audit_log（真实动作名是 `agentrule.add/import/"
   "del` 点号；判据里写成冒号模式会让这条永远不可能满足）：%s" % AD,
   "agentrule.add" in AD and "agentrule.import" in AD
   and "agentrule.del" in AD, "在册=%s" % AD)
summary()
