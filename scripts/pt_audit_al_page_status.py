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
# 会话是否真的生效：批八十七之前这里认的是 profile hero 的 HTML 标记
# （`up-hero__name"><h1`），结果页面一改就假红——常驻闸门长红会淹掉真信号。
# 改成 API 侧证据：带 cookie 打 /api/v1/me 必须 200 且回的是 root 的 id。
# 「未登录也被 200」这个坑仍然成立，所以判据不能只看状态码。
uname = q("SELECT username FROM users WHERE id = %s" % uid)
m_code, m_body = hit_raw("%s/api/v1/me" % BASE, CK, js=RAW_JS)
ok("A0 会话确实生效（API 侧证据：/me 200 且带 root 的 id，不再认 HTML 标记）",
   m_code == 200 and ('"id":%s' % uid) in m_body.replace(" ", ""),
   "http=%s id=%s user=%s" % (m_code, uid, uname))
g_code, g_body = hit_raw("%s/users/%s" % (BASE, uid), CK, js=RAW_JS)
ok("A0 前置没有踩到登录重定向（Location 不该是 /login）",
   g_code not in (301, 302, 307, 308), "http=%s" % g_code)

print("== A1 正例：真不存在的 id 的 HTTP 语义（批八十七按 P-298 分档）==")
# 这三段都带 `loading.tsx`：Next 先把 shell 以 200 冲出，页面里的 notFound()
# 执行了也改不回状态码（P-298，机理与全站清单见 pt_ef_page_status_boundary.py）。
# 所以这里不再让它们常驻红，而是三态判：
#   404  → 说明这一段已修好，**必须从 KNOWN_P298 删名**（不删就红，防欠账变哑）；
#   200  → 允许，但页面必须真渲染出「不存在」文案（空壳 200 = 新的洞）；
#   其它 → 一律红。新增带 boundary 的详情段请同步加进 KNOWN_P298 并去 EF 记账。
KNOWN_P298 = {"users", "torrent", "collections"}
# 「有没有渲染出『不存在』」必须扫**整份响应**：RAW_JS 只回前 12000 字节，
# 而文案在 flight 数据里更靠后——第一版因此把「其实有文案」误判成空壳。
NF_JS = ("fetch(process.argv[1],{headers:{cookie:process.argv[2]||''},"
         "redirect:'manual'}).then(async r=>{"
         "const t=await r.text();console.log(r.status+'|'+("
         "/不存在|未找到|找不到|not ?found/i.test(t)?'Y':'N'))})"
         ".catch(()=>console.log('0|E'))")


def status_and_404text(url, ck):
    p = sh(["docker", "exec", "flux-web", "node", "-e", NF_JS, url, ck])
    out = (p.stdout or "").strip().split("|")
    try:
        return int(out[0]), (len(out) > 1 and out[1] == "Y")
    except ValueError:
        return 0, False


for name, path in [("users", "/users/999999999"),
                   ("torrent", "/torrent/999999999"),
                   ("collections", "/collections/999999999")]:
    s, 文案 = status_and_404text(BASE + path, CK)
    if s == 404:
        ok("A1 %s 不存在 → 404：已修好，请把 %r 从 KNOWN_P298 删掉（P-298 "
           "清册只减不增）" % (name, name), name not in KNOWN_P298,
           "http=404 仍在册=%s" % (name in KNOWN_P298))
    elif s == 200:
        ok("A1 %s 不存在 → P-298 在册：流式 boundary 吃掉 404，但 200 里必须"
           "有「不存在」文案（空壳 200 不可接受）" % name,
           name in KNOWN_P298 and 文案,
           "http=200 在册=%s 全文案=%s" % (name in KNOWN_P298, 文案))
    else:
        ok("A1 %s 不存在 → 状态码只能是 404 或在册 200，实测 %s" % (name, s),
           False, "http=%s" % s)

print("== A2 反例：接口限流中，真实 id 不得被报成「不存在」 ==")
api = "%s/api/v1/users/%s" % (BASE, uid)
# 60 次打不满 60/分这一档（前置会假红：本轮实测 429×0、全是 200）——
# 判据的前提要自己成立，把次数提到 90 并在断言里带上实测分布。
# 前提要自己成立：逐次 `docker exec node -e` 起进程，90 次拉跨超过一分钟，
# 60/分的窗口中途回血 ⇒ 永远打不到 429（本轮实测 429×0 就是这么来的假红）。
# 改成单进程内 Promise.all 并发打满，必要时加大并发再试一轮。
HAMMER_JS = (
    "const u=process.argv[1],c=process.argv[2],n=Number(process.argv[3]);"
    "Promise.all(Array.from({length:n},()=>fetch(u,{headers:{cookie:c},"
    "cache:'no-store'}).then(r=>r.status))).then(a=>{"
    "const m={};a.forEach(s=>m[s]=(m[s]||0)+1);"
    "console.log(Object.entries(m).map(([k,v])=>k+'x'+v).join(' '));});")


def hammer(url, n):
    p = sh(["docker", "exec", "flux-web", "node", "-e", HAMMER_JS,
            url, CK, str(n)])
    line = (p.stdout or "").strip().splitlines()
    dist = {}
    for part in (line[-1] if line else "").split():
        k, _, v = part.partition("x")
        if k.isdigit():
            dist[int(k)] = int(v)
    return dist


dist = {}
for n in (120, 240):
    dist = hammer(api, n)
    if dist.get(429):
        break
throttled = dist.get(429, 0)
ok("A2 前置：并发打满确实把 SSR 反代打到限流（429>0）", throttled > 0,
   "分布=%s（旧写法逐次 exec 跨窗口 ⇒ 假红）" % (dist or "空",))
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
