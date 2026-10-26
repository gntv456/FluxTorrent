# -*- coding: utf-8 -*-
"""资深PT用户实测：考核不达标处置菜单（0397）端到端闸门。

覆盖三个真实踩过的坑：

1. **未知权限位炸结算**：user_permissions.permission_key 对
   permissions.key 有外键。后台提示里给的示例键（forum.view /
   games.play）本来就不在 permissions 表里——按提示填，结算那一夜
   撞 FK 违例 → 整个结算批次中断 → 全站考核认领卡在 status=0。
   判据：菜单里塞一个不存在的权限位，认领**仍要**正常结算为失败。

2. **流量扣减被对账抹平**：users.uploaded 是快照，权威在 traffic_ledger
   （worker reconcile_snapshots 每 6h 全量重算覆盖）。只改快照＝处罚
   6 小时后自动消失。判据：跑一次 reconcile 口径的 SQL，扣量仍在。

3. **实况失实 + 处置是黑箱**：文案要写**实扣**（余额/流量不足时扣到 0），
   且处置必须出现在站内信里（用户要知道被处置了什么）。判据：站内信
   正文含实扣数，且 spark_ledger / traffic_ledger 记的也是实扣数。

用法: python scripts/pt_audit_exam_penalty.py
"""
import base64
import hashlib
import hmac
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")
CI = bool(os.environ.get("DATABASE_URL"))
MARK = "zz_penalty_probe"
FAILED = []


def sh(*args):
    try:
        r = subprocess.run(args, capture_output=True, text=True,
                           encoding="utf-8", errors="replace")
    except OSError:
        return ""
    return r.stdout.strip()


def psql(sql):
    if CI:
        cmd = ["psql", os.environ["DATABASE_URL"], "-t", "-A", "-q"]
    else:
        # ON_ERROR_STOP=1：psql 走 stdin 时语句出错也返回 0（只有 -c
        # 才按错误码返回 1），不加这个开关则 SQL 写错也不报错——
        # 闸门读到空串照样能判绿。
        cmd = ["docker", "exec", "-i", "flux-postgres", "psql",
               "-U", "flux", "-d", "fluxtorrent", "-t", "-A", "-q",
               "-v", "ON_ERROR_STOP=1"]
    try:
        r = subprocess.run(cmd, input=sql, capture_output=True, text=True,
                           encoding="utf-8", errors="replace")
    except OSError as e:
        print("    ! psql 不可用：%s" % e)
        return ""
    if r.returncode != 0 or "ERROR" in (r.stderr or ""):
        raise RuntimeError("psql 失败：%s" % (r.stderr or "").strip()[:200])
    return "\n".join(r.stdout.splitlines()).strip()


def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=30) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}
    except Exception as e:
        return 0, {"exc": repr(e)}


def ok(name, cond, detail=""):
    print(("[PASS] " if cond else "[FAIL] ") + name +
          ("" if cond else "  <- " + str(detail)[:260]))
    if not cond:
        FAILED.append(name)
    return cond


def b64url(raw):
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


def login():
    """自铸职员令牌（HS256 + JWT_SECRET），与同作业的 ci_m_ops_panels.py 同口径。

    不按口令登录：前序装机步 `install_e2e.py` 的 S4 会把 root 口令改成随机
    `NEW_PW`，而随后的 seed 步 `ON CONFLICT DO UPDATE` 只回写 passkey/class_id、
    **刻意不回写 pass_hash** ⇒ 在这条 e2e 作业里按口令登 root 必得
    400/2004「凭证无效」（正控：占位哈希的号登录实测就是这个形状）。
    自铸令牌不吃装机步的口令轮换，也不吃登录限流与 PoW 墙。
    """
    sec = os.environ.get("JWT_SECRET", "") or sh(
        "docker", "exec", "flux-api", "printenv", "JWT_SECRET")
    uid = psql("SELECT id FROM users WHERE username='root'")
    if not sec or not uid:
        return None
    cls = psql("SELECT class_id FROM users WHERE id=%s" % uid) or "99"
    now = int(time.time())
    head = b64url(json.dumps({"alg": "HS256", "typ": "JWT"}).encode())
    body = b64url(json.dumps({"sub": int(uid), "class_id": int(cls),
                             "iat": now, "exp": now + 1800}).encode())
    sig = b64url(hmac.new(sec.encode(), ("%s.%s" % (head, body)).encode(),
                          hashlib.sha256).digest())
    return "%s.%s.%s" % (head, body, sig)


