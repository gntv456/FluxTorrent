# -*- coding: utf-8 -*-
"""ET 常驻闸门：过审副作用三条 SQL 的失败必须出声（P-323）

静态：
  ET0 正控  取 bc025ad4（本批修复之前）的 review_side_effects.rs，
            三条语句必须全是 `let _ = sqlx::query(` 且 0 处 warn
  ET1  现网体：`let _ = sqlx::query(` 归零；`if let Err(e) = side` 恰 3 处，
            每条都带 tracing::warn!；torrent_id 作为字段带上（否则日志查不到
            是哪枚种子的副作用失败）
现网只读（库不可达则本轮跳过）：
  ET2  auto_promo_rules 里 kind 不在枚举内的行必须为 0 —— 这类脏规则正是
       本批红证的引信（现网实测：命中规则 + 转正 ⇒ promotions 0 行、日志 0 行）
  ET3  本批取证不留痕：临时规则/临时候选都没残留、探针种价格回 0
pt_audit_lib.ok 的签名是 ok(name, cond, detail)；正控钉字面提交号。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RS = "apps/api/src/admin_http/review_side_effects.rs"
PREV = "bc025ad4"      # 本批修复之前的最后一个提交（正控取它那份原文）
PSQL = ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
        "-d", "fluxtorrent", "-tAc"]


def sh(args):
    return subprocess.run(args, cwd=ROOT, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")


def blob(rev, path):
    p = sh(["git", "show", "%s:%s" % (rev, path)])
    if p.returncode != 0:
        FAILURES.append("ET 取文失败 %s:%s / %s"
                        % (rev, path, (p.stderr or "")[:140]))
        return ""
    return p.stdout or ""


def here(path):
    p = os.path.join(ROOT, path)
    if not os.path.exists(p):
        FAILURES.append("ET 文件缺失 %s" % path)
        return ""
    return open(p, encoding="utf-8", errors="replace").read()


def strip_rs(s):
    out = []
    for line in s.splitlines():
        i = line.find("//")
        if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
            line = line[:i]
        out.append(line)
    return "\n".join(out)


def flat(s):
    return " ".join(strip_rs(s).replace("\\\n", " ").split())


print("== ET0/ET1 出声（不阻断）的形状 ==")
old = flat(blob(PREV, RS))
new = flat(here(RS))
ok("ET0a 正控：修前三条语句全是 let _ =（0 处 warn）",
   old.count("let _ = sqlx::query(") == 3 and "tracing::warn!" not in old,
   "let _ 计数=%s warn=%s" % (old.count("let _ = sqlx::query("),
                            "tracing::warn!" in old))
sw = new.count("let _ = sqlx::query(")
ok("ET1a 现网体不再有 let _ = sqlx::query", sw == 0, "残留 %s 处" % sw)
n = len(re.findall(r"if let Err\(e\) = side", new))
ok("ET1b 三个失败分支各自出声（恰 3 处）", n == 3, "实数 %s" % n)
w = new.count("tracing::warn!")
ok("ET1c 每处都走 tracing::warn!", w == 3, "实数 %s" % w)
ok("ET1d 日志带 torrent_id（否则查不到是哪枚种子）",
   len(re.findall(r"tracing::warn!\(\?e, torrent_id,", new)) == 3)

print("== ET2/ET3 现网只读 ==")


def q(stmt):
    p = sh(PSQL + [stmt])
    return "" if p.returncode != 0 else (p.stdout or "").strip()


if not q("SELECT 1"):
    print("   WARN 库不可达：ET2/ET3 本轮不判（静态四条照常承重）")
else:
    bad = q("SELECT count(*) FROM auto_promo_rules WHERE enabled "
            "AND kind NOT IN (SELECT unnest(enum_range(NULL::"
            "promotion_kind_enum))::text)")
    if bad == "":
        print("   WARN 枚举探测失败，ET2 本轮不判")
    else:
        ok("ET2 没有 kind 不在枚举内的启用规则（引信已拆）", bad == "0",
           "实读 %s 条" % bad)
    r = q("SELECT count(*) FROM auto_promo_rules WHERE name = 'ES37bogus'")
    o = q("SELECT count(*) FROM offers WHERE user_id = 1 AND torrent_id = "
          "40049 AND votes = 5")
    pr = q("SELECT price FROM torrents WHERE id = 40049")
    ok("ET3a 取证临时规则无残留", r == "0", "实读 %s" % r)
    ok("ET3b 取证临时候选无残留", o == "0", "实读 %s" % o)
    ok("ET3c 探针种 40049 价格已复原为 0", pr == "0", "实读 %s" % pr)

summary()
