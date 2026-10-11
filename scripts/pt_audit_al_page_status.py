# -*- coding: utf-8 -*-
"""批二十三闸门 AL：详情页的 HTTP 语义（P-35 的守卫）。

盯的是这一族：六个详情页 `catch { notFound(); }` 把**任何**后端失败
（429 限流、5xx、瞬断、非 JSON）渲染成 404「不存在」——根因是
`ApiError` 过去把所有非 2xx 压成 `code=1000`，前端拿不到状态。

两头都要钉，缺一条就会假绿：
  正例：真不存在的 id → 页面仍必须 404（不能被改成 500）；
  反例：把接口打到限流后，真实存在的 id → 页面**不得** 404。

页面状态从 web 容器内部取（node 20 自带 fetch）：SSR 与 `/api/v1/*`
反代共用同一条链路，才可能复现「同一条 URL 忽 200 忽 404」。
会话：先经反代登录拿 Set-Cookie，之后的请求都带上——
无会话时详情页对谁都 404（服务端 notFound 的既有约定），
那会让反例假绿、正例假红，所以会话是判据成立的前提。
"""
import os
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

BASE = "http://127.0.0.1:3000"
# 会话要两片：body 里的 token（API 侧 require_auth 认 Cookie/Bearer），
# 加 middleware 只做页面准入用的存在性标记 flux.session（见 web/middleware.ts）。
# 少任何一片都会被 307 送回 /login——假红的教训就写在这里。
LOGIN_JS = (
    "fetch('%s/api/v1/auth/login',{method:'POST',"
    "headers:{'content-type':'application/json'},"
    "body:JSON.stringify({username:'root',password:'password123'})})"
    ".then(r=>r.json())"
    ".then(j=>console.log('flux.session=1; flux_token='+j.data.token))"
    ".catch(e=>console.log('NONE|'+e))"
    % BASE)
HIT_JS = (
    "fetch(process.argv[1],{headers:{cookie:process.argv[2]||''},"
    "redirect:'manual'})"
    ".then(async r=>console.log(r.status+'|'+"
    "(await r.text()).replace(/<[^>]*>/g,' ').slice(0,400)))"
    ".catch(e=>console.log('0|'+e))")
# 会话生效判据要查 profile hero 专属标记（上导航栏也印 root，纯用户名会假绿），
# 而 hit_raw 会把标签剥掉——所以另开一条原样返回前 4000 字节的探针。
RAW_JS = (
    "fetch(process.argv[1],{headers:{cookie:process.argv[2]||''},"
    "redirect:'manual'})"
    ".then(async r=>console.log(r.status+'|'+(await r.text()).slice(0,12000)))"
    ".catch(e=>console.log('0|'+e))")


def sh(args):
    return subprocess.run(args, capture_output=True, text=True,
                          encoding="utf-8", timeout=60)


def q(sql_text):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    return p.stdout.strip()


def cookie_pair():
    p = sh(["docker", "exec", "flux-web", "node", "-e", LOGIN_JS])
    raw = (p.stdout or "").strip()
    return raw if raw.startswith("flux.session=") else ""


def hit_raw(url, cookie="", js=HIT_JS):
    p = sh(["docker", "exec", "flux-web", "node", "-e", js, url, cookie])
    out = (p.stdout or "").strip().splitlines()
    line = out[-1] if out else "0|"
    head, _, body = line.partition("|")
    try:
        return int(head), body
    except ValueError:
        return 0, line


def hit(url, cookie=""):
    return hit_raw(url, cookie)[0]


CK = cookie_pair()
print("== A0 前置：会话与真实对象 ==")
ok("A0 经反代拿到会话 cookie（判据前提）", len(CK) > 10, "cookie=%d 字节"
   % len(CK))
uid = q("SELECT id::text FROM users WHERE username = 'root' AND status < 2")
ok("A0 取到真实存在的站长 id", uid.isdigit(), "uid=%s" % uid)
# 会话是否真的生效，不能只看 200：未登录会被重定向到登录页，也是 200；
# 而「用户名出现在正文里」也不够——上导航栏也印当前登录用户名（root），
# 会假绿。钉死 profile hero 专属标记 up-hero__name（假绿的教训在这一行）。
uname = q("SELECT username FROM users WHERE id = %s" % uid)
marker = 'up-hero__name"><h1'
g_code, g_body = hit_raw("%s/users/%s" % (BASE, uid), CK, js=RAW_JS)
ok("A0 会话确实生效（profile hero 里出现该成员用户名）",
   g_code == 200 and marker in g_body and uname in g_body,
   "http=%s user=%s 命中=%s" % (g_code, uname, uname in g_body))
ok("A0 前置没有踩到登录重定向（Location 不该是 /login）",
   g_code not in (301, 302, 307, 308), "http=%s" % g_code)

print("== A1 正例：真不存在的 id 必须仍是 404 ==")
for name, path in [("users", "/users/999999999"),
                   ("torrent", "/torrent/999999999"),
                   ("collections", "/collections/999999999")]:
    s = hit(BASE + path, CK)
    ok("A1 %s 不存在 → 404（不是 500 也不是空白 200）" % name, s == 404,
       "http=%s" % s)

print("== A2 反例：接口限流中，真实 id 不得被报成「不存在」 ==")
api = "%s/api/v1/users/%s" % (BASE, uid)
codes = [hit(api, CK) for _ in range(60)]
throttled = codes.count(429)
ok("A2 前置：确实把 SSR 反代打到限流（429>0）", throttled > 0,
   "429×%d 其它=%s" % (throttled, sorted(set(codes))))
page = hit("%s/users/%s" % (BASE, uid), CK)
ok("A2 限流窗口里真实主页不是 404（读不到 ≠ 不存在）",
   page != 404, "http=%s" % page)

print("== A3 收尾：等限流窗口过去，同一 URL 回到 200 ==")
end = time.time() + 200
back = 0
while time.time() < end:
    back = hit("%s/users/%s" % (BASE, uid), CK)
    if back == 200:
        break
    time.sleep(5)
ok("A3 限流退去后主页 200（没有把页面改坏）", back == 200, "http=%s" % back)
summary()
