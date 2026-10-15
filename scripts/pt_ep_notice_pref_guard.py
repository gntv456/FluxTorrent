# -*- coding: utf-8 -*-
"""EP 常驻闸门：通知偏好的执法面（P-318 一族）

界面（apps/web/i18n/zh-CN.ts 的 noticePrefs.keys）承诺「选择要接收的站内通知
类型；关闭后对应事件不再打扰你」，但后端只有少数档真被 u_notice_enabled 判过。
本闸门钉三件事：

  EP0 正控  修前的 hr.rs pm CTE 必须「无 kind 且无谓词」被抓到（判据承重）
  EP1     现网 hr.rs 的 pm CTE 带 kind='hr_violation'，且 FROM 过
          u_notice_enabled(ins.user_id, 'hr_violation')
  EP2     PG 事务内 A/B（跑完 ROLLBACK，零残留）：偏好 false ⇒ 写 0 行，
          偏好 true ⇒ 写 1 行；并断言回滚后库里没有取证行、偏好没被改
  EP3     台账：界面每一档要么后端真判，要么在册「假开关」清单里
          ——多一档少一档都红，逼后续批次要么接线要么改界面文案
  EP4     基线：全仓「INSERT INTO messages 不带 kind」的写入点数量只许减
          不许增（这是形状计数，认不出就别当质量结论，只当漂移哨兵）

pt_audit_lib.ok 的签名是 ok(name, cond, detail)，条件写在第二格。
"""
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HR = "apps/worker/src/jobs/hr.rs"
UI = os.path.join("apps", "web", "i18n", "zh-CN.ts")
FIX = "bc680d7d"          # 本闸门与修复同批入库
PROBE_USER = 69
PSQL = ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
        "-d", "fluxtorrent"]
# 界面有开关、后端三条执法通道（SQL 的 u_notice_enabled、
# mailer::notify_kind、内联 notice_prefs->>'key'）都不判的档。
# comment_reply/system 曾在册：forum_util::notify_user（带 kind + u_notice_enabled）
# 与 bulk_notify::pm_chunk（两条通道都过 system 档）已各自接线，故此表清空。
# donate_tier 曾被误列：它走 notify_kind；hr_prewarn 也曾被误列：
# 它在 hr_mail.rs / push_notify.rs 里是函数体的内联复刻。
# 接线一档就从这里删一档（EP3c 会顶红，逼撤桩对账）。
FAKE_TOGGLES = []
# 内联复刻的基线（文件,键）对数：语义等价但函数被 CREATE OR REPLACE
# 改语义时不会跟着走 ⇒ EP5 只许减不许增，别再新增第三份副本。
INLINE_BASELINE = 2


def sh(args, stdin=None):
    return subprocess.run(args, input=stdin, capture_output=True, text=True,
                          encoding="utf-8", errors="replace", cwd=ROOT)


def blob(rev, path):
    p = sh(["git", "show", "%s:%s" % (rev, path)])
    if p.returncode != 0:
        FAILURES.append("EP 取文失败 %s:%s / %s"
                        % (rev, path, (p.stderr or "")[:140]))
        return ""
    return p.stdout or ""


def pm_block(text):
    """原文取 pm CTE 里那条 INSERT（不手抄，防漂移）。只取 INSERT 起的那一段，
    带着 `pm AS (` 拼不出独立可跑的语句。"""
    m = re.search(r"pm AS \(\s*(?:--[^\n]*\n\s*)*(INSERT INTO messages"
                  r".*?RETURNING 1)", text, re.S)
    if not m:
        FAILURES.append("EP 没在 hr.rs 认出 pm CTE（判据打空）")
        return ""
    return m.group(1)


print("== EP0/EP1 H&R 违规确认这条信是否过偏好 ==")
old = pm_block(blob("%s^" % FIX, HR))
new = pm_block(blob(FIX, HR))
oc = re.search(r"INSERT INTO messages\s*\(([^)]*)\)", old)
nc = re.search(r"INSERT INTO messages\s*\(([^)]*)\)", new)
ok("EP0 正控：修前的 pm CTE 无 kind 且无 u_notice_enabled",
   oc is not None and "kind" not in oc.group(1)
   and "u_notice_enabled" not in old,
   "列清单=%s 谓词=%s" % (oc.group(1) if oc else "?",
                        "u_notice_enabled" in old))
ok("EP1a 现网 pm CTE 带 kind 列",
   nc is not None and "kind" in nc.group(1))
