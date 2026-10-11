# -*- coding: utf-8 -*-
"""批十七闸门 AI：等级自动升降与晋升奖励链（worker 真跑）。

P-19 的实质是「后端实现完整、界面进不去」：`class_rules.promo_sparks`
常年 0，0072 的奖励发放与祝贺信因此空转。本闸门把这条链钉住——
设一档 `promo_sparks` → 造达标成员 → 真跑 `class_auto_adjust` →
逐出口断言（档位 / spark_balance / spark_ledger 幂等键 / 站内信），
并守住两条边界：staff 档不开放编辑、导航项在册（界面入口不能暗死）。

判据纪律：改的是**共享设定**（class_rules），收尾必须原值还原；
worker 与站点其它探针都在 60s tick 上，本闸门跑时别再并行改设定。
"""
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOT = login()
TIER = 3          # 探针用的目标档（阈值高，站内真实用户早已到位）
SPARKS = 70
CLEAN = []
GB = 2 ** 30


def q(sql_text):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    return p.stdout.strip()


def exec_sql(sql_text):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    if p.returncode:
        raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:220])


def job_tick(job, wait_s=200):
    """等「我改完数据之后 worker 真跑过一轮该任务」。

    cadence 是 60s，面板入队与定时触发抢同一把 advisory 锁：判据钉在
    `job_status.last_started_at` 晚于本次调用时刻，而不是自己那条 trigger
    行（否则撞锁会把良态竞争报成缺陷）。
    """
    t0 = q("SELECT to_char(now(), "
           "'YYYY-MM-DD\"T\"HH24:MI:SS.US')")
    call("POST", "/admin/jobs/run", {"job": job}, token=ROOT)
    end = time.time() + wait_s
    while time.time() < end:
        row = q("SELECT to_char(last_started_at, "
                "'YYYY-MM-DD\"T\"HH24:MI:SS.US')||'|'||"
                "COALESCE(last_ok::text,'-')||'|'||"
                "LEFT(COALESCE(last_result,''),60) FROM job_status "
                "WHERE job = '%s'" % job)
        if "|" in row:
            started, okv, res = row.split("|", 2)
            if started > t0:
                return okv == "true", res
        time.sleep(4)
    return False, "等待 worker 本轮执行超时"


print("== G0 前置：档位原值收口 + 达标成员 ==")
ORIG = dict(
    (k, v) for k, v in [
        ("name", q("SELECT name FROM class_rules WHERE class_id = %d" % TIER)),
        ("min_uploaded", q("SELECT min_uploaded::text FROM class_rules "
                           "WHERE class_id = %d" % TIER)),
        ("min_download_count", q("SELECT min_download_count::text "
                                 "FROM class_rules WHERE class_id = %d"
                                 % TIER)),
        ("min_seed_hours", q("SELECT min_seed_hours::text FROM class_rules "
                             "WHERE class_id = %d" % TIER)),
        ("min_account_age_days",
         q("SELECT min_account_age_days::text FROM class_rules "
           "WHERE class_id = %d" % TIER)),
        ("demotable", q("SELECT demotable::text FROM class_rules "
                        "WHERE class_id = %d" % TIER)),
        ("promo_sparks", q("SELECT promo_sparks::text FROM class_rules "
                           "WHERE class_id = %d" % TIER)),
    ])
ok("G0 档位 %d 原值可取（收尾要按它还原）" % TIER,
   all(v for k, v in ORIG.items() if k != "demotable"), str(ORIG))
# 探针档的 class_rules.name 与 user_classes.name 必须一致：PUT 一次写两列，
# 不一致的档做探针会在收尾时把展示名改成规则别名（还原不忠实）。
UC_NAME = q("SELECT name FROM user_classes WHERE id = %d" % TIER)
ok("G0 探针档两表同名（收尾能原样还原）", UC_NAME == ORIG["name"],
   "uc=%s cr=%s" % (UC_NAME, ORIG["name"]))
