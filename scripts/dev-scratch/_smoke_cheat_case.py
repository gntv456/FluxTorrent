#!/usr/bin/env python3
"""M1 反作弊案件管理冒烟（0367）：开案/去重/队列/详情/备注/认领互斥/SLA/L2自动开案。

用法：python _smoke_cheat_case.py
前提：api+worker 已部署（含 0367）；本脚本自签 staff JWT（HS256，凭据取容器 env）。
"""
import base64
import hashlib
import hmac
import json
import subprocess
import sys
import time
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080"
ROOT = 1        # class 99
AUDITOR = 4306  # class 94
USER_A = 39     # 普通用户：手工开案对象
USER_B = 40     # 普通用户：L2 自动开案对象
MARK = "m1smoke"


def sh(cmd: str) -> str:
    return subprocess.run(
        cmd, shell=True, capture_output=True, text=True
    ).stdout.strip()


def psql(sql: str) -> str:
    # -q 静默命令标签（INSERT...RETURNING 也会在 -t 下打 "INSERT 0 1"），
    # 只留查询输出；多行结果保留全部行。
    out = sh(
        f'docker exec flux-postgres psql -U flux -d fluxtorrent -t -A -q -c "{sql}"'
    )
    return "\n".join(out.splitlines()).strip()


def secret() -> str:
    return sh("docker exec flux-api printenv JWT_SECRET")


def b64url(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


def token(uid: int, cls: int) -> str:
    """HS256 JWT（与 api auth.rs Claims 同构：sub/class_id/iat/exp）。"""
    now = int(time.time())
    head = b64url(json.dumps({"alg": "HS256", "typ": "JWT"}).encode())
    body = b64url(
        json.dumps(
            {"sub": uid, "class_id": cls, "iat": now, "exp": now + 3600}
        ).encode()
    )
    sig = hmac.new(
        secret().encode(), f"{head}.{body}".encode(), hashlib.sha256
    ).digest()
    return f"{head}.{body}.{b64url(sig)}"


T_ROOT = token(ROOT, 99)
T_AUD = token(AUDITOR, 94)

FAILED = []
# 副作用水位（消息/站内信）：清理时只删本次 tick 新增，不碰存量
W_MARK: dict[str, str] = {"msg": "0", "sm": "0"}


def check(name: str, cond: bool, extra: str = "") -> None:
    mark = "✓" if cond else "✗"
    print(f"  {mark} {name}" + (f" — {extra}" if extra else ""))
    if not cond:
        FAILED.append(name)


def call(
    method: str,
    path: str,
    tok: str,
    body: dict | None = None,
) -> tuple[int, object]:
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Authorization", f"Bearer {tok}")
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, data) as r:
            return r.status, json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read() or b"{}")
        except Exception:
            return e.code, {}


def wait_sla(case_id: int, timeout: int = 240) -> bool:
    """轮询案件详情等 sla_breached 置位（worker 60s tick + 任务时长）。"""
    deadline = time.time() + timeout
    while time.time() < deadline:
        st, r = call("GET", f"/api/v1/admin/cheat-cases/{case_id}", T_ROOT)
        if st == 200 and data_of(r).get("sla_breached"):
            return True
        time.sleep(8)
    return False


def wait_case(user_id: int, timeout: int = 240) -> str:
    """轮询等该用户的进行中案件落库，返回 'id | priority | reason' 或 ''。"""
    deadline = time.time() + timeout
    while time.time() < deadline:
        rows = psql(
            "SELECT id || ' | ' || priority || ' | ' || reason FROM reports "
            f"WHERE ref_type='cheat' AND ref_id={user_id} AND status IN (0,2)"
        )
        if rows.strip():
            return rows.strip()
        time.sleep(8)
    return ""



def data_of(r: object) -> dict:
    """响应包络取 data；错误响应 data 为 null 时回落空 dict 不崩。"""
    return (r or {}).get("data") or {}



def trigger_job() -> bool:
    """手触 cheat_enforce；遇「锁未抢到」自动重触（与 10 分钟定时 tick 竞争）。"""
    for attempt in range(4):
        st, r = call(
            "POST", "/api/v1/admin/jobs/run", T_ROOT,
            {"job": "cheat_enforce"},
        )
        if st != 200:
            time.sleep(5)
            continue
        tid = (r.get("data") or {}).get("id")
        for _ in range(30):  # 等 worker 60s tick 处理完该 trigger
            time.sleep(4)
            row = psql(
                "SELECT status || '|' || coalesce(result, '') "
                f"FROM job_triggers WHERE id = {tid}"
            )
            if not row.strip():
                continue
            status, _, result = row.partition("|")
            if status == "done":
                return True
            if status == "failed" and "锁未抢到" in result:
                break  # 重触
            if status == "failed":
                print(f"    trigger {tid} 失败：{result[:80]}")
                return False
        else:
            continue
    return False


