# -*- coding: utf-8 -*-
"""闸门 HY：批量收种的浏览器通路必须过 CSRF，且失败要说出原因（批二百零五）。

活体 A/B（2026-10-20，root，同一条 POST /torrents/batch-download {"ids":[40]}）：
    A 只带 flux_token cookie          -> 403 {"code":2003,"message":"无权限"}
    B 同 cookie + 同名 x-csrf-token   -> 200  x-batch-skipped: 0
    C Bearer 令牌                     -> 200  x-batch-skipped: 0
后端 `csrf_mw` 的判据是「非安全方法 + /api/v1 + 带了非空会话 cookie」就必须
`X-CSRF-Token` 与 cookie 逐字相等（api-client.ts:41 的成文契约）。浏览器里
flux_token 是 HttpOnly cookie、同源必带 ⇒ 没有 csrf 头就是 A 那一档。

而 apps/web/components/torrent-batch.tsx:97 正是**组件内裸 fetch**（方案
§8.3.2 明令禁止，api-client.ts 顶部注释也写着「组件内禁止裸 fetch」），
它只带 Content-Type，不带 x-csrf-token，也够不到 api-client 私有的
`csrfHeaderAsync`（缺 flux_csrf 时会去 GET /me/csrf 续期那一步）⇒
批量收种在真浏览器里整条 403。第 102 行 `throw new Error(String(res.status))`
把信封丢掉、第 115 行 `catch {}` 统一显示 dict.common.loadFailed ⇒
用户看到的是「加载失败」，既不知道是 CSRF（前端 bug），也不知道是登录态/权限。

同一条链上已经接好的两半（本闸门按硬判据钉住，防回退）：
  · 服务端 batch.rs 会回 `X-Batch-Skipped`（件数差额）；
  · 前端第 103/112 行确实读它并显示 t.batchSkipped。
所以「少发几件」这一维已经出声；剩下的洞是「一件都没发出去时不出声」。

判据：
  HY1 硬判据：POST 只带会话 cookie 打到 /api/v1 写口必须被拒 403/2003
      （若哪天 CSRF 被整体关掉，A/B 的两档会塌成一档 ⇒ 此条红）；
  HY2 硬判据：补同名 x-csrf-token 必须放行 200（与 HY1 合成一对尺，
      证明 HY1 的红是 CSRF 判据本身在拦，不是登录/限流）；
  HY3 在册：torrent-batch.tsx 仍是组件内裸 fetch 且不带 csrf 头（改好即红，
      届时翻成「必须走 api.postBlob 或自带 csrf 头」的硬判据）；
  HY4 在册：批量口失败路径仍把信封丢掉、只显示通用 loadFailed；
  HY5 硬判据：服务端仍必须回 X-Batch-Skipped 且前端仍必须读它（两侧任一侧
      断线即红——件数这条不许退化）；
  HY6 承重：把「已收口」的合成形状（走 api.postBlob + apiErrorMessage）喂给
      HY3/HY4 探测器，必须都不报在册。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hy_batch_csrf.py`
      HY_LIVE=0 跳过 HY1/HY2（两条都是非破坏式只读探：ids=[40] 是 price=0
      的闸门种，且 HY1 直接被 CSRF 拦在业务之前，不会动账）。
"""
import io
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import BASE, login, ok, summary  # noqa: E402

LIVE = os.environ.get("HY_LIVE", "1") != "0"
BATCH_TS = "apps/web/components/torrent-batch.tsx"
BATCH_RS = "apps/api/src/torrent_http/batch.rs"

# HY6 正控：已收口的形状（走统一客户端 + 失败文案走 apiErrorMessage）
GOOD_SHAPE = """
async function download() {
  const meta: { skipped?: number } = {};
  try {
    const buf = await api.postBlob("/api/v1/torrents/batch-download",
      { ids }, meta);
    save(buf);
  } catch (e) {
    setMsg(apiErrorMessage(dict, e));
  }
}
"""


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def code_only(ts):
    """剥 // 与 /// 与 /* */ 注释：只认真码。"""
    ts = re.sub(r"/\*\*?.*?\*/", "", ts, flags=re.S)
    return "\n".join(ln for ln in ts.split("\n")
                     if not ln.strip().startswith("//"))


