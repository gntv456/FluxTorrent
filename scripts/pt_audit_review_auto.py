# -*- coding: utf-8 -*-
"""资深 PT 种审员·自动审种链路实测（2026-10-21）。

覆盖自动审种（0400 批）的对外面，每条断言都会红：
  B 管理面读链：stats（影子四指标+今日计数+switch_hint）/ logs / replay
  J 引擎裁决出口：x-flux-engine 用途头闸 + auto-decide 同尺落库 +
    通知自动口径 + 邮件主题自定义模板（auto_review_mail_pass_subject）

关键背景（2026-10-21 实证，别再凭印象判锅）：
  * worker 引擎 `load_config` 等处有 `.bind(0)` 绑无占位符查询——
    sqlx 的 Parse 会带参数类型声明，Postgres 接受多声明的类型
    （`PREPARE p (int) AS SELECT 1` 合法），故 bind(0) 是无害
    no-op，引擎配置读取正常。别用 psql `\bind`（不声明类型会真报
    "bind message supplies 1 parameters..."）去推 sqlx 的行为。
  * 免审连击满 5 后上传即免审（0077）：测引擎裁决前必须把 seed 号
    的 approve_streak / publish_credits 复位，否则新种直接过审、
    引擎裁决走 NotPending 跳过，断言会误判「模板没生效」。

姊妹篇（人工链路）：scripts/pt_audit_review_chain.py
跑法：栈已在跑，`python scripts/pt_audit_review_auto.py`
数据自清：临时种子/账号/设定改动收尾复位。
"""
import sys
import time
import json
import hashlib
import subprocess
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from pt_audit_lib import call, ok, summary  # noqa: E402

BASE = "http://127.0.0.1:8080/api/v1"
TAG = "ra" + str(int(time.time()))[-6:]
FAILED = []


def check(name, cond, detail=""):
    if not ok(name, cond, detail):
        FAILED.append(name)