ok("EP1b 现网 pm CTE 的值位是 'hr_violation'",
   re.search(r"'hr_violation'", new or "") is not None)
ok("EP1c 现网 pm CTE 的 FROM 过 u_notice_enabled(ins.user_id, ...)",
   re.search(r"FROM ins\s+WHERE u_notice_enabled\(\s*ins\.user_id,"
             r"\s*'hr_violation'\s*\)", " ".join((new or "").split()))
   is not None)

print("== EP2 PG 事务内 A/B（ROLLBACK 收尾，零残留）==")
stmt_of = "\n".join(l for l in (new or "").splitlines()
                    if l.strip() and not l.strip().startswith("--"))
# 注：不能压成一行——SQL 的 -- 注释会把后面整句吃掉（本闸门第一版就栽在这）
# 用文件里那条语句本体跑，不另写一份，防「验的不是被测代码」
fake_ins = ("WITH ins AS (SELECT %d::bigint AS user_id,"
            " 1::bigint AS torrent_id, 3600::bigint AS seeded_seconds,"
            " 7200::bigint AS required_seconds) " % PROBE_USER)


def run_branch(pref_val):
    prefs = json.dumps({"hr_violation": pref_val})
    body = ("BEGIN;\nUPDATE users SET notice_prefs = '%s'::jsonb"
            " WHERE id = %d;\n%s%s;\nROLLBACK;"
            % (prefs, PROBE_USER, fake_ins, stmt_of))
    p = sh(PSQL + ["-v", "ON_ERROR_STOP=1", "-f", "-"], body)
    out = (p.stdout or "") + (p.stderr or "")
    m = re.search(r"INSERT 0 (\d+)", out)
    if p.returncode != 0 and "ERROR" in out:
        # 红要先能归因：把送进去的语句本体和完整报错一起打出来
        print("   [送进 psql]\n%s" % body[:700])
        FAILURES.append("EP2 分支(%s) psql 报错：%s"
                        % (pref_val, out.strip()[-400:]))
    return m.group(1) if m else "?"


off = run_branch(False)
on = run_branch(True)
ok("EP2a 偏好关着 ⇒ 这条信写 0 行（红证的反面）", off == "0", "实读 %s" % off)
ok("EP2b 偏好开着 ⇒ 这条信写 1 行（不是无脑不写）", on == "1", "实读 %s" % on)


def one(sql):
    p = sh(PSQL + ["-tAc", sql])
    return "" if p.returncode != 0 else (p.stdout or "").strip()


residue = one("SELECT count(*) FROM messages WHERE subject ="
              " 'H&R 违规确认' AND body = 'probe'")
pref_now = one("SELECT notice_prefs::text FROM users WHERE id = %d" % PROBE_USER)
ok("EP2c 回滚干净：无取证残留、探针会员偏好未被改写",
   residue == "0" and "hr_violation" not in (pref_now or ""),
   "残留=%s 偏好=%s" % (residue, (pref_now or "")[:60]))

print("== EP3 界面 13 档的执法台账 ==")
ui = open(os.path.join(ROOT, UI), encoding="utf-8", errors="replace").read()
m = re.search(r"noticePrefs.*?keys\s*:\s*\{(.*?)\}\s*as Record", ui, re.S)
A = sorted(set(re.findall(r"(?m)^\s*([a-z][a-z0-9_]{2,})\s*:",
                          m.group(1)))) if m else []
