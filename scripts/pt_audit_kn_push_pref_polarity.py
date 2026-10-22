# -*- coding: utf-8 -*-
"""KN Web Push 用户级开关极性（P-560，2026-10-22 资深用户深测）。

缺陷本体：push_notify.rs 的 push_to_user 把 notice_prefs.push_<topic>
当「关闭标记」读——SQL 取 `COALESCE(..., false) AS pref_off`，Rust 侧
`pref_off == Some(true) => continue`。而全站其它口径都是 true=开：
`community_http/notice.rs` 白名单注释「缺省 true」、前端 notice-prefs
「显式 false 才视为关」、`hr_mail` 的 `COALESCE(..., true)`。于是任何
通过开放 API 把 push_message 打开（true）的会员反而收不到一条推送，
关掉（false）才收到——静默、无报错。

做法：从源码抠出 push_to_user 的订阅查询 SQL（不是抄一份），在事务内
造一条订阅夹具，三档偏好各跑一次：
  KN0  抠取自证 + Rust 判据静态钉桩（pref_on == Some(false)）
  KN0b 用户 1 存在
  KN1  push_message=true  => 1 条（开启的人必须收到）
  KN2  push_message=false => 0 条（关闭的人必须收不到）
  KN3  缺键              => 1 条（缺省发，与 notice.rs 口径一致）
  KN4  负控：旧判据在 true 档上 0 条（证明判据承重）
  KN5  回滚后夹具零残留 + root 偏好回到基线

只读性质：全部夹具在一个 BEGIN … ROLLBACK 会话里；KN5 在事务外回读。
"""
import io
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "apps", "api", "src", "push_notify.rs")

FAIL = []
PASS = 0
EP = "https://gate-kn.invalid/ep"


def ok(name, cond, detail=""):
    global PASS
    if cond:
        PASS += 1
        print("[PASS] %s%s" % (name, ("  " + detail) if detail else ""))
    else:
        FAIL.append(name)
        print("[FAIL] %s  %s" % (name, detail))


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


def source():
    s = io.open(SRC, encoding="utf-8", errors="replace").read()
    i = s.find("pub async fn push_to_user")
    if i < 0:
        return "", ""
    tail = s[i:]
    m = re.search(r'"(SELECT p\.id, p\.endpoint.*?LIMIT 20)"', tail, re.S)
    sql = ""
    if m:
        # Rust 的 `\` 续行会吞掉换行与下一行前导空白
        sql = re.sub(r"\\\n\s*", "", m.group(1))
    return sql, tail


def fixture(value):
    """一条 root 的有效订阅 + 指定的 push_message 偏好。"""
    return ("UPDATE users SET notice_prefs = jsonb_set(COALESCE("
            "notice_prefs, '{}'::jsonb), ARRAY['push_message'], "
            "to_jsonb(%s)) WHERE id = 1;\n"
            "INSERT INTO push_subscriptions (user_id, endpoint, p256dh, "
            "auth, topics)\n"
            "VALUES (1, '%s', 'BPk', 'auth', ARRAY['message'])\n"
            "ON CONFLICT (endpoint) DO UPDATE SET user_id = 1, "
            "expired_at = NULL, topics = EXCLUDED.topics;"
            % (value, EP))


def run_script(steps):
    return [x for x in psql("\n".join(steps)) if x and not x.startswith("###")]


def main():
    sql, tail = source()
    rust_ok = ("pref_on == Some(false)" in tail
               and "pref_off == Some(true)" not in tail)
    ok("KN0 抠出订阅 SQL + Rust 判据是 true=开",
       bool(sql) and "COALESCE((u.notice_prefs->>$2)::boolean, true)" in sql
       and "pref_on" in sql and "FROM push_subscriptions" in sql and rust_ok,
       "%d 字符 rust=%s" % (len(sql), rust_ok))
    if not sql:
        return
    uid = [x for x in psql(
        "SELECT 'U|' || id FROM users WHERE id = 1;") if x.startswith("U|")]
    ok("KN0b 用户 1 存在", uid == ["U|1"], str(uid))
    if not uid:
        return

    q = (sql.replace("$1", "1").replace("$2", "'push_message'")
         .replace("$3", "'message'").strip().rstrip(";"))
    # SQL 只取数，真正的跳过在 Rust 循环里；计数必须把那一层也复刻。
    count = ("SELECT 'CNT|' || count(*) FROM (" + q + ") x "
             "WHERE COALESCE(x.pref_on, true);")
    # 旧判据：SQL 取 pref_off（缺省 false），Rust 见 true 就跳过
    old = ("SELECT 'OLD|' || count(*) FROM (" + q + ") x WHERE NOT "
           "COALESCE((SELECT (notice_prefs->>'push_message')::boolean "
           "FROM users WHERE id = 1), false);")
    base_pref = [x for x in psql(
        "SELECT 'PRE|' || COALESCE(notice_prefs->>'push_message', '\\N') "
        "FROM users WHERE id = 1;") if x.startswith("PRE|")]

    tx = ["BEGIN;",
          "SELECT '### KN1 开启（true）应投递';", fixture("true"), count,
          "SELECT '### KN2 关闭（false）应跳过';", fixture("false"),
          count,
          "SELECT '### KN3 缺键应缺省投递';",
          "UPDATE users SET notice_prefs = COALESCE(notice_prefs, "
          "'{}'::jsonb) - 'push_message' WHERE id = 1;", count,
          "SELECT '### KN4 负控：旧判据在 true 档';", fixture("true"), old,
          "ROLLBACK;"]
    out = run_script(tx)

    cnt = [x for x in out if x.startswith("CNT|")]
    ok("KN1 开启（true）应投递", len(cnt) >= 1 and cnt[0] == "CNT|1",
       str(cnt[:3]))
    ok("KN2 关闭（false）应跳过", len(cnt) >= 2 and cnt[1] == "CNT|0",
       str(cnt[:3]))
    ok("KN3 缺键应缺省投递", len(cnt) >= 3 and cnt[2] == "CNT|1",
       str(cnt[:3]))
    oldn = [x for x in out if x.startswith("OLD|")]
    ok("KN4 负控：旧判据在 true 档 0 条（判据承重）",
       oldn == ["OLD|0"], str(oldn))

    resid = psql("SELECT 'kn_resid|' || count(*) FROM push_subscriptions "
                 "WHERE endpoint = '%s';" % EP)
    ok("KN5 订阅夹具零残留", resid == ["kn_resid|0"], str(resid))
    after = [x for x in psql(
        "SELECT 'PRE|' || COALESCE(notice_prefs->>'push_message', '\\N') "
        "FROM users WHERE id = 1;") if x.startswith("PRE|")]
    ok("KN5b root 偏好回到基线", after == base_pref,
       "%s -> %s" % (base_pref, after))

    print("\n%d 条通过 / %d 条失败" % (PASS, len(FAIL)))
    if FAIL:
        print("失败：" + "、".join(FAIL))
        sys.exit(1)


main()
