# -*- coding: utf-8 -*-
"""LY：设定「代码兜底默认」与「迁移播种默认」不许有两本账（P-447）。

形状：读方写 `COALESCE((SELECT value FROM site_settings WHERE name='k'), N)`，
迁移里同一把键 INSERT 出厂值 S。N != S ⇒ 这把键**一旦缺行**，站长拿到的是
没人文档过的值。缺行不是假想：P-442 实测已应用的迁移被改过、补种对存量站
永不执行；P-444 实测闸门把现网目录占满。

现网唯一一条真分叉（2026-10-27 实测）：
  upload_deny_approval_deny_count  代码兜底=2 (publish_http/upload.rs:172)
                                   播种=3 (0039_settings_meta.sql:68)，现网值=3
  ⇒ 键缺行时「累计被拒 N 次停发」提前一轮收紧（3→2），会员因违规被停上传，
    而设置页仍写着出厂 3。今天现网有行 ⇒ 不是线上故障，是潜伏分叉。
    修法是 upload.rs 那两处字面量 2→3；该文件在站长的在途 diff 里（+9/−2，
    且没碰这几行），**不由我代改**——代改会把这一改混进别人那次提交的 diff。

正控两条（不过就别信下面的数）：
  LY1 假阴须被抓到：resurrection_hours 兜底 240 且现网无行 ⇒ 必须在读数里
  LY2 假阳须消失：`4 * 1024 * 1024` 必须求成 4194304（第一版只截到 4，
      把 upload_torrent_max_bytes 误报成分叉；第三版还把 `)` 吃进表达式）
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

RUST_DIRS = ["apps/api/src", "apps/worker/src", "apps/tracker/src"]
PATS = [
    re.compile(r"WHERE\s+name\s*=\s*'([a-z0-9_\.]+)'\s*\)\s*,\s*([\d\s\*\+]+)"),
    re.compile(r'setting_(?:int|str)\(\s*\w+\s*,\s*"([a-z0-9_\.]+)"\s*,\s*'
               r'([\d\s\*\+]+)'),
]
# 已判定的合法例外：键 -> 不修的理由（多一个少一个都红）
LEDGER_DRIFT = {
    "maxusers": "0 = 不限是有意的哨兵值（repo/auth.rs:48 `if max_users > 0`），"
                "播种 50000 是站长策略不是默认",
    "upload_deny_approval_deny_count": "代码 2 vs 播种 3，现网有行故今日无影响；"
                "改点在站长在途文件 publish_http/upload.rs:172，等站长自己收",
}


def unwrap(text):
    """Rust 字符串续行：`\\` + CRLF + 缩进 → 空格（只归一 \\n 会漏，实测假阴）。"""
    return re.sub(r"\\\r?\n\s*", " ", text)


def numeric(expr):
    """只求值「积的和」，不 eval。多余字符一律 None。"""
    e = expr.strip().rstrip(",; ).")
    if not e or any(c not in "0123456789*+ " for c in e):
        return None
    total = 0
    for part in e.split("+"):
        prod = 1
        for n in part.strip().split("*"):
            if not n.strip().isdigit():
                return None
            prod *= int(n.strip())
        total += prod
    return total


code = {}
for d in RUST_DIRS:
    if not os.path.isdir(d):
        continue
    for dirpath, _dn, fns in os.walk(d):
        for fn in fns:
            if not fn.endswith(".rs"):
                continue
            txt = unwrap(open(os.path.join(dirpath, fn), encoding="utf-8",
                              errors="replace").read())
            for pat in PATS:
                for k, v in pat.findall(txt):
                    n = numeric(v)
                    if n is not None:
                        code.setdefault(k, (n, fn))

seeded = {}
MIG = "apps/api/migrations"
for f in sorted(os.listdir(MIG)):
    if not f.endswith(".sql"):
        continue
    txt = unwrap(open(os.path.join(MIG, f), encoding="utf-8",
                      errors="replace").read())
    for m in re.finditer(
            r"INSERT INTO site_settings[^;]*?VALUES(.*?)(?:ON CONFLICT[^;]*)?;",
            txt, re.S):
        for k, v in re.findall(r"\(\s*'([a-z0-9_\.]+)'\s*,\s*'([^']*)'",
                               m.group(1)):
            seeded.setdefault(k, (v, f))

live = {ln.strip() for ln in psql_stdin(
    "SELECT name FROM site_settings;").splitlines() if ln.strip()}

ok("LY1 正控（假阴须抓到）：resurrection_hours 兜底 240 且现网无行",
   code.get("resurrection_hours", (None,))[0] == 240
   and "resurrection_hours" not in live,
   "读到兜底=%s 库里有行=%s" % (code.get("resurrection_hours", (None,))[0],
                              "resurrection_hours" in live))
ok("LY2 正控（假阳须消失）：4*1024*1024 求成 4194304",
   code.get("upload_torrent_max_bytes", (None,))[0] == 4194304,
   "实得=%s" % (code.get("upload_torrent_max_bytes", (None,))[0],))

drift = {}
for k, (n, fn) in sorted(code.items()):
    if k in seeded:
        s = seeded[k][0].strip()
        if re.fullmatch(r"-?\d+", s) and int(s) != n:
            drift[k] = "代码=%s(%s) 播种=%s(%s)" % (n, fn, s, seeded[k][1])

ok("LY3 「两本账」集合=台账 %s（多一条=新分叉，少一条=改了要同步删号）"
   % sorted(LEDGER_DRIFT),
   set(drift) == set(LEDGER_DRIFT),
   "实测=%s" % sorted(drift))
for k, why in sorted(LEDGER_DRIFT.items()):
    print("   [台账理由] %s：%s" % (k, why))

ok("LY4 反空转：可比的代码兜底键须 > 20（解析坏会把集合读成空集而假绿）",
   len(code) > 20, "代码兜底键=%d 播种键=%d 现网键=%d"
   % (len(code), len(seeded), len(live)))

val = psql_stdin("SELECT COALESCE((SELECT value FROM site_settings WHERE "
                 "name='upload_deny_approval_deny_count'), '(缺行)')"
                 ).splitlines()[0].strip()
ok("LY5 现网实况：该键当前有行且=3 ⇒ 分叉是潜伏的，今天没咬人",
   val == "3", "现网值=%s" % val)

summary()
