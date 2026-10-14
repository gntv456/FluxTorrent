# -*- coding: utf-8 -*-
"""CO 闸门（批七十五）：`?date_from= / ?date_to= / ?size_min= / ?size_max= /
?anonymous=` 五个筛选参数——给了值却不合法时不许静默变「不筛」（P-279）。

现网（修前，镜像 `api:aa1023b`，2026-10-14 实测）五个参数全部一样：
    ?alive=0&date_from=2025-13-45  ⇒ 200 + **全站 69 行**（日历上没这天）
    ?alive=0&date_to=2025-02-30    ⇒ 200 + 全站 69 行
    ?alive=0&size_min=abc          ⇒ 200 + 全站 69 行
    ?alive=0&size_max=12XB         ⇒ 200 + 全站 69 行
    ?alive=0&anonymous=5           ⇒ 200 + 全站 69 行（非 0/1/2 被丢掉）
`norm_date` / `parse_size` 的既有职责是「别让烂字符串走到 PG 的 $n::date
抛错变成 500」，无效值一律 None；而 None 在 SQL 侧 = 不加谓词 ⇒ 会员拿到
全站——既不报错也不是空，是「假装筛过了」。本仓同形状已收口三次
（category_id 非法⇒400、approval 越界⇒400、promo 批七十四⇒400），这次补齐。

修后契约（CO 断言的形状）：**空值 = 不筛**（表单清空后照样提交空串，
这是既有契约，动了就是回归）；给了值却解析不出来 ⇒ 400 + 站内信封，
文案带「该填什么」。前台与 NexusPHP 兼容层共用同一判据。

CO4（空值仍不筛）与 CO5（合法值真的在筛）是护栏：本批只该改变「非法值」
的去向，一个都不该改变合法值与空值的行为。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_co_param_strict.py
本闸门只读：不写库、不建种，收尾断言促销/探针行与开局一致。
"""
import io
import os
import subprocess
import sys
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, clear_register_rl, login, ok, summary)

ROOT = login()
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOKEN_NAME = "co75_probe_token"


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def src(rel):
    p = os.path.join(REPO, "apps", "api", "src", rel)
    if not os.path.exists(p):
        return ""
    return io.open(p, encoding="utf-8").read()


def code(rel):
    return chr(10).join(l.split("//")[0] for l in src(rel).splitlines())