# 改 promo_sparks 是**共享设定**：窗口里 worker 一跑，所有「已达标但还没
# 升到探针档」的真实成员都会连带领到这份奖励。数量不为 0 就不该动手。
SAFE = (
    "WITH agg AS (SELECT user_id, "
    "count(*) FILTER (WHERE completed_at IS NOT NULL) dl, "
    "COALESCE(sum(seeded_seconds), 0) / 3600 sh FROM snatches "
    "GROUP BY user_id) SELECT count(*) FROM users u "
    "LEFT JOIN agg a ON a.user_id = u.id WHERE u.class_id < %d "
    "AND u.status <> 2 AND u.uploaded >= %s "
    "AND COALESCE(a.dl, 0) >= %s AND COALESCE(a.sh, 0) >= %s "
    "AND EXTRACT(DAY FROM now() - u.created_at)::bigint >= %s"
    % (TIER, ORIG["min_uploaded"], ORIG["min_download_count"],
       ORIG["min_seed_hours"], ORIG["min_account_age_days"]))
OTHERS = q(SAFE)
ok("G0 窗口安全：没有其它已达探针档的成员（否则改 sparks 会给真人发奖）",
   OTHERS == "0", "others=%s" % OTHERS)
CLEAN.append("__restore_tier__")

user = "ai%d" % (int(time.time()) % 100000)
pw = "Gate#123456"
call("POST", "/admin/adduser", {"username": user, "password": pw,
                                "email": "%s@ai.local" % user}, token=ROOT)
UID = int(q("SELECT id::text FROM users WHERE username = '%s'" % user))
CLEAN.append("UPDATE users SET status = 2 WHERE id = %d" % UID)
t0 = login(user, pw)
call("POST", "/me/password/change",
     {"old_password": pw, "new_password": pw + "x"}, token=t0)

# 达标档位 3：上传量/做种小时/账号天数都越过阈值（阈值本身一律不动）
exec_sql("UPDATE users SET uploaded = %d, "
         "created_at = now() - interval '70 day' WHERE id = %d"
         % (140 * GB, UID))
exec_sql("INSERT INTO snatches (user_id, torrent_id, downloaded, "
         "seeded_seconds, completed_at, progress, last_seen_at) "
         "SELECT %d, id, 4096, 570 * 3600, now() - interval '70 day', "
         "100, now() FROM torrents WHERE approval_status = 1 "
         "ORDER BY id DESC LIMIT 1" % UID)
CLEAN.append("DELETE FROM snatches WHERE user_id = %d" % UID)
CLEAN.append("DELETE FROM messages WHERE receiver_id = %d" % UID)
CLEAN.append("DELETE FROM spark_ledger WHERE user_id = %d" % UID)

def tier_payload(src):
    """字符串读数 → PUT body 的类型（bool / int / str 混在一列里）"""
    out = {}
    for k, v in src.items():
        out[k] = (v == "true") if k == "demotable" else (
            int(v) if v.lstrip("-").isdigit() else v)
    return out


# 只动 promo_sparks（走后台端点本身，顺带验写口能用）
body = tier_payload(ORIG)
body["promo_sparks"] = SPARKS
s, r = call("PUT", "/admin/classes/%d" % TIER, body, token=ROOT)
ok("G0 后台写口可改 promo_sparks（P-19 接的就是这个口）",
   s == 200 and r.get("code") == 0, "http=%s %s" % (s, str(r)[:90]))
ok("G0 写库生效",
   q("SELECT promo_sparks::text FROM class_rules WHERE class_id = %d"
     % TIER) == str(SPARKS),
   q("SELECT promo_sparks::text FROM class_rules WHERE class_id = %d" % TIER))

print("== G1 晋升：档位 / 余额 / 流水幂等键 / 祝贺信 四出口 ==")
jok, jres = job_tick("class_auto_adjust")
ok("G1 worker 真跑过一轮 class_auto_adjust", jok, "result=%s" % jres)
ok("G1 达标成员升到目标档",
   q("SELECT class_id::text FROM users WHERE id = %d" % UID) == str(TIER),
   "class=%s" % q("SELECT class_id::text FROM users WHERE id = %d" % UID))
ok("G1 奖励按档位发放（余额 +%d）" % SPARKS,
   q("SELECT spark_balance::text FROM users WHERE id = %d" % UID)
   == str(SPARKS),
   "balance=%s" % q("SELECT spark_balance::text FROM users WHERE id = %d"
                    % UID))
