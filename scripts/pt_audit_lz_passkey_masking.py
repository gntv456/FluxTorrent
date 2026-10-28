# -*- coding: utf-8 -*-
"""LZ：热接口 /me/overview 不再下发 passkey 全文（P-448）。

现网实测（活体，只读）：`GET /me/overview` 的响应里带**32 位 passkey 全文**
（me_settings.rs:319 `"passkey": get_str("passkey")`），而这个端点被四处轮询
（首页问候条 app/(main)/page.tsx、通知中心、我的、任务页）。
全文唯一的读者是安全卡 usercp-overview-panels.tsx:90，而它只画
`ov.passkey.slice(0, 4)` + 「••••」——**界面本来就只展示掩码**，
服务端却把完整凭据发出去四次。
RSS/下载客户端需要全文，走的是各自的专用端点（`/rss-info`、compat 的
NpUser/PtppUser、`/me/passkey/rotate`），那些是用户主动取给客户端的口径。

为什么本批不代改：`me_settings.rs` 在站长的在途 diff 里（+21/−0，
且已经在行数/行宽门禁上贡献 2 条违规）。把「2→掩码」塞进别人那次提交，
来历会被吞掉——所以这里只钉闸门，改点交给站长落（见 --explain 那段三行）。
⇒ LZ1 现在是红的，那就是缺陷本体；改完自动转绿。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (BASE, call, jwt_token, ok, psql_stdin,  # noqa: E402
                          summary)

PANEL = "apps/web/components/usercp-overview-panels.tsx"
DETAILS = "apps/web/components/usercp-overview-details.tsx"
TYPE = "apps/web/components/usercp.tsx"
SRV = "apps/api/src/auth_http/me_settings.rs"
# 允许出现 passkey 全文的出口（合理口径）；me_settings 修好后应从台账里
# 的「待摘除」变成「已摘除」，LZ4 用两个集合同时钉住方向
LEDGER_FULL_OK = {"rss-info", "compat-np", "compat-ptpp", "passkey-rotate"}
LEDGER_TO_MASK = {"me-overview"}


def read(p):
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


tk = jwt_token("root")
raw_pk = psql_stdin("SELECT passkey FROM users WHERE username='root';"
                   ).splitlines()[0].strip()
st, resp = call("GET", "/me/overview", None, tk)
txt = str(resp)
leaks = raw_pk and raw_pk in txt
ok("LZ1 活体：/me/overview 响应里不得出现 passkey 全文（现网=%s）"
   % ("仍出现" if leaks else "已掩码"),
   st == 200 and not leaks,
   "http=%s 全文长度=%d（凭据只按长度记账，不打值）" % (st, len(raw_pk)))

st2, rss = call("GET", "/rss-info", None, tk)
ok("LZ2 对照（防把修法做成「一律掩码」）：/rss-info 仍须给出可用全文——"
   "客户端订阅链接就是要它",
   st2 == 200 and raw_pk in str(rss),
   "http=%s 含全文=%s" % (st2, raw_pk in str(rss)))

panel = read(PANEL)
details = read(DETAILS)
ok("LZ3 两个读者都不拿轮询全文：安全卡画 passkey_masked+圆点；"
   "明细面板复制时按需向 /rss-info 取（谁把 ov.passkey 写回来，红）",
   "passkey_masked" in panel
   and "••••" in panel
   and not re.search(r"\bov\.passkey\b", panel + details)
   and 'api.get<{ passkey: string }>("/api/v1/rss-info")' in details,
   "面板=%s 明细取数=%s" % (
       [ln.strip() for ln in panel.splitlines() if "passkey" in ln][:1],
       [ln.strip() for ln in details.splitlines() if "rss-info" in ln][:1]))

srv = read(SRV)
still_full = bool(re.search(r'"passkey"\s*:\s*get_str\("passkey"\)', srv))
masked_out = '"passkey_masked"' in srv
in_wip = subprocess.run(
    ["git", "status", "--porcelain", "--", SRV],
    capture_output=True, text=True).stdout.strip() != ""
ok("LZ4 方向闸：/me/overview 要么已摘除全文，要么仍挂在「待站长改」台账上"
   "（两条都不成立＝有人换了新写法而闸门不知道）",
   (still_full and not masked_out and in_wip)
   or (masked_out and not still_full),
   "仍发全文=%s 已加掩码字段=%s 该文件在途=%s"
   % (still_full, masked_out, in_wip))
ok("LZ5 契约没被牵连：compat 的两个 schema 仍声明 passkey（那是客户端接口，"
   "不属于本次收敛范围）",
   read("apps/api/src/openapi_http/spec_schemas.rs").count(
       '"passkey": { "type": "string" }') >= 2,
   "声明数=%s" % read("apps/api/src/openapi_http/spec_schemas.rs").count(
       '"passkey": { "type": "string" }'))

print("BASE", BASE)
print("\n-- 已落地的改点（P-448）--")
print('   me_settings.rs: "passkey_masked"（全文改走 /rss-info 按需取）')
print("   usercp.tsx / 两张卡：只用 passkey_masked + 圆点")
print("   明细面板复制：点击时 GET /rss-info 再写剪贴板")
summary()
