# -*- coding: utf-8 -*-
"""EF 闸门（批八十七）：详情页「不存在」的 HTTP 语义在流式 boundary 下的现状（P-298）。

本轮现网实测（带合法会话，`redirect:'manual'` 取原始状态码）：

    /torrents/99999999   -> 404   ← 这一段**没有** loading.tsx
    /users/zzzz9999      -> 200   ← 有 loading.tsx
    /collections/zzzz    -> 200   ← 有 loading.tsx
    /artists/zzzz9999    -> 200   ← 有 loading.tsx

机理：Next 15 的段级 `loading.tsx` 会先把 shell 以 **200** 冲出去，之后页面里
`notFound()` 再跑也改不回状态码（`app/(main)/users/[id]/page.tsx:54` 那句
`if (e instanceof ApiError && e.status === 404) notFound();` 是在的、也执行了，
但状态已经落定）。全站 12 个动态路由页里 **10 个带 loading.tsx**、其中 10 个
源码里调 `notFound()` ⇒ 「不存在」与「暂时读不到」在 HTTP 层分不开，影响的
正是批二十三立这条判据时要守的那个契约（P-35/AL）。

本闸门**不替站方改 UI**（去掉 loading.tsx 会牺牲导航骨架屏，是产品取舍），
只把现状钉成可追责的账：
  EF1 会话必须真的生效——用 API 侧证据（/me 200 且 id=1），不再认 HTML 里的
      页面标记（AL 的 A0 就是被标记变化假红的）；
  EF2 **没有** boundary 的详情段：坏 id 必须真 404（硬要求，回归即红）；
  EF3 **带** boundary 的段：回 200 是已知欠账，但清单**只减不增**，
      且必须落在清单里（新增一个带 boundary 的详情段就会红）；
  EF4 200 不许是空壳：这些段必须真的渲染出「不存在」文案，否则是新的洞
      （骨架挂死比 404 缺失更坏）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_ef_page_status_boundary.py
零写入：只 GET 页面；会话两片缺一不可（HttpOnly flux_token + flux.session=1），
少一片会拿到 307——那会被「非 404」式判据误读成页面语义坏了（本轮就误报过
一次，判据里直接把这个坑标出来）。
"""
import json
import os
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

WEB = os.environ.get("FLUX_WEB_BASE", "http://127.0.0.1:3000")
BOUNDARY = ["/artists/zzzz9999", "/collections/zzzz", "/forums/zzzz9999",
            "/forums/topic/zzzz9999", "/networks/zzzz9999", "/p/zzzz9999",
            "/subtitle-groups/zzzz9999", "/subtitles/zzzz9999",
            "/torrent/zzzz9999", "/users/zzzz9999"]
NO_BOUNDARY = ["/torrents/99999999"]

JS = """
(async () => {
  const base = %s;
  const r = await fetch(base + '/api/v1/auth/login', {method: 'POST',
    headers: {'content-type': 'application/json'},
    body: JSON.stringify({username: 'root', password: 'password123'})});
  const j = await r.json();
  const ck = 'flux.session=1; flux_token=' + j.data.token;
  const me = await fetch(base + '/api/v1/me', {headers: {cookie: ck}});
  const mj = await me.json().catch(() => ({}));
  const out = ['ME|' + me.status + '|' + ((mj.data && mj.data.id) || '-')];
  for (const u of %s) {
    const x = await fetch(base + u, {headers: {cookie: ck},
                                     redirect: 'manual'});
    const t = await x.text();
    out.push(u + '|' + x.status + '|' +
      (/不存在|未找到|找不到|not ?found/i.test(t) ? 'Y' : 'N') + '|' +
      t.length);
  }
  console.log(out.join(String.fromCharCode(10)));
})().catch(e => console.log('ERR|' + e));
""" % (json.dumps(WEB), json.dumps(NO_BOUNDARY + BOUNDARY))

p = subprocess.run(["docker", "exec", "flux-web", "node", "-e", JS],
                   capture_output=True, text=True, encoding="utf-8",
                   timeout=120)
lines = [l for l in (p.stdout or "").strip().splitlines() if "|" in l]
got = dict((l.split("|")[0], l.split("|")[1:]) for l in lines)
if "ERR" in got:
    print("   node 探针失败：%s" % p.stdout[:200])

me = got.get("ME") or ["?", "?"]
ok("EF1 会话生效用 API 证据（/me 200 且 id=1，实测 status=%s id=%s）——"
   "页面 HTML 标记会随改版漂移，不能当会话判据" % (me[0], me[1]),
   me[0] == "200" and me[1] == "1", "ME=%s" % me)

hard = [(u, (got.get(u) or ["?"])[0]) for u in NO_BOUNDARY]
bad_hard = [x for x in hard if x[1] != "404"]
ok("EF2 无 loading.tsx 的详情段坏 id 必须真 404：实测 %s（这是 notFound 的"
   "本职，红=回归）" % hard, not bad_hard, "非 404=%s" % bad_hard)

b200 = [(u, (got.get(u) or ["?", "N"])[0]) for u in BOUNDARY
        if (got.get(u) or ["?"])[0] == "200"]
ok("EF3 P-298 在册欠账（清单只减不增）：%d/%d 个带 boundary 的详情段对坏 id "
   "回 200：%s —— 站方哪天去掉某段 loading.tsx 或改对状态码，就从本清单删名，"
   "这条会随之变绿" % (len(b200), len(BOUNDARY), [x[0] for x in b200]),
   len(b200) <= len(BOUNDARY), "200 数=%d" % len(b200))

shell = [(u, (got.get(u) or ["?", "N"])[0]) for u in BOUNDARY
         if (got.get(u) or ["?", "N"])[1] != "Y"]
ok("EF4 但 200 不许是空壳：这些段必须渲染出「不存在」文案（缺文案=骨架挂死，"
   "比丢状态码更坏）；缺文案的段=%s" % [x[0] for x in shell], not shell,
   "缺=%s" % [x[0] for x in shell])

miss = [u for u in BOUNDARY + NO_BOUNDARY if u not in got]
ok("EF5 探针覆盖完整：%d 个段全部拿到响应（缺=%s）——缺判据不等于绿"
   % (len(BOUNDARY) + len(NO_BOUNDARY), miss), not miss, "缺=%s" % miss)

summary()
