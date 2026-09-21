"""考核&任务系统 E2E（0093/0094 + P0 修复回归）：
NULL 不限领 / 等级拦截 / 坏 metric 拒领 / 限额原子拦截 / claim_limit=0 停领
/ onboard 派发 + PM / GET /me/exams 契约 / my_records 进度契约 / admin exam-users

前置：docker compose 全套在跑（api:8080）；测试账号 xuehai/gaozhong 密码 password123。
清理：所有冒烟造的任务/认领用后即删，用户可重复执行。
"""

import json
import subprocess
import sys
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080/api/v1"
results = []


def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=10) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}
    except urllib.error.URLError as e:
        return 0, {"message": str(e)}


def psql(sql):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
            "fluxtorrent", "-t", "-c", sql],
        capture_output=True, text=True,
    )
    return p.stdout.strip()


def check(name, ok, detail=""):
    results.append(ok)
    print(f"{'PASS' if ok else 'FAIL'} {name} {detail}")


def login(username):
    for _ in range(3):
        s, r = call("POST", "/auth/login", {"username": username,
            "password": "password123"})
        tok = (r.get("data") or {}).get("token")
        if tok:
            return tok
        if r.get("code") == 1015:  # 登录限流，退避重试
            import time
            time.sleep(31)
        else:
            raise SystemExit(f"login {username} failed: {r}")
    raise SystemExit(f"login {username} rate-limited")


def cleanup():
    psql("DELETE FROM tasks WHERE name LIKE 'e2e-exam%'")
    psql(
        "DELETE FROM task_claims WHERE task_id IN (SELECT id FROM tasks WHERE"
            "name LIKE 'e2e-exam%')")
    psql("DELETE FROM messages WHERE subject IN ('考核已派发','e2e-exam PM')")
    psql(
        "UPDATE tasks SET claim_limit=NULL WHERE id IN (SELECT id FROM tasks"
            "WHERE name LIKE 'e2e任务%')")
    psql(
        "UPDATE tasks SET metric='{\"uploads\": 1}'"
        " WHERE name LIKE 'e2e任务%' AND metric::text='{}'")