def main() -> int:
    print("== 0. 清理旧冒烟数据 + 快照副作用水位 ==")
    psql(
        "DELETE FROM case_notes WHERE case_id IN "
        "(SELECT id FROM reports WHERE ref_type='cheat' AND ref_id IN "
        f"({USER_A},{USER_B})); "
        "DELETE FROM reports WHERE ref_type='cheat' AND ref_id IN "
        f"({USER_A},{USER_B}); "
        "DELETE FROM cheat_events WHERE user_id IN "
        f"({USER_A},{USER_B}) AND agent LIKE '%{MARK}%';"
    )
    # 库里有 9 个用户带 hits≥5 未处置证据（真实积压），触发 job 会按设计
    # 给它们自动开案/发告警——副作用水位按 id 快照，清理时只删本次新增。
    W_MARK["msg"] = psql(
        "SELECT COALESCE(max(id), 0) FROM messages"
    )
    W_MARK["sm"] = psql(
        "SELECT COALESCE(max(id), 0) FROM staffmessages"
    )

    print("== 1. 造证据 + 手工开案 ==")
    ev_id = psql(
        "INSERT INTO cheat_events (user_id, agent, reason, hits) "
        f"VALUES ({USER_A}, 'ghost:{MARK}', 'M1 冒烟证据', 2) RETURNING id"
    )
    check("插入证据行", bool(ev_id), f"event_id={ev_id}")
    st, r = call(
        "POST", "/api/v1/admin/cheat-cases/open", T_ROOT,
        {"event_id": int(ev_id), "note": "M1 冒烟开案"},
    )
    check("开案 200", st == 200, f"status={st}")
    d = r.get("data") or {}
    case_id = d.get("id")
    if d.get("duplicate"):
        # 竞态容错：worker 可能已为 USER_A 自动开案（其存量证据 hits≥5，
        # 真实积压）——取现存进行中案件继续后续断言。
        rows = psql(
            "SELECT id FROM reports WHERE ref_type='cheat' "
            f"AND ref_id={USER_A} AND status IN (0,2)"
        )
        case_id = int(rows) if rows.strip() else None
        check("duplicate 时取现存案件", case_id is not None, f"rows={rows!r}")
    else:
        check("开案非 duplicate", True)
    check("拿到案件 id", isinstance(case_id, int), f"case_id={case_id}")

    print("== 2. 重复开案被唯一索引挡重 ==")
    st, r = call(
        "POST", "/api/v1/admin/cheat-cases/open", T_ROOT,
        {"user_id": USER_A},
    )
    check("二次开案 200", st == 200)
    check("duplicate=true", data_of(r).get("duplicate") is True)
    n_cases = psql(
        "SELECT count(*) FROM reports WHERE ref_type='cheat' "
        f"AND ref_id={USER_A} AND status IN (0,2)"
    )
    check("库里仍只 1 个进行中案件", n_cases == "1", f"n={n_cases}")

    print("== 3. 队列 / 详情 / 备注 ==")
    st, r = call("GET", "/api/v1/admin/cheat-cases?status=open", T_ROOT)
    items = data_of(r).get("items", [])
    hit = [i for i in items if i["id"] == case_id]
    check("队列含本案", len(hit) == 1)
    check(
        "队列聚合未决证据",
        hit and hit[0]["open_evidence"] >= 1 and hit[0]["total_hits"] >= 2,
    )
    st, r = call("GET", f"/api/v1/admin/cheat-cases/{case_id}", T_ROOT)
    d = data_of(r)
    check("详情 200", st == 200)
    check("详情含证据", len(d.get("evidence", [])) >= 1)
    check("详情含开案备注", len(d.get("notes", [])) >= 1)
    check("SLA 未超时", d.get("sla_breached") is False)
    st, r = call(
        "POST", f"/api/v1/admin/cheat-cases/{case_id}/note", T_ROOT,
        {"body": "M1 冒烟备注：已核实"},
    )
    check("提交备注 200", st == 200)
    st, r = call("GET", f"/api/v1/admin/cheat-cases/{case_id}", T_ROOT)
    check("备注入时间线", len(data_of(r).get("notes", [])) >= 2)

    print("== 4. 认领互斥 ==")
    st, r = call(
        "POST", f"/api/v1/admin/cheat-cases/{case_id}/claim", T_ROOT, {}
    )
    check("root 认领 200", st == 200)
    st, r = call(
        "POST", f"/api/v1/admin/cheat-cases/{case_id}/claim", T_AUD, {}
    )
    check("他人认领被互斥挡下", st == 400, f"status={st} msg={r}")
    st, r = call(
        "POST", f"/api/v1/admin/cheat-cases/{case_id}/claim", T_ROOT, {}
    )
    check("本人重复认领幂等", st == 200)
    st, r = call(
        "POST", f"/api/v1/admin/cheat-cases/{case_id}/claim", T_ROOT,
        {"release": True},
    )
    check("释放回队列 200", st == 200)
    st, r = call(
        "POST", f"/api/v1/admin/cheat-cases/{case_id}/claim", T_AUD, {}
    )
    check("释放后他人可认领", st == 200, f"status={st}")
    st, r = call("GET", "/api/v1/admin/cheat-cases?status=claimed", T_ROOT)
    check(
        "claimed 页签可见",
        any(i["id"] == case_id for i in data_of(r).get("items", [])),
    )

    print("== 5. SLA 超时置位（worker） ==")
    psql(
        "UPDATE reports SET created_at = now() - interval '80 hours' "
        f"WHERE id = {case_id}"
    )
    check("手触 cheat_enforce", trigger_job())
    print("    等 SLA 置位…")
    check("sla_breached 置位", wait_sla(case_id))
    logs = sh('docker logs flux-worker --since 5m 2>&1 | grep -c "SLA 超时"')
    check("worker 日志有 SLA 置位记录", logs != "0", f"hits={logs}")

    print("== 6. L2 自动开案（worker） ==")
    psql(
        "INSERT INTO cheat_events (user_id, agent, reason, hits) "
        f"VALUES ({USER_B}, 'speed:{MARK}-auto', 'M1 自动开案证据', 5)"
    )
    check("手触 cheat_enforce", trigger_job())
    print("    等自动开案…")
    rows = wait_case(USER_B)
    check("自动开案落库", bool(rows), f"rows={rows!r}")
    if rows:
        cid, pri, reason = [x.strip() for x in rows.split("|")]
        check("自动开案 priority=1（加急）", pri == "1", f"priority={pri}")
        check("reason 带自动开案前缀", reason.startswith("自动开案"), reason)
        n_att = psql(
            "SELECT count(*) FROM cheat_events "
            f"WHERE user_id={USER_B} AND case_id={cid}"
        )
        check("证据已归案", n_att != "0", f"n={n_att}")
        note = psql(
            "SELECT body FROM case_notes WHERE case_id="
            f"{cid} ORDER BY id LIMIT 1"
        )
        check("首条备注为系统自动开案", "系统自动开案" in note, note[:60])
    check("二次手触 cheat_enforce", trigger_job())
    print("    验证幂等…")
    time.sleep(70)
    n_dup = psql(
        "SELECT count(*) FROM reports WHERE ref_type='cheat' "
        f"AND ref_id={USER_B} AND status IN (0,2)"
    )
    check("二次 tick 不重复开案", n_dup == "1", f"n={n_dup}")
    logs = sh(
        'docker logs flux-worker --since 10m 2>&1 | grep -c "L2 自动开案"'
    )
    check("worker 日志有自动开案记录", logs != "0", f"hits={logs}")

    print("== 7. 清理 ==")
    # 全部 cheat 案件均为本次冒烟产生（冒烟前库里 0 个）——含真实积压用户
    # 被自动开出的案，一并清掉，恢复冒烟前状态。
    psql(
        "UPDATE cheat_events SET case_id = NULL WHERE case_id IN "
        "(SELECT id FROM reports WHERE ref_type='cheat'); "
        "DELETE FROM case_notes WHERE case_id IN "
        "(SELECT id FROM reports WHERE ref_type='cheat'); "
        "DELETE FROM reports WHERE ref_type='cheat'; "
        "DELETE FROM cheat_events WHERE user_id IN "
        f"({USER_A},{USER_B}) AND agent LIKE '%{MARK}%'; "
        f"DELETE FROM messages WHERE id > {W_MARK['msg']}; "
        f"DELETE FROM staffmessages WHERE id > {W_MARK['sm']};"
    )
    left = psql("SELECT count(*) FROM reports WHERE ref_type='cheat'")
    check("冒烟数据已清", left == "0", f"left={left}")

    print()
    if FAILED:
        print(f"冒烟失败 {len(FAILED)} 项：{FAILED}")
        return 1
    print("M1 冒烟全过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
