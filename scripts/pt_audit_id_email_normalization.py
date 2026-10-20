# -*- coding: utf-8 -*-
"""闸门 ID：邮箱「写入口归一化 / 读入口按 lower(email) 比」必须同尺（P-437）。

来历：注册口 `register.rs:22` 曾是 `body.email.trim().to_string()`（只 trim 不
lowercase），而找回口 `reset.rs:55` 读 `WHERE email = $1`、$1 绑的是
`trim().to_lowercase()` ⇒ 邮箱里带大写字母的人此后找回密码**永远命中 0 行**：
不铸令牌、不发信，而响应刻意与成功一致（防枚举）——「失败演成成功」。
现网 `email <> lower(email)` 共 460 行。修见提交 62a54555。

判据（一把尺量两侧，全部不依赖现网镜像版本）：
  ID1 写入口：所有把请求体邮箱塞进 NewUser/INSERT 的赋值，必须出现
      `to_lowercase()`（台账：register.rs 一处）。
  ID2 读入口：所有 `users`/`applications` 上按邮箱取人的 SQL 谓词，
      必须写成 `lower(email)`，或两侧都已归一化（台账见 EXACT_OK）。
  ID3 承重：把 P-437 的旧形状（写口不 lowercase、读口 `email = $1`）喂给
      ID1/ID2 那对探测器，必须双双判违例——否则判据恒真，不算承重。
  ID4 真库 A/B（行为级红证）：对一枚现网大小写混合邮箱，
      `email = lower(存储值)` 命中必须为 0 而 `lower(email) = lower(存储值)`
      命中必须 >0——这正是修前后的差额；两数相等说明库里已无混合大小写邮箱，
      此时改判「A/B 都 0 ⇒ 台账为空」并出声，不谎称通过。
  ID5 活体读数（默认只读）：给混合大小写邮箱打一次 forgot，令牌行数增量
      应为 1（旧镜像为 0，即 P-437 的现网表征）；只记读数，部署后升硬判据。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_id_email_normalization.py`
      ID_LIVE=1 额外做一次活体读数（对探针号铸一行 reset 令牌，用完即删）。
"""
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import BASE, login, ok, summary  # noqa: E402

LIVE = os.environ.get("ID_LIVE", "0") == "1"
SRC = os.path.join(ROOT, "apps", "api", "src")

# 读入口白名单：这些谓词两侧都已是小写值（applications 的 email 在
# apply_submit 里就 `trim().to_lowercase()` 了），等值比较是合法的。
EXACT_OK = {"gaps_http/applications.rs"}


def rd(rel):
    p = os.path.join(SRC, rel)
    if not os.path.isfile(p):
        return ""
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


def strip_comments(text):
    out = []
    for line in text.split("\n"):
        i = line.find("//")
        out.append(line[:i] if i >= 0 else line)
    return "\n".join(out)


def walk():
    res = []
    for dirpath, _dirs, files in os.walk(SRC):
        for fn in files:
            if fn.endswith(".rs"):
                rel = os.path.relpath(
                    os.path.join(dirpath, fn), SRC).replace("\\", "/")
                res.append((rel, strip_comments(rd(rel))))
    return res


ALL = walk()


def bad_write(v):
    """写入口探测器：把请求体邮箱交给 NewUser 却没小写 ⇒ 违例。"""
    return "body" in v and "to_lowercase" not in v


def bad_read(sql):
    """读入口探测器：按邮箱等值查 users/applications 却没 lower(email)。"""
    HIT = re.compile(r"\bemail\s*=\s*\$")
    return "lower(email)" not in sql and bool(HIT.search(sql))


# ID1 写入口：把邮箱交给 NewUser / users INSERT 之前必须 lowercase
w_pairs = []
for rel, t in ALL:
    for m in re.finditer(r"email:\s*([^,\n]+)", t):
        v = m.group(1)
        if bad_write(v):
            w_pairs.append((rel, v.strip()))
ok("ID1 写入口归一化：进 users 的邮箱必须先小写（台账内不该有例外）",
   not w_pairs, "违例=%s" % w_pairs)

# ID2 读入口：按邮箱取人的 SQL 不许拿原始大小写做等值比较
bad_r = []
for rel, t in ALL:
    SEL = r"(SELECT[^\"']*?FROM (?:users|applications)[^\"']*)"
    for m in re.finditer(SEL, t, re.S):
        sql = m.group(1)
        if bad_read(sql) and rel not in EXACT_OK:
            bad_r.append((rel, sql[:70].replace("\n", " ")))
ok("ID2 读入口同尺：按邮箱查 users 的谓词必须 lower(email)（或两侧已归一化）",
   not bad_r, "违例=%s" % bad_r)

# ID3 承重：**同一对探测器**（不是另写一遍条件）喂旧形状必须双双判违例
ok("ID3 承重：P-437 的两处旧形状被 ID1/ID2 的探测器判违例（防恒真）",
   bad_write("body.email.trim().to_string()")
   and bad_read("SELECT id FROM users WHERE email = $1")
   and not bad_read("SELECT id FROM users WHERE lower(email) = $1"),
   "旧形状若仍算合格＝探测器不承重；新形状若被误判＝探测器过宽")


def psql(q):
    x = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-q", "-tA", "-F", "|"], input=q,
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if x.returncode != 0 or "ERROR" in (x.stderr or ""):
        raise RuntimeError("psql: " + (x.stderr or "")[:200])
    return (x.stdout or "").strip()


row = psql("SELECT id || ' ' || email FROM users "
           "WHERE email <> lower(email) ORDER BY id DESC LIMIT 1")
if not row:
    ok("ID4 真库 A/B：库里已无大小写混合邮箱（台账清空）", True,
       "本轮无探针可取；ID1/ID2/ID3 仍承重")
else:
    uid, em = row.split(" ", 1)
    a = int(psql("SELECT count(*) FROM users WHERE email = lower('%s')"
                 % em.replace("'", "''")) or 0)
    b = int(psql("SELECT count(*) FROM users WHERE lower(email) = "
                 "lower('%s')" % em.replace("'", "''")) or 0)
    ok("ID4 真库 A/B：旧谓词命中必须 0、新谓词命中必须 >0（差额＝被锁的人）",
       a == 0 and b > 0,
       "邮箱=%s 旧=%d 新=%d；两数相等说明库里没有混合大小写邮箱，"
       "该情形由上面那条分支出声" % (em, a, b))
    if LIVE:
        before = int(psql("SELECT count(*) FROM password_resets "
                          "WHERE user_id = %s" % uid) or 0)
        tok = login("root", "password123")
        import json
        import urllib.request
        req = urllib.request.Request(
            BASE + "/auth/password/forgot", method="POST",
            headers={"Authorization": "Bearer " + tok,
                     "Content-Type": "application/json"})
        with urllib.request.urlopen(req, data=json.dumps(
                {"email": em}).encode(), timeout=20) as r:
            code = r.status
        after = int(psql("SELECT count(*) FROM password_resets "
                         "WHERE user_id = %s" % uid) or 0)
        psql("DELETE FROM password_resets WHERE user_id = %s" % uid)
        left = int(psql("SELECT count(*) FROM password_resets "
                        "WHERE user_id = %s" % uid) or 0)
        print("   [读数] http=%s 令牌行 %d→%d（旧镜像 0＝P-437 现网表征；"
              "新镜像应为 1），清理后残留=%d" % (code, before, after, left))
    else:
        print("[ID5] ID_LIVE=0 跳过活体读数（会铸一行令牌，用完即删）")

summary()