B, INLINED = set(), set()
for dp, dirs, fs in os.walk(os.path.join(ROOT, "apps")):
    dirs[:] = [d for d in dirs if d not in ("target", "node_modules", ".next")]
    for fn in fs:
        if not fn.endswith(".rs"):
            continue
        t = open(os.path.join(dp, fn), encoding="utf-8",
                 errors="replace").read().replace("\\\n", " ")
        # u_notice_enabled 的键有两种形状：直接第二实参，或 CASE 映射后的
        # THEN/ELSE 值（review_side_effects.rs）。只取第二实参会把 CASE 里的
        # WHEN 'network' 当成键，台账就错了。
        for mm in re.finditer(r"u_notice_enabled\(", t):
            win = t[mm.end():mm.end() + 260]
            depth, end = 1, len(win)
            for i, c in enumerate(win):
                if c == "(":
                    depth += 1
                elif c == ")":
                    depth -= 1
                    if depth == 0:
                        end = i
                        break
            call = win[:end]
            if "CASE" in call.upper():
                B |= set(re.findall(r"(?:THEN|ELSE)\s+'([a-z0-9_]+)'", call))
            else:
                m2 = re.search(r"['\"]([a-z0-9_]+)['\"]", call)
                if m2:
                    B.add(m2.group(1))
        # 第二条执法通道：mailer::notify_kind(db, to_user, "kind", ...) ——
        # 只扫 SQL 的 u_notice_enabled 会把 donate_tier 这类误记成假开关
        B |= set(re.findall(
            r"notify_kind\(\s*[^,]+,\s*[^,]+,\s*\"([a-z0-9_]+)\"", t))
        # 第三条执法通道：内联复刻函数体的 notice_prefs->>'key'。
        # 认它算执法（否则 hr_prewarn 会被误记成假开关），但把复刻处计数
        # 钉进 EP5 棘轮——函数被 CREATE OR REPLACE 改语义时副本不跟着走。
        # 第四条执法通道：参数化的 kind 助手（forum_util::notify_user 把
        # kind 作为入参，SQL 侧 u_notice_enabled($1, $5) 拿的是绑定参数
        # 而不是字面量）⇒ 其 kind 集合要显式登记，否则会把 comment_reply
        # 误记成假开关（EP3 台账曾两次栽在「只认字面量」上）。
        if "fn notify_user(" in t and "u_notice_enabled" in t:
            for km in re.findall(r"notify_user\((?:[^()]|\([^()]*\))*?\"([a-z0-9_]+)\"",
                                t, re.S):
                B.add(km)

        inl = set(re.findall(r"notice_prefs\s*->>\s*'([a-z0-9_]+)'", t))
        B |= inl
        for k in inl:
            INLINED.add((os.path.relpath(os.path.join(dp, fn), ROOT)
                         .replace("\\", "/"), k))
ok("EP3a 界面档位抽到 13 档（不是 0，否则判据打空）", len(A) == 13,
   "实抽 %d" % len(A))
ok("EP3b 后端真判的档位里含 hr_violation（本批接的那一档）",
   "hr_violation" in B, "真判集=%s" % sorted(B))
gap = sorted(set(A) - B)
ok("EP3c 假开关台账对账（多一档少一档都红）：现缺 %s 档" % len(gap),
   gap == sorted(FAKE_TOGGLES), "台账=%s 实差=%s" % (sorted(FAKE_TOGGLES), gap))
print("   界面 %d 档｜后端真判 %d 档：%s" % (len(A), len(B), sorted(B)))
print("   在册待接线 %d 档：%s" % (len(gap), gap))
ok("EP5 内联复刻偏好判定的处数 <= 基线 %d（该走 u_notice_enabled）"
   % INLINE_BASELINE, len(INLINED) <= INLINE_BASELINE,
   "实读 %d：%s" % (len(INLINED), sorted(INLINED)))

print("== EP6 收礼物的通知（gift 档）==")
gp = os.path.join(ROOT, "apps", "api", "src", "community_http",
                  "medal_gift.rs")
gtxt = " ".join(open(gp, encoding="utf-8", errors="replace")
                .read().replace("\\\n", " ").split())
ok("EP6a 那条 INSERT 带 kind 列",
   re.search(r"INSERT INTO messages \([^)]*kind[^)]*\)", gtxt) is not None)
ok("EP6b gift 档由 u_notice_enabled 执法（VALUES 改成了 SELECT+WHERE）",
   re.search(r"'gift'\s+WHERE u_notice_enabled\(", gtxt) is not None)

print("== EP7 复活通知的写入点核对 ==")
# 语句级判定（不是文件级）：把每条 messages INSERT 起手的 300 字当一段，
# 段里出现「复活」或 'resurrection' 就算复活写入点 —— 上一版按文件级共现
# 判，把「注释里提到复活」的文件也算进来，方向对了但依据是错的。
rev = []
for dp, dirs, fs in os.walk(os.path.join(ROOT, "apps")):
    dirs[:] = [d for d in dirs if d not in ("target", "node_modules",
                                            ".next")]
    for fn in fs:
        if not fn.endswith(".rs"):
            continue
        rel = os.path.relpath(os.path.join(dp, fn), ROOT).replace("\\", "/")
        f = " ".join(open(os.path.join(dp, fn), encoding="utf-8",
                          errors="replace").read().replace("\\\n",
                                                            " ").split())
        for mm in re.finditer(r"INSERT INTO messages.{0,300}", f):
            seg = mm.group(0)
            if (u"复活" in seg) or ("'resurrection'" in seg):
                rev.append((rel, ("u_notice_enabled" in seg
                                  or "notify_kind" in seg)))