def bare_post_fetch(src):
    """HY3：组件内 `fetch(...)` 打 /api/v1 且请求头里没有 csrf 头。"""
    hits = []
    for m in re.finditer(r"fetch\((.{0,400}?)\)\s*;", src, flags=re.S):
        seg = m.group(1)
        if "/api/v1" in seg and ("method: \"POST\"" in seg or
                                 re.search(r'method: "POST', src)):
            if "csrf" not in seg.lower():
                hits.append(seg[:60].replace("\n", " "))
    # 整文件维度也要认：有裸 fetch 打 /api/v1、全文却没有 csrf 头字样
    if not hits and re.search(r"fetch\(\s*[\"'`]/api/v1", src) \
            and "csrf" not in src.lower():
        hits.append("fetch(/api/v1…) 全文无 csrf")
    return hits


def drops_envelope(src):
    """HY4：失败路径只留状态码/通用文案（throw new Error(String(res.status))
    或 catch 块里没有 apiErrorMessage/e.message）。"""
    if re.search(r"throw new Error\(String\(res\.status\)\)", src):
        return True
    for m in re.finditer(r"catch\s*(?:\([^)]*\))?\s*\{([^{}]*)\}", src,
                         flags=re.S):
        body = m.group(1)
        if "loadFailed" in body and not re.search(
                r"apiErrorMessage|e\.message|err\.message", body):
            return True
    return False


ts = code_only(rd(BATCH_TS))
rs = code_only(rd(BATCH_RS))

bare = bare_post_fetch(ts)
ok("HY3 在册：批量收种仍是组件内裸 fetch 且不带 x-csrf-token",
   bool(bare) and "api.postBlob" not in ts,
   "命中=%s；方案 §8.3.2 禁止组件内裸 fetch，这里够不到 csrf 续期"
   % (bare[:1],))
ok("HY4 在册：批量口失败仍把信封丢掉、只显示通用 loadFailed",
   drops_envelope(ts) and "apiErrorMessage" not in ts,
   "403/2003「无权限」与「没登录」「被限流」在界面上是同一句「加载失败」")

ok("HY5 硬判据：件数差额两侧都必须在（服务端发 + 前端读）",
   re.search(r'X-Batch-Skipped",\s*skipped\.to_string', rs) is not None
   and re.search(r'headers\.get\("X-Batch-Skipped"\)', ts) is not None,
   "服务端 batch.rs 与 torrent-batch.tsx:103 任一侧断线即红")

good = code_only(GOOD_SHAPE)
ok("HY6 承重：已收口形状不被 HY3/HY4 探测器误报",
   not bare_post_fetch(good) and not drops_envelope(good),
   "裸fetch命中=%s 丢信封=%s" % (bool(bare_post_fetch(good)),
                              drops_envelope(good)))

if LIVE:
    tok = login("root", "password123")

    def post(headers):
        req = urllib.request.Request(
            "%s/torrents/batch-download" % BASE,
            data='{"ids":[40]}'.encode(),
            headers=dict({"Content-Type": "application/json"}, **headers))
        try:
            with urllib.request.urlopen(req) as r:
                return r.status, r.read()[:120].decode("utf-8", "replace")
        except urllib.error.HTTPError as e:
            return e.code, e.read()[:120].decode("utf-8", "replace")

    sa, ba = post({"Cookie": "flux_token=" + tok})
    sb, bb = post({"Cookie": "flux_token=%s; flux_csrf=hyprobe" % tok,
                   "x-csrf-token": "hyprobe"})
    ok("HY1 在册：只带会话 cookie 的 POST 被 csrf_mw 拒（浏览器同形）",
       sa == 403 and '"code":2003' in ba,
       "A档 http=%s body=%s ⇒ 组件裸 fetch 在真浏览器里就是这一档"
       % (sa, ba[:70]))
    ok("HY2 对照：补同名 x-csrf-token 必须放行（证明 HY1 拦的是 CSRF 本身）",
       sb == 200 and '"code"' not in bb,
       "B档 http=%s body=%s" % (sb, bb[:60]))
else:
    print("[HY1/HY2] HY_LIVE=0 跳过活体两条")

summary()
