# -*- coding: utf-8 -*-
"""常驻闸门：私信「保存短讯至发件箱」必须决定发件箱有没有副本（P-448）。

会员视角：控制面板 → 个人信息 → 「保存短讯至发件箱」。这个勾选框在 0024
就建好了列（`users.save_pm BOOLEAN NOT NULL DEFAULT FALSE`），设置页能改、
`PUT /me/settings` 能落盘、GET 能回显——**全站没有任何一处读它**；
发信口 `message_send.rs` 把 `saved` 恒写成 1。

于是默认值（FALSE＝不保存）反而是被违反的那一侧：现网 2961 个号里
`save_pm=true` 的有 **0 个**，而已有 30 行发件副本正挂在
「已关着开关」的人名下。发件箱口 `/messages/sent` 的谓词就是
`sender_id = $1 AND saved = 1`，所以这一列写对，开关就成立。

判据（读侧口径用真接口取，不靠替身）：
  V0 正控  修前的发信口：INSERT 的 saved 位是字面量 1、函数体不出现 save_pm
  V1     现网发信口：saved 位是 $5，同函数里有 `SELECT save_pm FROM users
          WHERE id = $1` 与按它绑定的表达式
  V2     界面层 A/B：给探针号铺 saved=1 / saved=0 两行（同一 marker），
          `GET /messages/sent` 只认前者 ⇒「不保存副本」这条口径真的成立
  V3     界面层：`GET /messages/inbox`（收件人身份）两行都在 ⇒
          不留副本 ≠ 吞信，收件人照样收到
  V4     收尾：marker 行删干净（回读 0 行），别把取证行留在现网
  V5     账面（只报读数不判红）：现网「发件人 save_pm=false 却 saved=1」
          的行数——它只该随部署下降，上升＝写侧修复还没进镜像

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_pm_outbox_copy.py
"""
import io
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
import uuid

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import BASE, dig, ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SEND = os.path.join("apps", "api", "src", "community_http", "message_send.rs")
SEND_POSIX = "apps/api/src/community_http/message_send.rs"
FIX = "be7b00c5"            # 本批修复的提交（V0 从它的父提交取旧形状）
SENDER = 6924               # hrprobe493220（闸门探针号，status=0）
RECV = 72                   # e2ecap3565（闸门探针号，status=0）
PSQL = ["docker", "exec", "-i", "flux-postgres",
        "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-v", "ON_ERROR_STOP=1", "-F", "|",
        "-f", "-"]


def sh(args, stdin=None):
    return subprocess.run(args, input=stdin, capture_output=True,
                          text=True, encoding="utf-8", errors="replace",
                          cwd=ROOT)


def psql(sql):
    p = sh(PSQL, sql)
    if p.returncode != 0:
        raise RuntimeError("psql 失败：%s" % ((p.stderr or "")[:200]))
    return (p.stdout or "").strip()


def src_text(path):
    return io.open(os.path.join(ROOT, path), encoding="utf-8",
                   errors="replace").read()


# 与 pt_audit_dead_settings.py 同一把尺：表单/写口/i18n 只证明「有控件」，
# 注释里提一句字段名也不算读者（H5b 曾被日志字符串洗白过一次）。
FORM = re.compile(r"(usercp[\w.-]*\.tsx?|me_settings"
                  r"|/i18n/(zh-CN|en|zh-TW|ja)\.ts)")
SCAN_DIRS = ["apps/api/src", "apps/worker/src", "apps/tracker/src", "crates",
             "apps/web"]


def strip_comments(t):
    t = re.sub(r"/\*[\s\S]*?\*/", " ", t)
    return re.sub(r"(?<!:)//[^\n]*", " ", t)


def readers():
    hits = []
    for d in SCAN_DIRS:
        for dp, _, files in os.walk(os.path.join(ROOT, d)):
            if "node_modules" in dp or ".next" in dp:
                continue
            for fn in files:
                if not fn.endswith((".rs", ".ts", ".tsx")):
                    continue
                rel = os.path.relpath(os.path.join(dp, fn), ROOT)
                rel = rel.replace("\\", "/")
                if FORM.search(rel):
                    continue
                if re.search(r"\bsave_pm\b",
                             strip_comments(src_text(rel))):
                    hits.append(rel)
    return sorted(hits)


