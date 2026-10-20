# -*- coding: utf-8 -*-
"""常驻闸门：H&R「达标消除」信必须听控制面板那个开关（P-440）。

会员视角：控制面板 → 个人信息 → 勾掉「H&R 达标时通知我」→ 保存 → 回显也
是关着的。之后我把那条违规的做种时长补足，违规确实自动消除了，可站内信
照样落进信箱。查下去：`users.notify_hr` 是 0024 建的列，`PUT /me/settings`
收它、GET 回显它、usercp 画了控件——**全站没有任何一处读它**；而达标消除
这条信是 `hr_resolve.rs::resolve_met_violations` 无条件写的。

H&R 一共三档通知，另两档都有执法：预警走 `notice_prefs.hr_prewarn`
（hr_mail.rs）、违规确认走 `hr_violation`（hr.rs / hr_warn_ladder.rs）。
「达标」这一档在 13 档偏好面板里根本没有对应键，所以本批按列判
（给已有控件补读者），不另立新键（立新键属加档位，不是接线）。

判据（SQL 本体一律从源码正则取出，不手抄，防「验的不是被测代码」）：
  H0 正控  修前的本体：那条 messages INSERT 段既无 notify_hr 也无 kind
  H1     现网本体 pm 段带 `WHERE u.notify_hr`，且 INSERT 带 kind
         ='hr_fulfilled'（0286 口径：站内信要带 kind 才可归类/做幂等锚）
  H2     PG 事务内 A/B（跑完 ROLLBACK）：开关开着 ⇒ 消除 1 条 + 发信 1 条；
         关掉 ⇒ **仍消除 1 条**、发信 0 条。关掉的是打扰，不是清账。
  H3     残留自检：回滚后该号近 5 分钟 0 封信、夹具三张表 0 行、
         notify_hr 回到原值
  H4     唯一写入点核对（EP7 手法，语句级共现）：凡写这条 subject 的
         messages INSERT 段，都必须过 notify_hr
  H5     台账联动：notify_hr 已从 pt_audit_dead_settings.py 的死开关台账
         移出，且排除表单/写口/i18n 后全仓真能扫到消费点

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hr_fulfilled_pref.py
      负控：同上加 --selftest（只翻 WHERE u.notify_hr 那一行，跑完还原）
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join("apps", "worker", "src", "jobs", "hr_resolve.rs")
SRC_POSIX = "apps/worker/src/jobs/hr_resolve.rs"   # git show 认斜杠不认反斜杠
FIX = "4ab4ff2a"        # 本批修复的提交（H0 从它的父提交取旧形状）
PROBE_USER = 4348         # e2e_probe_retired：status=4、无 HR 历史 ⇒ 计数精确
PROBE_TORRENT = 40
SUBJ = u"H&R 违规已消除"
PSQL = ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
        "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-F", "|",
        "-f", "-"]
# 「有控件/能写能回显」的地方不算读者（与死开关台账同一把尺）
FORM = re.compile(
    r"(usercp[\w.-]*\.tsx?|me_settings|/i18n/(zh-CN|en|zh-TW|ja)\.ts)")
SCAN_DIRS = ["apps/api/src", "apps/worker/src", "apps/tracker/src", "crates",
             "apps/web"]


def read(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip_comments(text):
    """剥掉 `//` 行注释与 `/* */` 块注释：注释里提一句字段名不算读者。
    `https://` 里那个 `//` 不剥（(?<!:) 挡住），否则 URL 后半行会被吃掉，
    把真读者误删成「死开关」。"""
    t = re.sub(r"/\*[\s\S]*?\*/", " ", text)
    return re.sub(r"(?<!:)//[^\n]*", " ", t)


def body(text):
    """取 resolve_met_violations 那条 WITH…SELECT 本体（原文，含注释）。"""
    m = re.search(r'r#"\s*(WITH met AS.*?)\s*"#', text, re.S)
    if not m:
        FAILURES.append("H 没在 hr_resolve.rs 认出那条 SQL（判据打空）")
        return ""
    return m.group(1)


def pm_seg(text):
    """只取写这条信的 INSERT 段（语句级，不是文件级共现）。"""
    i = text.find("INSERT INTO messages")
    if i < 0:
        FAILURES.append("H 没在 SQL 里找到 messages INSERT（判据打空）")
        return ""
    j = text.find("RETURNING 1", i)
    return text[i:j if j > 0 else len(text)]


def psql(sql):
    p = subprocess.run(PSQL, input=sql, capture_output=True, text=True,
                       encoding="utf-8", errors="replace", cwd=ROOT)
    out = (p.stdout or "") + (p.stderr or "")
    if p.returncode != 0:
        FAILURES.append("H psql 失败 rc=%d：%s"
                        % (p.returncode, out[:220].replace("\n", " / ")))
    return out


FIXTURE = (
    "DELETE FROM snatches WHERE user_id = %d AND torrent_id = %d;\n"
    "DELETE FROM hr_violations WHERE user_id = %d AND torrent_id = %d;\n"
    "DELETE FROM hr_snapshots WHERE user_id = %d AND torrent_id = %d;\n"
    "INSERT INTO hr_snapshots (user_id, torrent_id, required_seconds,\n"
    "  deadline, seeded_seconds, status) VALUES\n"
    "  (%d, %d, 3600, now() + interval '7 day', 0, 'violated');\n"
    "INSERT INTO snatches (user_id, torrent_id, seeded_seconds)\n"
    "  VALUES (%d, %d, 7200);\n"
    "INSERT INTO hr_violations (user_id, torrent_id, seeded_seconds,\n"
    "  required_seconds) VALUES (%d, %d, 0, 3600);\n"
    % ((PROBE_USER, PROBE_TORRENT) * 6)
)


def run_ab(sql_body, notify_on):
    """事务内铺夹具→跑本体→数信→ROLLBACK。返回 (消除数, 发信数, 原值)。"""
    sql = ("BEGIN;\n"
           "SELECT 'ORIG=' || notify_hr::text FROM users WHERE id = %d;\n"
           "UPDATE users SET notify_hr = %s WHERE id = %d;\n"
           "%s\n%s;\n"
           "SELECT 'PMROWS=' || count(*) FROM messages WHERE receiver_id = %d"
           " AND subject = '%s' AND created_at >= now();\n"
           "ROLLBACK;\n"
           % (PROBE_USER, str(notify_on).upper(), PROBE_USER, FIXTURE,
              sql_body, PROBE_USER, SUBJ))
    out = psql(sql)
    orig = re.search(r"ORIG=(\w+)", out)
    met_pm = re.search(r"^(\d+)\|(\d+)$", out, re.M)
    rows = re.search(r"PMROWS=(\d+)", out)
    if not (orig and met_pm and rows):
        FAILURES.append("H A/B(%s) 读数不全：%s"
                        % (notify_on, out[:260].replace("\n", " / ")))
        return None, None, None
    return int(met_pm.group(1)), int(rows.group(1)), orig.group(1)


def selftest():
    """负控：只把执法那一行翻成 `WHERE 1 = 1`（SQL 仍可跑，红必须来自断言
    而不是 psql 报错），期望 H1a/H2b/H4b/H5b 四条转红；跑完还原并再跑一次
    正向用例——还原没验过等于没还原。"""
    path = os.path.join(ROOT, SRC)
    with io.open(path, encoding="utf-8", newline="") as f:
        src = f.read()
    n = src.count("WHERE u.notify_hr")
    if n != 1:
        print("负控打空：notify_hr 谓词出现 %d 次（应恰好 1 次）" % n)
        return 2
    flipped = src.replace("WHERE u.notify_hr", "WHERE 1 = 1")

    def gate():
        p = subprocess.run(
            [sys.executable, os.path.abspath(__file__)],
            capture_output=True, text=True, encoding="utf-8",
            errors="replace", cwd=ROOT)
        return p.returncode, (p.stdout or "") + (p.stderr or "")

    rc_red = 2
    try:
        with io.open(path, "w", encoding="utf-8", newline="") as f:
            f.write(flipped)
        rc, out = gate()
        want = ["H1a", "H2b", "H4b", "H5b"]
        hit = [w for w in want if re.search(r"\[FAIL\] %s" % w, out)]
        print("[负控] 翻掉谓词后 rc=%d，转红的判据=%s" % (rc, hit))
        print("[负控] 仍绿的判据=%s"
              % [w for w in want if w not in hit])
        if "build failed" in out or "psql 失败" in out \
                or "Traceback" in out:
            print("[负控] 红里混进了跑不动的错（判据自己崩了），不算证据")
            return 2
        rc_red = 0 if (rc != 0 and len(hit) == len(want)) else 1
    finally:
        with io.open(path, encoding="utf-8", newline="") as f:
            cur = f.read()
        assert cur.count("WHERE 1 = 1") == 1, "还原前状态不对，停手"
        with io.open(path, "w", encoding="utf-8", newline="") as f:
            f.write(cur.replace("WHERE 1 = 1", "WHERE u.notify_hr"))
    rc2, out2 = gate()
    print("[负控] 还原后正向复跑 rc=%d（须为 0）" % rc2)
    return rc_red if rc2 == 0 else 2


if "--selftest" in sys.argv:
    sys.exit(selftest())

print("== H0/H1 本体形状 ==")
old_full = subprocess.run(
    ["git", "show", "%s:%s" % (FIX + "^", SRC_POSIX)], capture_output=True,
    text=True, encoding="utf-8", errors="replace", cwd=ROOT)
if old_full.returncode != 0:
    FAILURES.append("H 取文失败 %s^:%s / %s"
                    % (FIX, SRC_POSIX, (old_full.stderr or "")[:160]))
old_b = body(old_full.stdout or "")
new_b = body(read(SRC))
ok("H0 正控：修前那条 INSERT 段既无 notify_hr 也无 kind（尺能抓到旧形状）",
   old_b != "" and "notify_hr" not in pm_seg(old_b)
   and "kind" not in pm_seg(old_b).split("\n")[0],
   "修前列=%s" % (pm_seg(old_b).split("\n")[0][:60] if old_b else "?"))
ok("H1a 现网 pm 段带 `WHERE u.notify_hr`",
   re.search(r"FROM met m\s+JOIN users u ON u\.id = m\.user_id\s+"
             r"WHERE u\.notify_hr", new_b or "") is not None)
ok("H1b 现网那条 INSERT 带 kind 列、值为 'hr_fulfilled'",
   "kind" in pm_seg(new_b or "").split("\n")[0]
   and "'hr_fulfilled'" in (new_b or ""))

print("== H2 事务内 A/B（ROLLBACK 收尾）==")
m_on, pm_on, orig = run_ab(new_b, True)
m_off, pm_off, _ = run_ab(new_b, False)
ok("H2 前置：夹具让消除数可精确断言（探针号无历史 HR 行）",
   m_on is not None, "读数见下")
ok("H2a 开关开着 ⇒ 消除 1 条且发信 1 条"
   "（met=%s pm=%s）" % (m_on, pm_on),
   m_on == 1 and pm_on == 1,
   "发信数取自那条 INSERT 的真实写入行数，不是本体自报")
ok("H2b 开关关掉 ⇒ 仍消除 1 条、发信 0 条"
   "（met=%s pm=%s）" % (m_off, pm_off),
   m_off == 1 and pm_off == 0,
   "关掉的是打扰不是清账：违规必须照样消除，否则这条开关变成惩罚开关")

print("== H3 回滚残留 ==")
res = psql(
    "SELECT 'V=' || (SELECT count(*) FROM hr_violations WHERE user_id = %d"
    " AND torrent_id = %d) || ' S=' || (SELECT count(*) FROM hr_snapshots"
    " WHERE user_id = %d AND torrent_id = %d) || ' N=' ||"
    " (SELECT count(*) FROM snatches WHERE user_id = %d AND torrent_id = %d)"
    " || ' PM=' || (SELECT count(*) FROM messages WHERE receiver_id = %d"
    " AND subject = '%s' AND created_at > now() - interval '5 min')"
    " || ' HR=' || (SELECT notify_hr::text FROM users WHERE id = %d);"
    % ((PROBE_USER, PROBE_TORRENT) * 3 + (PROBE_USER, SUBJ, PROBE_USER)))
kv = dict(re.findall(r"(\w+)=(\w+)", res))
ok("H3 夹具/信零残留且偏好已回滚（读数 %s）" % (kv or res[:80]),
   kv.get("PM") == "0" and kv.get("HR") == (orig or ""),
   "活体闸门最怕 finally 没跑：这里回读原值 %s" % orig)

print("== H4 凡写这条信的语句都过偏好 ==")
hits = []
for d in SCAN_DIRS:
    base = os.path.join(ROOT, d)
    for dp, _, files in os.walk(base):
        if "node_modules" in dp or ".next" in dp:
            continue
        for fn in files:
            if not fn.endswith((".rs", ".ts", ".tsx")):
                continue
            rel = os.path.relpath(os.path.join(dp, fn), ROOT)
            rel = rel.replace("\\", "/")
            t = " ".join(strip_comments(read(rel)).replace("\\\n",
                                                            " ").split())
            for mm in re.finditer(r"INSERT INTO messages.{0,420}", t):
                if SUBJ in mm.group(0) or "hr_fulfilled" in mm.group(0):
                    hits.append((rel, "notify_hr" in mm.group(0)))
ok("H4a 认到写入点（>0，否则判据打空）：%d 处" % len(hits), len(hits) > 0,
   sorted(set(p for p, _ in hits)))
ok("H4b 每个写入点都过 notify_hr", all(on for _, on in hits),
   "未执法=%s" % sorted(set(p for p, on in hits if not on)))

print("== H5 与死开关台账联动 ==")
ds = read(os.path.join("scripts", "pt_audit_dead_settings.py"))
m = re.search(r"DEAD_LEDGER = \{([^}]*)\}", ds or "")
ledger = set(re.findall(r"[a-z_]+", m.group(1))) if m else set()
ok("H5a notify_hr 已移出死开关台账（%s）"
   % ("在册＝桩没撤" if "notify_hr" in ledger else "已撤"),
   "notify_hr" not in ledger,
   "修好了却留桩＝台账说谎；台账集合级判据会跟着 D1 红")


def readers():
    out = []
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
                if re.search(r"\bnotify_hr\b", strip_comments(read(rel))):
                    out.append(rel)
    return out


r = readers()
ok("H5b 全仓（排除表单/写口/i18n）扫到 notify_hr 消费点：%s" % sorted(r),
   len(r) > 0, "空＝这条信又变回无条件写")

summary()
