# -*- coding: utf-8 -*-
"""闸门 BW：字典式后台 CRUD 四族（勋章 / 稀有度 / 促销档位 / 考核类型）。
零覆盖路由：/admin/medals、/admin/medal-rarities、/admin/promo-kinds、
/admin/jixiao-types 共 12 条（`grep -l` 在常驻 pt_audit_* 里 0 命中），
而它们是「一次把同一把尺写四遍」的高危面——本仓已经因此在四处漏过修复。

两条产品洞（红证取自已部署的未修版 `api:aa1014c`）：
  - **P-219 护栏 fail-open**：四族里 6 处护栏查询写成 `.unwrap_or(false)` /
    `.unwrap_or(0)`，3 处列表读口写成 `to_value(rows).unwrap_or_default()`。
    语义：**查询/序列化一失败，护栏就当「没重复、没引用」，破坏性操作照做**。
    而护栏守的是真数据——代码注释自己写着「有勋章在用就不许删，否则勋章会变成
    未收录外观」「历史购买引用在则改为停用不删」。
  - **P-220 审计无对象**：`medal_rarity.add/update/del` 与
    `promo_kind.add/update/disable/delete` 全是 `audit(actor, action, None)` ⇒
    「谁建/改/删/停用了哪个稀有度、哪个档位」读不出来（P-202/204/208/212/216
    之后同族第七、八处）。这两类字典的主键是**字符串键**（value/kind），所以
    `ref` 放不进去，更要靠 extra 把 value/kind/label 留住。

判据纪律：正控与静态各打一遍——护栏的 fail-open 造不出「查询失败」，所以
W4a/W4b 是**静态判据**（四族文件里不许再出现这三种吞错写法、审计不许有
`, None)`），HEAD 侧的命中数当红证写进读数；行为侧（W1/W2/W3）用探针
稀有度 + 探针勋章真打「在用不许删」「删除留 label」「重复值被拦且不留行」。
探针只动自己造的行，收尾按前缀清并复算基线。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bw_dictionaries.py
"""
import io
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
RV = "bwr%d" % TAG
RL = "布偶稀有度%d" % TAG
KD = "bwk%d" % TAG
KL = "布偶档位%d" % TAG
MID = [0]
REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                    "apps", "api", "src", "admin_p3_http")
FILES = ("medals.rs", "medal_rarities.rs", "promo_kinds.rs",
         "jixiao_types.rs")


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:48], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def src(f):
    return io.open(os.path.join(REPO, f), encoding="utf-8").read()


def cleanup():
    if MID[0]:
        psql("DELETE FROM medals WHERE id = %d" % MID[0])
    psql("DELETE FROM medal_rarities WHERE value = '%s'" % RV)
    psql("DELETE FROM promo_kinds WHERE kind = '%s'" % KD)


import atexit  # noqa: E402

atexit.register(cleanup)

T0 = psql("SELECT to_char(now(), 'YYYY-MM-DD HH24:MI:SS.US')")
RR0 = int(psql("SELECT count(*) FROM medal_rarities") or -1)
KK0 = int(psql("SELECT count(*) FROM promo_kinds") or -1)
MM0 = int(psql("SELECT count(*) FROM medals") or -1)
ok("W0a 基线：稀有度 %d、档位 %d、勋章 %d 行" % (RR0, KK0, MM0),
   RR0 >= 0 and KK0 >= 0 and MM0 >= 0, "RR=%s KK=%s MM=%s"
   % (RR0, KK0, MM0))


def audit_rows(prefix):
    d = (call("GET", "/admin/audit?q=%s&limit=60" % prefix, None,
              token=ROOT)[1] or {}).get("data") or []
    return [r for r in d
            if str(r.get("created_at") or "").replace("T", " ")[:19]
            >= T0[:19]]


def detail_ok(rows, key):
    return bool(rows) and all(key in str(r.get("detail")) for r in rows)


# ===== W1 稀有度：建 / 重复 / 改名 =====
s, b = call("POST", "/admin/medal-rarities", {"value": RV, "label": RL,
                                              "tone": "gold", "sort": 3},
            token=ROOT)
ok("W1a 建探针稀有度（中文显示名）应成功：http=%s msg=%s"
   % (s, str((b or {}).get("message", ""))[:24]), s == 200, "http=%s" % s)
s2, b2 = call("POST", "/admin/medal-rarities", {"value": RV, "label": "再建一次"},
              token=ROOT)
cnt = psql("SELECT count(*) FROM medal_rarities WHERE value = '%s'" % RV)
ok("W1b 正控：重复 value 要被拦且**库里不许多一行**（护栏查询失败时旧写法会"
   "当「不重复」放行）：http=%s 行数=%s" % (s2, cnt),
   s2 == 400 and cnt == "1", "http=%s 行数=%s" % (s2, cnt))
add_a = audit_rows("medal_rarity.add")
ok("W1c 稀有度新建的审计要带 value（ref 装不下字符串主键，extra 就是唯一出处）："
   "detail=%s" % str([r.get("detail") for r in add_a][:1])[:52],
   detail_ok(add_a, RV), "本轮 add=%d 条" % len(add_a))
s3, _ = call("PUT", "/admin/medal-rarities/%s" % RV,
             {"value": RV, "label": RL + "改"}, token=ROOT)