def cleanup(uid, tid):
    # 账本行要整行清（不只挑 idempotency_key 前缀）：探针号是测试专用，
    # 留着就是「流水还在、人没了」的悬案——HL1 闸门会当场判红。
    psql("DELETE FROM exam_penalties WHERE task_id = %s; "
         "DELETE FROM task_claims WHERE task_id = %s; "
         "DELETE FROM traffic_ledger WHERE user_id = %s; "
         "DELETE FROM spark_ledger WHERE user_id = %s; "
         "DELETE FROM messages WHERE receiver_id = %s; "
         "DELETE FROM tasks WHERE id = %s; "
         "DELETE FROM balance_baseline WHERE user_id = %s; "
         "DELETE FROM users WHERE id = %s;" %
         (tid, tid, uid, uid, uid, tid, uid, uid))


def purge_probe():
    """清掉历次跑剩下的探针号（含它们的流水）——探针必须自清干净，
    否则下一轮的行数判据会被上一次的残骸污染。"""
    psql("DELETE FROM spark_ledger WHERE user_id IN "
         "(SELECT id FROM users WHERE username LIKE '%s%%'); "
         "DELETE FROM traffic_ledger WHERE user_id IN "
         "(SELECT id FROM users WHERE username LIKE '%s%%'); "
         "DELETE FROM messages WHERE receiver_id IN "
         "(SELECT id FROM users WHERE username LIKE '%s%%'); "
         "DELETE FROM balance_baseline WHERE user_id IN "
         "(SELECT id FROM users WHERE username LIKE '%s%%'); "
         "DELETE FROM users WHERE username LIKE '%s%%';" %
         (MARK, MARK, MARK, MARK, MARK))