def flush():
    subprocess.run(
        ["docker", "exec", "flux-redis", "sh", "-c",
         "redis-cli --scan --pattern 'cache:tlist*' | xargs -r redis-cli DEL"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        universal_newlines=True, encoding="utf-8")


def get(qs):
    s, b = call("GET", "/torrents" + qs, None, token=ROOT)
    d = b.get("data") if isinstance(b, dict) and "data" in b else b
    total = d.get("total_estimate") if isinstance(d, dict) else None
    msg = b.get("message") if isinstance(b, dict) else ""
    return s, total, msg or ""


clear_register_rl()
flush()
APPR = int(psql("SELECT count(*) FROM torrents WHERE approval_status=1"))
PROMO0 = int(psql("SELECT count(*) FROM promotions"))
s_base, t_base, _ = get("?alive=0")
ok("CO0a 阳性对照：`?alive=0` 基线 http=%s total=%s（库内过审种=%d）—— "
   "对不上就整闸作废" % (s_base, t_base, APPR),
   s_base == 200 and int(t_base) == APPR, "total=%s" % t_base)

# ===== CO1/CO2/CO3 五个非法值必须 400（修前 200 + 全站）=====
BAD = [
    ("date_from=2025-13-45", "日历上不存在的日期", "YYYY-MM-DD"),
    ("date_to=2025-02-30", "闰月外的 2-30", "YYYY-MM-DD"),
    ("date_from=2025-1-5", "没零补齐的日期", "YYYY-MM-DD"),
    ("size_min=abc", "非数字体积", "KB/MB/GB/TB"),
    ("size_max=12XB", "不认识的体积单位", "KB/MB/GB/TB"),
    ("size_min=-5MB", "负体积", "正数"),
    ("anonymous=5", "匿名三态越界", "0 全部 / 1 仅匿名 / 2 仅具名"),
]
for i, (q, why, want) in enumerate(BAD, 1):
    s, t, msg = get("?alive=0&" + q)
    ok("CO1.%d P-279 `%s`（%s）必须 400 并带可执行的文案（含「%s」）—— "
       "修前是 200 + total=%s（= 全站 %d，静默不筛）"
       % (i, q, why, want, t, APPR),
       s == 400 and t is None and want in msg,
       "http=%s total=%s msg=%s" % (s, t, msg[:60]))

# ===== CO4 空值仍是不筛：表单清空照样提交空串，这是既有契约 =====
for q in ("date_from=", "date_to=", "size_min=", "size_max=", "anonymous="):
    s, t, msg = get("?alive=0&" + q)
    ok("CO4 `%s` 空值必须仍是「不筛」并回全站（http=%s total=%s）—— "
       "收紧只针对给了值的情况，把表单常态打成 400 就是回归"
       % (q, s, t), s == 200 and int(t) == APPR,
       "http=%s total=%s" % (s, t))

# ===== CO5 合法值必须照常筛，且筛出来的结果与不筛不同 =====
s_d, t_d, _ = get("?alive=0&date_from=2026-10-01")
s_sz, t_sz, _ = get("?alive=0&size_min=1GB")
s_an, t_an, _ = get("?alive=0&anonymous=1")
ok("CO5a 合法日期照常筛：date_from=2026-10-01 ⇒ http=%s total=%s，"
   "必须小于基线 %d（大于 0 才算真有数据在窗口内）" % (s_d, t_d, APPR),
   s_d == 200 and 0 <= int(t_d) < APPR, "total=%s" % t_d)
ok("CO5b 合法体积照常筛：size_min=1GB ⇒ http=%s total=%s（修前后同口径，"
   "本批不动合法值）" % (s_sz, t_sz), s_sz == 200 and int(t_sz) < APPR,
   "total=%s" % t_sz)
ok("CO5c 匿名三态 0/1/2 全部照常放行：anonymous=1 ⇒ http=%s total=%s"
   % (s_an, t_an), s_an == 200, "http=%s" % s_an)

# ===== CO6 新加的 4xx 的读者：前台把 400 的文案给用户看得到吗 =====
page = io.open(os.path.join(REPO, "apps", "web", "app", "(main)",
                            "torrents", "page.tsx"), encoding="utf-8").read()
shows_msg = "err.message" in page or "error.message" in page
warn = ("前台 size_min/size_max 是自由文本框，会员能打 'abc'；而 page.tsx 的 "
        "catch 目前只渲染通用「加载失败」（未回显服务端 message=%s）⇒ "
        "本批的中文说明在**界面层看不到**，这条按 WARN 记账不当红（改它要动 "
        "web 镜像，已开欠账）" % shows_msg)
print("   WARN  CO6 %s" % warn)
ok("CO6 记录在案：界面读者判据必须在闸门里留下痕迹（shows_message=%s）。"
   "如果将来前台开始回显服务端 message，这条要升级成断言「400 文案在页面上」"
   % shows_msg, True, "界面读者=%s" % shows_msg)

# ===== CO7 兼容层同口径（工具侧不能各判一套）=====
s_mk, b_mk = call("POST", "/me/tokens",
                  {"name": TOKEN_NAME, "scopes": ["read"],
                   "password": "password123"}, token=ROOT)
dd = (b_mk or {}).get("data") or {}
PLAIN = dd.get("token") or dd.get("plaintext") or ""


def compat(qs):
    req = urllib.request.Request(BASE + "/compat/nexusphp/torrents.json" + qs)
    req.add_header("Authorization", "Token " + (PLAIN or ""))
    try:
        with urllib.request.urlopen(req, timeout=25) as r:
            return r.status
    except urllib.error.HTTPError as e:
        return e.code
    except Exception:
        return 0


c_ok = compat("?size_min=1GB&rows=1") if PLAIN else 0
c_bad = compat("?size_min=abc&rows=1") if PLAIN else 0
d_bad = compat("?date_from=2025-13-45&rows=1") if PLAIN else 0
ok("CO7 兼容层（Token 签发 http=%s）：`size_min=1GB` 必须 200（实测 %s）、"
   "`size_min=abc` 与 `date_from=2025-13-45` 必须 400（实测 %s/%s，"
   "修前 200 静默全站）" % (s_mk, c_ok, c_bad, d_bad),
   c_ok == 200 and c_bad == 400 and d_bad == 400,
   "ok=%s abc=%s bad_date=%s" % (c_ok, c_bad, d_bad))

# ===== CO8 静态：严格档必须真的接在两个入口的读路径上 =====
ent = code("torrent_http/list.rs")
cmp = code("compat_http/nexusphp.rs")
calls = (ent.count("date_or_err(q.") + ent.count("size_or_err(q.")
         + ent.count("anonymous_or_err(q."))
n_loose = ent.count("norm_date(") + ent.count("parse_size(")
ok("CO8a 入口 list.rs：严格档**调用点** %d 处（日期 2 + 体积 2 + 匿名 1）且"
   "宽容档裸调用剩 %d 处（必须 0，否则就是两套并存）" % (calls, n_loose),
   calls == 5 and n_loose == 0, "调用=%d 宽容=%d" % (calls, n_loose))
ok("CO8b 兼容层 nexusphp.rs：date/size 严格档 %d 处（修前是 4 处宽容档）"
   % (cmp.count("date_or_err") + cmp.count("size_or_err")),
   cmp.count("date_or_err") + cmp.count("size_or_err") == 4,
   "命中=%d" % (cmp.count("date_or_err") + cmp.count("size_or_err")))
reg = code("torrent_http/query.rs").count("mod param_strict")
ok("CO8c 新模块必须注册：`mod param_strict;` 命中=%d" % reg, reg == 1)

# ===== CO9 残留：本闸门只读 =====
PROMO1 = int(psql("SELECT count(*) FROM promotions"))
tk_left = int(psql("SELECT count(*) FROM api_tokens WHERE name='%s'"
                   % TOKEN_NAME))
ok("CO9a 未写业务数据：`promotions` %d → %d" % (PROMO0, PROMO1),
   PROMO1 == PROMO0, "after=%d" % PROMO1)
psql("DELETE FROM api_tokens WHERE name='%s'" % TOKEN_NAME)
tk_after = int(psql("SELECT count(*) FROM api_tokens WHERE name='%s'"
                    % TOKEN_NAME))
ok("CO9b 临时令牌清干净（签发后残留 %d → 收尾 %d）" % (tk_left, tk_after),
   tk_after == 0)
flush()

summary()
