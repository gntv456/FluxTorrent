# -*- coding: utf-8 -*-
"""闸门 JC：站点设置里不许留着「闸门的探针值」（批二百四十六）。

两起现网事故的共同形状：
  · `upload_dup_policy = warn`——一支审链脚本探针期写的档位，代码只认
    suggest/block/group，读侧对未登记值静默按最严「拦下」执行 ⇒ 23 支发种
    闸门集体红（P-510 收读侧兜底，脚本侧归站方）；
  · `hr_days/hr_hours/cancel_hr/hr_scan_lookback_hours` 四把同时等于 ah_hr 的
    探针签名（1/1/100/72），而迁移出厂值是 14/120/10000/6 ⇒ 现网的 H&R 一直
    按「考察 1 天、消违规 100 火花」判人（污染窗口内新增 11 条违规，逐账号
    核对全落在探针号上，真实会员 0 条——不夸大，但参数确实偏离出厂口径）。
    根因是闸门「还原」读到的就是上次泄漏的探针值，于是把污染当原值写回。

判据：
  JC1 尺自证：探针写值集合必须抽得到（含 setkv/setting/UPDATE 三种写法），
      且至少能认出这两把已知键的探针值——抽不到就是尺瞎了，结论作废；
  JC2 出厂值对质：现网值 == 某支闸门的探针值 且 != 该键最后一次迁移写入值
      ⇒ 判红并点名（谁写的、出厂该是多少）；
  JC3 无主键：`site_settings` 里 descr='probe'（探针插入时用的标记）的行
      必须为零——闸门该用 ON CONFLICT 复原而不是留新行；
  JC4 投递能力一致性：配了 smtp_host/smtp_port 就必须在 API 所在位置真连得上，
      且 `mail_outbox` 里不许烂着超 30 分钟的未发行。豁免那两把邮件键之后，
      这条才是真正要盯的不变式（站长可以选诱捕信箱，但"配了发不出去"不行）。

正控（承重自证，改完必撤）：
  python scripts/pt_audit_jc_probe_leak.py --selftest
  临时把 hr_days 设成 ah_hr 的探针值 ⇒ JC2 必须红；随后必须复原为出厂值
  并复跑为绿。

用法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_jc_probe_leak.py
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

SCRIPTS = os.path.join(ROOT, "scripts")
MIG = os.path.join(ROOT, "apps", "api", "migrations")


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-v", "ON_ERROR_STOP=1", "-tA", "-F", "|",
         "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0 or "ERROR" in (r.stderr or ""):
        raise RuntimeError("psql 失败：%s" % (r.stderr or "").strip()[:250])
    return (r.stdout or "").strip()


def probe_pairs():
    """抽闸门脚本里写 site_settings 的字面 (键, 值) 对。"""
    pairs = {}
    pats = [
        r'(?:setkv|setting|setval)\(\s*"([\w\.]+)"\s*,\s*"([^"]*)"\s*\)',
        r"UPDATE site_settings SET value='([^']*)'[^']*WHERE name='([\w\.]+)'",
        r"VALUES \('([\w\.]+)', '([^']*)'",
    ]
    for fn in sorted(os.listdir(SCRIPTS)):
        if not fn.endswith(".py"):
            continue
        txt = io.open(os.path.join(SCRIPTS, fn), encoding="utf-8",
                      errors="replace").read()
        for i, pat in enumerate(pats):
            for m in re.finditer(pat, txt):
                a, b = m.group(1), m.group(2)
                k, v = (a, b) if i != 1 else (b, a)
                if not re.fullmatch(r"[a-z][a-z0-9_\.]{2,64}", k):
                    continue
                pairs.setdefault((k, v), fn)
    return pairs


# 豁免清单（可审计，不是蒙眼）：现网值等于某闸门探针值、也确实不等于迁移出厂值，
# 但证据指向「站长/站型包的意图」而非闸门泄漏。每条都要写清依据，
# 且读数行会把 updated_at 一起打出来供人工复核。
WHITELIST = {
    # 与 enable_hr / hr_violation_limit 同一批（2026-09-25 13:26）由站型包/
    # 演示铺数写入，且 0148 之后没有任何闸门运行时间戳落在它们身上。
    "subtitle_workflow": "09-25 与站型包同批写入（站长 10-22 确认：站型意图）",
    "subtitle_award": "同上",
    # 站长 10-22 拍板：演示站的投递目标是 compose 网内的审计诱捕信箱
    # （flux-audit-smtp-sink:2525，encryption=none）。这两把键于是**不该**回到
    # 迁移出厂值（465/ssl 会把链路指回一个不存在的服务）。真正要盯的是
    # 「配置与投递能力一致」⇒ 见 JC4。
    "smtp_port": "演示站走内网诱捕信箱（站长 10-22 拍板），出厂 465 不适用",
    "encryption": "同上：诱捕信箱不加密，出厂 ssl 会把链路指回不可达服务",
}


def factory_values(keys):
    """每把键「最后一次迁移写入」的值（文件名升序，后者覆盖前者）。

    注意多行 VALUES：`INSERT INTO site_settings (...) VALUES ('a','1'),
    ('hr_hours','120'), ...` 只匹配第一个元组会漏键（首跑就漏了 hr_hours，
    它的 120 正是这么写进 0025 的）。因此先定位整条语句，再抽全部元组。
    """
    out = {}
    tup = re.compile(r"\(\s*'([\w\.]+)'\s*,\s*'([^']*)'")
    for fn in sorted(os.listdir(MIG)):
        if not fn.endswith(".sql"):
            continue
        txt = io.open(os.path.join(MIG, fn), encoding="utf-8",
                      errors="replace").read()
        for m in re.finditer(r"INSERT INTO site_settings.*?VALUES", txt, re.S):
            tail = txt[m.end():]
            tail = tail[: tail.find(";")] if ";" in tail else tail
            for t in tup.finditer(tail):
                if t.group(1) in keys:
                    out[t.group(1)] = t.group(2)
        for m in re.finditer(
                r"UPDATE site_settings SET value\s*=\s*'([^']*)'[^;]*?"
                r"WHERE name\s*(?:=|IN)\s*([^;]+)", txt, re.S):
            for name in re.findall(r"'([\w\.]+)'", m.group(2)):
                if name in keys:
                    out[name] = m.group(1)
    return out


pairs = probe_pairs()
live, stamped = {}, {}
for line in psql("SELECT name || '#' || value || '#' || "
                 "COALESCE(updated_at::text,'-') FROM site_settings").split(
        "\n"):
    parts = line.split("#", 2)
    if len(parts) == 3:
        live[parts[0]] = parts[1]
        stamped[parts[0]] = parts[2][:19]
keys = sorted({k for k, _v in pairs})
fac = factory_values(set(keys))

# ── JC1 尺自证 ───────────────────────────────────────────
known = [("upload_dup_policy", "warn"), ("hr_days", "1"),
         ("cancel_hr", "100"), ("hr_scan_lookback_hours", "72")]
blind = [kv for kv in known if kv not in pairs]
print("   [尺自证] 探针写值 %d 对，覆盖 %d 把键；迁移里认得出出厂值的键 %d 把"
      % (len(pairs), len(keys), len(fac)))
ok("JC1 尺自证：已知两轮事故的探针写值必须都抽得到", not blind,
   "抽不到=%s ⇒ 这把尺看不见探针写入，下面结论作废" % blind)

# ── JC2 现网值等于探针值且不等于出厂值 ───────────────────
leaks = []
waived = []
for (k, v), fn in sorted(pairs.items()):
    if k not in live or live[k] != v:
        continue
    if k not in fac or fac[k] == v:
        # 没有迁移基线，或探针值本来就跟出厂值一样 ⇒ 不能判泄漏
        continue
    if k in WHITELIST:
        waived.append((k, v, fac[k], stamped.get(k, "-"), WHITELIST[k]))
        continue
    leaks.append((k, v, fac[k], fn, stamped.get(k, "-")))
print("   [读数] 现网 %d 把键；判为探针泄漏 %d 把" % (len(live), len(leaks)))
for k, v, f0, fn, ts in leaks:
    print("      · %s = %r（出厂 %r，探针来自 %s，现值写入于 %s）"
          % (k, v, f0, fn, ts))
for k, v, f0, ts, why in waived:
    print("      [豁免] %s = %r（出厂 %r，写入于 %s）依据：%s"
          % (k, v, f0, ts, why))
ok("JC2 没有任何设置键停在闸门的探针值上（≠迁移出厂值）", not leaks,
   "泄漏=%d 把 %s" % (len(leaks), leaks[:4]))
# 把涉案闸门那一组探针写入的「全套现值」打出来：smtp_host/smtp_from 这类
# 没有迁移基线的键不会被判红，但它们是同一次探针的一部分，缺了它们就看不出
# 「邮件其实整条指向一个已不存在的诱捕容器」。
for fn in sorted({x[3] for x in leaks}):
    block = [(k, v, live.get(k, "（键不存在）"))
             for (k, v), f2 in sorted(pairs.items()) if f2 == fn]
    print("      [同组上下文] %s 的探针写入共 %d 把：%s"
          % (fn, len(block),
             ", ".join("%s=%s(现=%s)" % t for t in block[:12])))

# ── JC3 探针行残留 ───────────────────────────────────────
probe_rows = psql("SELECT count(*) FROM site_settings "
                  "WHERE descr = 'probe'")
ok("JC3 site_settings 里不许留 descr='probe' 的行（闸门该复原不该留新行）",
   probe_rows == "0", "残留 %s 行" % probe_rows)

# ── JC4 配置与投递能力一致（本轮邮件事故的真正不变式）──────
# 现网 smtp_host 曾指向一个已被删掉的诱捕容器 ⇒ 出信全挂在 DNS 上，
# 10-21 那封「您的种子未通过自动审核」在 mail_outbox 里重试到 5 次变死信。
# 豁免掉那两把键之后，这条才是真正该盯住的：值合不合理由站长发，
# 「配了就要发得出去、发不出去不许烂在队列里」由本条盯。
host, port = live.get("smtp_host", ""), live.get("smtp_port", "")
if host and port:
    r = subprocess.run(
        ["docker", "exec", "flux-api", "sh", "-c",
         "wget -T 3 -O /dev/null http://%s:%s/ 2>&1 | head -1" % (host, port)],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    txt = ((r.stdout or "") + (r.stderr or "")).lower()
    dead = [k for k in ("connection refused", "bad address",
                        "unable to connect", "cannot assign") if k in txt]
    conn_ok = not dead
    where = "API 容器内"
elif host:
    conn_ok, txt, where = True, "（smtp_port 缺省，按 ssl/tls 推）", "API 容器内"
else:
    conn_ok, txt, where = True, "SMTP 未配置（待命是合法态）", "-"
pend = psql("SELECT count(*) FROM mail_outbox WHERE sent_at IS NULL")
stale = psql("SELECT count(*) FROM mail_outbox WHERE sent_at IS NULL "
             "AND created_at < now() - interval '30 minutes'")
dead = psql("SELECT count(*) FROM mail_outbox WHERE sent_at IS NULL "
            "AND attempts >= 5")
print("   [投递能力] smtp=%s:%s 连通判读=%s（%s）；队列 未发=%s 超30分=%s 死信=%s"
      % (host or "-", port or "-", conn_ok, txt.strip()[:60] or where,
         pend, stale, dead))
ok("JC4 配了 SMTP 就得发得出去，且队列里不许烂着超 30 分钟的未发行",
   conn_ok and stale == "0",
   "连通=%s 滞留=%s 死信=%s（死信只能靠 staffmessages 告警+人工恢复，"
   "无 requeue 出口＝待站长裁）" % (conn_ok, stale, dead))

# ── 自测：正控必须能红，且必须还原 ────────────────────────
if "--selftest" in sys.argv:
    if leaks:
        print("!! 现版本身就红，自测无从判读，请先清干净再自测")
        sys.exit(2)
    key = "hr_days"
    if key not in dict(pairs) and (key, "1") not in pairs:
        print("!! 自测找不到 hr_days 的探针值")
        sys.exit(2)
    orig = live[key]
    psql("UPDATE site_settings SET value='1' WHERE name='%s'" % key)
    r2 = subprocess.run([sys.executable, "-u", __file__],
                        capture_output=True, text=True, encoding="utf-8",
                        errors="replace")
    red = "JC2" in (r2.stdout or "")
    psql("UPDATE site_settings SET value='%s' WHERE name='%s'" % (orig, key))
    back = psql("SELECT value FROM site_settings WHERE name='%s'" % key)
    print("   [自测] 注入 hr_days=1 ⇒ JC2 红=%s；还原后回读=%s（应=%s）"
          % (red, back, orig))
    sys.exit(0 if red and back == orig else 1)

summary()
