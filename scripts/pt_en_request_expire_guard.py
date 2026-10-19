# -*- coding: utf-8 -*-
"""EN 常驻闸门：求种过期回收链 + 火花流水 kind 的人话名（P-315/P-316）

静态（CI 跑，不写库）：
  EN0 正控  批一百三十修之前的作业体必须被「无守卫翻面」抓到，且旧体里
            退还确实排在翻面之前——次序才是双花的成因
  EN1       现网作业体 0 处无守卫翻面；抢坑带 status=0 守卫、排在退还之前、
            跑在同一笔事务里（抢不到就整笔回滚，不动账）
  EN2       参照物：/requests/fulfill 的抢坑 UPDATE 仍带 status=0 守卫
  EN3       现役后端写进 spark_ledger 的每个 kind 都要在
            apps/web/lib/spark-kind-labels.ts 里有中文名
            正控＝e04a0298 那版表里确实没有 torrent_buy（判据抓得住洞）

现网只读（库不可达则本轮跳过，不判红）：
  EN4  同一求种单不得既有 req-payout 又有 req-refund；已应种的单不得有
       退还流水；现网历史/探针 kind 无标签只 WARN 记数（归站方裁）

运行时红证（造临时单跑 A 红 B 绿）不常驻，配方见
 _doc/资深PT用户实测审计-2026-10-10.md 批一百三十。
pt_audit_lib.ok 的签名是 ok(name, cond, detail)，条件写在第二格。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
JOB = "apps/worker/src/jobs/request_expire.rs"
API = "apps/api/src/content_http/requests.rs"
LBL = "apps/web/lib/spark-kind-labels.ts"
FIX = "e04a0298"  # 批一百三十：P-315 双花修复提交

UNGUARDED = re.compile(r"UPDATE requests SET status\s*=\s*2\s+WHERE"
                      r"\s+id\s*=\s*\$1(?!.*status\s*=\s*0)")
GUARDED = re.compile(r"UPDATE requests SET status\s*=\s*2\s+WHERE\s+id"
                     r"\s*=\s*\$1\s+AND\s+status\s*=\s*0")


def sh(args):
    return subprocess.run(args, cwd=ROOT, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")


def blob(rev, path):
    """取某提交里的原文；取不到就记 FAIL，别把「没读到」当「没违例」"""
    p = sh(["git", "show", "%s:%s" % (rev, path)])
    if p.returncode != 0:
        FAILURES.append("EN 取文失败 %s:%s / %s"
                        % (rev, path, (p.stderr or "")[:120]))
        return ""
    return p.stdout or ""


def here(rel):
    p = os.path.join(ROOT, rel)
    if not os.path.exists(p):
        FAILURES.append("EN 文件缺失 %s" % rel)
        return ""
    return open(p, encoding="utf-8", errors="replace").read()


def strip_rs(s):
    """先剥 // 注释：补丁注释会造假红，注释里的键名能洗白真洞"""
    out = []
    for line in s.splitlines():
        i = line.find("//")
        if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
            line = line[:i]
        out.append(line)
    return "\n".join(out)


def strip_ts(s):
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    return re.sub(r"(?m)^\s*\*.*$", "", s)


def flat(s):
    """Rust 长 SQL 用 \\ 续行，压平才认得出跨行的整句"""
    return s.replace("\\\n", " ").replace("\n", " ")


def cnt_unguarded(text):
    return len(UNGUARDED.findall(flat(strip_rs(text))))


print("== EN0/EN1 过期回收的抢坑守卫 ==")
old_job = blob("%s^" % FIX, JOB)
old_f = flat(strip_rs(old_job))
new_job = here(JOB)
new_f = flat(strip_rs(new_job))
ok("EN0 正控：修前的无守卫翻面被抓到 1 处", cnt_unguarded(old_job) == 1,
   "实抓 %s" % cnt_unguarded(old_job))
ok("EN0b 正控：旧体里退还排在翻面之前（双花次序）",
   old_f.find("'request_refund'") >= 0
   and old_f.find("'request_refund'") < old_f.find(
       "UPDATE requests SET status = 2"))
ok("EN1a 现网作业体无守卫翻面 0 处", cnt_unguarded(new_job) == 0,
   "抓到 %s" % cnt_unguarded(new_job))
