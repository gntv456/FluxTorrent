# -*- coding: utf-8 -*-
"""ER 常驻闸门：信箱文件夹口（P-322 的两处静默相反）

静态（CI 跑，不写库、不起服务）：
  ER0  正控：修前的 BoxReq 必须「无 deny_unknown_fields」，且删除分支
       必须是 `if let Some(bid) = body.id { … }` + 无条件 deleted:true
  ER1  现网 BoxReq 带 deny_unknown_fields；删除分支缺 id 直接 1002，
       删不动（rows_affected=0）走 1004，不再谎报成功
  ER2  mod.rs 认到了新模块（mod + pub use 两行齐），服务表还挂着两口
  ER3  拆分后两个文件各在 300 行内，且 message_box.rs 比修前更瘦（只许减）
现网只读：
  ER4  pmboxes 里没有本轮取证用的测试夹名残留
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OLD = "apps/api/src/community_http/message_box.rs"
NEW = "apps/api/src/community_http/message_folders.rs"
MOD = "apps/api/src/community_http/mod.rs"
FIX = "HEAD"          # 与修复同批入库
PSQL = ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
        "-d", "fluxtorrent", "-tAc"]


def sh(args):
    return subprocess.run(args, cwd=ROOT, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")


def blob(rev, path):
    p = sh(["git", "show", "%s:%s" % (rev, path)])
    if p.returncode != 0:
        FAILURES.append("ER 取文失败 %s:%s / %s"
                        % (rev, path, (p.stderr or "")[:120]))
        return ""
    return p.stdout or ""


def here(path):
    p = os.path.join(ROOT, path)
    if not os.path.exists(p):
        FAILURES.append("ER 文件缺失 %s" % path)
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
    return " ".join(s.split())


print("== ER0/ER1 文件夹口的两处静默相反 ==")
old = flat(strip_rs(blob("%s^" % FIX, OLD)))
new = flat(strip_rs(here(NEW)))
ok("ER0a 正控：修前 BoxReq 确实没有 deny_unknown_fields",
   re.search(r"struct BoxReq", old) is not None
   and "deny_unknown_fields" not in old[old.find("struct BoxReq") - 200:
                                        old.find("struct BoxReq")])
ok("ER0b 正控：修前删除分支是 if let Some(bid)（没 id 也照样报 deleted）",
   "if let Some(bid) = body.id" in old)
ok("ER1a 现网 BoxReq 带 deny_unknown_fields",
   re.search(r"#\[serde\(deny_unknown_fields\)\] struct BoxReq", new)
   is not None)
ok("ER1b 缺 id 的空名请求被正面拒下（1002 要 id）",
   re.search(r"let Some\(bid\) = body\.id else", new) is not None
   and "删除文件夹要给出它的 id" in new)
ok("ER1c 删不动（rows_affected=0）走 1004，不再谎报 deleted:true",
   re.search(r"if gone == 0 \{ return Err\(DomainError::NotFound\(bid\)\)",
             new) is not None)

print("== ER2/ER3 拆分与门禁 ==")
mod = here(MOD)
ok("ER2a mod.rs 注册了 message_folders（mod + pub use 两行齐）",
   "mod message_folders;" in mod and "pub use message_folders::*;" in mod)
ok("ER2b 服务表仍挂着两个文件夹口",
   ".service(message_boxes)" in mod and ".service(message_box_upsert)" in mod)
box_lines = len(here(OLD).splitlines())
old_lines = len(blob("%s^" % FIX, OLD).splitlines())
new_lines = len(here(NEW).splitlines())
ok("ER3a message_box.rs 拆分后变瘦（%d < 基线 %d）" % (box_lines, old_lines),
   box_lines < old_lines)
ok("ER3b 两个文件都在 300 行内（%d / %d）" % (box_lines, new_lines),
   box_lines <= 300 and new_lines <= 300)

print("== ER4 现网只读：取证测试夹残留 ==")
p = sh(PSQL + ["SELECT count(*) FROM pmboxes WHERE name LIKE 'P322%' "
               "OR name LIKE 'EP322%'"])
if p.returncode != 0:
    print("   WARN 库不可达：ER4 本轮不判（静态六条照常承重）")
else:
    left = (p.stdout or "").strip()
    ok("ER4 本轮取证用的测试夹名残留 0 个", left == "0", "实读 %s" % left)

summary()
