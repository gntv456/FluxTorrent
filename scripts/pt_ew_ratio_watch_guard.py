# -*- coding: utf-8 -*-
"""EW 常驻闸门：分享率观察期（Ratio Watch）四段链齐备 + 每段的读者在场（P-328）

这批是把 10-06 那条「无即时分享率闸门」的旧账重跑一遍得出的台账：
链本身是**健康的**——① 新跌破进观察期并写信、② 到期按 ratio_watch_action 分级
处置、③ 期内恢复自动解除（连带解开自己锁的 download_enabled）；闸门只在
announce（tracker），.torrent 下载口不判——那是口径，不在本文件断言里。
真正修的一条是 P-328：warn 档那张「观察期已满」的信被 `let _ =` 吞掉失败。

pt_audit_lib.ok 的签名是 ok(name, cond, detail)——条件在第二个参数。
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADJ = "apps/worker/src/jobs/audit.rs"
FIX = "f127e31f"  # 本批修复前最后一个提交（正控取它，别写 HEAD）


def sh(args):
    return subprocess.run(args, cwd=ROOT, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")


def blob(rev):
    p = sh(["git", "show", "%s:%s" % (rev, ADJ)])
    if p.returncode != 0:
        FAILURES.append("EW 取文失败 %s / %s" % (rev, (p.stderr or "")[:120]))
        return ""
    return p.stdout or ""


def here(rel):
    p = os.path.join(ROOT, rel)
    if not os.path.exists(p):
        FAILURES.append("EW 文件缺失 %s" % rel)
        return ""
    return open(p, encoding="utf-8", errors="replace").read()


def cycle(src):
    i = src.find("pub async fn ratio_watch")
    return src[i:] if i >= 0 else ""


old = cycle(blob(FIX))
cur = cycle(here(ADJ))
print("== EW1 正控：修前 warn 档那张信是 let _ = 吞掉的 ==")
ok("EW1a 两段都能取到 ratio_watch 函数体（空串=参照物失效）",
   len(old) > 400 and len(cur) > 400,
   "旧 %d / 新 %d 字符" % (len(old), len(cur)))
ok("EW1b 修前「观察期已满」的信用 let _ = 且全程无 warn 出声",
   "let _ = sqlx::query(\n                \"INSERT INTO messages" in old
   and "观察期已满的警告信未写入" not in old)
print("== EW2 现网体：P-328 已出声 ==")
ok("EW2a 「观察期已满」的信改绑到具名变量并判 Err",
   "let side = sqlx::query(" in cur
   and 'if let Err(e) = side' in cur)
ok("EW2b warn 文案在（否则红得没有理由）",
   'tracing::warn!(%e, uid, "警告信未写入")' in cur)
ok("EW2c ratio_watch 体内不再有裸 let _ = 的 messages 写入",
   "let _ = sqlx::query(\n                \"INSERT INTO messages" not in cur)
print("== EW3 三段链都在场（少一段就会误锁或漏放） ==")
ok("EW3a 进观察期 + 一次性警告信（①）",
   "SET ratio_watch_until = now() + make_interval" in cur
   and "INSERT INTO messages" in cur)
ok("EW3b 到期处置分 warn / limit_download 两档（②）",
   'action == "warn"' in cur
   and "download_locked_by = 'ratio_watch'" in cur)
ok("EW3c 期内恢复自动解除，且只解自己锁的那把（③）",
   "SET ratio_watch_until = NULL" in cur
   and "download_locked_by = 'ratio_watch'" in cur.split("③ 期内恢复")[1]
   if "③ 期内恢复" in cur else False)
print("== EW4 设定键的读者（更正 10-06 的「ratiolimit 零消费」旧账） ==")
guard = here("apps/tracker/src/http_track/guard_refresh.rs")
ok("EW4a tracker 侧读 ratiolimit / ratio_gate / ratio_gate_max / grace_days",
   all(k in guard for k in ("ratiolimit", "ratio_gate", "ratio_gate_max",
                            "ratio_gate_grace_days")))
ok("EW4b worker 侧读 ratio_watch_threshold / days / action",
   all(k in cur for k in ("ratio_watch_threshold", "ratio_watch_days",
                          "ratio_watch_action")))
def psql(stmt):
    p = sh(["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
            "-d", "fluxtorrent", "-tAc", stmt])
    return "?" if p.returncode != 0 else (p.stdout or "").strip()


# 仍零消费的键（隐私/等级页那套）：按文件普查，留作台账不判红
census = sh(["git", "grep", "-l", "-E", "min_ratio|psratio", "--",
             "apps/api/src", "apps/tracker/src", "apps/web"])
lives = sorted({ln.split(":")[-2] for ln in (census.stdout or "").splitlines()
                if ln.count(":") >= 1})
print("   WARN 仍在被读的文件（%d 个）：%s"
      % (len(lives), ", ".join(lives) or "无"))
print("   WARN 现网读数：观察期中 %s 人、已过期 %s 人（本批跑时）"
      % (psql("SELECT count(*) FROM users WHERE ratio_watch_until "
              "IS NOT NULL"),
         psql("SELECT count(*) FROM users WHERE ratio_watch_until "
              "IS NOT NULL AND ratio_watch_until < now()")))

summary()
