# -*- coding: utf-8 -*-
"""EM 常驻闸门：读列表失败不许被吞成「没有记录」（P-314 一族的防回退 + 防再长）

来历（2026-10-14~15 批一百一十二~一百一十五实测，共修 15 只会员面板）：
  形状是 `try { setRows(await api.get(...)) } catch { setRows([]) }`，渲染侧写
  `{rows.length === 0 && <空态文案>}` ⇒ 接口一挂，界面就宣称「你没有好友 /
     「你没有好友 / 还没存过钱 / 一个 passkey 都没有」。
     凭证面（passkeys / api-tokens）后果最重：用户会据此去重注册凭证。
  因为用户会据此去重注册凭证或以为被清。
  仓里的正确先例是 `request-board.tsx`：`setRows(null); setFailed(true);`
  再渲染 `dict.common.loadFailed`——本闸门把它钉住，别让先例被改回去。

判据口径（两条自伤都写在这里，避免下一个人重踩）：
  1. 必须**跨行**匹配。按单行 `catch { setX([]) }` 匹配时现网报 0 命中，
     而真实代码是多行块——「命中 0」是判据坏，不是仓里干净。
  2. 「体内有没有失败信号」要看全：toast / flash / setErr / setMsg /
     setXFailed / throw / console 都算。
     只写 setFailed 会把 setOvFailed、setMsg 这些当成裸吞（首跑两条假红
     就是这么来的：判据的否定分支比肯定分支更需要列举齐全。
现网裸吞清单（PENDING）是要**逐只吃掉**的队列，主要在职员/管理面：那些面板的
空态文案与列表语义要先逐个对着前端核，不一刀切。闸门的作用只是：
  - 已修 18 只不许回退（EM2）；
  - 不许**新长出**裸吞（EM3）；
  - PENDING 里被修掉却不撤桩的，EM5 会点名提醒清账（防清单只增不减）。
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WEB = os.path.join(ROOT, "apps", "web")

# 已修的 18 只（回归钉：这些文件里不许再出现裸吞）
FIXED = [
    "components/bank-actions.tsx", "components/resurrection-panel.tsx",
    "components/funding-panel.tsx", "components/forum-search.tsx",
    "components/snatch-list.tsx", "components/subtitle-board.tsx",
    "components/passkeys-card.tsx", "components/api-tokens.tsx",
    "components/invite-manager.tsx", "components/shout-box.tsx",
    "components/community-extra.tsx", "components/friends-actions.tsx",
    "components/task-board.tsx", "components/fun-box.tsx",
    "components/subtitle-request-panel.tsx",
    "components/wishlist.tsx", "components/contact-staff.tsx",
    "components/torrent-hover-preview.tsx",
    "components/staff-wiki.tsx",
]
# 现网仍存在的裸吞（逐只吃掉后从这里删；新增了不在这里的就必须红）
PENDING = [
    "app/(main)/admin/page.tsx",
    "app/(main)/admin/_parts/admin-freeleech.tsx",
    "app/(main)/admin/_parts/admin-promo-intents.tsx",
    "components/admin-applications.tsx",
    "components/admin-arcade.tsx",
    "components/admin-gacha.tsx",
    "components/admin-p2-tools-claims.tsx",
    "components/admin-p2-tools-menus.tsx",
    "components/admin-p2-tools-promos.tsx",
    "components/admin-p2-tools-templates.tsx",
    "components/admin-subtitles-awards.tsx",
    "components/admin-torrents-list.tsx",
    "components/admin-torrents-records.tsx",
    "components/admin-user-fields-panel.tsx",
    "components/staff-tools-site.tsx",
    "components/staff-tools-sys-agentrules.tsx",
    # staff-wiki 于批一百一十九撤桩（catch 里已置 null + flash 提示）
    # torrent-hover-preview / contact-staff 于批一百一十八撤桩
    # wishlist.tsx 于批一百一十六撤桩（EM5 先提醒，grep 过 {msg && 确认可见才改）
    "app/(auth)/register/page.tsx",
]

BODY = re.compile(r"catch\s*(?:\([^)]*\))?\s*\{([^{}]*)\}", re.S)
# 失败信号的写法不止一种：toast/flash/setMsg/setErr/setXFailed/throw/console
# 都算。漏认 setMsg 会把「已修好」的救援面板与「本来就对」的 embed-rules 一起
# 判成裸吞（本闸门首跑的两条假红就是这个成因）。
SIGNAL = re.compile(
    r"toast|flash|setErr|setMsg|set\w*Failed|set\w*Error|throw|console", re.I)
# 口径：只把 [] 换成 null（不谎报了但仍无提示）**不算**有信号——
# 闸门要的是可诊断。admin-applications.tsx 就因此仍留在 PENDING。
EMPTYSET = re.compile(r"set\w+\(\s*(?:\[\]|null|\{\})\s*\)")


def strip_comments(text):
    out = []
    for line in text.splitlines():
        s = line.strip()
        if s.startswith(("//", "*", "/*")):
            continue
        i = line.find("//")
        if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
            line = line[:i]
        out.append(line)
    return "\n".join(out)


def bare_sites(text):
    """返回 (文件内裸吞数, 样例) ——跨行匹配，体内无失败信号才算裸吞"""
    hits = []
    for m in BODY.finditer(text):
        body = m.group(1)
        if EMPTYSET.search(body) and not SIGNAL.search(body):
            hits.append(" ".join(body.split())[:44])
    return hits


def scan_dir(base):
    found = {}
    for dp, dirs, fs in os.walk(base):
        dirs[:] = [d for d in dirs if d not in (".next", "node_modules", "e2e")]
        for f in fs:
            if not f.endswith(".tsx"):
                continue
            fp = os.path.join(dp, f)
            rel = os.path.relpath(fp, base).replace("\\", "/")
            h = bare_sites(strip_comments(io.open(fp, encoding="utf-8",
                                                 errors="replace").read()))
            if h:
                found[rel] = h
    return found


print("== S0 判据自证（正控）==")
old = subprocess.run(
    ["git", "show", "528c4d52^:apps/web/components/bank-actions.tsx"],
    capture_output=True, text=True, encoding="utf-8", errors="replace")
if old.returncode != 0:
    ok("EM0 取到修复前的 bank-actions（拿不到就不能做正控）", False,
       "rc=%s" % old.returncode)
else:
    before = bare_sites(strip_comments(old.stdout))
    ok("EM0 判据认得缺陷本体：修复前 bank-actions 必须判为裸吞（%d 处）"
       % len(before), len(before) >= 3, str(before[:2]))
now = scan_dir(WEB)
after_ba = now.get("components/bank-actions.tsx", [])
ok("EM1 同一判据跑修复后的 bank-actions ＝ 0 处裸吞（旧 3 处 → 新 0 处）",
   not after_ba, str(after_ba))

print("== S2 已修 15 只不许回退 ==")
regress = {k: v for k, v in now.items() if k in FIXED}
ok("EM2 已修 18 只里 0 处裸吞（回退即红）", not regress,
   "; ".join("%s:%s" % (k, v[0]) for k, v in list(regress.items())[:3]))

print("== S3 现网裸吞只能减不能增 ==")
extra = sorted(set(now) - set(PENDING) - set(FIXED))
ok("EM3 没有新增的裸吞文件（不在 PENDING 里就红）", not extra,
   "新增=%s" % extra)
print("   现存裸吞文件=%d 个，PENDING 登记=%d 个" % (len(now), len(PENDING)))

print("== S4 先例与撤桩 ==")
rb = strip_comments(io.open(
    os.path.join(WEB, "components", "request-board.tsx"),
    encoding="utf-8", errors="replace").read())
m = BODY.search(rb)
pair = [mm.group(1) for mm in BODY.finditer(rb) if EMPTYSET.search(mm.group(1))]
ok("EM4 先例 request-board 仍是「置 null + 置 failed」同体",
   any("null" in b and "Failed" in b for b in pair), str(pair[:1]))
stale = sorted(set(PENDING) - set(now))
if stale:
    print("   提醒撤桩：%d 个 PENDING 已不再裸吞，应从清单删掉：%s"
          % (len(stale), stale))
ok("EM5 PENDING 与现网一致（有陈旧项不算红，只提醒；防清单只增不减）", True,
   "陈旧=%d" % len(stale))
summary()
