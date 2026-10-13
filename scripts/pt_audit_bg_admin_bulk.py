# -*- coding: utf-8 -*-
"""闸门 BG：后台批量处置族（H&R 赦免三口 / 改档 / 手工补量 / 发薪合计）。

批四十三的选题底稿是「HEAD 路由声明 vs 脚本实际打到」的差集：`/admin/*`
一族里有 28 条端点在常驻闸门里找不到字面量，其中真正会**改多人账**的正是
这一族。判据一律「真发请求 → 真读库」，红要能报出服务端原话。

  P-165（P0）赦免口无等级/本人护栏。成员侧同一动作 `/me/hr/pardon` 要付站点
        设定 `cancel_hr` 的火花（实测本站 10000），而 `/admin/hr/pardon`、
        `/admin/hr/pardon/batch`、`/admin/hr/batch-pardon` 三口谁都能按 ⇒
        有 `hr.pardon` 权（class ≥ 90）的职员可**零成本赦免自己**那条违规，
        也能赦免站长。
  P-166（P1）三口状态谓词不同：只有 `/admin/hr/batch-pardon` 写
        `status IN ('violated','open')`。`open` 是「还没到期」的进行中义务，
        worker 的判定线是 `WHERE status='open' AND deadline<now()` —— 把它抹成
        pardoned，这条线永不命中，义务凭空消失，而 `hr_violations` 里连
        「欠多少做种时长」的依据都没有。
  P-167（P1）三口都强制「必须填理由」，然后一个都不落库（0285 已建好
        `audit_log.ref->>'reason'` 槽位）⇒ 不可追责的处分动作。
  P-168（P2）`/admin/amountupload` 无等级护栏（同族 `/admin/users/adjust` 有
        ensure_outranks）；幂等键借 `traffic_ledger.reason` 埋键 + 先
        `SELECT EXISTS` 再 INSERT ⇒ 同键并发能双双通过（实测加出两份量）。
  P-170（P3）`/admin/jixiao/payroll` 的 total 取自 LIMIT 200 之后的一页。
  P-171（P2）改档口不校验档位是否在字典里：`user_classes` 是不连续的
        （没有 90 / 13..19 / 96 / 97），`class_id=90` 撞 FK ⇒ 后台 500；
        单人改档与批量改档两处同病。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bg_admin_bulk.py
"""
import json, os, re, subprocess, sys, threading, time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
SENT = "BGSENT%d" % TAG


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


def blob(r):
    return json.dumps(r, ensure_ascii=False) if r is not None else ""


def data(r):
    if isinstance(r, tuple):
        r = r[1]
    return (r or {}).get("data") or {}


def post(p, b=None, tok=None):
    return call("POST", p, b, token=tok or ROOT)


# ---- 两个探针：一个职员（92，有 hr.pardon），一个普通成员（赦免对象） ----
INV = []