ok("G1 流水落 class_promotion 且幂等键锚在新档",
   q("SELECT count(*) FROM spark_ledger WHERE user_id = %d AND "
     "kind = 'class_promotion' AND idempotency_key = "
     "'class_promo:%d:%d'" % (UID, UID, TIER)) == "1",
   q("SELECT COALESCE(string_agg(amount||'/'||idempotency_key, ','), '无') "
     "FROM spark_ledger WHERE user_id = %d" % UID))
ok("G1 祝贺信发出",
   q("SELECT count(*) FROM messages WHERE receiver_id = %d AND "
     "subject = '等级晋升祝贺'" % UID) == "1",
   q("SELECT COALESCE(string_agg(subject, ','), '无') FROM messages "
     "WHERE receiver_id = %d" % UID))

print("== G2 幂等：再跑一轮不重复入账 ==")
jok, _ = job_tick("class_auto_adjust")
ok("G2 二轮跑完", jok, "")
ok("G2 流水仍只有 1 条、余额不变",
   q("SELECT count(*) FROM spark_ledger WHERE user_id = %d AND "
     "kind = 'class_promotion'" % UID) == "1"
   and q("SELECT spark_balance::text FROM users WHERE id = %d" % UID)
   == str(SPARKS),
   "ledger=%s balance=%s"
   % (q("SELECT count(*) FROM spark_ledger WHERE user_id = %d AND "
        "kind = 'class_promotion'" % UID),
      q("SELECT spark_balance::text FROM users WHERE id = %d" % UID)))

print("== G3 降级：demotable 为真的档要能降回去 ==")
exec_sql("UPDATE users SET uploaded = 0 WHERE id = %d" % UID)
jok, _ = job_tick("class_auto_adjust")
cls = q("SELECT class_id::text FROM users WHERE id = %d" % UID)
ok("G3 越档成员被降回仍满足的最高档（1）", jok and cls == "1",
   "class=%s" % cls)
ok("G3 降级不额外发奖",
   q("SELECT count(*) FROM spark_ledger WHERE user_id = %d AND "
     "kind = 'class_promotion'" % UID) == "1",
   "ledger=%s" % q("SELECT count(*) FROM spark_ledger WHERE user_id = %d"
                    % UID))

print("== G4 边界：staff/无规则档拒绝 + 全集与展示名 ==")
STAFF91 = q("SELECT name FROM user_classes WHERE id = 91")
s, r = call("PUT", "/admin/classes/91", body, token=ROOT)
ok("G4 staff 档不开放后台编辑", s == 400 and r.get("code") != 0,
   "http=%s %s" % (s, str(r)[:80]))
ok("G4 staff 档未被动过（拒绝要真拒绝，不是先写后报错）",
   q("SELECT name FROM user_classes WHERE id = 91") == STAFF91,
   "%s -> %s" % (STAFF91,
                 q("SELECT name FROM user_classes WHERE id = 91")))
s, r = call("PUT", "/admin/classes/20", body, token=ROOT)
ok("G4 无升降级规则的档位（贵宾 20）PUT 被拒",
   s == 400 and r.get("code") != 0, "http=%s %s" % (s, str(r)[:90]))
s, r = call("PUT", "/admin/classes/3", dict(body, promo_sparks=-5),
            token=ROOT)
ok("G4 负奖励被拒（不能拿负数扣别人的火花）",
   s == 400 and r.get("code") == 1002, "http=%s %s" % (s, str(r)[:80]))
CN = "晋升验证档位名称十二字"
s, r = call("PUT", "/admin/classes/%d" % TIER, dict(body, name=CN),
            token=ROOT)
ok("G4 档名按字符限长（12 汉字=36 字节，按字节误拒就是中文站自断）",
   s == 200 and r.get("code") == 0, "http=%s %s" % (s, str(r)[:90]))
ok("G4 改名两表同步（成员档案显示的是 user_classes 那列）",
   q("SELECT name FROM user_classes WHERE id = %d" % TIER) == CN
   and q("SELECT name FROM class_rules WHERE class_id = %d" % TIER) == CN,
   "uc=%s cr=%s" % (q("SELECT name FROM user_classes WHERE id = %d" % TIER),
                    q("SELECT name FROM class_rules WHERE class_id = %d"
                      % TIER)))