def q(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-q", "-t", "-A", "-c", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode:
        raise RuntimeError(r.stderr.strip()[:300])
    return (r.stdout or "").strip()


def login_retry(user, pw):
    for i in range(10):
        s, r = call("POST", "/auth/login", {"username": user, "password": pw})
        if s == 200 and (r.get("data") or {}).get("token"):
            return r["data"]["token"]
        time.sleep(min(2 + i * 2, 15))
    raise SystemExit("login failed: %s %s" % (user, r.get("message")))


# ---------- bencode / 上传（与 D 批同款夹具） ----------
def benc(obj, out=None):
    out = bytearray() if out is None else out
    if isinstance(obj, int):
        out += b"i" + str(obj).encode() + b"e"
    elif isinstance(obj, bytes):
        out += b"%d:" % len(obj) + obj
    elif isinstance(obj, list):
        out += b"l"
        for v in obj:
            benc(v, out)
        out += b"e"
    elif isinstance(obj, dict):
        out += b"d"
        for k in sorted(obj.keys()):
            benc(k, out)
            benc(obj[k], out)
        out += b"e"
    return bytes(out)


BOUNDARY = "----raprobe"


def tor(name, length=1_048_576, piece_len=16384):
    info = {b"piece length": piece_len, b"name": name.encode(),
            b"private": 1}
    n = max(1, (length + piece_len - 1) // piece_len)
    info[b"pieces"] = hashlib.sha1(name.encode()).digest() * n
    info[b"length"] = length
    root = {b"info": info, b"creation date": int(time.time())}
    return benc(root)


def upload_torrent(tok, tor_bytes, **fields):
    crlf = chr(13) + chr(10)
    dq = chr(34)
    parts = b""
    for k, v in fields.items():
        if v is None:
            continue
        parts += (("--" + BOUNDARY + crlf
                   + "Content-Disposition: form-data; name=" + dq + k + dq
                   + crlf + crlf + str(v) + crlf)).encode()
    parts += (("--" + BOUNDARY + crlf
               + "Content-Disposition: form-data; name=" + dq + "file" + dq
               + "; filename=" + dq + "probe.torrent" + dq + crlf
               + "Content-Type: application/x-bittorrent" + crlf
               + crlf)).encode() + tor_bytes
    parts += (crlf + "--" + BOUNDARY + "--" + crlf).encode()
    req = urllib.request.Request(BASE + "/torrents", method="POST",
                                 data=parts)
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, json.loads(r.read().decode("utf-8", "replace"))
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read().decode("utf-8", "replace"))
        except Exception:
            return e.code, {"message": "<non-json>"}


def up(tok, name, **fields):
    f = {"category_id": 1, "name": name,
         "small_descr": "自动审种探针",
         "descr": "自动审种链路探针：正常长度描述，用于走完整审核动线。"
                  "本资源为测试用途，包含充分的说明文字以通过最低内容标准检查。"
                  "补充来源与出处说明，长度足够通过 upload_min_descr_len 校验。"}
    f.update(fields)
    return upload_torrent(tok, tor(name), **f)


def tno(res):
    return (res.get("data") or {}) if isinstance(res, dict) else {}


def unlock(user, pw):
    """后台建的号带 must_reset_password，发种前必须先改一次密码。"""
    t = login_retry(user, pw)
    s, r = call("POST", "/me/password/change",
                {"old_password": pw, "new_password": pw + "x"}, token=t)
    if s != 200:
        raise SystemExit("改密失败 %s: %s %s" % (user, s, r.get("message")))
    return login_retry(user, pw + "x")


def engine_decide(admin, tid, approve=True, reason="引擎自动"):
    """引擎裁决出口（带 x-flux-engine 用途头；缺头应 403）。"""
    req = urllib.request.Request(
        BASE + "/admin/reviews/auto-decide", method="POST",
        data=json.dumps({"torrent_id": tid, "approve": approve,
                         "reason": reason}).encode())
    req.add_header("Content-Type", "application/json")
    req.add_header("Authorization", "Bearer " + admin)
    req.add_header("x-flux-engine", "auto-review")
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return resp.status, json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        return e.code, {"raw": e.read()[:200].decode("utf-8", "replace")}


# ================= 准备 =================
admin = login_retry("root", "password123")
u_seed = "ra_seed_" + TAG
PW = "Gate#123456"
s, r = call("POST", "/admin/adduser",
            {"username": u_seed, "password": PW,
             "email": "%s@audit.invalid" % u_seed}, token=admin)
check("C0 建探针账号", s == 200, "%s %s" % (s, r.get("message")))
uid_seed = int(q("SELECT id FROM users WHERE username='%s'" % u_seed))
q("UPDATE users SET class_id=1 WHERE id=%d" % uid_seed)
seed = unlock(u_seed, PW)

SAVED_MAIL = q("SELECT value FROM site_settings"
               " WHERE name='auto_review_mail_pass_subject'")
# 上传闸门原值必须先读后改（读放后面会把污染值当原值存下来，
# 「还原」实则固化污染——本轮实测踩过）
SAVED_GATES = ("upload_min_descr_len", "upload_require_screenshots",
               "upload_dup_policy", "upload_title_pattern",
               "upload_deny_approval_deny_count")
GATE_SAVED = {k: q("SELECT value FROM site_settings WHERE name='%s'" % k)
              for k in SAVED_GATES}


def setkv(k, v):
    q("INSERT INTO site_settings (name, value, descr, grp)"
      " VALUES ('%s','%s','probe','torrent')"
      " ON CONFLICT (name) DO UPDATE SET value=EXCLUDED.value" % (k, v))


def restore_kv(k, v):
    """按探针开始时的状态还原：空值=当时无此行 ⇒ 删键（见姊妹篇同款注记）。"""
    if v:
        setkv(k, v)
    else:
        q("DELETE FROM site_settings WHERE name='%s'" % k)


setkv("upload_min_descr_len", "0")
setkv("upload_require_screenshots", "0")
setkv("upload_dup_policy", "warn")
setkv("upload_title_pattern", "")
setkv("upload_deny_approval_deny_count", "99")

t_ids = []
try:
    # ================= B 自动审种管理面 =================
    s, r = call("GET", "/admin/auto-review/stats", token=admin)
    check("B1 auto-review/stats 200", s == 200,
          "status=%s body=%s" % (s, str(r)[:200]))
    sd = tno(r) if s == 200 else {}
    check("B1b stats 字段齐全",
          all(k in sd for k in ("switch_hint", "shadow", "today",
                                "pending_human", "rule_health")),
          str(sorted(sd.keys()))[:160])
    check("B1c switch_hint 回显非空（后台改得动、界面看得到）",
          bool(sd.get("switch_hint")), repr(sd.get("switch_hint"))[:80])
    s, r = call("GET", "/admin/auto-review/logs?limit=5", token=admin)
    check("B2 auto-review/logs 200", s == 200, "status=%s" % s)
    s, r = call("POST", "/admin/auto-review/replay", {}, token=admin)
    check("B3 auto-review/replay 200", s == 200,
          "status=%s body=%s" % (s, str(r)[:200]))
    if s == 200:
        rd = tno(r)
        check("B3b replay 返回重放计数",
              all(k in rd for k in ("replayed", "same", "changed")),
              str(rd)[:160])

    # ================= J 引擎裁决端点 =================
    # 连击复位：免审连击满 5 后上传即免审（0077），引擎裁决会走
    # NotPending 跳过、不产新消息——断言须在干净的连击态下做。
    q("UPDATE users SET approve_streak=0, publish_credits=0 WHERE id=%d"
      % uid_seed)
    s, r = up(seed, "ra-%s-engine" % TAG)
    te = int(tno(r)["id"])
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": te, "approve": True}, token=admin)
    check("J0 前置人工过审", s == 200, "%s" % s)
    s, r = up(seed, "ra-%s-engine2" % TAG)
    te2 = int(tno(r)["id"])
    # 缺用途头：即使有有效 staff JWT 也应 403（内部端点）
    req = urllib.request.Request(
        BASE + "/admin/reviews/auto-decide", method="POST",
        data=json.dumps({"torrent_id": te2, "approve": True,
                         "reason": "引擎自动"}).encode())
    req.add_header("Content-Type", "application/json")
    req.add_header("Authorization", "Bearer " + admin)
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            s2 = resp.status
    except urllib.error.HTTPError as e:
        s2 = e.code
    check("J1 缺 x-flux-engine 头被拒(403)", s2 == 403, "got %s" % s2)
    s2, body = engine_decide(admin, te2)
    check("J2 引擎裁决 200", s2 == 200, "status=%s %s" % (s2, body))
    st = q("SELECT approval_status FROM torrents WHERE id=%d" % te2)
    check("J3 引擎裁决同尺落库(1)", st == "1", st)
    subj = q("SELECT subject FROM messages WHERE receiver_id=%d"
             " AND kind='review_approved' ORDER BY id DESC LIMIT 1"
             % uid_seed)
    check("J4 引擎过审通知用自动口径", "自动" in subj, subj[:80])
    # J5 邮件主题自定义模板（by_auto 才读；人工裁决不读模板）
    setkv("auto_review_mail_pass_subject", "【探针站点】{torrent} 已自动放行")
    s, r = up(seed, "ra-%s-engine3" % TAG)
    te3 = int(tno(r)["id"])
    st = q("SELECT approval_status FROM torrents WHERE id=%d" % te3)
    check("J5a 复位连击后新种进待审（未被免审抢跑）", st == "0", st)
    s2, body = engine_decide(admin, te3)
    subj3 = q("SELECT subject FROM messages WHERE receiver_id=%d"
              " AND kind='review_approved' ORDER BY id DESC LIMIT 1"
              % uid_seed)
    check("J5b 自动过审邮件主题模板生效",
          s2 == 200 and "已自动放行" in subj3 and "ra-" in subj3,
          "status=%s subj=%s" % (s2, subj3[:80]))
    t_ids += [te, te2, te3]

finally:
    # ================= 清理 =================
    for t in t_ids:
        try:
            q("DELETE FROM torrent_operation_logs WHERE torrent_id=%d" % t)
            q("DELETE FROM messages WHERE params->>'id' = '%d'" % t)
            q("DELETE FROM promotions WHERE torrent_id=%d" % t)
            q("DELETE FROM snatches WHERE torrent_id=%d" % t)
            q("DELETE FROM auto_review_log WHERE torrent_id=%d" % t)
            q("DELETE FROM torrents WHERE id=%d" % t)
        except Exception:
            pass
    try:
        call("POST", "/admin/users/status",
             {"user_id": uid_seed, "status": 2}, token=admin)
        q("DELETE FROM user_permissions WHERE user_id=%d" % uid_seed)
        call("DELETE", "/admin/users/%d" % uid_seed, token=admin)
    except Exception:
        pass
    for k, v in GATE_SAVED.items():
        restore_kv(k, v)
    if SAVED_MAIL:
        setkv("auto_review_mail_pass_subject", SAVED_MAIL)
    else:
        q("DELETE FROM site_settings WHERE"
          " name='auto_review_mail_pass_subject'")

print()
if FAILED:
    print("RED %d 条：" % len(FAILED))
    for f in FAILED:
        print("  - " + f)
    sys.exit(1)
print("GREEN：自动审种链路不变式通过")
summary()
