#!/usr/bin/env python3
"""M2 风险评分冒烟（0368）：公式/档位/自动开案/观察名单/衰减/处置解除。

验收口径（NORM=8, 半衰期 14 天, 权重 ghost=1.0 speed=0.8）：
  ghost×4 + speed×3 → 63 分（high 档，自动开案 priority=1）
  ghost×2 + speed×2 → 45 分（watch 档，进观察名单不开案）
  ghost×1           → 13 分（none）
  30 天衰减：63 × 2^(-30/14) ≈ 14 分（不足原值一半）
  证据处置（resolved_at）→ 分数清零、退出名单

用法：python _smoke_risk_score.py
前提：api+worker 已部署（含 0368）；本脚本自签 staff JWT（HS256）。
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
USER_C = 52     # ghost×4 + speed×3 → high
USER_D = 72     # ghost×1 → none
USER_E = 76     # ghost×2 + speed×2 → watch
MARK = "m2smoke"
# 造证据时 hits 压着 L1 线（3）以下能不发用户通知——但 C 的 4/3 会发，
# 属设计内行为（教育性告知），清理时按水位删。
EVENTS = [
    # (user, agent, hits)
    (USER_C, f"ghost:{MARK}-c", 4),
    (USER_C, f"speed:{MARK}-c", 3),
    (USER_D, f"ghost:{MARK}-d", 1),
    (USER_E, f"ghost:{MARK}-e", 2),
    (USER_E, f"speed:{MARK}-e", 2),
]


def sh(cmd: str) -> str:
    return subprocess.run(
        cmd, shell=True, capture_output=True, text=True
    ).stdout.strip()


def psql(sql: str) -> str:
    # -q 静默命令标签（INSERT...RETURNING 也会在 -t 下打 "INSERT 0 1"）
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

FAILED = []
# 副作用水位：只删本次 tick 之后新增的消息/告警，不碰存量
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


def data_of(r: object) -> dict:
    return (r or {}).get("data") or {}


def trigger_job(job: str) -> bool:
    """手触指定 job；遇「锁未抢到」自动重触（与 10 分钟定时 tick 竞争）。"""
    for attempt in range(4):
        st, r = call(
            "POST", "/api/v1/admin/jobs/run", T_ROOT, {"job": job}
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


def score_row(uid: int) -> tuple[int, str]:
    """(score, band)；无行返回 (-1, '')。"""
    row = psql(
        "SELECT score || '|' || band FROM user_risk_scores "
        f"WHERE user_id = {uid}"
    )
    if not row.strip():
        return -1, ""
    s, _, b = row.partition("|")
    return int(s), b


def wait_score(uid: int, want_band: str, timeout: int = 240) -> tuple[int, str]:
    """轮询等分数行落到目标档位，返回 (score, band)。"""
    deadline = time.time() + timeout
    last = (-1, "")
    while time.time() < deadline:
        last = score_row(uid)
        if last[1] == want_band:
            return last
        time.sleep(6)
    return last


def main() -> int:
    print("== 0. 清理旧冒烟数据 + 快照副作用水位 ==")
    psql(
        "DELETE FROM case_notes WHERE case_id IN "
        "(SELECT id FROM reports WHERE ref_type='cheat' AND ref_id IN "
        f"({USER_C},{USER_D},{USER_E})); "
        "DELETE FROM reports WHERE ref_type='cheat' AND ref_id IN "
        f"({USER_C},{USER_D},{USER_E}); "
        "DELETE FROM cheat_events WHERE user_id IN "
        f"({USER_C},{USER_D},{USER_E}) AND agent LIKE '%{MARK}%'; "
        "DELETE FROM user_risk_scores WHERE user_id IN "
        f"({USER_C},{USER_D},{USER_E});"
    )
    W_MARK["msg"] = psql("SELECT COALESCE(max(id), 0) FROM messages")
    W_MARK["sm"] = psql("SELECT COALESCE(max(id), 0) FROM staffmessages")

    print("== 1. 造证据 + 手触 risk_score ==")
    for uid, agent, hits in EVENTS:
        psql(
            "INSERT INTO cheat_events (user_id, agent, reason, hits) "
            f"VALUES ({uid}, '{agent}', 'M2 冒烟证据', {hits})"
        )
    check("证据行落库", all(score_row(u)[0] == -1 for u in (USER_C, USER_D, USER_E)))
    check("手触 risk_score（增量轮）", trigger_job("risk_score"))

    print("== 2. 公式与档位 ==")
    sc, band = wait_score(USER_C, "high")
    check("C 达 high 档", band == "high", f"score={sc} band={band}")
    check("C 分数≈63（ghost×4+speed×3）", sc >= 60, f"score={sc}")
    sd, bandd = wait_score(USER_D, "none")
    check("D 为 none 档", bandd == "none", f"score={sd}")
    check("D 分数≈13（ghost×1）", sd >= 10 and sd <= 16, f"score={sd}")
    check("C 分数 > D 分数", sc > sd, f"{sc} > {sd}")
    se, bande = wait_score(USER_E, "watch")
    check("E 达 watch 档", bande == "watch", f"score={se}")
    check("E 分数≈45（ghost×2+speed×2）", 40 <= se < 60, f"score={se}")

    print("== 3. high 档自动开案（无重复） ==")
    rows = psql(
        "SELECT id || ' | ' || priority || ' | ' || reason FROM reports "
        f"WHERE ref_type='cheat' AND ref_id={USER_C} AND status IN (0,2)"
    )
    check("C 有进行中案件", bool(rows.strip()), f"rows={rows!r}")
    if rows.strip():
        cid, pri, reason = [x.strip() for x in rows.split("|")]
        check("自动开案 priority=1", pri == "1", f"priority={pri}")
        check("reason 带风险分前缀", reason.startswith("风险分自动开案"), reason)
        n_att = psql(
            "SELECT count(*) FROM cheat_events "
            f"WHERE user_id={USER_C} AND case_id={cid}"
        )
        check("证据已归案", n_att == "2", f"n={n_att}")
        note = psql(
            "SELECT body FROM case_notes WHERE case_id="
            f"{cid} ORDER BY id LIMIT 1"
        )
        check("首条备注为系统自动开案", "系统自动开案" in note, note[:60])
    check("E（watch 档）不自动开案", True)
    n_e = psql(
        "SELECT count(*) FROM reports WHERE ref_type='cheat' "
        f"AND ref_id={USER_E} AND status IN (0,2)"
    )
    check("E 无进行中案件", n_e == "0", f"n={n_e}")
    check("二次手触 risk_score", trigger_job("risk_score"))
    time.sleep(20)
    n_dup = psql(
        "SELECT count(*) FROM reports WHERE ref_type='cheat' "
        f"AND ref_id={USER_C} AND status IN (0,2)"
    )
    check("二次 tick 不重复开案", n_dup == "1", f"n={n_dup}")

    print("== 4. 观察名单 API ==")
    st, r = call("GET", "/api/v1/admin/risk-scores?limit=200", T_ROOT)
    items = data_of(r).get("items", [])
    check("名单 200", st == 200, f"status={st}")
    hit_c = [i for i in items if i["user_id"] == USER_C]
    check("名单含 C", len(hit_c) == 1)
    check("C 行带 open_case", hit_c and hit_c[0]["open_case"] is not None)
    check("名单按分数倒序",
          [i["score"] for i in items] == sorted(
              (i["score"] for i in items), reverse=True))
    st, r = call(
        "GET", f"/api/v1/admin/risk-scores?band=watch&limit=200", T_ROOT
    )
    items_w = data_of(r).get("items", [])
    check("band=watch 过滤只回 watch",
          all(i["band"] == "watch" for i in items_w) and
          any(i["user_id"] == USER_E for i in items_w),
          f"n={len(items_w)}")
    st, r = call("GET", f"/api/v1/admin/users/{USER_C}/risk", T_ROOT)
    d = data_of(r)
    check("用户构成 200", st == 200, f"status={st}")
    fams = (d.get("shadow_breakdown") or {}).get("families") or {}
    check("构成含 ghost/speed 两族",
          "ghost" in fams and "speed" in fams, f"fams={list(fams)}")
    check("构成 hits 正确",
          fams.get("ghost", {}).get("hits") == 4 and
          fams.get("speed", {}).get("hits") == 3)
    check("构成含证据明细", len(d.get("evidence", [])) == 2,
          f"n={len(d.get('evidence', []))}")
    st, r = call("GET", f"/api/v1/admin/users/{USER_D}/risk", T_ROOT)
    check("无案用户构成也可查", st == 200 and
          data_of(r).get("score") == sd)
    st, r = call("GET", "/api/v1/admin/risk-scores?band=bogus", T_ROOT)
    check("非法 band 被 400 挡下", st == 400, f"status={st}")

    print("== 5. 30 天衰减（回拨 last_seen + updated_at 模拟时间流逝） ==")
    psql(
        "UPDATE cheat_events SET last_seen = last_seen - interval '30 days' "
        f"WHERE user_id = {USER_C} AND agent LIKE '%{MARK}%'; "
        "UPDATE user_risk_scores SET updated_at = updated_at - interval '31 days' "
        f"WHERE user_id = {USER_C}"
    )
    check("手触 risk_score", trigger_job("risk_score"))
    deadline = time.time() + 240
    sc2 = -1
    while time.time() < deadline:
        sc2, band2 = score_row(USER_C)
        if sc2 > 0 and sc2 < sc / 2:
            break
        time.sleep(6)
    check("衰减后不足原值一半", 0 < sc2 < sc / 2,
          f"{sc} → {sc2}（阈值 <{sc / 2:.1f}）")
    check("衰减后掉出 high 档", band2 != "high", f"band={band2}")

    print("== 6. 档位广播 + 处置解除 ==")
    sm_n = psql(
        "SELECT count(*) FROM staffmessages WHERE id > "
        f"{W_MARK['sm']} AND subject = '风险评分档位变化'"
    )
    check("档位变化广播了 staffmessages", sm_n != "0", f"n={sm_n}")
    sm_body = psql(
        "SELECT body FROM staffmessages WHERE id > "
        f"{W_MARK['sm']} AND subject = '风险评分档位变化' "
        "ORDER BY id DESC LIMIT 1"
    )
    check("广播带用户与分数", f"用户 #{USER_C}" in sm_body and
          "风险评分" in sm_body, sm_body[:70])
    # 处置证据 → resolved_at 进脏集 → 重算清零 → 退出名单
    psql(
        "UPDATE cheat_events SET resolved_at = now() "
        f"WHERE user_id = {USER_C} AND agent LIKE '%{MARK}%'"
    )
    check("手触 risk_score", trigger_job("risk_score"))
    deadline = time.time() + 240
    while time.time() < deadline:
        sc3, band3 = score_row(USER_C)
        if sc3 == 0 and band3 == "none":
            break
        time.sleep(6)
    check("处置后清零", sc3 == 0 and band3 == "none",
          f"score={sc3} band={band3}")
    st, r = call("GET", "/api/v1/admin/risk-scores?limit=200", T_ROOT)
    items2 = data_of(r).get("items", [])
    check("清零后仍可查（0 分留存）",
          any(i["user_id"] == USER_C and i["score"] == 0 for i in items2))

    print("== 7. 全量轮 ==")
    check("手触 risk_score_daily", trigger_job("risk_score_daily"))
    time.sleep(20)
    sc4, band4 = score_row(USER_C)
    check("全量轮后 C 仍为 0 分", sc4 == 0 and band4 == "none",
          f"score={sc4} band={band4}")

    print("== 8. 清理 ==")
    psql(
        "UPDATE cheat_events SET case_id = NULL WHERE case_id IN "
        "(SELECT id FROM reports WHERE ref_type='cheat' AND ref_id IN "
        f"({USER_C},{USER_D},{USER_E})); "
        "DELETE FROM case_notes WHERE case_id IN "
        "(SELECT id FROM reports WHERE ref_type='cheat' AND ref_id IN "
        f"({USER_C},{USER_D},{USER_E})); "
        "DELETE FROM reports WHERE ref_type='cheat' AND ref_id IN "
        f"({USER_C},{USER_D},{USER_E}); "
        "DELETE FROM cheat_events WHERE user_id IN "
        f"({USER_C},{USER_D},{USER_E}) AND agent LIKE '%{MARK}%'; "
        "DELETE FROM user_risk_scores WHERE user_id IN "
        f"({USER_C},{USER_D},{USER_E}); "
        f"DELETE FROM messages WHERE id > {W_MARK['msg']}; "
        f"DELETE FROM staffmessages WHERE id > {W_MARK['sm']};"
    )
    left = psql(
        "SELECT (SELECT count(*) FROM user_risk_scores WHERE user_id IN "
        f"({USER_C},{USER_D},{USER_E})) || '/' || "
        "(SELECT count(*) FROM cheat_events WHERE user_id IN "
        f"({USER_C},{USER_D},{USER_E}) AND agent LIKE '%{MARK}%')"
    )
    check("冒烟数据已清", left == "0/0", f"left={left}")

    print()
    if FAILED:
        print(f"冒烟失败 {len(FAILED)} 项：{FAILED}")
        return 1
    print("M2 冒烟全过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
