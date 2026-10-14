# -*- coding: utf-8 -*-
"""CX 闸门（批八十 P-287）：保种口的 scope/status 不再静默放行未知值。

红证（现网 `api:aa1025c`，2026-10-14，批七十九的 CW5 读数里顺出来的）：
    GET /preserve?status=not_a_status → http=200，板面照旧（不是 400、也不是空）
    GET /preserve?scope=9            → http=200，同上
⇒ 与种列表 P-279 同一病形：**不认识的值被当成「没筛」**。保种区的筛选是
  会员用来找「官方保种 / 普通保种 / 已移出」这些子集的，打错值看到全量板面，
红证（现网 `api:aa1025c`，10-14，批七十九 CW5 读数里顺出来的）：

判据三层，缺一不可：
  CX1 非法值必须 400 + 1002 + 中文「取值无效」并把可用值列出来；
  会员用来找「官方保种 / 普通保种 / 已移出」这些子集的，打错值看到全量
  CX3 空串仍按「未填」（表单清空后照样提交空串，动了就是回归）；
  CX4 既有边界不打断：`page=2`、`keyword=种子`、`category=<真 id>` 照常 200；
  CX5 静态：`let _ = scope_ok;` 这条「算了又丢」的死码必须已消失，
      且 `norm_choice` 在保种口有两个消费点。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_cx_preserve_params_strict.py
只读：只发 GET。
"""
import io  # noqa: E402
import os  # noqa: E402
import subprocess  # noqa: E402
import sys  # noqa: E402

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ROOT = login()


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def get(qs):
    st, r = call("GET", qs, token=ROOT)
    data = (r or {}).get("data") if isinstance(r, dict) else None
    msg = (r or {}).get("message", "") if isinstance(r, dict) else ""
    code = (r or {}).get("code") if isinstance(r, dict) else None
    n = None
    if isinstance(data, dict):
        items = data.get("items")
        n = len(items) if isinstance(items, list) else None
    return st, code, str(msg), n


def src(rel):
    p = os.path.join(REPO, "apps", "api", "src", rel)
    return io.open(p, encoding="utf-8", errors="replace").read().replace(
        "\r\n", "\n")


def strip_code(s):
    return "\n".join(l.split("//")[0] for l in s.splitlines())


print("== CX0 前置读数：库里各档到底有没有东西 ==")
N_ALL = psql("SELECT count(*) FROM seed_preserve")
N_OFF = psql("SELECT count(*) FROM seed_preserve sp JOIN torrents t "
             "ON t.id=sp.torrent_id WHERE t.official_tag")
print("   seed_preserve 共 %s 行（官种位 %s）" % (N_ALL, N_OFF))

print("== CX1 未知值必须 400 + 中文「该填什么」（修前是 200 静默全量）==")
for qs, want in (("/preserve?status=not_a_status", "保种状态取值无效"),
                 ("/preserve?scope=9", "保种类型取值无效"),
                 ("/preserve?status=active&scope=oops", "保种类型取值无效"),
                 ("/preserve?scope=ALL", "保种类型取值无效")):
    st, code, msg, n = get(qs)
    ok("CX1 `%s` → 400/1002 且含「%s」与可用值 http=%s" % (qs, want, st),
       st == 400 and code == 1002 and want in msg and "可用" in msg,
       "http=%s code=%s msg=%s" % (st, code, msg[:80]))

print("== CX2 正控：合法值各自真的筛（不是全 400 也不是全放行）==")
base_st, _, _, base_n = get("/preserve")
for qs in ("/preserve?scope=official", "/preserve?scope=general",
           "/preserve?status=active", "/preserve?status=grace",
           "/preserve?status=expired", "/preserve?status=current",
           "/preserve?scope=all"):
    st, code, msg, n = get(qs)
    ok("CX2 `%s` 仍 200 且返回条数 %s（无参数时 %s）" % (qs, n, base_n),
       st == 200 and code == 0 and n is not None,
       "http=%s code=%s n=%s msg=%s" % (st, code, n, msg[:60]))

print("== CX3 空串=未填（表单清空后照样提交空串，动了就是回归）==")
for qs in ("/preserve?status=", "/preserve?scope=", "/preserve?scope=&status="):
    st, code, msg, n = get(qs)
    ok("CX3 `%s` 按未填处理 http=%s 条数=%s" % (qs, st, n),
       st == 200 and code == 0, "http=%s code=%s msg=%s" % (st, code, msg[:60]))

print("== CX4 既有边界不打断 ==")
cat = psql("SELECT category_id FROM seed_preserve sp JOIN torrents t "
           "ON t.id=sp.torrent_id WHERE t.category_id IS NOT NULL LIMIT 1")
for qs in ("/preserve?page=2", "/preserve?keyword=%E7%A7%8D%E5%AD%90",
           "/preserve?category=%s" % (cat or "1"),
           "/preserve?status=active&page=1"):
    st, code, msg, n = get(qs)
    ok("CX4 `%s` 照常 200（未知参数值不在白名单外的仍要过）" % qs,
       st == 200, "http=%s code=%s n=%s msg=%s" % (st, code, n, msg[:60]))

print("== CX5 静态：死码已替换、两个消费点都在 ==")
pv = strip_code(src("ops_http/preserve.rs"))
ch = strip_code(src("ops_http/preserve/preserve_choice.rs"))
ok("CX5a `let _ = scope_ok;` 这条「算了又丢」的死码已消失",
   "scope_ok" not in pv, "仍出现=%s" % ("scope_ok" in pv))
ok("CX5b preserve.rs 有两个 norm_choice 消费点（scope + status）",
   pv.count("norm_choice(") == 2, "命中=%d" % pv.count("norm_choice("))
ok("CX5c 白名单常量与结构体注释一致（scope 3 档 / status 4 档）",
   'SCOPES: [&str; 3]' in ch and 'STATUSES: [&str; 4]' in ch,
   "SCOPES=%s STATUSES=%s" % ("SCOPES" in ch, "STATUSES" in ch))

sys.exit(summary())
