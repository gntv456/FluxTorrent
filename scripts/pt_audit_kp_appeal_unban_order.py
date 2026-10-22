# -*- coding: utf-8 -*-
"""KP 申诉受理的解封顺序与墓碑保护（P-561/P-562，2026-10-22 深测）。

P-561 缺陷本体：appeal_handle 先把 appeals.status 翻成 accepted，再调
restore_if_outranked 做 0285 的等级护栏。护栏一旦拒绝（低档 staff 受理
高档目标——正是 0285 要挡的「放出被封的管理员/站长」场景）：
  - appeals 已消费（status='accepted'）
  - 账号仍封（restore 的 UPDATE 没跑）
  - 当事人收不到任何通知（notify 在错误返回之后）
  - staff 重试只得到「申诉不存在或已处理」
一条申诉被静默吃掉。

修复：翻转状态**之前**取目标与解封意图（SELECT 只认 status='open'），
解封动作整体前置；其内部仍二次校验等级。

P-562：解封谓词旧写 `status >= 2`，会把 status=3 的墓碑一起复活。
user_del 虽然会删申诉，但「受理读到 2 → 删号提交 → 解封写回」的并发
窗口里仍可把已匿名化的号变成空壳账号。status=3 是删除态，解封只认 2。

  KP0 解封动作早于 UPDATE appeals SET status
  KP1 SELECT 的解封意图带 a.status = 'open'（已处理申诉不解封）
  KP2 restore_if_outranked 实参仍带 actor id/等级/目标
  KP3 解封动作内部 ensure_outranks 早于 UPDATE users
  KP4 解封谓词只认 status = 2（不再是 >= 2）
  KP5 申诉状态翻转仍要求 status='open'（不重复消费）
  KP6 SELECT 的解封意图只认 u.status = 2
  KP7 真语句正控：对被封号（2）解封 1 行
  KP8 真语句：对墓碑（3）解封 0 行
  KP9 负控：旧谓词 >= 2 对同一枚墓碑解封 1 行（判据承重）
  KP10 事务回滚后两枚夹具状态原样

KP7~KP9 直接从源码抠出那条 UPDATE（不是抄一份），在 BEGIN…ROLLBACK
里造/借夹具跑真语句；KP10 在事务外回读。
"""
import io
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
APPEALS = os.path.join(ROOT, "apps", "api", "src", "gaps_http", "appeals.rs")
UNBAN = os.path.join(ROOT, "apps", "api", "src", "gaps_http",
                     "appeal_unban.rs")

FAIL = []
PASS = 0


def ok(name, cond, detail=""):
    global PASS
    if cond:
        PASS += 1
        print("[PASS] %s%s" % (name, ("  " + detail) if detail else ""))
    else:
        FAIL.append(name)
        print("[FAIL] %s  %s" % (name, detail))


def read(path):
    return io.open(path, encoding="utf-8", errors="replace").read()


def handler_body(src):
    """只取 appeal_handle 的函数体，避免被同文件别的 handler 洗白。"""
    i = src.find("pub async fn appeal_handle")
    if i < 0:
        return ""
    j = src.find("\n}", i)
    return src[i:j] if j > i else src[i:]


def flat(s):
    """把 Rust 字符串续行（反斜杠+换行）压成一行再匹配。"""
    return re.sub(r"\\\s*\n\s*", " ", s)


def order_ok(body):
    i = body.find("restore_if_outranked(")
    j = body.find("UPDATE appeals SET status")
    return 0 <= i < j


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-v", "ON_ERROR_STOP=1", "-tA", "-F", "|"],
        input=sql, capture_output=True, text=True, encoding="utf-8",
        errors="replace")
    err = (r.stderr or "").strip()
    if r.returncode != 0 or "ERROR" in err:
        raise RuntimeError("psql 失败：%s" % err[:400])
    return (r.stdout or "").strip().splitlines()


def unban_sql():
    m = re.search(r'"UPDATE users SET status = 0[^"]*"', read(UNBAN))
    return m.group(0).strip('"') if m else ""


def finish():
    print("\n%d 条通过 / %d 条失败" % (PASS, len(FAIL)))
    if FAIL:
        print("失败：" + "、".join(FAIL))
        sys.exit(1)
    sys.exit(0)