def send_fn(text):
    """取 message_send 这个 handler 的函数体（切到行首那一个 `}`），
    免得把别的函数里的 save_pm 也算成读者。数大括号会被 format! 的
    `{}` 字符串骗到，故按缩进断句。"""
    i = text.find("async fn message_send")
    if i < 0:
        raise AssertionError("没找到 message_send")
    j = text.find("\n}\n", i)
    if j < 0:
        raise AssertionError("message_send 函数体结尾没找到")
    return text[i:j + 3]


def tok(uid):
    out = sh([sys.executable, os.path.join(ROOT, "_mint_jwt.py"),
              "--uid", str(uid), "--class", "1"]).stdout
    m = re.search(r"[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+", out)
    return m.group(0) if m else None


def get(path, token):
    req = urllib.request.Request(BASE + path, method="GET")
    req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req, timeout=25) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"raw": raw[:100].decode("utf-8", "replace")}
    except Exception as ex:
        return 0, {"err": str(ex)[:100]}


PAIRS = [("VALUES ($1, $2, $3, $4, 1, $5, true)",
          "VALUES ($1, $2, $3, $4, 1, 1, true)"),
         ("SELECT save_pm FROM users WHERE id = $1",
          "SELECT parked FROM users WHERE id = $1")]
# 翻转后必须转红的判据（写侧三条；V2/V3 打的是现网读侧口径，
# 不受写侧翻转影响——这是本闸门的分工，不是漏网）
MUST_RED = ["V1a", "V1b", "V7"]


def selftest():
    path = os.path.join(ROOT, SEND)
    with io.open(path, encoding="utf-8", newline="") as f:
        src = f.read()
    for a, b in PAIRS:
        if src.count(a) != 1:
            print("负控打空：%s 出现 %d 次（应恰好 1 次）"
                  % (a[:40], src.count(a)))
            return 2
    flipped = src
    for a, b in PAIRS:
        flipped = flipped.replace(a, b)

    def gate():
        p = subprocess.run([sys.executable, os.path.abspath(__file__)],
                           capture_output=True, text=True, encoding="utf-8",
                           errors="replace", cwd=ROOT)
        return p.returncode, (p.stdout or "") + (p.stderr or "")

    rc_red = 2
    try:
        with io.open(path, "w", encoding="utf-8", newline="") as f:
            f.write(flipped)
        rc, out = gate()
        hit = [w for w in MUST_RED if re.search(r"\[FAIL\] %s" % w, out)]
        print("[负控] 翻回旧形状后 rc=%d，转红=%s" % (rc, hit))
        print("[负控] 仍绿=%s（V2/V3 是读侧口径，写侧翻转不动它们）"
              % [w for w in MUST_RED if w not in hit])
        if "Traceback" in out or "psql 失败" in out:
            print("[负控] 红里混进了闸门自己崩的错，不算证据")
            return 2
        rc_red = 0 if (rc != 0 and len(hit) == len(MUST_RED)) else 1
    finally:
        with io.open(path, encoding="utf-8", newline="") as f:
            cur = f.read()
        for a, b in PAIRS:
            if cur.count(b) != 1:
                print("[负控] 还原前状态不对（%s 出现 %d 次），停手人工查"
                      % (b[:40], cur.count(b)))
                return 2
        back = cur
        for a, b in PAIRS:
            back = back.replace(b, a)
        with io.open(path, "w", encoding="utf-8", newline="") as f:
            f.write(back)
    dirty = subprocess.run(["git", "diff", "--stat", "--", SEND_POSIX],
                           capture_output=True, text=True, encoding="utf-8",
                           cwd=ROOT).stdout.strip()
    if dirty:
        print("[负控] 还原后与 HEAD 仍有差异，不算还原：%s" % dirty[:150])
        return 2
    rc2, _ = gate()
    print("[负控] 还原后正向复跑 rc=%d（须为 0；文件与 HEAD 逐字节一致）"
          % rc2)
    return rc_red if rc2 == 0 else 2


if "--selftest" in sys.argv:
    sys.exit(selftest())

print("== V0/V1 发信口形状 ==")
old = sh(["git", "show", "%s:%s" % (FIX + "^", SEND_POSIX)])
if old.returncode != 0:
    raise SystemExit("取文失败 %s^:%s / %s" % (FIX, SEND_POSIX,
                                               (old.stderr or "")[:150]))
old_fn = send_fn(old.stdout or "")
new_fn = send_fn(src_text(SEND))
ok("V0 正控：修前 saved 位是字面量 1 且函数体不出现 save_pm（尺抓得到旧形状）",
   re.search(r"VALUES \(\$1, \$2, \$3, \$4, 1, 1, true\)", old_fn) is not None
   and "save_pm" not in old_fn)