def main():
    token = login()
    if not token:
        print("!! 铸令牌失败：JWT_SECRET 取不到（env 与 flux-api 容器都空）"
              "或 root 不在册——不是口令问题，本闸门已不按口令登录")
        return 1
    # 造数据：一次性用户 + 一次性考核任务 + 一条已过期的认领
    purge_probe()
    uid = psql(
        "INSERT INTO users (username, email, pass_hash, passkey, class_id, "
        "spark_balance, uploaded, downloaded, status) VALUES "
        "('%s_u', '%s@example.com', 'x', repeat('a', 32), 1, 50, 500, 0, 0) "
        "RETURNING id;" % (MARK, MARK))
    if not uid:
        print("!! 建用户失败")
        return 1
    uid = int(uid.splitlines()[-1])
    # fail_penalty 故意带一个不存在的权限位（forum.view）——必须不炸结算
    fp = json.dumps([
        {"kind": "spark_fine", "amount": 100},
        {"kind": "traffic_fine", "amount": 1000, "target": "uploaded"},
        {"kind": "ban_pages", "pages": ["forum.view"]},
    ], ensure_ascii=False)
    tid = psql(
        "INSERT INTO tasks (name, metric, starts_at, ends_at, duration_days, "
        "kind, fail_penalty, reward, penalty) VALUES "
        "('%s_task', '{}'::jsonb, now() - interval '10 days', "
        "now() + interval '10 days', 1, 'onboard', '%s'::jsonb, 0, 0) "
        "RETURNING id;" % (MARK, fp.replace("'", "''")))
    if not tid:
        print("!! 建任务失败")
        cleanup(uid, 0)
        return 1
    tid = int(tid.splitlines()[-1])
    # 基线登记：reconcile 的口径是 base_up + SUM(traffic_ledger.delta_up)，
    # 没有基线行的话「扣量被抹平」这条判据测不出真伪
    psql("INSERT INTO balance_baseline (user_id, base_up, through) "
         "VALUES (%s, 500, now()) ON CONFLICT (user_id) DO UPDATE "
         "SET base_up = 500, through = now();" % uid)
    psql("INSERT INTO task_claims (task_id, user_id, status, claimed_at, "
         "exam_stage_seq) VALUES (%s, %s, 0, now() - interval '5 days', 1);" %
         (tid, uid))
    print("探针：uid=%s tid=%s" % (uid, tid))

    # 1) 后台保存时必须拒掉不存在的权限位
    s, r = call("POST", "/admin/tasks", {
        "name": MARK + "_save",
        "starts_at": "2026-01-01T00:00:00Z",
        "ends_at": "2027-01-01T00:00:00Z",
        "fail_penalty": [{"kind": "ban_pages", "pages": ["forum.view"]}],
    }, token)
    ok("保存时拒掉不存在的权限位", s == 400 and "forum.view" in
       json.dumps(r, ensure_ascii=False), "HTTP %s %s" % (s, r))
    s, r = call("POST", "/admin/tasks", {
        "name": MARK + "_save2",
        "starts_at": "2026-01-01T00:00:00Z",
        "ends_at": "2027-01-01T00:00:00Z",
        "fail_penalty": [{"kind": "spark_fine"}],
    }, token)
    ok("保存时拒掉缺数额的扣款", s == 400, "HTTP %s %s" % (s, r))
    psql("DELETE FROM tasks WHERE name LIKE '%s_save%%';" % MARK)

    # 2) 触发结算
    s, r = call("POST", "/admin/jobs/run", {"job": "task_settle"}, token)
    ok("入队 task_settle", s == 200 and r.get("code") == 0,
       "HTTP %s %s" % (s, r))

    status = ""
    for _ in range(90):
        time.sleep(2)
        status = psql("SELECT status FROM task_claims WHERE task_id = %s "
                      "AND user_id = %s;" % (tid, uid))
        if status.strip() not in ("", "0"):
            break
    ok("认领已结算（status=2）", status.strip() == "2",
       "status=%s" % status.strip())

    # 3) 处置落账
    pen = psql(
        "SELECT kind || '|' || applied::text FROM exam_penalties "
        "WHERE claim_id IN (SELECT id FROM task_claims "
        "WHERE task_id = %s AND user_id = %s) ORDER BY kind;" % (tid, uid)
    )
    ok("exam_penalties 有落账", "traffic_fine" in pen and "spark_fine" in pen,
       pen)
    ok("扣魔力按实扣记（余额 50 / 罚 100 → 记 50）",
       '"took": 50' in pen or '"took":50' in pen, pen)
    sp = psql("SELECT amount FROM spark_ledger WHERE user_id = %s AND "
              "idempotency_key LIKE 'exam_penalty:%%';" % uid)
    ok("spark_ledger 记实扣 -50", sp.strip() == "-50", sp)
    tl = psql("SELECT delta_up FROM traffic_ledger WHERE user_id = %s AND "
              "reason LIKE 'exam penalty:%%';" % uid)
    ok("traffic_ledger 记实扣 -500（不是 -1000）", tl.strip() == "-500", tl)
    up = psql("SELECT uploaded FROM users WHERE id = %s;" % uid)
    ok("users.uploaded 扣到 0", up.strip() == "0", up)

    # 4) 对账不抹平（reconcile 口径重算后仍是扣后的值）
    psql("UPDATE users u SET uploaded = COALESCE((SELECT base_up FROM "
         "balance_baseline WHERE user_id = u.id), 0) + COALESCE((SELECT "
         "sum(delta_up) FROM traffic_ledger WHERE user_id = u.id), 0) "
         "WHERE u.id = %s;" % uid)
    up2 = psql("SELECT uploaded FROM users WHERE id = %s;" % uid)
    ok("对账重算后扣量仍在（流水口径生效，没被抹回 500）",
       up2.strip() == "0", "reconcile 后 uploaded=%s" % up2.strip())

    # 5) 站内信必须说清楚处置与实扣
    msg = psql("SELECT body FROM messages WHERE receiver_id = %s AND "
               "subject = '任务超时通知' ORDER BY id DESC LIMIT 1;" % uid)
    ok("站内信写明处置内容", "扣除" in msg and ("魔力" in msg or "上传量" in msg),
       msg[:240])
    ok("站内信写实扣数不写配置数（50 不是 100）",
       "50" in msg and "扣除魔力 100" not in msg, msg[:240])
    ok("站内信写明流量实扣（500 不是 1000）",
       "500" in msg and "1000" not in msg, msg[:240])

    # 6) 审计行
    aud = psql("SELECT count(*) FROM audit_log WHERE action = 'exam.penalty';")
    ok("audit_log 有 exam.penalty 记录", aud.strip() not in ("", "0"), aud)

    cleanup(uid, tid)
    print("\n===== 处置菜单闸门：%d 项失败 =====" % len(FAILED))
    return 1 if FAILED else 0


if __name__ == "__main__":
    sys.exit(main())