s, r = call("GET", "/admin/classes", None, token=ROOT)
rows = {}
for x in r.get("data") or []:
    rows[int(x["class_id"])] = x
ok("G4 面板全集含贵宾/职务档（只读 class_rules 时这 8 档根本不出现）",
   all(k in rows for k in (20, 91, 92, 93, 94, 95, 98, 99)),
   "ids=%s" % ",".join(str(k) for k in sorted(rows)))
probe20 = rows.get(20) or {}
ok("G4 无规则档阈值为 null（界面据此显示「—」，0 会被读成零门槛）",
   bool(rows.get(20)) and probe20.get("min_uploaded") is None
   and probe20.get("promo_sparks") is None, str(probe20)[:110])
ok("G4 档位名取成员看到的展示名（user_classes.name）",
   (rows.get(1) or {}).get("name")
   == q("SELECT name FROM user_classes WHERE id = 1"),
   "api=%s uc=%s cr=%s" % ((rows.get(1) or {}).get("name"),
                           q("SELECT name FROM user_classes WHERE id = 1"),
                           q("SELECT name FROM class_rules "
                             "WHERE class_id = 1")))
ok("G4 管理台导航项在册（DB 里要有这一行）",
   q("SELECT count(*) FROM staff_panel_entries WHERE tab_key = 'classes'")
   == "1",
   q("SELECT count(*) FROM staff_panel_entries "
     "WHERE tab_key = 'classes'"))
# P-25：在册 ≠ 可见。导航判据 module_on_sql 旧版只认 NULL，迁移手写
# module_key='' 的行（0342 classes、0339 trumps）接口不报错、DB 有行、
# 一致性守卫全绿，但管理台导航里就是不出现。这里按渲染口径断言。
s, r = call("GET", "/admin/staffpanel", None, token=ROOT)
nav = {x.get("tab_key") for x in (r.get("data") or {}).get("entries") or []}
ok("G4 等级规则项出现在导航接口里（在册不等于可见）",
   "classes" in nav, "http=%s nav=%d 项" % (s, len(nav)))
ok("G4 举报处理（trumps）同坑同修（0339 起就没在导航出现过）",
   "trumps" in nav, "nav=%s" % ",".join(sorted(x for x in nav if x)[:6]))

print("== G6 成员侧等级页：贵宾不能被画进自动晋升阶梯 ==")
mt = login(user, pw + "x")
s, r = call("GET", "/classes", None, token=mt)
cl = {int(x["id"]): x for x in (r.get("data") or {}).get("classes") or []}
ok("G6 公开接口带 has_rule 标记（前端据此把阶梯与授予档分开）",
   bool(cl) and all("has_rule" in v for v in cl.values()),
   "http=%s ids=%s" % (s, ",".join(str(k) for k in sorted(cl))))
ok("G6 成长档 0-12 全为 has_rule=true（这些才是统计晋升线）",
   all(cl.get(i, {}).get("has_rule") is True for i in range(13)),
   ",".join("%d=%s" % (i, cl.get(i, {}).get("has_rule"))
            for i in range(13)))
ok("G6 贵宾（20）标为无规则档（四维是 COALESCE 的 0，展示成门槛会误导）",
   cl.get(20, {}).get("has_rule") is False, str(cl.get(20))[:110])

print("== G5 收尾：原值还原 + 残料清零 ==")
for stmt in reversed(CLEAN):
    if stmt == "__restore_tier__":
        call("PUT", "/admin/classes/%d" % TIER, tier_payload(ORIG),
                 token=ROOT)
        continue
    exec_sql(stmt)
ok("G5 档位 promo_sparks 已还原",
   q("SELECT promo_sparks::text FROM class_rules WHERE class_id = %d" % TIER)
   == ORIG["promo_sparks"],
   q("SELECT promo_sparks::text FROM class_rules WHERE class_id = %d" % TIER))
ok("G5 探针成员已墓碑化",
   q("SELECT count(*) FROM users WHERE email LIKE '%@ai.local' "
     "AND status < 2") == "0",
   q("SELECT count(*) FROM users WHERE email LIKE '%@ai.local' "
     "AND status < 2"))
summary()