def main():
    src = read(APPEALS)
    body = handler_body(src)
    fb = flat(body)
    ok("KP0 解封早于申诉状态翻转", order_ok(body),
       "restore@%d update@%d" % (body.find("restore_if_outranked("),
                                 body.find("UPDATE appeals SET status")))
    ok("KP1 SELECT 的解封意图带 open 守卫",
       bool(re.search(r"\(\s*\$2 AND a\.kind = 'ban' AND a\.status = 'open'"
                      r"\s+AND u\.status = 2\s*\)", fb)))
    ok("KP2 解封调用实参完整",
       bool(re.search(r"restore_if_outranked\(\s*&state,\s*auth\.id,\s*"
                      r"auth\.class_id,\s*user_id,\s*\)", body)))
    u = read(UNBAN)
    uc = "\n".join(l for l in u.splitlines()
                   if not l.strip().startswith("//"))
    ok("KP3 解封内部护栏早于写 users",
       0 <= uc.find("ensure_outranks(") < uc.find("UPDATE users"))
    sql = unban_sql()
    ok("KP4 解封谓词只认 status = 2",
       "status = 2" in sql and ">= 2" not in uc, sql)
    ok("KP5 申诉翻转仍只消费 open",
       bool(re.search(r"UPDATE appeals SET status[\s\S]{0,200}?"
                      r"AND status = 'open'", body)))
    ok("KP6 SELECT 解封意图只认 u.status = 2",
       bool(re.search(r"u\.status = 2\s*\)", fb))
       and "u.status >= 2" not in fb)
    if not sql:
        return finish()

    ids = psql("SELECT 'BANNED|' || id FROM users WHERE status = 2"
               " AND id <> 1 ORDER BY id DESC LIMIT 1;\n"
               "SELECT 'TOMB|' || id FROM users WHERE status = 3"
               " ORDER BY id DESC LIMIT 1;")
    banned = [x.split("|")[1] for x in ids if x.startswith("BANNED|")]
    tomb = [x.split("|")[1] for x in ids if x.startswith("TOMB|")]
    ok("KP7a 取到被封号夹具", len(banned) == 1, str(ids))
    ok("KP7b 取到墓碑夹具", len(tomb) == 1, str(ids))
    if not (banned and tomb):
        return finish()
    b, t = banned[0], tomb[0]
    new_sql = sql.replace("$1", b)
    new_tomb = sql.replace("$1", t)
    old_tomb = new_tomb.replace("status = 2", "status >= 2")
    tx = ["BEGIN;",
          "UPDATE users SET status = 2 WHERE id = %s;" % b,
          "UPDATE users SET status = 3 WHERE id = %s;" % t,
          "WITH hit AS (%s RETURNING id)" % new_sql
          + " SELECT 'NEW_BANNED|' || count(*) FROM hit;",
          "WITH hit AS (%s RETURNING id)" % new_tomb
          + " SELECT 'NEW_TOMB|' || count(*) FROM hit;",
          "WITH hit AS (%s RETURNING id)" % old_tomb
          + " SELECT 'OLD_TOMB|' || count(*) FROM hit;",
          "ROLLBACK;"]
    out = psql("\n".join(tx))

    def get(tag):
        rows = [x for x in out if x.startswith(tag + "|")]
        return rows[0].split("|")[1] if len(rows) == 1 else None

    n_b, n_t, o_t = get("NEW_BANNED"), get("NEW_TOMB"), get("OLD_TOMB")
    ok("KP7 新谓词对封禁号解封 1 行", n_b == "1", "NEW_BANNED=%s" % n_b)
    ok("KP8 新谓词对墓碑解封 0 行", n_t == "0", "NEW_TOMB=%s" % n_t)
    ok("KP9 负控：旧谓词复活墓碑 1 行", o_t == "1", "OLD_TOMB=%s" % o_t)
    after = psql("SELECT 'A|' || id || '|' || status FROM users"
                 " WHERE id IN (%s, %s) ORDER BY id;" % (b, t))
    got = dict(x.split("|")[1:] for x in after if x.startswith("A|"))
    ok("KP10 回滚后夹具状态原样",
       got.get(b) == "2" and got.get(t) == "3", str(after))
    finish()


main()