m_new = re.search(r"INSERT INTO messages \([^)]*saved[^)]*\) \.*\s*"
                  r"VALUES \(([^)]*)\)", new_fn.replace("\\", " "))
vals = (m_new.group(1) if m_new else "")
ok("V1a 现网 INSERT 的 saved 位改成了绑定值（%s）" % vals.strip(),
   "1, 1, true" not in vals and "$5" in vals)
ok("V1b 同函数里真去读了本人那一列 save_pm",
   re.search(r"SELECT save_pm FROM users WHERE id = \$1", new_fn) is not None
   and re.search(r"keep_copy", new_fn) is not None)
ok("V1c 读不到就不发信（失败没被 let _ = 吞成默认值）",
   "map_err(|e| DomainError::Internal(e.into()))" in new_fn)

print("== V2/V3 界面层 A/B（真接口读侧口径）==")
marker = "P441-%s" % uuid.uuid4().hex[:8]
s_tok, r_tok = tok(SENDER), tok(RECV)
if not (s_tok and r_tok):
    raise SystemExit("探针令牌取不到（_mint_jwt.py 需要容器在跑）")
try:
    psql("INSERT INTO messages (sender_id, receiver_id, subject, body,\n"
         "  location, saved, unread) VALUES\n"
         "  (%d, %d, '%s-A', '闸门取证行', 1, 1, false),\n"
         "  (%d, %d, '%s-B', '闸门取证行', 1, 0, false);\n"
         % (SENDER, RECV, marker, SENDER, RECV, marker))
    st_s, sent = get("/messages/sent?limit=100", s_tok)
    subs_s = [r.get("subject") for r in (dig(sent, "data") or [])]
    ok("V2 GET /messages/sent（%d）认 saved=1 不认 saved=0：%s / %s"
       % (st_s, marker + "-A" in subs_s, marker + "-B" in subs_s),
       st_s == 200 and marker + "-A" in subs_s
       and marker + "-B" not in subs_s,
       "两行都列出来＝发件箱不看 saved，写侧修好也没用")
    st_r, inbox = get("/messages/inbox?limit=100", r_tok)
    subs_r = [r.get("subject") for r in (dig(inbox, "data") or [])]
    ok("V3 收件人两条都收得到（不留副本 ≠ 吞信）：%s" % st_r,
       st_r == 200 and marker + "-A" in subs_r and marker + "-B" in subs_r)
finally:
    psql("DELETE FROM messages WHERE subject LIKE '%s%%';" % marker)
    left = psql("SELECT count(*) FROM messages WHERE subject LIKE "
                "'%s%%';" % marker)
    ok("V4 取证行已删净（回读 %s 行）" % left, left == "0",
       "活体闸门最怕留残料：marker=%s" % marker)

print("== V5 现网账面（只报数，不判红）==")
book = psql("SELECT count(*) || ' 行 / ' ||"
            " count(DISTINCT m.sender_id) || ' 个发件人' FROM messages m"
            " JOIN users u ON u.id = m.sender_id"
            " WHERE NOT u.save_pm AND m.saved = 1")
print("   [在册] 发件人 save_pm=false 却挂着发件副本：%s"
      "（该数字只随部署下降，上升＝写侧修复没进镜像）" % book)
print("   [待裁] 存量清理属改他人账目，本闸门不代删。站方若要清，语义是"
      "「按本人当前开关重算 saved」：UPDATE messages m SET saved = 0"
      " FROM users u WHERE u.id = m.sender_id AND NOT u.save_pm"
      " AND m.saved = 1;（收件方 location 不动，双删语义不受影响）")

print("== V6/V7 与死开关台账联动 ==")
ds = src_text(os.path.join("scripts", "pt_audit_dead_settings.py"))
mm = re.search(r"DEAD_LEDGER = \{([^}]*)\}", ds or "")
ledger = set(re.findall(r"[a-z_]+", mm.group(1))) if mm else set()
ok("V6 save_pm 已移出死开关台账（%s）"
   % ("仍在册＝桩没撤" if "save_pm" in ledger else "已撤"),
   "save_pm" not in ledger,
   "修好了却留桩＝台账说谎，D1 集合级判据会跟着红")
r = readers()
ok("V7 全仓（剥注释、排除表单/写口/i18n）扫到 save_pm 读者：%s" % r,
   len(r) > 0, "空＝发信口又回到恒写 saved=1")

summary()
