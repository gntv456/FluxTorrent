# -*- coding: utf-8 -*-
"""CW 闸门（批七十九 P-286）：保种区不许把「取数失败」演成「没有要救的种」。

现网（修前，2026-10-14：把渲染上游指到一个死端口实测）：
    GET /preserve  → http=200，页面写「共 0 个结果 · 暂无保种需求」，
                      统计六格全 0，**没有任何错误提示**
    同一故障下 /collections、/torrents 都回「加载失败 fetch failed」
⇒ `lib/data-content.ts::getPreserve` 把异常 catch 成「items=[] + stats 全 0」
  的空壳，页面按「确实没有」渲染。保种区是催会员去救种的板块，
  失败被读成「一切安好」比报错严重得多。
修：getPreserve 不再吞错，页面按四态渲染 ErrorPanel（复用批七十七搬出来的
  `lib/render-error.ts::normalizeError`，服务端组件不得调用 client 模块的函数）。

修后 A/B（同一份源码，两个上游）：
    死上游 → 可见文本含「加载失败」「fetch failed」+ 重试链接，无「暂无保种需求」
    活上游 → 正常出保种行（含「认领」按钮），无错误面板

本闸门常驻判据（不依赖临时 dev 端口，跑在部署栈 + HEAD 源码上）：
  CW1 静态：getPreserve 函数体内不得再出现「catch 后返回空壳」的形状；
  CW2 静态：保种页必须真的接上错误面板（ErrorPanel + normalizeError 从 lib 取）；
  CW3 现网：`/preserve` 正常态不得出现错误面板（防反向误伤）；
  CW4 现网：整族形状还剩几处只 WARN 记账（本批不动别人的活）；
  CW5 现网读数：`/preserve?status=不存在的值` 这类筛选参数的实际口径。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_cw_preserve_fail_visible.py
只读：只发 GET。
"""
import io  # noqa: E402
import json  # noqa: E402
import os  # noqa: E402
import re  # noqa: E402
import subprocess  # noqa: E402
import sys  # noqa: E402
import urllib.error  # noqa: E402
import urllib.request  # noqa: E402

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, login, ok, summary  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WEB = os.environ.get("FLUX_WEB_BASE", "http://127.0.0.1:3000")
ROOT = login()


def src(rel):
    return io.open(os.path.join(REPO, rel), encoding="utf-8",
                   errors="replace").read().replace("\r\n", "\n")


def page(qs):
    req = urllib.request.Request(WEB + qs)
    req.add_header("Cookie", "flux_token=%s; flux.session=1" % ROOT)
    try:
        r = urllib.request.urlopen(req, timeout=90)
        return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")


def strip_code(s):
    return "\n".join(l.split("//")[0] for l in s.splitlines())


print("== CW1 静态：getPreserve 不再把异常吞成空壳 ==")
dc = src("apps/web/lib/data-content.ts")
m = re.search(r"export async function getPreserve\((.*?)\n\}", dc, re.S)
body = m.group(1) if m else ""
swallow = ("catch" in strip_code(body)) and ("items: []" in strip_code(body))
ok("CW1 getPreserve 体内没有「catch → 返回空列表/全 0 统计」的形状 "
   "（catch=%s 空壳=%s，函数体 %d 字符）" % ("catch" in strip_code(body),
                                          "items: []" in strip_code(body),
                                          len(body)),
   bool(body) and not swallow)

print("== CW2 静态：保种页真的接上了错误面板 ==")
pp = src("apps/web/app/(main)/preserve/page.tsx")
bare = strip_code(pp)
ok("CW2 页面同时有 try/catch、ErrorPanel、以及从 lib/render-error 取的 "
   "normalizeError（不从 client 模块取）",
   "catch (e)" in bare and "ErrorPanel" in bare
   and 'from "@/lib/render-error"' in bare
   and "normalizeError" in bare
   and "retryHref" in bare,
   "catch=%s panel=%s" % ("catch (e)" in bare, "ErrorPanel" in bare))

print("== CW3 现网：正常态不得被反向误伤成错误面板 ==")
st, h = page("/preserve")
empty_ok = ("ui-err__msg" not in h) and ("暂无保种需求" in h or "认领" in h)
ok("CW3 `/preserve` 健康时是板面不是错误面板 http=%s（板面标志=%s）"
   % (st, "认领" in h), st == 200 and "ui-err__msg" not in h and empty_ok,
   "http=%s ui-err=%d 长度=%d" % (st, h.count("ui-err"), len(h)))

print("== CW4 同族形状余量（只记账，不顺手改别人的）==")
n_left = len(re.findall(r"} catch \{\s*\n\s*return (?:\[\]|\{)",
                        strip_code(dc)))
print("   WARN `lib/data-content.ts` 里还有 %d 处「catch 后 return 空值」——"
      "各自的页面语义要逐个判（有些返回 null 是有意的缺省），本批只收保种区"
      % n_left)
ok("CW4 至少 getPreserve 这一处已经不在该形状里",
   "getPreserve" in dc and not swallow)

print("== CW5 现网读数：保种筛选参数的严格口径 ==")
for qs in ("/preserve?status=not_a_status", "/preserve?scope=9",
           "/preserve?keyword=%E7%A7%8D%E5%AD%90"):
    st2, h2 = page(qs)
    code = None
    try:
        j = json.loads(h2)
        code = j.get("code")
    except Exception:  # noqa: BLE001
        pass
    print("   读数 %-34s http=%s 页面含「共 %s」=%s 含错误面板=%s api码=%s"
          % (qs, st2, "0 个", "共 0 个结果" in h2,
             "ui-err__msg" in h2, code))

sys.exit(summary())