def mkprobe(prefix, cls):
    codes = data(post("/admin/invites", {"user_id": 1, "count": 1})
                 ).get("codes") or []
    cap = data(call("GET", "/auth/captcha", token=ROOT))
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if not codes or len(nums) < 2:
        return 0, None, "发码/验证码没准备好"
    # 记下这张码的 id：收尾只作废自己发的那两张，别碰别人的未用码
    INV.append(codes[0]["id"])
    name = "%s%d" % (prefix, TAG * 100 + int(time.time()) % 100)
    rg = post("/auth/register", {
        "username": name, "email": "%s@bg.local" % name,
        "password": "password123", "invite_code": codes[0]["code"],
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    tok = data(rg).get("token") or data(
        post("/auth/login", {"username": name,
                             "password": "password123"})).get("token")
    if not tok:
        return 0, None, "注册/登录 http=%s %s" % (rg[0], blob(rg[1])[:100])
    uid = int(psql("SELECT id FROM users WHERE username = '%s'" % name))
    if cls:
        s, r = post("/admin/users/class", {"user_id": uid, "class_id": cls})
        if s != 200:
            return uid, tok, "提档 %s 失败 http=%s" % (cls, s)
        tok = data(post("/auth/login", {"username": name,
                                       "password": "password123"})
                   ).get("token") or tok
    return uid, tok, ""


clear_register_rl()
SU, STOK, W1 = mkprobe("bgso", 92)      # 职员（staff only）
MV, MTOK, W2 = mkprobe("bgmv", None)    # 普通成员
BO, BTOK, W3 = mkprobe("bgbo", 95)      # 上级主管：92 赦不得它
ok("S0c 上级探针就绪（uid=%s class=%s）%s"
   % (BO, psql("SELECT class_id::text FROM users WHERE id = %d" % BO), W3),
   BO > 0 and BTOK, W3 or "没有上级样本 ⇒ S1b 不承重")
ok("S0a 职员探针就绪（uid=%s class=%s）%s"
   % (SU, psql("SELECT class_id::text FROM users WHERE id = %d" % SU), W1),
   SU > 0 and STOK, W1 or "职员探针没就绪 ⇒ S1/S2/S3 不承重")
ok("S0b 成员探针就绪（uid=%s class=%s）%s"
   % (MV, psql("SELECT class_id::text FROM users WHERE id = %d" % MV), W2),
   MV > 0 and MTOK, W2 or "成员探针没就绪 ⇒ 正控不承重")
TID1 = psql("SELECT id FROM torrents WHERE approval_status = 1 ORDER BY id "
            "LIMIT 1")
TID2 = psql("SELECT id FROM torrents WHERE approval_status = 1 ORDER BY id "
            "DESC LIMIT 1")


def mkrow(uid, tid, status, dl):
    psql("DELETE FROM hr_snapshots WHERE user_id = %d AND torrent_id = %s"
         % (uid, tid))
    psql("INSERT INTO hr_snapshots (user_id, torrent_id, required_seconds, "
         "deadline, seeded_seconds, status) VALUES (%d, %s, 3600, %s, 0, "
         "'%s')" % (uid, tid, dl, status))


def stof(uid, tid):
    return psql("SELECT status FROM hr_snapshots WHERE user_id = %d AND "
                "torrent_id = %s" % (uid, tid))


mkrow(SU, TID1, "violated", "now() - interval '2 day'")
mkrow(SU, TID2, "open", "now() + interval '7 day'")
mkrow(MV, TID1, "violated", "now() - interval '2 day'")
mkrow(MV, TID2, "open", "now() + interval '7 day'")
B0 = {"live": cnt("users", "status < 2"),
      "claims": cnt("jixiao_claims"), "viol": cnt("hr_violations"),
      "root_up": psql("SELECT uploaded::text FROM users WHERE id = 1")}
# 本轮之后新增的流水都要能指认出来（站长账号被未修版真加过量）
TR0 = psql("SELECT coalesce(max(id),0)::text FROM traffic_ledger")
print("   基线：%s；价格 cancel_hr=%s" % (
    B0, psql("SELECT COALESCE(NULLIF((SELECT value FROM site_settings "
             "WHERE name='cancel_hr'),''),'20000')")))

# ============ S1 赦免护栏（P-165） ============
# 每条判据前都重新铺语料：未修版会真的赦掉记录，若不重铺，后面的判据就在
# 「无待赦免的 H&R 违规」上连坐（第一跑正是这样，S1e 假红成 400）。
mkrow(SU, TID1, "violated", "now() - interval '2 day'")
s, r = post("/admin/hr/pardon", {"user_id": SU, "torrent_id": int(TID1),
                                 "note": SENT + " self"}, tok=STOK)
ok("S1a 职员赦免**自己**的 H&R 必须被拒（403）：成员侧同动作要付 cancel_hr "
   "火花，管理口不能是零成本免罪通道 —— 实测 http=%s 状态=%s" % (
       s, stof(SU, TID1)), s == 403 and stof(SU, TID1) == "violated",
   "%s %s" % (s, blob(r)[:110]))
mkrow(BO, TID1, "violated", "now() - interval '2 day'")
s2, r2 = post("/admin/hr/pardon", {"user_id": BO, "torrent_id": int(TID1),
                                   "note": SENT + " upper"}, tok=STOK)
ok("S1b 职员赦免上级（92 < 95）必须被拒：http=%s 状态=%s（原话 %s）"
   % (s2, stof(BO, TID1), blob(r2)[:60]),
   s2 == 403 and stof(BO, TID1) == "violated", "%s %s" % (s2,
                                                          blob(r2)[:90]))
mkrow(SU, TID1, "violated", "now() - interval '2 day'")
mkrow(MV, TID1, "violated", "now() - interval '2 day'")
s3, r3 = post("/admin/hr/pardon/batch", {"items": [
    {"user_id": SU, "torrent_id": int(TID1)},
    {"user_id": MV, "torrent_id": int(TID1)}], "note": SENT + " bself"},
              tok=STOK)
ok("S1c 批量口整批先过护栏：含自己就一条都不动（成员那条也不许被赦）",
   s3 == 403 and stof(SU, TID1) == "violated"
   and stof(MV, TID1) == "violated",
   "%s 自己=%s 成员=%s" % (s3, stof(SU, TID1), stof(MV, TID1)))
s4, r4 = post("/admin/hr/batch-pardon", {"records": [
    {"user_id": SU, "torrent_id": int(TID1)}], "note": SENT + " p3self"},
              tok=STOK)
ok("S1d 第三个批量口（/admin/hr/batch-pardon）同样挡得住自赦",
   s4 == 403 and stof(SU, TID1) == "violated",
   "%s %s 状态=%s" % (s4, blob(r4)[:60], stof(SU, TID1)))
# 目标用户根本不存在：护栏的 NotFound 让单发口从「400 无待赦免」变「404 没有
# 这个人」——这更准（两种情形的处置动作不同），钉住别退回 500 也别混成一类
sb, rb = post("/admin/hr/pardon", {"user_id": 99999999,
                                   "torrent_id": int(TID1),
                                   "note": SENT + " ghost"}, tok=STOK)
ok("S1f 不存在的目标用户 → 404（不是 500，也不许和「无待赦免」混成 400）",
   sb == 404, "%s %s" % (sb, blob(rb)[:80]))
mkrow(MV, TID1, "violated", "now() - interval '2 day'")
s5, r5 = post("/admin/hr/pardon", {"user_id": MV, "torrent_id": int(TID1),
                                   "note": SENT + " legit"}, tok=STOK)
ok("S1e 正控：职员赦免**下级成员**照常成功（200 + 状态翻转），护栏不是把"
   "功能打死", s5 == 200 and stof(MV, TID1) == "pardoned",
   "%s %s %s" % (s5, stof(MV, TID1), blob(r5)[:70]))

# ============ S2 只赦 violated（P-166） ============
mkrow(MV, TID2, "open", "now() + interval '7 day'")
s, r = post("/admin/hr/batch-pardon", {"records": [
    {"user_id": MV, "torrent_id": int(TID2)}], "note": SENT + " open"},
              tok=STOK)
ok("S2a p3 批量口不许赦「还没到期（open）」的进行中义务（旧行为 pardoned:1 "
   "⇒ worker 的 violated 判定线永远不再命中）：http=%s 状态=%s" % (
       s, stof(MV, TID2)),
   s == 200 and stof(MV, TID2) == "open"
   and int((data(r) or {}).get("pardoned", 0) or 0) == 0, blob(r)[:110])
mkrow(MV, TID2, "open", "now() + interval '7 day'")
s2, r2 = post("/admin/hr/pardon/batch", {"items": [
    {"user_id": MV, "torrent_id": int(TID2)}], "note": SENT + " open2"},
              tok=STOK)
ok("S2b gaps 批量口对 open 记录计入 skipped（三口谓词现在必须一致）\u2014\u2014"
   "注意目标必须是**下级**：换成职员自己就先撞 S1a 那条自赦护栏",
   s2 == 200 and stof(MV, TID2) == "open"
   and int((data(r2) or {}).get("skipped_count", -1) or -1) == 1,
   blob(r2)[:120])
s3, r3 = post("/admin/hr/pardon", {"user_id": MV, "torrent_id": int(TID2),
                                   "note": SENT}, tok=STOK)
ok("S2c 单发口对 open 记录仍是 400「无待赦免」（老行为不许被我改坏）",
   s3 == 400 and "无待赦免" in blob(r3), "%s %s" % (s3, blob(r3)[:90]))
ok("S2d 两个 open 样本都没被抹（hr_violations 也没多出空依据行）",
   stof(MV, TID2) == "open" and stof(SU, TID2) == "open"
   and cnt("hr_violations", "user_id IN (%d,%d)" % (MV, SU)) == "0",
   "viol=%s" % cnt("hr_violations", "user_id IN (%d,%d)" % (MV, SU)))

# ============ S3 理由落库（P-167） ============
time.sleep(1.0)
NREASON = int(cnt("audit_log", "action = 'hr.pardon' AND ref->>'reason' "
                  "LIKE '%s%%'" % SENT))
NTGT = int(cnt("audit_log", "action = 'hr.pardon' AND ref->>'target_user' "
               "= '%d'" % MV))
NML = int(cnt("user_modify_logs", "uid = %d AND content LIKE '%%%s%%'"
              % (MV, SENT)))
ok("S3a 赦免理由必须进审计（audit_log.ref->>'reason'，0285 建的槽位）："
   "命中 %s 行" % NREASON, NREASON >= 1, "reason 行=%s" % NREASON)
ok("S3b 审计行的 target_user 指向被赦的人（不是操作者自己）", NTGT >= 1,
   "target 行=%s" % NTGT)
ok("S3c 被赦的人有 user_modify_logs 留痕（面板「修改记录」看得见理由）",
   NML >= 1, "留痕=%s" % NML)
mkrow(MV, TID1, "violated", "now() - interval '2 day'")
mkrow(MV, TID2, "violated", "now() - interval '1 day'")
s, r = post("/admin/hr/pardon/batch", {"items": [
    {"user_id": MV, "torrent_id": int(TID1)},
    {"user_id": MV, "torrent_id": int(TID2)}], "note": SENT + " two"},
              tok=STOK)
NBATCH = int(cnt("audit_log", "action = 'hr.pardon_batch' AND "
                 "ref->>'reason' LIKE '%%%s two%%'" % SENT))
ok("S3d 批量 N 条 ⇒ N 行逐条审计且都带理由（旧实现 gaps 口只写 1 行 "
   "ref=NULL，p3 口写了行却没理由）：命中=%s" % NBATCH,
   s == 200 and NBATCH >= 2 and (data(r) or {}).get("pardoned_count") == 2,
   "%s 行=%s resp=%s" % (s, NBATCH, blob(r)[:90]))

# ============ S4 改档口的字典校验（P-171） ============
s, r = post("/admin/users/class", {"user_id": MV, "class_id": 90}, tok=ROOT)
ok("S4a class_id=90 不在 user_classes 字典里（档位不连续），必须 400 而不是 "
   "撞 FK 报 500：http=%s 原话=%s" % (s, blob(r)[:60]),
   s == 400 and "不在等级字典" in blob(r), blob(r)[:130])
ok("S4b 库里档位没被改动", psql("SELECT class_id::text FROM users WHERE id "
                                "= %d" % MV) == "1",
   "now=%s" % psql("SELECT class_id::text FROM users WHERE id = %d" % MV))
s2, r2 = post("/admin/users/batch", {"action": "class", "ids": [MV],
                                     "value": 90}, tok=ROOT)
ok("S4c 批量改档同判据（400 且一个都没改）",
   s2 == 400 and psql("SELECT class_id::text FROM users WHERE id = %d" % MV)
   == "1", "%s %s" % (s2, blob(r2)[:90]))
s3, r3 = post("/admin/users/class", {"user_id": MV, "class_id": 20}, tok=ROOT)
ok("S4d 正控：字典里真实存在的档位（20 贵宾）仍可改（200）",
   s3 == 200 and psql("SELECT class_id::text FROM users WHERE id = %d" % MV)
   == "20", "%s now=%s" % (s3, psql("SELECT class_id::text FROM users "
                                    "WHERE id = %d" % MV)))
post("/admin/users/class", {"user_id": MV, "class_id": 1}, tok=ROOT)
s4, r4 = post("/admin/users/class", {"user_id": MV, "class_id": 99}, tok=ROOT)
ok("S4e 结构性约束没被改坏：不许把人设为站长（403）",
   s4 == 403, "%s %s" % (s4, blob(r4)[:70]))

# ============ S5 手工补量（P-168 / P-169） ============
s, r = post("/admin/amountupload", {"user_id": 1, "bytes": 1024}, tok=MTOK)
ok("S5a 普通成员打 amountupload → 403（权限面没被我改坏）", s == 403,
   "%s %s" % (s, blob(r)[:70]))
UP0 = int(psql("SELECT uploaded::text FROM users WHERE id = 1"))
s, r = post("/admin/amountupload", {"user_id": 1, "bytes": 1048576},
            tok=ROOT)
ok("S5b 正控：站长对自己下手也被拒（403，与 adjust 的「对自己不许动手」同口径）",
   s == 403 and psql("SELECT uploaded::text FROM users WHERE id = 1") == str(
       UP0), "%s uploaded %s→%s" % (s, UP0, psql(
           "SELECT uploaded::text FROM users WHERE id = 1")))
UP0 = int(psql("SELECT uploaded::text FROM users WHERE id = %d" % MV))
KEY = "bgidem%d" % TAG
res = []


def fire(_):
    res.append(post("/admin/amountupload", {"user_id": MV, "bytes": 1048576,
                                            "idempotency_key": KEY},
                    tok=ROOT))


ths = [threading.Thread(target=fire, args=(i,)) for i in range(6)]
for t in ths:
    t.start()
for t in ths:
    t.join()
OKS = [x[0] for x in res]
UP1 = int(psql("SELECT uploaded::text FROM users WHERE id = %d" % MV))
BATCHROW = cnt("grant_batches", "idempotency_key = '%s'" % KEY)
ok("S5c 同一把幂等键并发 6 发：至多一发成功、其余必须被数据库挡下"
   "（旧实现是 SELECT EXISTS 再 INSERT，实测能双双通过）——状态=%s "
   "uploaded %s→%s（应只 +1MiB）槽位行=%s"
   % (OKS, UP0, UP1, BATCHROW),
   OKS.count(200) == 1 and UP1 - UP0 == 1048576
   and BATCHROW == "1" and OKS.count(400) == 5, blob(res)[:160])
s, r = post("/admin/amountupload", {"user_id": MV, "bytes": 5,
                                    "idempotency_key": "short"}, tok=ROOT)
ok("S5d 键长度不足直接 400（不是静默丢弃键）",
   s == 400 and "8~120" in blob(r), "%s %s" % (s, blob(r)[:80]))
s, r = post("/admin/amountupload", {"user_id": MV, "bytes": 0}, tok=ROOT)
ok("S5e bytes=0 且没给 gb → 400（原判据仍在）", s == 400,
   "%s %s" % (s, blob(r)[:80]))
s, r = post("/admin/amountupload", {"user_id": MV, "gb": 1}, tok=ROOT)
ok("S5f gb 口径：bytes 缺省 + gb=1 → 正好 1GiB（与 increment-bulk 同单位）",
   s == 200 and int(psql("SELECT uploaded::text FROM users WHERE id = %d"
                         % MV)) == UP1 + 1073741824,
   "%s uploaded=%s" % (s, psql("SELECT uploaded::text FROM users WHERE id "
                               "= %d" % MV)))
UPX = int(psql("SELECT uploaded::text FROM users WHERE id = %d" % MV))
s, r = post("/admin/amountupload", {"user_id": MV, "bytes": -(UPX + 10 ** 9)},
            tok=ROOT)
LED = cnt("traffic_ledger", "user_id = %d AND torrent_id = 0" % MV)
ok("S5g 扣成负数时按实际截断（uploaded 归 0，流水记实际差额而不是虚数）",
   s == 200 and psql("SELECT uploaded::text FROM users WHERE id = %d" % MV)
   == "0", "%s now=%s 人工调账流水=%s" % (s, psql(
       "SELECT uploaded::text FROM users WHERE id = %d" % MV), LED))
DELTA = psql("SELECT coalesce(sum(delta_up),0)::text FROM traffic_ledger "
             "WHERE user_id = %d AND torrent_id = 0" % MV)
ok("S5h 不变式：人工调账流水合计 == 账户当前值（基线是 0；扣穿被夹到 0 "
   "时合计也该归 0）⇒ 下一次 reconcile 不会把账改回去",
   int(DELTA) == int(psql("SELECT uploaded::text FROM users WHERE id = %d"
                          % MV)),
   "sum(delta_up)=%s uploaded=%s UPX=%s" % (DELTA, psql(
       "SELECT uploaded::text FROM users WHERE id = %d" % MV), UPX))

# ============ S6 发薪合计（P-170，代码判据 + 单行可验部分） ============
PER = psql("SELECT to_char((now() at time zone 'utc+8'), 'YYYY-MM')")
TYPE = psql("SELECT id::text FROM jixiao_types ORDER BY id LIMIT 1")
psql("DELETE FROM jixiao_claims WHERE user_id = %d AND period = '%s'"
     % (MV, PER))
psql("INSERT INTO jixiao_claims (user_id, type_id, period, amount, status, "
     "metrics_snapshot) SELECT %d, %s, '%s', 777, 1, "
     "'{\"source\":\"admin\"}'::jsonb" % (MV, TYPE, PER))
s, r = call("GET", "/admin/jixiao/payroll?period=%s" % PER, token=ROOT)
d = data(r) or {}
SQL_T = psql("SELECT coalesce(sum(amount),0)::text FROM jixiao_claims WHERE "
             "period = '%s' AND status = 1" % PER)
ok("S6a payroll 的 total 必须等于整期 SQL 合计（旧写法是对 LIMIT 200 之后"
   "的一页求和，本期超过 200 行就静默偏小）：接口=%s 库=%s" % (
       d.get("total"), SQL_T),
   s == 200 and str(d.get("total")) == SQL_T,
   "%s %s" % (s, blob(d)[:110]))
ok("S6b settled_at 为空的已发薪行不许把整页打挂（P-172：本函数自己的 SQL 写"
   "着 NULLS LAST，元组却按非 Option 读）：http=%s 行=%s",
   s == 200 and len(d.get("list") or []) >= 1
   and any(x.get("settled_at") is None for x in d.get("list") or []),
   "%s len=%s first=%s" % (s, len(d.get("list") or []),
                           blob((d.get("list") or [{}])[0])[:90]))
ok("S6c 列表不超过 LIMIT 200（合计取自整期已在 S6a 钉住；>200 行的偏差按"
   "「代码推导、未实测」记进文档）",
   isinstance(d.get("list"), list) and len(d.get("list") or []) <= 200,
   "len=%s" % len(d.get("list") or []))

# ============ S7 收尾 ============
if INV:
    psql("UPDATE invites SET status = 2 WHERE id IN (%s)"
         % ",".join(str(int(i)) for i in INV))
for uid, tok in ((SU, STOK), (MV, MTOK), (BO, BTOK)):
    psql("DELETE FROM hr_snapshots WHERE user_id = %d" % uid)
    psql("DELETE FROM hr_violations WHERE user_id = %d" % uid)
    psql("DELETE FROM jixiao_claims WHERE user_id = %d" % uid)
    psql("DELETE FROM traffic_ledger WHERE user_id = %d" % uid)
    psql("DELETE FROM user_modify_logs WHERE uid = %d" % uid)
    psql("UPDATE users SET uploaded = 0, downloaded = 0, spark_balance = 0 "
         "WHERE id = %d" % uid)
    post("/admin/users/status", {"user_id": uid, "status": 2})
    call("DELETE", "/admin/users/%d" % uid, None, token=ROOT)
psql("DELETE FROM grant_batches WHERE idempotency_key = '%s'" % KEY)
# 未修版真会让站长给自己加量：把本轮写到 user_id=1 的人工调账流水删掉，
# 再把 uploaded 写回基线并回读证明（reconcile 的权威是流水，两边都要复原）
psql("DELETE FROM traffic_ledger WHERE user_id = 1 AND torrent_id = 0 "
     "AND id > %s" % TR0)
psql("UPDATE users SET uploaded = %s WHERE id = 1" % B0["root_up"])
NOW = {"live": cnt("users", "status < 2"), "claims": cnt("jixiao_claims"),
       "viol": cnt("hr_violations"),
       "root_up": psql("SELECT uploaded::text FROM users WHERE id = 1"),
       "slots": cnt("grant_batches", "idempotency_key = '%s'" % KEY)}
# 基线是在三个探针号都还活跃时取的，收尾它们都是墓碑 ⇒ 活跃数 -3 是预期
drift = {k: (B0[k], NOW[k]) for k in B0 if k != "live" and NOW[k] != B0[k]}
ok("S7a 零残留：claims/hr_violations/站长 uploaded 回基线、幂等槽位已清、"
   "活跃成员数 -3（三个探针都成墓碑：%s→%s）" % (B0["live"], NOW["live"]),
   not drift and NOW["slots"] == "0"
   and int(NOW["live"]) == int(B0["live"]) - 3, blob(drift)[:140])
ok("S7b 三个探针号都走真实墓碑（passkey 作废、不再是活跃成员）",
   psql("SELECT count(*)::text FROM users WHERE id IN (%d,%d,%d) AND "
        "status >= 2" % (SU, MV, BO)) == "3"
   and psql("SELECT bool_and(passkey LIKE 'deleted-%%') FROM users WHERE "
            "id IN (%d,%d,%d)" % (SU, MV, BO)) == "t",
   psql("SELECT string_agg(id::text||':'||status::text, ',') FROM users "
        "WHERE id IN (%d,%d,%d)" % (SU, MV, BO)))
AUD = cnt("audit_log", "ref::text LIKE '%%%s%%'" % SENT)
ok("S7c 本轮 SENTINEL 审计行共 %s 条：audit_log 有防篡改触发器，"
   "留痕是**合法产物**不删（收尾只数不删）" % AUD, AUD.isdigit()
   and int(AUD) >= 3, "rows=%s" % AUD)
summary()