upd_a = audit_rows("medal_rarity.update")
ok("W3a 改稀有度：http=%s，update 审计要带 value=%s（detail=%s）"
   % (s3, RV, str([r.get("detail") for r in upd_a][:1])[:40]),
   s3 == 200 and detail_ok(upd_a, RV), "update=%d 条" % len(upd_a))

# ===== W2 在用不许删（真打一遍护栏） =====
s4, b4 = call("POST", "/admin/medals", {"name": "BW 探针勋章%d" % TAG,
                                        "rarity": RV, "price": 0},
              token=ROOT)
MID[0] = int(((b4 or {}).get("data") or {}).get("id") or 0)
s5, b5 = call("DELETE", "/admin/medal-rarities/%s" % RV, None, token=ROOT)
alive = psql("SELECT count(*) FROM medal_rarities WHERE value = '%s'" % RV)
ok("W2a 稀有度被 %s 枚勋章引用时删除必须被拦（http=%s，行仍在=%s，引用数要写进"
   "文案：%s）" % (psql("SELECT count(*) FROM medals WHERE rarity = '%s'"
                        % RV), s5, alive,
                   str((b5 or {}).get("message", ""))[:26]),
   s5 == 400 and alive == "1" and "1" in str((b5 or {}).get("message", "")),
   "http=%s 行=%s" % (s5, alive))
psql("DELETE FROM medals WHERE id = %d" % MID[0])
MID[0] = 0
s6, _ = call("DELETE", "/admin/medal-rarities/%s" % RV, None, token=ROOT)
del_a = audit_rows("medal_rarity.del")
ok("W2b 没人用了才真删（http=%s），且删除审计要留着 value 与 label（行没了以后"
   "这是唯一出处）：detail=%s" % (s6, str([r.get("detail") for r in del_a][:1])
                                 [:56]),
   s6 == 200 and psql("SELECT count(*) FROM medal_rarities WHERE value = '%s'"
                      % RV) == "0" and detail_ok(del_a, RL + "改"),
   "http=%s del 审计=%d 条" % (s6, len(del_a)))

# ===== W3 促销档位 =====
s7, _ = call("POST", "/admin/promo-kinds", {"kind": KD, "label_zh": KL,
                                           "effect": "x2",
                                           "sort_order": 400}, token=ROOT)
ka = audit_rows("promo_kind.add")
ok("W3b 建档位 http=%s，add 审计要带 kind=%s（detail=%s）"
   % (s7, KD, str([r.get("detail") for r in ka][:1])[:40]),
   s7 == 200 and detail_ok(ka, KD), "add=%d 条" % len(ka))
s8, _ = call("DELETE", "/admin/promo-kinds/%s" % KD, None, token=ROOT)
kd_a = audit_rows("promo_kind.delete")
ok("W3c 无历史购买的档位真删（http=%s），delete 审计带 kind；库里行已清=%s"
   % (s8, psql("SELECT count(*) FROM promo_kinds WHERE kind = '%s'" % KD)),
   s8 == 200 and psql("SELECT count(*) FROM promo_kinds WHERE kind = '%s'"
                      % KD) == "0" and detail_ok(kd_a, KD),
   "http=%s del=%d 条" % (s8, len(kd_a)))

# ===== W4 静态：fail-open 的三种写法与 None 审计 =====
bad_guard, bad_audit = [], []
for f in FILES:
    t = src(f)
    for pat in (".unwrap_or(false);", ".unwrap_or(0);",
                "unwrap_or_default())", ", None)"):
        if pat in t:
            (bad_audit if pat == ", None)" else bad_guard).append(
                "%s:%s" % (f, pat.strip(".;")))
ok("W4a 静态：四族文件里护栏/读口不许有 fail-open 写法（unwrap_or(false)/"
   "(0)/default）——命中=%s" % bad_guard, not bad_guard,
   "命中=%d 处" % len(bad_guard))
ok("W4b 静态：审计调用不许再传 None 而无对象标识——命中=%s" % bad_audit,
   not bad_audit, "命中=%d 处" % len(bad_audit))

# ===== W9 收尾 =====
cleanup()
RR1 = int(psql("SELECT count(*) FROM medal_rarities") or -1)
KK1 = int(psql("SELECT count(*) FROM promo_kinds") or -1)
MM1 = int(psql("SELECT count(*) FROM medals") or -1)
ok("W9a 探针行全清，三张表回基线：稀有度 %d→%d、档位 %d→%d、勋章 %d→%d"
   % (RR0, RR1, KK0, KK1, MM0, MM1),
   RR1 == RR0 and KK1 == KK0 and MM1 == MM0,
   "RR=%s KK=%s MM=%s" % (RR1, KK1, MM1))
ok("W9b 库里没有 bw 前缀的探针残留",
   psql("SELECT count(*) FROM medal_rarities WHERE value LIKE 'bw%%'") == "0"
   and psql("SELECT count(*) FROM promo_kinds WHERE kind LIKE 'bw%%'") == "0",
   "残=%s / %s" % (psql("SELECT count(*) FROM medal_rarities WHERE value "
                      "LIKE 'bw%%'"),
                   psql("SELECT count(*) FROM promo_kinds WHERE kind "
                        "LIKE 'bw%%'")))
summary()
