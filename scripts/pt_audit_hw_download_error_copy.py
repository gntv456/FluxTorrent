# -*- coding: utf-8 -*-
"""闸门 HW：下载失败的「可行动原因」必须一路走到界面（批二百零三 P-430）。

洞（现网实测 2026-10-20，root @ :8080）：
    GET /api/v1/torrents/49417/download
      -> 400 {"code":1002,
              "message":"参数校验失败: 种子文件缺失（原始 .torrent 未存档），
                        请联系管理组补档","request_id":"c1967f92-…"}
    GET /api/v1/torrents/49424/download -> 200 application/x-bittorrent
后端刻意区分两种失败（download.rs 评审 P1-4：种子行不在=真不存在；行在而原文
丢失=站点数据异常，必须给可行动提示），同口还有 charge.rs:44「您的下载权限已
被暂停，请联系管理组」与火花不足 4001/409。而前端 blob 口 `requestBlob` 从不
读响应体：任何非 2xx 一律造 `ApiError(1000, 下载失败（HTTP {status}）)`——这句
中文在 fetch 层就被丢掉，管理组没补档 / 下载权被暂停 / 余额不足三种处境在界面
上长一个样，会员只能盲重试或报障。四条 getBlob 出口同一个洞。

判据分层（静态五 + 活体二）：
  HW1 blob 口失败分支必须走共用归一化，不许再用裸状态码造错；
  HW2 归一化必须真读 body（res.json()）、带上 code/message/request_id，
      且只在「不是 JSON」时才回落兜底文案；
  HW3 同尺对质：JSON 口 `request` 与 blob 口 `requestBlob` 必须都调
      toApiError——只修一处等于没修（同 HJ3 手法）；
  HW4 出口文案台账：调 getBlob 的文件里，报错文案要么走 apiErrorMessage，
      要么在「已知未统一」台账里（现=字幕四出口）；新增一只即红、
      台账里少一只也红（改了就得回来销账，不许悄悄留着）；
  HW5 承重自证：把旧形状（只看 res.status）喂给 HW1 判据，必须报违例；
  HW6 活体：缺存档的过审种，下载口必须 400 + code=1002 + 中文判据在信封里；
  HW7 活体对照：有存档的过审种必须 200 + application/x-bittorrent
      （否则 HW6 的 400 可能只是这条路由整个挂了）。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hw_download_error_copy.py`
      HW_LIVE=0 跳过 HW6/HW7 两条只读取证（不写库、不扣费：两枚种都取
      min(id) 且缺存档那只根本不进扣费段）。
"""
import io
import os
import re
import subprocess
import sys
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402

LIVE = os.environ.get("HW_LIVE", "1") != "0"
AC = "apps/web/lib/api-client.ts"
BE = "apps/web/lib/blob-error.ts"
# HW4 已知未统一台账：字幕四出口仍直抛 err.message
RAW_LEDGER = {
    "subtitle-board-table.tsx",
    "subtitle-card-list.tsx",
    "torrent-subtitle-cards.tsx",
    "torrent-subtitles.tsx",
}

# 旧形状（本批修之前 requestBlob 的真码）：只按 HTTP 状态码造错
OLD_SHAPE = """
async function requestBlob(path: string): Promise<ArrayBuffer> {
  const res = await fetch(path);
  if (!res.ok) {
    throw new ApiError(
      1000,
      netMsg(lang, "httpDownloadFail").replace("{status}", String(res.status)),
    );
  }
  return res.arrayBuffer();
}
"""


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip(ts):
    """剥 // 行注释与 /* */ 块注释：判据只认真码，别被自己的注释洗白。"""
    ts = re.sub(r"/\*.*?\*/", "", ts, flags=re.S)
    return "\n".join(ln for ln in ts.split("\n")
                     if not ln.strip().startswith("//"))


def body_of(src, sig):
    """取函数体（按花括号配对，跳过字符串里的括号——本仓 TS 够用了）。"""
    i = src.index(sig)
    j = src.index("{", i)
    d, k = 0, j
    while k < len(src):
        if src[k] == "{":
            d += 1
        elif src[k] == "}":
            d -= 1
            if d == 0:
                return src[j:k + 1]
        k += 1
    return src[j:]


def blob_outlets_missing_helper():
    """返回「调了 getBlob、自己报错、却没走 apiErrorMessage」的文件名清单。"""
    out = []
    for root, dirs, files in os.walk(os.path.join(ROOT, "apps/web")):
        dirs[:] = [d for d in dirs if d not in ("node_modules", ".next")]
        for fn in files:
            if not fn.endswith((".ts", ".tsx")):
                continue
            t = strip(rd(os.path.join(root, fn).replace(ROOT + os.sep, "")))
            # 出口有两种形态：直接 getBlob，或经 downloadSubtitle 这类
            # 只搬运不报错的包装函数——后者一样把文案送到界面，都要算出口
            if "getBlob(" not in t and "downloadSubtitle(" not in t:
                continue
            # 只判「有 catch 且把文案交给界面」的出口
            if not re.search(r"toast\.error|onMsg|setMsg", t):
                continue
            if "apiErrorMessage(" in t:
                continue
            out.append(fn)
    return sorted(set(out))


