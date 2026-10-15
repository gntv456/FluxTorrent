# -*- coding: utf-8 -*-
"""EX 常驻闸门：LLM 宿主能力的出网纪律与闸门顺序（批一百四十九）

站方 `b624b5e8` 新加的 `host.llm_chat` 注释声称「挂在适配器层就天然继承
沙箱 + 出网白名单 + 限流 + 能力声明」，但它用的是 `reqwest::blocking`
**自建客户端**，绕过了 `adapter_call.rs::adapter_http_exec` 那条已成文的
P2 纪律——`redirect::Policy::none()`（白名单域 302 跳 169.254.169.254 之类
必须拒绝，而不是跟随；跟随还会把 bearer key 带到别的 host）。
本批补上同一纪律，并把「顺序」钉住：先查能力、再查限流、最后才出网。
"""
import glob
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# LLM 那一个导入已从 adapter_host.rs 拆到 adapter_host_llm.rs（守 300 行
# 门禁）；判据跟着走，否则 EX2/EX3 会在空窗口上判红——那是「判据打空」
# 不是产品红。EX4 普查两个文件都扫。
HOST = "apps/api/src/adapter_host_llm.rs"
CALL = "apps/api/src/adapter_call.rs"
PREV_FILE = "apps/api/src/adapter_host.rs"
PREV = "b624b5e8"  # 新能力落地的那次提交（正控取它，别写 HEAD）


def sh(args):
    return subprocess.run(args, cwd=ROOT, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")


def blob(rev, rel):
    p = sh(["git", "show", "%s:%s" % (rev, rel)])
    if p.returncode != 0:
        FAILURES.append("EX 取文失败 %s:%s / %s"
                        % (rev, rel, (p.stderr or "")[:120]))
        return ""
    return p.stdout or ""


def here(rel):
    p = os.path.join(ROOT, rel)
    if not os.path.exists(p):
        FAILURES.append("EX 文件缺失 %s" % rel)
        return ""
    return open(p, encoding="utf-8", errors="replace").read()


def llm_chat_body(src):
    """只取 fn llm_chat 到下一个 fn 之间的体，避免把别的客户端算进来。"""
    i = src.find("fn llm_chat(")
    if i < 0:
        return ""
    j = src.find("\nfn ", i + 5)
    k = src.find("pub(crate) fn register_llm", i)
    ends = [x for x in (j, k) if x > i]
    return src[i:min(ends)] if ends else src[i:]


old = llm_chat_body(blob(PREV, PREV_FILE))
print("== EX1 正控：新能力落地那天，出网客户端没有禁跟随重定向 ==")
ok("EX1a 正控能取到修前 llm_chat 体（空串=参照物失效）",
   "reqwest::blocking" in old, "取到 %d 字符" % len(old))
ok("EX1b 修前确实没有 Policy::none（有就不是这个洞了）",
   "Policy::none" not in old)

now = llm_chat_body(here(HOST))
print("== EX2 现网体：出网纪律与先例一致 ==")
ok("EX2a llm_chat 的客户端禁止跟随重定向",
   "redirect(reqwest::redirect::Policy::none())" in now,
   "取到 %d 字符" % len(now))
ok("EX2b 超时仍在（60s，别把闸门改成无限等）", "from_secs(60)" in now)
ok("EX2c 未配置时仍先返回 -4，不出网",
   now.find("llm_configured") < now.find("reqwest::blocking"))
call = here(CALL)
ok("EX2d 先例那条执行器仍禁跟随重定向（不许反向拆掉）",
   "redirect(reqwest::redirect::Policy::none())" in call)

print("== EX3 闸门顺序：能力 → 限流 → 出网（越权不该吃配额） ==")
# 窗口必须取到 register_llm 之内：rate_take() 在别的主机导入里也出现，
# 拿整篇文件的下标比顺序会把我自己的判据搞反（首轮就红在这里）。
head = here(HOST)
reg_i = head.find("pub(crate) fn register_llm")
reg = head[reg_i:] if reg_i >= 0 else ""
i_cap = reg.find("manifest.llm_routes.iter().any")
i_rate = reg.find("store.rate_take()")
i_net = reg.find("llm_chat(&route")
ok("EX3a 三段都在且顺序对（窗口只取 register_llm 体内）",
   -1 < i_cap < i_rate < i_net,
   "cap=%d rate=%d net=%d 体长=%d" % (i_cap, i_rate, i_net, len(reg)))
ok("EX3b 能力闸门是精确等值匹配，不是前缀/包含（防 `x/` 式绕过）",
   "r == &route" in reg and "starts_with(&route" not in reg)
ok("EX3c 越权拒绝要出声（warn 级，别静默回 -2）",
   "LLM 路由越权拒绝" in reg)

print("== EX4 普查：凡自建 reqwest 客户端的文件都要禁跟随重定向 ==")
ports = []
for f in glob.glob(os.path.join(ROOT, "apps/api/src/*.rs")):
    src = open(f, encoding="utf-8", errors="replace").read()
    if "Client::builder()" in src or "ClientBuilder" in src:
        ports.append(os.path.relpath(f, ROOT).replace("\\", "/"))
for rel in sorted(ports):
    src = here(rel)
    print("   %-34s Policy::none=%d" % (rel, src.count("Policy::none")))
bad_files = [r for r in ports if "Policy::none" not in here(r)]
ok("EX4 每个自建客户端都禁跟随重定向（只判真建客户端的文件）",
   not bad_files and len(ports) >= 2,
   "在册 %d 个，漏禁的=%s" % (len(ports), bad_files or "无"))

summary()
