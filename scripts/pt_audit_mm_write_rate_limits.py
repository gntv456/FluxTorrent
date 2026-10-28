# -*- coding: utf-8 -*-
"""闸门 MM：会员面写口的频率闸在册情况（批一百八十九，2026-10-28）。

为什么要这条：本仓限流**没有中央路由表**（`http/throttle.rs` 的
`throttle`/`throttle_limited`/`rate_window` 都要在 handler 里显式调），所以
「某个写口有没有闸」只能逐口看——而且必须**跟着跳一层**（调用点写成
`foo(&state, ..)`、闸在 `fn foo` 里也是合规的），否则会把合规口判成违例
（同族坑：守卫常在鉴权层/共享取数层）。

实测焦点缺口（逐条读码定性，不是候选数当缺陷数）：
  · `POST /torrents/{id}/comments`：只有「非空 + COMMENT_MAX_CHARS + 禁言」三关，
    **没有任何频率闸**，而每条评论都会给种主发一条 `torrent_comment` 通知
    （`torrent_http/notify.rs::notify_comment`，无去重无冷却）⇒ 谁都能对某个
    种主无限发通知（骚扰 + 收件箱灌满 + 详情页聚合缓存被反复打掉）。
    载体文件 `torrent_http/interact.rs` 是站方在途件 ⇒ 只报不改（改动会混进别人的
    commit 失去出处）。补丁就两行，照 `reports_http.rs:47` 的先例：
        crate::http::throttle_limited(&state,
            format!("cmt:{}", auth.id), 12).await?;
    数字（12 次/分）是建议值，取的是「正常讨论打不到、脚本刷屏立刻 429」那一档，
    由站长定。
  · `POST /requests` 的 `bounty=0` 分支：标题/简介有长度上限、有幂等键，但
    **建单条数没有日上限** ⇒ 免费求种榜可被灌满。`bounty>0` 那支靠扣款天然限速。

判据：
  MM1 正控：`/auth/login` 与 `/reports` 必须被判为「有闸」（尺子认得闸的形状）
  MM2 焦点点名：这几条高滥用口的状态必须与台账一字不差（改好了要翻号）
  MM3 棘轮：会员面无闸写口总数只减不增（新加一个口忘带闸＝当场红）
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "apps", "api", "src")
TOK = re.compile(r"\b(throttle_limited|throttle|check_rate_scoped|"
                 r"check_rate|rate_window|RateScope)\b")
ATTR = re.compile(r'#\[(post|put|delete|patch)\("([^"]+)"')
FN = re.compile(r"(?:pub(?:\([a-z_]+\))?\s+)?(?:async\s+)?fn\s+"
                r"([A-Za-z_][a-z0-9_]*)\s*\(")
STAFF = ("/admin/", "/staff/", "/v4/")

# 焦点台账：路由 → 期望状态（L=有闸，O=确实无闸且已读码定性为缺口/可接受）
FOCUS = {
    "POST /torrents/{id}/comments": ("L", "throttle_limited(cmt:uid,12)"),
    "POST /requests": ("O", "bounty=0 分支无建单条数上限"),
    "POST /subtitles/requests": ("O", "公开求做单，条数无上限"),
    "POST /auth/login": ("L", "throttle 登录档"),
    "POST /reports": ("L", "reports_http throttle_limited(rep:uid,8)"),
}
# 棘轮基线：会员面无闸写口条数（2026-10-28 判据自己那次实跑=166）。
# 评论口补闸后应减一（165）；只减不增，新增无闸写口才红。
MM_BASELINE_MAX = 165


def strip_comments(txt):
    txt = re.sub(r"/\*.*?\*/", " ", txt, flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in txt.split("\n"))


def body_at(txt, i):
    j = txt.find("{", i)
    if j < 0:
        return ""
    depth, k = 1, j + 1
    while k < len(txt) and depth:
        depth += (txt[k] == "{") - (txt[k] == "}")
        k += 1
    return txt[j:k]


files = {}
for dp, _d, ns in os.walk(SRC):
    for n in ns:
        if n.endswith(".rs"):
            p = os.path.join(dp, n)
            files[p] = strip_comments(
                open(p, encoding="utf-8", errors="replace").read())
assert len(files) > 200, "扫到的 .rs 太少＝ROOT 错（%d）" % len(files)

bodies = {}
for t in files.values():
    for m in FN.finditer(t):
        b = body_at(t, m.end())
        if len(b) > len(bodies.get(m.group(1), "")):
            bodies[m.group(1)] = b


def limited(path_body):
    if TOK.search(path_body):
        return True
    for name in set(re.findall(r"\b([a-z_][a-z0-9_]{3,})\s*\(", path_body)):
        if name in bodies and TOK.search(bodies[name]):
            return True
    return False


routes = {}
for t in files.values():
    for m in ATTR.finditer(t):
        fm = FN.search(t, m.end())
        if not fm:
            continue
        path = "/%s" % m.group(2).lstrip("/")
        key = "%s %s" % (m.group(1).upper(), path)
        if key not in routes:
            routes[key] = limited(body_at(t, fm.end()))
print("[读数] .rs=%d 解析 fn=%d 写路由=%d 条"
      % (len(files), len(bodies), len(routes)))

ctrl = [("POST /auth/login", routes.get("POST /auth/login")),
        ("POST /reports", routes.get("POST /reports"))]
ok("MM1 正控：两处已知有闸的口必须被判为有闸（尺子认得限流形状）",
   len(routes) >= 200 and all(v is True for _, v in ctrl),
   "实测=%s 路由总数=%d" % (ctrl, len(routes)))

drift = []
for path, (want, why) in sorted(FOCUS.items()):
    got = routes.get(path)
    if got is None:
        drift.append("%s 找不到（改名/搬走了？）" % path)
    elif (want == "L") != got:
        drift.append("%s 期望 %s 实得 %s（%s）"
                     % (path, want, "L" if got else "O", why))
ok("MM2 焦点点名：高滥用口的状态与台账一致（修好了要翻号，别让它飘）",
   not drift, "; ".join(drift) or "-")

open_member = sorted(k for k, v in routes.items()
                     if not v and not any(k.endswith(p) or p in k
                                          for p in STAFF))
ok("MM3 棘轮：会员面无闸写口条数不得上升（新口忘带闸＝当场红）",
   len(open_member) <= MM_BASELINE_MAX,
   "实测=%d 基线上限=%d（降到 %d 以下请把常量调低并销号）"
   % (len(open_member), MM_BASELINE_MAX, len(open_member)))
print("[读数] 会员面无闸写口共 %d 条，前 30：%s"
      % (len(open_member), open_member[:30]))

summary()