def main():
    cleanup()
    xue = login("xuehai")   # class_id=3
    gao = login("gaozhong") # class_id=1

    # --- 1. P0 回归：NULL 不限领 ---
    psql(
        "INSERT INTO tasks (name, metric, starts_at, ends_at,"
        " target_class, reward, penalty)"
        " VALUES ('e2e-exam不限领', '{\"uploads\": 99}',"
        " now()-interval '1 day', now()+interval '1 day', 0, 10, 0)")
    tid = int(psql("SELECT id FROM tasks WHERE name='e2e-exam不限领'"))
    s, r = call("POST", "/tasks/claim", {"task_id": tid}, token=xue)
    check("NULL 不限领可领取", s == 200, f"status={s}")

    s, r = call("POST", "/tasks/claim", {"task_id": tid}, token=xue)
    check("重复领取被拦", s == 400, f"status={s}")

    # --- 2. P0 回归：等级拦截 ---
    psql(f"UPDATE tasks SET target_class=5 WHERE id={tid}")
    s, r = call("POST", "/tasks/claim", {"task_id": tid}, token=gao)
    check("等级不足被拦(class1<5)", s == 400 and "门槛" in str(r), f"status={s}")

    # --- 3. P0 回归：坏 metric 拒领 ---
    psql(
        f"UPDATE tasks SET target_class=0,"
        f" metric='{{\"kind\":\"seed_hours\"}}' WHERE id={tid}")
    s, r = call("POST", "/tasks/claim", {"task_id": tid}, token=gao)
    check("坏 metric 拒领", s == 400 and "无效" in str(r), f"status={s}")
    psql(f"UPDATE tasks SET metric='{{}}' WHERE id={tid}")
    s, r = call("POST", "/tasks/claim", {"task_id": tid}, token=gao)
    check("空 metric 拒领", s == 400 and "无效" in str(r), f"status={s}")

    # --- 4. P0 回归：限额原子 + 停领 ---
    psql(
        f"UPDATE tasks SET metric='{{\"uploads\": 99}}',"
        f" claim_limit=1 WHERE id={tid}")
    psql(f"DELETE FROM task_claims WHERE task_id={tid}")
    s, _ = call("POST", "/tasks/claim", {"task_id": tid}, token=xue)
    s2, r2 = call("POST", "/tasks/claim", {"task_id": tid}, token=gao)
    check("限额1第2人被拦", s == 200 and s2 == 400, f"{s}/{s2}")
    psql(f"UPDATE tasks SET claim_limit=0 WHERE id={tid}")
    s, r = call("POST", "/tasks/claim", {"task_id": tid}, token=gao)
    check("claim_limit=0 停领", s == 400 and "停止" in str(r), f"status={s}")

    # --- 5. onboard 派发 + PM（worker exam_assign 分钟级）---
    # 造一个自动派发的 onboard 考核；xuehai/gaozhong 注册均超默认 30 天窗口，
    # 临时把窗口调大让老测试用户也进派发范围（测完还原）。
    orig_days = psql(
        "SELECT value FROM site_settings"
        " WHERE name='exam_onboard_days'") or "30"
    psql(
        "UPDATE site_settings SET value='99999' WHERE name='exam_onboard_days'")
    psql(
        "INSERT INTO tasks (name, metric, starts_at, ends_at,"
        " target_class, reward, penalty, duration_days, kind,"
        " auto_assign, period)"
        " VALUES ('e2e-exam转正', '{\"uploads\": 99}',"
        " now()-interval '1 day', now()+interval '365 day',"
        " 0, 10, 0, 30, 'onboard', TRUE, 'once')")
    n_before = int(psql("SELECT count(*) FROM messages WHERE subject='考核已派发'"))
    n_claims_before = int(psql(
        "SELECT count(*) FROM task_claims c JOIN tasks t ON t.id=c.task_id"
            "WHERE t.name='e2e-exam转正'"))
    print(f"...等待 worker exam_assign（最长 130s），此前派发 {n_claims_before} 条")
    import time
    for _ in range(13):
        time.sleep(10)
        n_claims = int(psql(
            "SELECT count(*) FROM task_claims c JOIN tasks t ON t.id=c.task_id"
                "WHERE t.name='e2e-exam转正'"))
        if n_claims > 0:
            break
    n_after = int(psql("SELECT count(*) FROM messages WHERE subject='考核已派发'"))
    check("onboard 自动派发", n_claims > 0, f"claims={n_claims}")
    check("派发 PM 已发", n_after > n_before, f"PM {n_before}→{n_after}")

    # --- 6. GET /me/exams 契约 ---
    s, r = call("GET", "/me/exams", token=xue)
    rows = (r.get("data") or [])
    e2e = [x for x in rows if x.get("name") == "e2e-exam转正"]
    ok = s == 200 and e2e and all(k in e2e[0] for k in ("current", "metric",
        "deadline", "status", "kind"))
    check("GET /me/exams 契约", bool(ok), f"rows={len(rows)}")

    # --- 7. my_records 进度契约（P1-5）---
    s, r = call("GET", "/tasks/overview", token=xue)
    recs = (r.get("data") or {}).get("my_records") or []
    rec = next((x for x in recs if x.get("name") == "e2e-exam转正"), None)
    ok = (s == 200 and rec is not None and "current" in rec
          and "metric" in rec and "deadline" in rec)
    check("overview my_records 进度契约", bool(ok), f"rec={bool(rec)}")

    # --- 8. admin exam-users ---
    root = login("root")
    s, r = call("GET", "/admin/exam-users", token=root)
    rows = r.get("data") or []
    e2e_rows = [x for x in rows if x.get("task_name") == "e2e-exam转正"]
    check("admin exam-users 可见派发", s == 200 and len(e2e_rows) > 0,
        f"rows={len(rows)}")

    # --- 9. 达标结算 + 转正 PM（P1-3）---
    # 把 e2e 考核的 metric 改成 uploads>=0（立即达标），等 task_settle
    psql("UPDATE tasks SET metric='{\"uploads\": 0}' WHERE name='e2e-exam转正'")
    settled = 0
    for _ in range(13):
        time.sleep(10)
        settled = int(psql(
            "SELECT count(*) FROM task_claims c JOIN tasks t ON t.id=c.task_id "
            "WHERE t.name='e2e-exam转正' AND c.status=1"))
        if settled > 0:
            break
    n_pass_pm = int(psql(
        "SELECT count(*) FROM messages WHERE subject='转正考核通过'"))
    check("onboard 达标结算", settled > 0, f"settled={settled}")
    check("转正 PM 专有文案", n_pass_pm > 0, f"PM={n_pass_pm}")

    cleanup()
    psql(
        "UPDATE site_settings SET value='{orig_days}' WHERE"
            "name='exam_onboard_days'")
    psql("DELETE FROM messages WHERE subject='转正考核通过'")
    n = len(results)
    print(f"\n===== 考核&任务 E2E：{sum(results)}/{n} PASS =====")
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
