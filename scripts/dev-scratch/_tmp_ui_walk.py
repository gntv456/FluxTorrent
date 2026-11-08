# -*- coding: utf-8 -*-
"""登录态 SSR 走查：用自签 token 当 cookie，比对匿名/登录两态是否真的不同。"""
import io
import re
import subprocess
import sys

sys.path.insert(0, "scripts")
from pt_audit_lib import jwt_token

TOK = jwt_token("root")
PAGES = ["/", "/torrents", "/forums", "/me", "/contests", "/shop"]


def fetch(path, cookie):
    args = ["curl", "-sL", "--max-time", "30", "-o", "-",
            "-w", "\n__STATUS__%{http_code}__%{size_download}"]
    if cookie:
        args += ["-H", "Cookie: " + cookie]
    args.append("http://127.0.0.1:3000" + path)
    r = subprocess.run(args, capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    out = r.stdout or ""
    m = re.search(r"__STATUS__(\d+)__(\d+)\s*$", out)
    body = out[:m.start()] if m else out
    return (int(m.group(1)) if m else 0,
            int(m.group(2)) if m else len(body), body)


COOKIE = "flux_token=%s; flux.session=%s" % (TOK, TOK)
for p in PAGES:
    sa, la, ha = fetch(p, "")
    sb, lb, hb = fetch(p, COOKIE)
    # 登录态特征：匿名页里几乎没有的用户名/退出入口
    logged_marks = ("退出", "我的", "root")
    hit = sum(hb.count(k) for k in logged_marks)
    diff = "不同" if ha != hb else "完全相同"
    print("%-10s anon=%s/%dB  login=%s/%dB  %s  登录特征命中=%d"
          % (p, sa, la, sb, lb, diff, hit))