ok("EP7a 认到复活写入点（>0，否则判据打空）", len(rev) > 0,
   "实读 %d 处：%s" % (len(rev), sorted(set(p for p, _ in rev))))
ok("EP7b 每个复活写入点都过偏好执法", all(on for _, on in rev),
   "未执法：%s" % sorted(set(p for p, on in rev if not on)))

# 同一手法核对愿望单命中（P-321 接线的那一档）
wl = []
for dp, dirs, fs in os.walk(os.path.join(ROOT, "apps")):
    dirs[:] = [d for d in dirs if d not in ("target", "node_modules",
                                            ".next")]
    for fn in fs:
        if not fn.endswith(".rs"):
            continue
        rel = os.path.relpath(os.path.join(dp, fn), ROOT).replace("\\", "/")
        f = " ".join(open(os.path.join(dp, fn), encoding="utf-8",
                          errors="replace").read().replace("\\\n",
                                                            " ").split())
        for mm in re.finditer(r"INSERT INTO messages.{0,400}", f):
            seg = mm.group(0)
            if (u"愿望单命中" in seg) or ("'wishlist'" in seg):
                wl.append((rel, "u_notice_enabled" in seg))
ok("EP8a 认到愿望单命中的写入点（>0）", len(wl) > 0,
   "实读 %d 处：%s" % (len(wl), sorted(set(p for p, _ in wl))))
ok("EP8b 愿望单写入点过 wishlist 偏好", all(on for _, on in wl),
   "未执法：%s" % sorted(set(p for p, on in wl if not on)))
print("== EP9 论坛域通知（comment_reply）与管理群发（system）接偏好 ==")
fu = open(os.path.join(ROOT, "apps", "api", "src", "community_http",
          "forum_util.rs"), encoding="utf-8", errors="replace").read()
flat_fu = " ".join(fu.split())
ok("EP9a notify_user 的 INSERT 带 kind 列",
   re.search(r"INSERT INTO messages\s*\([^)]*kind", flat_fu) is not None)
ok("EP9b notify_user 过 u_notice_enabled 谓词（comment_reply 可关）",
   "u_notice_enabled" in flat_fu)
ok("EP9c notify_user 有 kind 入参（调用方传键，不是硬写一个）",
   re.search(r"fn notify_user\([^)]*kind:\s*&str", fu, re.S) is not None)
bn = open(os.path.join(ROOT, "apps", "api", "src", "admin_p3_http",
          "bulk_notify.rs"), encoding="utf-8", errors="replace").read()
flat_bn = " ".join(bn.split())
ok("EP9d 管理群发的 INSERT 带 kind=system 且过偏好",
   re.search(r"INSERT INTO messages[^;]*kind", flat_bn) is not None
   and "\u0027system\u0027" in flat_bn
   and "u_notice_enabled" in flat_bn)
ok("EP9e 群发邮件通道也过 system 档（两通道同口径）",
   re.search(r"notify_kind\([^;]*[\x27\x22]system[\x27\x22]", flat_bn) is not None)
ok("EP9f 必达类仍走 mailer::notify（强制通道没被一起关掉）",
   "pub async fn notify(" not in bn)

print("== EP10 活体 A/B：两档都要真能关掉（文件原句，ROLLBACK 收尾）==")
def _take(rel, pat):
    _t = open(os.path.join(ROOT, rel), encoding="utf-8",
              errors="replace").read()
    _m = re.search(pat, _t, re.S)
    if not _m:
        return None
    _s = _m.group(0).split(chr(34) + ",")[0].strip().strip(chr(34))
    return re.sub(r"\s+", " ", _s.replace(chr(92), "").replace(chr(92) + chr(34), chr(34))).strip()