ac = strip(rd(AC))
be = strip(rd(BE))
blob = body_of(ac, "async function requestBlob")
json_body = body_of(ac, "async function request")

hw1_bad = []
if "toApiError(res, netMsg(lang, \"httpDownloadFail\"))" not in blob:
    hw1_bad.append("blob 失败分支没走 toApiError")
if re.search(r"new ApiError\(\s*1000,", blob):
    hw1_bad.append("blob 失败分支仍在裸造 ApiError(1000,…)")
ok("HW1 blob 口失败必须走共用归一化", hw1_bad == [], "违例=%s" % (hw1_bad,))

hw2_bad = [w for w, hit in (
    ("res.json()", "res.json()" in be),
    ("env.code", "env.code" in be),
    ("request_id", "request_id" in be),
    ("非 JSON 才兜底", 'ctype.includes("application/json")' in be),
    ("状态码回填 {status}", 'replace("{status}"' in be),
) if not hit]
ok("HW2 归一化真读信封且只在非 JSON 时兜底", hw2_bad == [],
   "缺项=%s" % (hw2_bad,))

ok("HW3 两口同尺（request 与 requestBlob 都调 toApiError）",
   "toApiError(res, netMsg(lang, \"httpServerError\"))" in json_body
   and "toApiError(res," in blob,
   "只修一处等于没修：JSON 口与 blob 口必须共用这一把尺")

ledger_now = set(blob_outlets_missing_helper())
ok("HW4 报错文案台账＝已知未统一的字幕四出口（只准销账不准新增）",
   ledger_now == RAW_LEDGER,
   "在册=%s 台账=%s" % (sorted(ledger_now), sorted(RAW_LEDGER)))

old_bad = []
old_blob = body_of(strip(OLD_SHAPE), "async function requestBlob")
if "toApiError(res," not in old_blob:
    old_bad.append("没走 toApiError")
if re.search(r"new ApiError\(\s*1000,", old_blob):
    old_bad.append("裸造 ApiError(1000,…)")
ok("HW5 承重：旧形状被 HW1 判据抓住", len(old_bad) == 2,
   "旧违例=%s" % (old_bad,))

if LIVE:
    def psql(q):
        r = subprocess.run(
            ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
             "-d", "fluxtorrent", "-q", "-tA"], input=q,
            capture_output=True, text=True, encoding="utf-8", errors="replace")
        err = (r.stderr or "")
        if r.returncode != 0 or "ERROR" in err:
            raise RuntimeError("psql: " + err[:300])
        return (r.stdout or "").strip()

    # 注意：NOT EXISTS 里必须 SELECT 1 —— 聚合当 EXISTS 用（P-428 同族）时，
    # SELECT count(*) 对空表也回一行，判据恒假，min(id) 直接取空。
    NO_RAW = "SELECT 1 FROM torrent_files f WHERE f.torrent_id = t.id"
    no_raw_id = psql("SELECT min(t.id) FROM torrents t WHERE t.approval_"
                     "status = 1 AND NOT EXISTS (%s)" % NO_RAW)
    raw_id = psql("SELECT min(t.id) FROM torrents t WHERE t.approval_"
                  "status = 1 AND EXISTS (%s)" % NO_RAW)
    tok = login("root", "password123")
    st, body = call("GET", "/torrents/%s/download" % no_raw_id, None, tok,
                    raw=True)
    txt = body if isinstance(body, str) else (body or b"").decode(
        "utf-8", "replace")
    m = re.search(r'"message":"([^"]*)"', txt)
    msg = m and m.group(1) or ""
    ok("HW6 缺存档种(id=%s)下载口回的是可行动的中文判据" % no_raw_id,
       st == 400 and '"code":1002' in txt and "管理组" in msg,
       "http=%s message=%r；后端写清楚而前端读不到＝本闸门要钉的那条链"
       % (st, msg[:60]))
    # 对照组要 content-type，call() 不回头部，这里自己打一次
    req = urllib.request.Request(
        "%s/torrents/%s/download" % (BASE, raw_id),
        headers={"Cookie": "flux_token=%s" % tok})
    with urllib.request.urlopen(req) as r:
        ctype = r.headers.get("content-type") or ""
        nbytes, code = len(r.read()), r.status
    ok("HW7 对照：有存档种(id=%s)必须 200 且是 bencode 流" % raw_id,
       code == 200 and "bittorrent" in ctype and nbytes > 80,
       "http=%s ctype=%s 字节=%s（否则 HW6 的 400 可能只是整条路由挂了）"
       % (code, ctype, nbytes))
else:
    print("[HW6/HW7] HW_LIVE=0 跳过活体两条")

summary()