ok("EN1b 带 status=0 守卫的抢坑翻面在且仅 1 处",
   len(GUARDED.findall(new_f)) == 1, "实抓 %s" % len(GUARDED.findall(new_f)))
ok("EN1c 抢坑排在退还之前（先定归属再动账）",
   0 <= new_f.find("UPDATE requests SET status = 2")
   < new_f.find("'request_refund'"))
ok("EN1d 抢坑跑在退还那笔事务里（抢不到即回滚）",
   "&mut *tx" in new_f[new_f.find("UPDATE requests SET status = 2"):
                       new_f.find("'request_refund'")])

print("== EN2 应种口的抢坑守卫（参照物别被拆）==")
ok("EN2 /requests/fulfill 仍用 AND status = 0 抢坑",
   re.search(r"UPDATE requests SET status\s*=\s*1,[^;]*AND status\s*=\s*0",
             flat(strip_rs(here(API)))) is not None)

print("== EN3 后端写的 kind 都得有人话名 ==")


def split_top(s):
    """按顶层逗号切：nextval('x') 里的逗号不算分隔"""
    parts, depth, cur, quote = [], 0, "", None
    for ch in s:
        if quote:
            cur += ch
            if ch == quote:
                quote = None
            continue
        if ch in "'\"":
            quote = ch
            cur += ch
            continue
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur.strip())
            cur = ""
        else:
            cur += ch
    parts.append(cur.strip())
    return parts


def kinds_from_sql(text):
    """INSERT INTO spark_ledger (…kind…) VALUES (…) 里 kind 列位置的字面量"""
    out = []
    f = flat(strip_rs(text))
    for m in re.finditer(r"INSERT INTO spark_ledger\s*\(([^)]*)\)"
                         r"\s*VALUES\s*\(", f, re.I):
        cols = [c.strip() for c in split_top(m.group(1))]
        if "kind" not in cols:
            continue
        depth, end = 1, len(f)
        for i in range(m.end(), len(f)):
            if f[i] == "(":
                depth += 1
            elif f[i] == ")":
                depth -= 1
                if depth == 0:
                    end = i
                    break
        vals = split_top(f[m.end():end])
        if len(vals) != len(cols):
            continue
        mm = re.match(r"^'([A-Za-z][\w-]{2,})'$", vals[cols.index("kind")])
        if mm:
            out.append(mm.group(1))
    return out


REF_WORDS = {"request", "torrent", "user", "team", "bank", "forum", "post",
             "subtitle", "loan", "deposit", "spark", "admin", "msg", "topic",
             "comment", "article", "wiki", "game", "farm", "medal", "item"}


def call_args(f, start):
    """从左括号 start 处按深度配平取整段实参（format!(…) 里的括号不算终点）"""
    depth, i = 0, start
    quote = None
    while i < len(f):
        c = f[i]
        if quote:
            if c == quote:
                quote = None
        elif c in "'\"":
            quote = c
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return f[start + 1:i], i + 1
        i += 1
    return "", i


def kinds_from_calls(text):
    """earn_spark*/spend_spark* 调用点：kind 固定在第 4 个实参位
    （&tx/&db, user_id, amount, kind, …）。只取那一个，别把 ref_type
    的字符串（avatar_frame / medal_gift 之类）混进来当 kind。"""
    out, skipped = [], 0
    f = flat(strip_rs(text))
    pos = 0
    while True:
        m = re.compile(r"(?:earn|spend)_spark[\w_]*\s*\(").search(f, pos)
        if not m:
            break
        args, nxt = call_args(f, m.end() - 1)
        pos = nxt
        if not args:
            skipped += 1
            continue
        vals = split_top(args)
        if len(vals) < 4:
            skipped += 1
            continue
        mm = re.match(r'^"([A-Za-z][\w-]{2,})"$', vals[3].strip())
        if mm:
            out.append(mm.group(1))
        else:
            skipped += 1  # kind 是变量/拼接（如促销名）——本判据认不出，记数
    return out, skipped



skipped = 0
used = set()
for dirpath, _, files in os.walk(os.path.join(ROOT, "apps")):
    if any(p in dirpath for p in ("target", "node_modules", ".next", "e2e")):
        continue
    for fn in files:
        if not fn.endswith(".rs"):
            continue
        s = open(os.path.join(dirpath, fn), encoding="utf-8",
                 errors="replace").read()
        kc, sk = kinds_from_calls(s)
        used |= set(kinds_from_sql(s)) | set(kc)
        skipped += sk