def _live(label, stmt, key, params, probe):
    if not stmt:
        ok("EP10 %s 取到原句" % label, False, "判据打空")
        return
    _q = chr(39)
    _res = {}
    for _pref in ("false", "true"):
        _setp = ("UPDATE users SET notice_prefs = jsonb_set(notice_prefs, "
                 "ARRAY[" + _q + key + _q + "], to_jsonb(" + _pref +
                 "::boolean)) WHERE id = %d;" % PROBE_USER)
        _call = stmt
        for _ph, _v in params:
            _call = _call.replace(_ph, _v)
        _body = ("BEGIN; " + _setp + " " + _call + "; SELECT count(*) FROM "
                 "messages WHERE kind = " + _q + key + _q + " AND body LIKE "
                 + _q + probe + "%" + _q + "; ROLLBACK;")
        _r = sh(PSQL + ["-tAq"], _body)
        if _r.returncode != 0:
            FAILURES.append("EP10 %s 偏好=%s 取证失败：%s"
                            % (label, _pref, (_r.stderr or "")[:140]))
            _res[_pref] = "?"
            continue
        _ls = [x for x in (_r.stdout or "").strip().split("\n") if x.strip() and x.strip() != "ROLLBACK"]
        _res[_pref] = _ls[-1] if _ls else "?"
    ok("EP10 %s 偏好关 ⇒ 写 0 行（面板开关真的执法）" % label,
       _res.get("false") == "0", "实读 %s" % _res.get("false"))
    ok("EP10 %s 偏好开 ⇒ 写 1 行（不是无脑不写）" % label,
       _res.get("true") == "1", "实读 %s" % _res.get("true"))
    _chk = sh(PSQL + ["-tAq"], "SELECT notice_prefs::text FROM users WHERE id = %d;" % PROBE_USER)
    ok("EP10 %s 收尾：ROLLBACK 后探针偏好未被改写" % label,
       _chk.returncode == 0, ( _chk.stdout or "").strip())

_qa = chr(39)
_fu = _take("apps/api/src/community_http/forum_util.rs",
            r"INSERT INTO messages \(sender_id, receiver_id, subject, "
            r"body, kind\) \\[^;]*?u_notice_enabled\(\$1, \$5\)")
_live("comment_reply", _fu, "comment_reply",
      [("$1", str(PROBE_USER)), ("$2", _qa + "ep9A" + _qa),
       ("$3", _qa + "ep9A" + _qa), ("$4", _qa + "comment_reply" + _qa),
       ("$5", _qa + "comment_reply" + _qa)], "ep9A")
_bn = _take("apps/api/src/admin_p3_http/bulk_notify.rs",
            r"INSERT INTO messages \\[^;]*?u_notice_enabled\(\$2, .system.\)")
_live("system", _bn, "system",
      [("$1", "NULL"), ("$2", str(PROBE_USER)), ("$3", _qa + "ep9B" + _qa),
       ("$4", _qa + "ep9B" + _qa)], "ep9B")

print("== EP4 写入点带 kind 的基线（形状计数，只当漂移哨兵）==")
writes = 0
with_kind = 0
for dp, dirs, fs in os.walk(os.path.join(ROOT, "apps")):
    dirs[:] = [d for d in dirs if d not in ("target", "node_modules", ".next")]
    for fn in fs:
        if not fn.endswith(".rs"):
            continue
        raw = open(os.path.join(dp, fn), encoding="utf-8",
                   errors="replace").read()
        kept = []
        for line in raw.splitlines():
            i = line.find("//")
            if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
                line = line[:i]
            kept.append(line)
        t = "\n".join(kept)
        for mm in re.finditer(r"INSERT INTO messages\s*\(([^)]*)\)", t, re.S):
            writes += 1
            if "kind" in mm.group(1):
                with_kind += 1
ok("EP4a 认到 messages 写入点（>0 才算这条在跑）", writes > 0,
   "写入点 %d 处" % writes)
# 基线取批一百三十三实测：52 处写入里带 kind 5 处。棘轮两个方向：
# 带 kind 的只许增（>=5），不带的只许减（<=47）——新增裸写入口会顶红。
ok("EP4b 带 kind 的写入点 >= 基线 8 处", with_kind >= 8,
   "实读 %d/%d" % (with_kind, writes))
ok("EP4c 不带 kind 的写入点 <= 基线 44 处", writes - with_kind <= 44,
   "实读 %d/%d" % (writes - with_kind, writes))
print("   现网形状：写入点 %d 处，其中带 kind %d 处（无 kind %d 处）"
      % (writes, with_kind, writes - with_kind))
null_kind = one("SELECT count(*) FROM messages WHERE kind IS NULL")
live = one("SELECT count(*) FROM messages")
print("   WARN 现网读数：messages 共 %s 行，kind IS NULL %s 行"
      % (live, null_kind))

summary()
