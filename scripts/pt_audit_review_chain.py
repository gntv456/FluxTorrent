# -*- coding: utf-8 -*-
"""资深 PT 种审员·人工链路实测（2026-10-21）：种审全动线不变式。

覆盖种审员一天的真实动线，每条断言都会红：
  A 队列读链：待审列表结构/分页/排序/筛选/拒因字典/越权守卫
  C 上传链路：普通用户发种 → 待审（approval_status=0）
  D 队列可见性：待审种进队列、分页不丢最新、最老优先、上传者筛选
  E 单条过审：状态/approved_at/连击/信用分/站内信/留痕
  F 单条拒审：字典拒因/deny_count/信用分扣减/通知含拒因
  G 参数校验与利益冲突：空理由/通过带拒因/坏拒因/重复裁决/
    自审拦截（staff 级上传者形态）/无权限用户 403
  H 批量裁决：handled/skipped 语义与自审跳过
  I 暂缓/恢复：0→4→0 全链路 + 暂缓不出现在待审视图
  K 过审副作用：自动促销规则命中（连击复位防免审抢跑）
  L 通知链：messages kind/params/三语渲染键
  M 拒因字典 CRUD round-trip

姊妹篇（自动审种链路）：scripts/pt_audit_review_auto.py
跑法：栈已在跑，`python scripts/pt_audit_review_chain.py`
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
TAG = "rc" + str(int(time.time()))[-6:]
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


BOUNDARY = "----rcprobe"


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
         "small_descr": "种审链路探针",
         "descr": "种审链路探针：正常长度描述，用于走完整审核动线。"
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


# ================= 准备 =================
admin = login_retry("root", "password123")
u_seed = "rc_seed_" + TAG
u_rev = "rc_rev_" + TAG
PW = "Gate#123456"
for uname in (u_seed, u_rev):
    s, r = call("POST", "/admin/adduser",
                {"username": uname, "password": PW,
                 "email": "%s@audit.invalid" % uname}, token=admin)
    check("C0 建探针账号 %s" % uname, s == 200, "%s %s" % (s, r.get("message")))
uid_seed = int(q("SELECT id FROM users WHERE username='%s'" % u_seed))
uid_rev = int(q("SELECT id FROM users WHERE username='%s'" % u_rev))
q("UPDATE users SET class_id=1 WHERE id=%d" % uid_seed)
# 种审员探针号：class 91（<92 免审线，发的种会进待审）+ 显式授予
# staff.panel / torrent.review（class 91 默认不含这两个权限）
q("UPDATE users SET class_id=91 WHERE id=%d" % uid_rev)
q("INSERT INTO user_permissions (user_id, permission_key, granted)"
  " VALUES (%d, 'staff.panel', true), (%d, 'torrent.review', true)"
  % (uid_rev, uid_rev))
seed = unlock(u_seed, PW)
rev = unlock(u_rev, PW)

# 上传闸门设定：探针期放宽，收尾还原
GATE_KEYS = ("upload_min_descr_len", "upload_require_screenshots",
             "upload_dup_policy", "upload_title_pattern",
             "upload_deny_approval_deny_count")
SAVED = {k: q("SELECT value FROM site_settings WHERE name='%s'" % k)
         for k in GATE_KEYS}


def setkv(k, v):
    q("INSERT INTO site_settings (name, value, descr, grp)"
      " VALUES ('%s','%s','probe','torrent')"
      " ON CONFLICT (name) DO UPDATE SET value=EXCLUDED.value" % (k, v))


def restore_kv(k, v):
    """按探针开始时的状态还原：读到空值=当时库里没有这一行 ⇒ 删键，
    而不是留一个空串行（`if v:` 的老写法会把「本来不存在的键」留成
    探针值，站点设定被悄悄改写）。"""
    if v:
        setkv(k, v)
    else:
        q("DELETE FROM site_settings WHERE name='%s'" % k)


setkv("upload_min_descr_len", "0")
setkv("upload_require_screenshots", "0")
setkv("upload_dup_policy", "warn")
setkv("upload_title_pattern", "")
setkv("upload_deny_approval_deny_count", "99")

t_ids, t_ids2 = [], []
tp = tpr = None
try:
    # ================= A 队列读链 =================
    s, r = call("GET", "/admin/reviews?limit=5&offset=0", token=admin)
    check("A1 审核队列 200", s == 200, "%s %s" % (s, r.get("message")))
    d = tno(r)
    check("A2 队列信封字段齐全",
          all(k in d for k in ("total", "limit", "offset", "oldest_first",
                               "items")),
          str(sorted(d.keys()))[:120])
    s, r = call("GET", "/admin/reviews", token=seed)
    check("A3 普通用户读队列被拒", s in (401, 403), "got %s" % s)
    s, r = call("GET", "/admin/reviews")
    check("A4 匿名读队列被拒", s in (401, 403), "got %s" % s)
    s, r = call("GET", "/admin/deny-reasons", token=admin)
    check("A5 拒因字典 200", s == 200, "%s %s" % (s, r.get("message")))
    reasons = tno(r) or []
    check("A6 拒因字典非空", len(reasons) >= 7, "n=%d" % len(reasons))

    # ================= C 上传链路 =================
    for i in range(4):
        s, r = up(seed, "rc-%s-%d 待审种子" % (TAG, i))
        tid = tno(r).get("id")
        check("C%d 发种进待审" % (i + 1), s == 200 and bool(tid),
              "%s %s" % (s, r.get("message")))
        if tid:
            t_ids.append(int(tid))
    st = q("SELECT approval_status FROM torrents WHERE id=%d" % t_ids[0])
    check("C5 落库为待审(0)", st == "0", "got %s" % st)

    # ================= D 队列可见性 =================
    s, r = call("GET", "/admin/reviews?limit=50&offset=0&oldest_first=false",
                token=admin)
    ids = [x["id"] for x in tno(r).get("items", [])]
    check("D1 四个待审种全部可见（最新优先）",
          all(t in ids for t in t_ids),
          "queue=%s probe=%s" % (ids[:8], t_ids))
    item = next((x for x in tno(r).get("items", [])
                 if x["id"] == t_ids[0]), None)
    check("D2 队列条目带内容面（简介/上传者/查重）",
          item is not None and item.get("small_descr")
          and item.get("owner_name") == u_seed
          and "descr_excerpt" in item,
          str(item)[:200] if item else "missing")
    s, r = call("GET", "/admin/reviews?limit=200", token=admin)
    ids_old = [x["id"] for x in tno(r).get("items", [])]
    pos = [ids_old.index(t) for t in t_ids if t in ids_old]
    check("D3 默认最老优先：4 条探针种都在队里", len(pos) == 4, str(pos))
    s, r = call("GET", "/admin/reviews?limit=50&owner_id=%d" % uid_seed,
                token=admin)
    f_ids = [x["id"] for x in tno(r).get("items", [])]
    check("D4 owner_id 筛选只回该上传者的种",
          f_ids and all(t in t_ids for t in f_ids),
          "filtered=%s" % f_ids[:8])
    check("D4b total 与筛选一致",
          tno(r).get("total") == len(f_ids),
          "total=%s len=%s" % (tno(r).get("total"), len(f_ids)))

    # ================= E 单条过审 =================
    before = q("SELECT approve_streak, publish_credits FROM users"
               " WHERE id=%d" % uid_seed)
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": t_ids[0], "approve": True, "reason": ""},
                token=admin)
    check("E1 过审 200", s == 200, "%s %s" % (s, r.get("message")))
    row = q("SELECT approval_status, approved_at IS NOT NULL,"
            " deny_reason_id IS NULL FROM torrents WHERE id=%d" % t_ids[0])
    check("E2 过审落库：status=1 + approved_at + 无拒因",
          row == "1|t|t", row)
    after = q("SELECT approve_streak, publish_credits FROM users"
              " WHERE id=%d" % uid_seed)
    bs, as_ = before.split("|"), after.split("|")
    check("E3 连击/信用分 +1",
          int(as_[0]) == int(bs[0]) + 1 and int(as_[1]) == int(bs[1]) + 1,
          "%s -> %s" % (before, after))
    msg = q("SELECT count(*) FROM messages WHERE receiver_id=%d"
            " AND kind='review_approved'" % uid_seed)
    check("E4 过审站内信落库", int(msg) >= 1, msg)
    log = q("SELECT count(*) FROM torrent_operation_logs WHERE torrent_id=%d"
            " AND action='approve'" % t_ids[0])
    check("E5 审核留痕 approve", int(log) == 1, log)

    # ================= F 单条拒审（字典拒因） =================
    rid = int(reasons[0]["id"])
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": t_ids[1], "approve": False,
                 "reason": "", "deny_reason_id": rid}, token=admin)
    check("F1 字典拒因拒审 200", s == 200, "%s %s" % (s, r.get("message")))
    row = q("SELECT approval_status, deny_reason_id FROM torrents"
            " WHERE id=%d" % t_ids[1])
    check("F2 拒审判因落库", row == "2|%d" % rid, row)
    after = q("SELECT deny_count, approve_streak FROM users"
              " WHERE id=%d" % uid_seed).split("|")
    check("F3 被拒计数 +1 / 连击清零",
          int(after[0]) == 1 and int(after[1]) == 0, str(after))
    mrow = q("SELECT subject, body FROM messages WHERE receiver_id=%d"
             " AND kind='review_rejected' ORDER BY id DESC LIMIT 1"
             % uid_seed)
    check("F4 被拒站内信含拒因文案",
          reasons[0]["reason"] in mrow.replace("|", " "), mrow[:120])

    # ================= G 参数校验与利益冲突 =================
    # 全站约定：Validation → HTTP 400（errors.rs 中央映射；OpenAPI 注释里的
    # 422 是陈旧文档，契约码 1002 才是判据）
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": t_ids[2], "approve": False, "reason": "  "},
                token=admin)
    check("G1 空理由拒审被拒(400/1002)", s == 400,
          "got %s %s" % (s, r.get("message")))
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": t_ids[2], "approve": True,
                 "deny_reason_id": rid}, token=admin)
    check("G2 通过带拒因被拒(400)", s == 400, "got %s" % s)
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": t_ids[2], "approve": False,
                 "deny_reason_id": 999999}, token=admin)
    check("G3 坏拒因 ID 被拒(400)", s == 400, "got %s" % s)
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": t_ids[2], "approve": True}, token=seed)
    check("G4 无审核权用户裁决被拒(403)", s == 403, "got %s" % s)
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": t_ids[0], "approve": True}, token=admin)
    check("G5 重复裁决被拒(400)", s == 400, "got %s" % s)
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": t_ids[2], "approve": False,
                 "reason": "自由文本拒因也行"}, token=admin)
    check("G6 自由文本拒审 200", s == 200, "%s %s" % (s, r.get("message")))
    note = q("SELECT deny_note FROM torrents WHERE id=%d" % t_ids[2])
    check("G7 自由文本落 deny_note", note == "自由文本拒因也行", note)
    # 自审拦截（staff 级上传者形态）：class91 版主自己发的种，自己不能审
    s, r = up(rev, "rc-%s-selfcheck" % TAG)
    tself = tno(r).get("id")
    check("G8 版主探针号发种进待审", s == 200 and bool(tself),
          "%s %s" % (s, r.get("message")))
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": tself, "approve": True}, token=rev)
    check("G9 自审拦截(400 + 利益冲突文案)",
          s == 400 and "自己" in str(r.get("message", "")),
          "got %s %s" % (s, r.get("message")))
    st = q("SELECT approval_status FROM torrents WHERE id=%d" % tself)
    check("G10 自审被拦后仍为待审", st == "0", st)
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": tself, "approve": True}, token=admin)
    check("G11 他人可审该种（排除误伤）", s == 200,
          "%s %s" % (s, r.get("message")))
    if tself:
        t_ids.append(int(tself))

    # ================= H 批量裁决 =================
    for i in range(3):
        s, r = up(seed, "rc-%s-batch-%d" % (TAG, i))
        tid = tno(r).get("id")
        if tid:
            t_ids2.append(int(tid))
    check("H0 批量前置：再发 3 个待审", len(t_ids2) == 3, str(t_ids2))
    s, r = call("POST", "/admin/reviews/batch",
                {"torrent_ids": t_ids2, "approve": True, "reason": ""},
                token=admin)
    d = tno(r)
    check("H1 批量过审 200 + handled=3",
          s == 200 and d.get("handled") == 3,
          "%s %s" % (s, str(d)[:160]))
    s, r = call("POST", "/admin/reviews/batch",
                {"torrent_ids": t_ids2 + [t_ids[0]], "approve": False,
                 "reason": "重复批量"}, token=admin)
    d = tno(r)
    sk = {x["id"]: x["why"] for x in d.get("skipped", [])}
    check("H2 已审条目进 skipped 且带原因",
          t_ids[0] in sk and d.get("handled") == 0, str(d)[:200])
    s, r = call("POST", "/admin/reviews/batch",
                {"torrent_ids": [], "approve": True}, token=admin)
    check("H3 空批量被拒(400)", s == 400, "got %s" % s)

    # ================= I 暂缓/恢复 =================
    s, r = up(seed, "rc-%s-postpone" % TAG)
    tp = int(tno(r)["id"])
    s, r = call("POST", "/admin/reviews/postpone",
                {"torrent_id": tp, "reason": "证据不足，待补 MediaInfo"},
                token=admin)
    check("I1 暂缓 200", s == 200, "%s %s" % (s, r.get("message")))
    st = q("SELECT approval_status FROM torrents WHERE id=%d" % tp)
    check("I2 暂缓落库 status=4", st == "4", st)
    log = q("SELECT count(*) FROM torrent_operation_logs WHERE torrent_id=%d"
            " AND action='postpone'" % tp)
    check("I3 暂缓留痕", int(log) == 1, log)
    s, r = call("GET", "/admin/reviews?limit=50&status=0", token=admin)
    ids0 = [x["id"] for x in tno(r).get("items", [])]
    check("I3b 暂缓后不出现在待审视图(0)", tp not in ids0,
          "got %s" % ids0[:8])
    s, r = call("POST", "/admin/reviews/resume",
                {"torrent_id": tp, "reason": ""}, token=admin)
    check("I5 恢复待审 200", s == 200, "%s %s" % (s, r.get("message")))
    st = q("SELECT approval_status FROM torrents WHERE id=%d" % tp)
    check("I6 恢复落库 status=0", st == "0", st)
    log = q("SELECT count(*) FROM torrent_operation_logs WHERE torrent_id=%d"
            " AND action='resume'" % tp)
    check("I7 恢复留痕", int(log) == 1, log)

    # ================= K 过审副作用 =================
    # 连击复位：seed 号到这里的免审连击可能已满 5（E+H 连过），
    # 满连击上传即免审（0077 口径），decide 会回「不在待审」——
    # 那是免审特性正确生效，不是链路故障。复位后测副作用。
    q("UPDATE users SET approve_streak=0, publish_credits=0 WHERE id=%d"
      % uid_seed)
    s, r = up(seed, "rc-%s-promo" % TAG)
    tpr = int(tno(r)["id"])
    st = q("SELECT approval_status FROM torrents WHERE id=%d" % tpr)
    check("K0 复位后发的种进待审", st == "0", st)
    q("INSERT INTO auto_promo_rules (name, name_regex, min_size, max_size,"
      " category_id, kind, hours, enabled, position)"
      " VALUES ('rc-probe-rule', 'rc-%s-promo', 0, 0, NULL, 'free', 24,"
      " true, 1) " % TAG)
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": tpr, "approve": True}, token=admin)
    check("K1 过审 200（副作用探针）", s == 200,
          "%s %s" % (s, r.get("message")))
    promo_after = q("SELECT count(*) FROM promotions WHERE torrent_id=%d"
                    % tpr)
    check("K2 自动促销规则命中挂 free", int(promo_after) >= 1, promo_after)
    q("DELETE FROM auto_promo_rules WHERE name='rc-probe-rule'")

    # ================= L 通知链 =================
    # 本脚本对 seed 号的口径：过审×5（E1/H1×3/K1）+ 拒审×2（F1/G6）= 7。
    # （自审被拦/重复裁决/暂缓恢复/删因拒审都不产通知——精确相等才是链路断言，
    #  「≥N」会在多产一条垃圾通知时假装通过。）
    cnt = q("SELECT count(*) FROM messages WHERE receiver_id=%d"
            " AND kind IN ('review_approved','review_rejected')" % uid_seed)
    check("L1 通知计数与裁决数精确匹配(7)", cnt == "7", "got %s" % cnt)
    kinds = q("SELECT DISTINCT kind FROM messages WHERE receiver_id="
              + str(uid_seed) + " AND kind LIKE 'review%'")
    check("L2 通知 kind 双语键齐全",
          "review_approved" in kinds and "review_rejected" in kinds, kinds)

    # ================= M 拒因字典 CRUD =================
    s, r = call("POST", "/admin/deny-reasons",
                {"reason": "探针拒因-待删", "sort": 60}, token=admin)
    nrid = tno(r).get("id")
    check("M1 新增拒因 200", s == 200 and bool(nrid),
          "%s %s" % (s, r.get("message")))
    s, r = call("PUT", "/admin/deny-reasons/%d" % nrid,
                {"reason": "探针拒因-改"}, token=admin)
    check("M2 改名 200", s == 200, "%s %s" % (s, r.get("message")))
    row = q("SELECT reason FROM torrent_deny_reasons WHERE id=%d" % nrid)
    check("M2b 改名落库", row == "探针拒因-改", row)
    s, r = call("DELETE", "/admin/deny-reasons/%d" % nrid, token=admin)
    check("M3 删除 200", s == 200, "%s %s" % (s, r.get("message")))
    s, r = call("POST", "/admin/reviews/decide",
                {"torrent_id": tp, "approve": False,
                 "deny_reason_id": nrid}, token=admin)
    check("M4 删掉的拒因不能再用于裁决(400)", s == 400, "got %s" % s)

finally:
    # ================= 清理 =================
    for t in (t_ids + t_ids2 + [x for x in (tp, tpr) if x]):
        try:
            q("DELETE FROM torrent_operation_logs WHERE torrent_id=%d" % t)
            q("DELETE FROM messages WHERE params->>'id' = '%d'" % t)
            q("DELETE FROM promotions WHERE torrent_id=%d" % t)
            q("DELETE FROM snatches WHERE torrent_id=%d" % t)
            q("DELETE FROM auto_review_log WHERE torrent_id=%d" % t)
            q("DELETE FROM torrents WHERE id=%d" % t)
        except Exception:
            pass
    for uid in (uid_seed, uid_rev):
        try:
            call("POST", "/admin/users/status",
                 {"user_id": uid, "status": 2}, token=admin)
            q("DELETE FROM user_permissions WHERE user_id=%d" % uid)
            call("DELETE", "/admin/users/%d" % uid, token=admin)
        except Exception:
            pass
    for k, v in SAVED.items():
        restore_kv(k, v)

print()
if FAILED:
    print("RED %d 条：" % len(FAILED))
    for f in FAILED:
        print("  - " + f)
    sys.exit(1)
print("GREEN：种审人工链路全动线不变式通过")
summary()