def label_keys(text):
    return set(re.findall(
        r"(?m)^\s*[\"']?([A-Za-z][\w-]{2,})[\"']?\s*:\s*[\"']", text))


ZH = "apps/web/i18n/zh-CN.ts"
LBLFILE = here(LBL)
if re.search(r"=\s*zhCN\.my\.kinds", LBLFILE):
    # i18n 收口批把标签表搬进 zh-CN 的 my.kinds 块（本文件只留键序与
    # 回落逻辑）⇒ 判据必须跟着搬，否则抓到 0 个键会把全部 kind 误报缺名
    def kinds_block(t):
        i = t.find("kinds:")
        if i < 0:
            return ""
        j = t.find("{", i)
        d = 0
        for k in range(j, len(t)):
            if t[k] == "{":
                d += 1
            elif t[k] == "}":
                d -= 1
                if d == 0:
                    return t[j:k + 1]
        return ""
    labels_now = label_keys(kinds_block(here(ZH)))
else:
    labels_now = label_keys(LBLFILE)
ok("EN3e 正控：标签表读到的键数必须 >30（搬家后判据别又读空）",
   len(labels_now) > 30, "实读=%s" % len(labels_now))
labels_pin = label_keys(strip_ts(blob(FIX, LBL)))
ok("EN3a 正控：抽到的 kind 全集里确实含 torrent_buy/request_refund 两只",
   {"torrent_buy", "request_refund"} <= used, "实抽 %s 个" % len(used))
ok("EN3b 正控：修复前那版标签表确实没有 torrent_buy",
   "torrent_buy" not in labels_pin)
# 待站方定性的两只：ops_http/task_overview.rs 里的 task_fee/task_penalty
# 落在 kind 位，但那条口是统计口径，认不出它真会写 spark_ledger——
# 宁可不接名字，也别套个最近的中文做假收口。多一只少一只都红。
PENDING_KINDS = {"task_fee", "task_penalty"}
miss = sorted(used - labels_now)
ok("EN3c 后端写入的 %s 个 kind 除在册待裁 %s 只外都有中文名"
   % (len(used), len(PENDING_KINDS)), miss == sorted(PENDING_KINDS),
   "实缺：%s" % miss)
dup = [k for k in re.findall(r"(?m)^\s*[\"']?([A-Za-z][\w-]{2,})[\"']?\s*:",
                             here(LBL))
       if list(re.findall(r"(?m)^\s*[\"']?([A-Za-z][\w-]{2,})[\"']?\s*:",
                          here(LBL))).count(k) > 1]
ok("EN3d 标签表没有重复键（同名键是 TS1117）", not dup, "重复：%s" % sorted(set(dup)))
print("   在册 kind %s 个｜标签表 %s 个键" % (len(used), len(labels_now)))

print("== EN4 现网只读：同单双入账留痕 ==")


def psql(sql):
    p = sh(["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
            "-d", "fluxtorrent", "-tAc", sql])
    return "" if p.returncode != 0 else (p.stdout or "").strip()


both = psql("SELECT count(*) FROM spark_ledger p JOIN spark_ledger r"
            " ON r.idempotency_key ="
            " 'req-refund:' || split_part(p.idempotency_key, ':', 2)"
            " WHERE p.idempotency_key LIKE 'req-payout:%'")
if both == "":
    print("   WARN 库不可达：EN4 本轮不判（静态四条照常承重）")
else:
    ok("EN4a req-payout 与 req-refund 同单共存 0 对", both == "0",
       "实读 %s" % both)
    orphan = psql("SELECT count(*) FROM spark_ledger WHERE kind ="
                  " 'request_refund' AND split_part(idempotency_key, ':', 2)"
                  "::bigint IN (SELECT id FROM requests WHERE status = 1)")
    ok("EN4b 已应种的单不得有退还流水", orphan == "0", "实读 %s" % orphan)
    unknown = psql("SELECT string_agg(k, ',') FROM (SELECT DISTINCT kind AS k"
                   " FROM spark_ledger) t WHERE t.k NOT IN ('"
                   + "','".join(sorted(labels_now)) + "')")
    if unknown:
        print("   WARN 现网历史/探针 kind 无标签（归站方）：%s" % unknown[:180])

summary()
