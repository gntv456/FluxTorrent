#!/usr/bin/env python3
"""M3 处置与申诉闭环冒烟（0369）：warn 顺延 / ban 四眼 pending→approve /
停权闸门（下载）/ 一次性令牌申诉 / 复核 overturned 反向解除 / 到期清扫。

用法：python _smoke_cheat_sanction.py
前提：api+worker 已部署（含 0369）；本脚本自签 staff JWT（HS256）。
不改动用户余额（不测 reclaim_up，避免污染对账）。
"""
import base64
import hashlib
import hmac
import json
import re
import subprocess
import sys
import time
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080"
ROOT = 1        # class 99：决定人
AUDITOR = 4306  # class 94：批准人 / 复核人（四眼另一侧）
USER_A = 40     # 目标用户（class 1）
TORRENT_DL = 40  # 已过审种子（下载闸门测试用）
MARK = "m3smoke"


def sh(cmd: str) -> str:
    return subprocess.run(
        cmd, shell=True, capture_output=True, text=True
    ).stdout.strip()


def psql(sql: str) -> str:
    r = subprocess.run(
        f'docker exec flux-postgres psql -U flux -d fluxtorrent -t -A -q -c "{sql}"',
        shell=True,
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        print(f"    ! psql 失败：{r.stderr.strip()[:200]}")
    return "\n".join(r.stdout.splitlines()).strip()


def secret() -> str:
    return sh("docker exec flux-api printenv JWT_SECRET")


def b64url(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


def token(uid: int, cls: int) -> str:
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
T_USER = token(USER_A, 1)

FAILED = []
W_MSG = "0"


def check(name: str, cond: bool, extra: str = "") -> None:
    mark = "✓" if cond else "✗"
    print(f"  {mark} {name}" + (f" — {extra}" if extra else ""))
    if not cond:
        FAILED.append(name)


def call(
    method: str, path: str, tok: str, body: dict | None = None
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
    for _ in range(4):
        st, r = call("POST", "/api/v1/admin/jobs/run", T_ROOT, {"job": job})
        if st != 200:
            time.sleep(5)
            continue
        tid = (r.get("data") or {}).get("id")
        for _ in range(30):
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
                break
            if status == "failed":
                print(f"    trigger {tid} 失败：{result[:80]}")
                return False
    return False


def open_case() -> int:
    st, r = call(
        "POST",
        "/api/v1/admin/cheat-cases/open",
        T_ROOT,
        {"user_id": USER_A, "priority": 1, "note": f"{MARK} 冒烟开案"},
    )
    if st != 200 or not (data_of(r) or {}).get("id"):
        raise RuntimeError(
            f"开案失败：HTTP {st} {json.dumps(r, ensure_ascii=False)[:160]}"
            "（多半是该用户仍有进行中案件 → 上一步没结案）"
        )
    return int(data_of(r)["id"])


def case_state(cid: int) -> tuple[int, str, str]:
    """(status, sanction_kind, sanction_state)"""
    row = psql(
        "SELECT status || '|' || coalesce(sanction->>'kind', '') || '|' "
        f"|| coalesce(sanction->>'state', '') FROM reports WHERE id = {cid}"
    )
    if not row.strip():
        return -1, "", ""
    a, _, b = row.partition("|")
    b, _, c = b.partition("|")
    return int(a), b, c


def user_sanction() -> tuple[str, str, str]:
    """(cheat_banned_until, warned_until, appeal_token_hash IS NULL)"""
    return tuple(  # type: ignore[return-value]
        psql(
            "SELECT coalesce(cheat_banned_until::text, '-') || '|' "
            "|| coalesce(warned_until::text, '-') || '|' || "
            "(appeal_token_hash IS NULL)::text FROM users "
            f"WHERE id = {USER_A}"
        ).split("|")
    )


def main() -> int:
    global W_MSG
    print("== 0. 清理旧冒烟数据 + 副作用水位 ==")
    psql(
        "DELETE FROM case_notes WHERE case_id IN (SELECT id FROM reports "
        f"WHERE ref_type='cheat' AND reason LIKE '%{MARK}%'); "
        "DELETE FROM appeals WHERE user_id = "
        f"{USER_A} AND (body LIKE '%{MARK}%' OR case_id IN "
        f"(SELECT id FROM reports WHERE ref_type='cheat' AND reason "
        f"LIKE '%{MARK}%')); "
        # cheat_events.case_id 有 FK（无 ON DELETE SET NULL）——先解开引用
        # 再删案件，否则 reports 的 DELETE 整条失败（本脚本曾在清理段静默
        # 失败、把未结案件留给下一轮，第 1 步开案撞 uq_cheat_case_open）
        "UPDATE cheat_events SET case_id = NULL WHERE case_id IN "
        f"(SELECT id FROM reports WHERE ref_type='cheat' AND reason "
        f"LIKE '%{MARK}%'); "
        "DELETE FROM reports WHERE ref_type='cheat' AND reason LIKE "
        f"'%{MARK}%'; "
        "DELETE FROM cheat_events WHERE user_id = "
        f"{USER_A} AND agent LIKE '%{MARK}%'; "
        "UPDATE users SET cheat_banned_until = NULL, warned_until = NULL, "
        "warned_reason = NULL, appeal_token_hash = NULL, "
        f"appeal_token_expires_at = NULL WHERE id = {USER_A};"
    )
    W_MSG = psql("SELECT COALESCE(max(id), 0) FROM messages")

    # 证据：供案件归案（resolve_evidence 会处置它）
    psql(
        "INSERT INTO cheat_events (user_id, agent, reason, hits) VALUES "
        f"({USER_A}, 'ghost:{MARK}-1', 'M3 冒烟证据', 4)"
    )

    print("== 1. warn 处置：立即生效 + 顺延 + 关案 ==")
    cid = open_case()
    check("开案成功", cid > 0, f"case #{cid}")
    st, r = call(
        "POST",
        f"/api/v1/admin/cheat-cases/{cid}/resolve",
        T_ROOT,
        {"sanction": "warn", "warn_weeks": 2, "close": True,
         "note": f"{MARK} warn"},
    )
    check("resolve warn 200", st == 200, f"HTTP {st}")
    status, kind, sstate = case_state(cid)
    check("warn 立即 applied", (kind, sstate) == ("warn", "applied"),
          f"{kind}/{sstate}")
    check("warn 后案件关闭", status == 1, f"status={status}")
    ban_until, warn_until, _ = user_sanction()
    check("warned_until 已设置", warn_until != "-" and ban_until == "-",
          f"warned={warn_until[:16]}")
    evs = psql(
        "SELECT count(*) FROM cheat_events WHERE case_id = "
        f"{cid} AND resolved_at IS NOT NULL"
    )
    check("证据已归案处置", evs == "1", f"{evs} 条")

    print("== 2. warn 顺延（GREATEST 从现有到期时刻起算）==")
    psql(
        f"UPDATE users SET warned_until = now() + interval '10 days' "
        f"WHERE id = {USER_A}"
    )
    cid2 = open_case()
    call(
        "POST",
        f"/api/v1/admin/cheat-cases/{cid2}/resolve",
        T_ROOT,
        {"sanction": "warn", "warn_weeks": 1, "note": f"{MARK} warn2"},
    )
    ok_shift = psql(
        "SELECT (warned_until > now() + interval '16 days' "
        "AND warned_until < now() + interval '18 days')::text "
        f"FROM users WHERE id = {USER_A}"
    )
    check("10d + 1w ≈ 17d（顺延非覆盖）", ok_shift == "true", f"={ok_shift}")

    print("== 3. ban 走四眼：pending 不生效 ==")
    psql(
        f"UPDATE users SET warned_until = NULL WHERE id = {USER_A}; "
        "INSERT INTO cheat_events (user_id, agent, reason, hits) "
        f"VALUES ({USER_A}, 'ghost:{MARK}-2', 'M3 冒烟证据2', 5)"
    )
    cid3 = open_case()
    st, r = call(
        "POST",
        f"/api/v1/admin/cheat-cases/{cid3}/resolve",
        T_ROOT,
        {"sanction": "ban", "ban_days": 3, "close": True,
         "note": f"{MARK} ban"},
    )
    check("resolve ban 200", st == 200, f"HTTP {st}")
    status, kind, sstate = case_state(cid3)
    check("ban 落 pending", (kind, sstate) == ("ban", "pending"),
          f"{kind}/{sstate}")
    check("pending 不关案", status in (0, 2), f"status={status}")
    ban_until, _, tok_null = user_sanction()
    check("pending 不落停权", ban_until == "-", f"ban_until={ban_until[:16]}")

    print("== 4. 四眼拒绝 + 另一人批准 ==")
    st, r = call(
        "POST",
        f"/api/v1/admin/cheat-cases/{cid3}/approve",
        T_ROOT,
        {"note": f"{MARK} 自我批准"},
    )
    check("决定人自我批准被拒（四眼）", st == 400,
          f"HTTP {st} {(r or {}).get('message', '')[:30]}")
    st, r = call(
        "POST",
        f"/api/v1/admin/cheat-cases/{cid3}/approve",
        T_AUD,
        {"note": f"{MARK} 批准"},
    )
    check("另一 staff 批准 200", st == 200, f"HTTP {st}")
    status, kind, sstate = case_state(cid3)
    check("批准后 applied + 关案",
          (kind, sstate, status) == ("ban", "applied", 1),
          f"{kind}/{sstate}/status={status}")
    ban_until, _, tok_null = user_sanction()
    near3d = psql(
        "SELECT (cheat_banned_until > now() + interval '2 days' AND "
        "cheat_banned_until < now() + interval '4 days')::text "
        f"FROM users WHERE id = {USER_A}"
    )
    check("停权 3 天已生效", near3d == "true", f"ban_until={ban_until[:16]}")
    check("签发一次性申诉令牌", tok_null == "false", f"hash_null={tok_null}")

    print("== 5. 停权闸门：下载被拒 ==")
    st, r = call("GET", f"/api/v1/torrents/{TORRENT_DL}/download", T_USER)
    check("停权期间下载 403/404", st in (403, 404),
          f"HTTP {st}（期望 403）")

    print("== 6. 一次性令牌申诉 ==")
    body = psql(
        "SELECT body FROM messages WHERE receiver_id = "
        f"{USER_A} AND id > {W_MSG} AND subject LIKE '%处罚通知%' "
        "ORDER BY id DESC LIMIT 1"
    )
    m = re.search(r"\n(\d+\.[0-9a-f]{16,})\n", body + "\n")
    check("站内信含令牌明文", m is not None,
          f"原始长度 {len(body)}")
    tok_raw = m.group(1) if m else ""
    st, r = call(
        "POST",
        "/api/v1/appeals/token",
        T_ROOT,
        {"token": tok_raw, "body": f"{MARK} 令牌申诉：请求复核"},
    )
    check("令牌申诉 200", st == 200, f"HTTP {st}")
    ap_id = int(data_of(r).get("id") or 0) if st == 200 else 0
    row = psql(
        "SELECT kind || '|' || coalesce(case_id::text, '-') || '|' || "
        "(token_hash IS NOT NULL)::text FROM appeals "
        f"WHERE id = {ap_id}"
    )
    check("申诉落库（cheat/绑定案件/留痕令牌）",
          row == f"cheat|{cid3}|true", f"{row}")
    _, _, tok_null = user_sanction()
    check("令牌用后即焚", tok_null == "true", f"hash_null={tok_null}")
    st, r = call(
        "POST",
        "/api/v1/appeals/token",
        T_ROOT,
        {"token": tok_raw, "body": f"{MARK} 重复用同一令牌"},
    )
    check("重复用令牌被拒", st == 400,
          f"HTTP {st} {(r or {}).get('message', '')[:30]}")

    print("== 7. 申诉队列可见 ==")
    st, r = call("GET", "/api/v1/admin/cheat-appeals", T_AUD)
    check("队列 200", st == 200, f"HTTP {st}")
    ids = [x.get("id") for x in (r.get("data") or [])]
    check("队列含本申诉", ap_id in ids, f"{len(ids)} 条")

    print("== 8. 四眼复核：决定人不得复核 ==")
    st, r = call(
        "POST",
        f"/api/v1/admin/cheat-appeals/{ap_id}/resolve",
        T_ROOT,
        {"outcome": "overturned", "note": f"{MARK} 自我复核"},
    )
    check("决定人复核自己处置被拒", st == 400,
          f"HTTP {st} {(r or {}).get('message', '')[:30]}")

    print("== 9. 复核 overturned：反向解除 ==")
    st, r = call(
        "POST",
        f"/api/v1/admin/cheat-appeals/{ap_id}/resolve",
        T_AUD,
        {"outcome": "overturned", "note": f"{MARK} 推翻，恢复账号"},
    )
    check("复核 overturned 200", st == 200, f"HTTP {st}")
    rev = (data_of(r) or {}).get("reversed") or []
    check("返回反向解除明细", len(rev) > 0, f"{rev}")
    ban_until, _, _ = user_sanction()
    check("停权已解除", ban_until == "-", f"ban_until={ban_until[:16]}")
    status, kind, sstate = case_state(cid3)
    check("sanction.state = reversed", sstate == "reversed", f"{sstate}")
    arec = psql(
        "SELECT status || '|' || coalesce(outcome, '-') FROM appeals "
        f"WHERE id = {ap_id}"
    )
    check("申诉 accepted/overturned", arec == "accepted|overturned", f"{arec}")
    fp = psql(
        "SELECT count(*) FROM cheat_events WHERE case_id = "
        f"{cid3} AND false_positive IS TRUE"
    )
    check("本案证据标 false_positive", fp != "0", f"{fp} 条")

    print("== 10. 登录用户自助申诉 ==")
    psql(
        "INSERT INTO cheat_events (user_id, agent, reason, hits) "
        f"VALUES ({USER_A}, 'ghost:{MARK}-3', 'M3 冒烟证据3', 6)"
    )
    cid4 = open_case()
    st, r = call(
        "POST",
        "/api/v1/cheat-appeals",
        T_USER,
        {"case_id": cid4, "body": f"{MARK} 登录用户自助申诉"},
    )
    check("自助申诉 200", st == 200, f"HTTP {st}")
    st, r = call("GET", "/api/v1/my/cheat-appeals", T_USER)
    mine = (r.get("data") or []) if st == 200 else []
    check("我的申诉列表含 2 条", st == 200 and len(mine) >= 2,
          f"HTTP {st} n={len(mine)}")

    print("== 11. 停权到期自动清扫 ==")
    # 上一步的 cid4 仍开着（同用户至多一个进行中案件）——先以 none 结案
    call(
        "POST",
        f"/api/v1/admin/cheat-cases/{cid4}/resolve",
        T_ROOT,
        {"sanction": "none", "close": True, "note": f"{MARK} 结案"},
    )
    cid5 = open_case()
    call(
        "POST",
        f"/api/v1/admin/cheat-cases/{cid5}/resolve",
        T_ROOT,
        {"sanction": "ban", "ban_days": 3, "note": f"{MARK} ban2"},
    )
    call(
        "POST",
        f"/api/v1/admin/cheat-cases/{cid5}/approve",
        T_AUD,
        {"note": f"{MARK} 批准2"},
    )
    # 伪造「已到期」：停权时刻与 applied_at 都拨回过去
    psql(
        f"UPDATE users SET cheat_banned_until = now() - interval '1 hour' "
        f"WHERE id = {USER_A}; "
        "UPDATE reports SET sanction = jsonb_set(sanction, '{applied_at}', "
        "to_jsonb((now() - interval '10 days')::text)) "
        f"WHERE id = {cid5};"
    )
    check("手触 cheat_enforce", trigger_job("cheat_enforce"))
    ban_until, _, _ = user_sanction()
    check("到期停权已清扫", ban_until == "-", f"ban_until={ban_until[:16]}")
    _, _, sstate5 = case_state(cid5)
    check("处置标记 expired", sstate5 == "expired", f"{sstate5}")

    print("== 12. 清理 ==")
    psql(
        "DELETE FROM case_notes WHERE case_id IN (SELECT id FROM reports "
        f"WHERE ref_type='cheat' AND reason LIKE '%{MARK}%'); "
        f"DELETE FROM appeals WHERE user_id = {USER_A} AND case_id IN "
        f"(SELECT id FROM reports WHERE ref_type='cheat' AND reason "
        f"LIKE '%{MARK}%'); "
        "UPDATE cheat_events SET case_id = NULL WHERE case_id IN "
        f"(SELECT id FROM reports WHERE ref_type='cheat' AND reason "
        f"LIKE '%{MARK}%'); "
        "DELETE FROM reports WHERE ref_type='cheat' AND reason LIKE "
        f"'%{MARK}%'; "
        "DELETE FROM cheat_events WHERE user_id = "
        f"{USER_A} AND agent LIKE '%{MARK}%'; "
        "UPDATE users SET cheat_banned_until = NULL, warned_until = NULL, "
        "warned_reason = NULL, appeal_token_hash = NULL, "
        f"appeal_token_expires_at = NULL WHERE id = {USER_A}; "
        f"DELETE FROM messages WHERE id > {W_MSG} AND receiver_id = "
        f"{USER_A} AND subject LIKE '%处罚通知%';"
    )
    left = psql(
        "SELECT count(*) FROM reports WHERE ref_type='cheat' AND reason "
        f"LIKE '%{MARK}%'"
    )
    check("清理归零", left == "0", f"残留 {left}")

    print()
    if FAILED:
        print(f"✗ {len(FAILED)} 项失败：")
        for f in FAILED:
            print(f"   - {f}")
        return 1
    print("✓ 全部通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
