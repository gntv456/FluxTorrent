# -*- coding: utf-8 -*-
"""常驻闸门：开放 API 的「我的数据」四个口必须真分页（P-451）。

会员/工具视角：手机壳与同步脚本用 API Token 拉「我的站内信 / 我的做种」。
现网实测（root，153 封信）：
    GET /open/me/messages?limit=100            → 100 条，count=100
    GET /open/me/messages?limit=100&offset=100 → **与上一行完全相同的 100 条**
    GET /open/me/messages?limit=100&page=2     → 还是这 100 条
⇒ 后 53 封永久取不到，HTTP 200、code=0，没有任何字段说「被截断了」。
同族的 `/open/announces` 走的是另一套（`since_id` 游标 + 真 count），
所以这不是风格差异，是这四个口缺了分页参数却收了 limit。

修法：`paging.rs` 一份 `page_params`（limit 缺省 50 夹到 [1,MAX_ROWS]、
offset 缺省 0 负数夹 0）+ `count_with`（**与列表同一个 FROM/WHERE 字符串**）
+ `has_more`；四个口的 SQL 由 `LIMIT $2` 变成 `LIMIT $2 OFFSET $3`，
响应加 `total / offset / limit / has_more`，`count` 保持「本次返回条数」
（与 /open/announces 同口径，不改旧客户端的语义）。

判据分层：
  O0 正控   修前两个文件：SQL 里没有 OFFSET、响应里没有 total/has_more
  O1        现网接线：四个口都吃 page_params；列表与计数共用同一个 fw 变量；
            paging.rs 在册且被 mod 声明
  O2        纯函数行为：cargo test --bin flux-api paging::（6 条断言）
  O3        SQL 层 A/B：从源码里取出那段字面 FROM/WHERE，在现网库里跑
            OFFSET 0 / OFFSET 100 两页 ⇒ 页不重叠、并集等于 total
  O4        界面层：默认只报读数（现网二进制还没带上本批修复），
            站方重建后加 `--live` 升硬；这一步会签发并撤销一枚探针 Token

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_openapi_paging.py
      站方重建 api 之后：同上加 --live
"""
import io
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import BASE, ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ME = "apps/api/src/openapi_http/me.rs"
MT = "apps/api/src/openapi_http/me_torrents.rs"
PG = "apps/api/src/openapi_http/paging.rs"
FIX = "26a9acf3"           # 本批修复的提交（O0 从它的父提交取旧形状）
LIVE = "--live" in sys.argv
PSQL = ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
        "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-F", "|",
        "-f", "-"]
# 四个口：文件 → 该文件里必须出现的口名
ROUTES = {ME: ["me_hr", "me_messages"], MT: ["me_seeding", "me_history"]}


def read(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def psql(sql):
    p = subprocess.run(PSQL, input=sql, capture_output=True, text=True,
                       encoding="utf-8", errors="replace", cwd=ROOT)
    if p.returncode != 0:
        raise RuntimeError("psql 失败：%s" % ((p.stderr or "")[:220]))
    return (p.stdout or "").strip()


def git_show(rev, path):
    p = subprocess.run(["git", "show", "%s:%s" % (rev, path)],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", cwd=ROOT)
    if p.returncode != 0:
        raise SystemExit("取文失败 %s:%s / %s"
                         % (rev, path, (p.stderr or "")[:150]))
    return p.stdout or ""


print("== O0 正控（修前的形状）==")
old_me, old_mt = git_show(FIX + "^", ME), git_show(FIX + "^", MT)
ok("O0a 修前两个文件里都没有 OFFSET（所以 offset 一定被吞）",
   "OFFSET" not in old_me and "OFFSET" not in old_mt)
ok("O0b 修前响应里没有 total/has_more（客户端无从知道被截断）",
   '"total"' not in old_me and '"has_more"' not in old_me
   and '"total"' not in old_mt and '"has_more"' not in old_mt)

print("== O1 现网接线 ==")
new_me, new_mt, pg = read(ME), read(MT), read(PG)
ok("O1a paging.rs 在册且被 mod 声明",
   os.path.exists(os.path.join(ROOT, PG))
   and re.search(r"^mod paging;$", read(
       "apps/api/src/openapi_http.rs"), re.M) is not None)
ok("O1b page_params：limit 夹界、offset 负数夹 0、脏值回落缺省",
   re.search(r"\.clamp\(1, MAX_ROWS\)", pg) is not None
   and ".max(0)" in pg and 'get("offset")' in pg)
all_ok = True
detail = []
for text, names in ((new_me, ROUTES[ME]), (new_mt, ROUTES[MT])):
    for fn in names:
        i = text.find("async fn %s" % fn)
        seg = text[i:text.find("\n}\n", i)] if i >= 0 else ""
        got = ("page_params" in seg and '"total": total' in seg
               and '"has_more": has_more(' in seg)
        detail.append("%s=%s" % (fn, "OK" if got else "缺"))
        all_ok = all_ok and got
ok("O1c 四个口都吃 page_params 并回显 total/has_more（%s）"
   % " ".join(detail), all_ok)
ok("O1d 列表与计数共用同一个 FROM/WHERE 变量（不是各写一份）",
   new_me.count("count_with(&state, tk.uid, &fw)") == 2
   and re.search(r"my_snatches\(&state, tk\.uid, (true|false), limit, "
                 r"offset\)", new_mt) is not None
   and new_mt.count("count_with(&state, tk.uid, &snatch_fw(") == 2)
ok("O1e SQL 位确实是 LIMIT $2 OFFSET $3",
   new_me.count("LIMIT $2 OFFSET $3") == 2
   and "LIMIT $2 OFFSET $3" in new_mt)

print("== O2 纯函数行为 ==")
t = subprocess.run(["cargo", "test", "--bin", "flux-api", "paging::"],
                   capture_output=True, text=True, encoding="utf-8",
                   errors="replace", cwd=ROOT)
out = (t.stdout or "") + (t.stderr or "")
ok("O2 cargo test paging:: 通过（rc=%d）" % t.returncode,
   t.returncode == 0 and "2 passed" in out,
   "红账=%s" % [l for l in out.splitlines() if "FAILED" in l][:3])

print("== O3 SQL 层 A/B（现网库，只读）==")
# 从源码里取那段字面 FROM/WHERE，不另抄一份（防「验的不是被测代码」）
m = re.search(r'let fw = format!\(\s*"(messages m[^"]*)",?\s*cond',
               new_me.replace("\\\n", " ").replace("\n      ", "\n"))
# `{}` 是 Rust 的格式占位（cond 拼在那里），取数时要摘掉；反斜杠续行也压平
frag = " ".join((m.group(1) if m else "").replace("{}", "").split())
ok("O3 前置：认到 messages 的 FROM/WHERE 片段（否则判据打空）",
   "messages m" in frag and frag.endswith("receiver_id = $1"), frag[:70])
UID = 1
where = frag.replace("$1", str(UID))
total = int(psql("SELECT count(*) FROM %s;" % where))
ids1 = [int(x) for x in psql(
    "SELECT m.id FROM %s ORDER BY m.id DESC LIMIT 100 OFFSET 0;"
    % where).split("\n") if x.strip()]
ids2 = [int(x) for x in psql(
    "SELECT m.id FROM %s ORDER BY m.id DESC LIMIT 100 OFFSET 100;"
    % where).split("\n") if x.strip()]
ok("O3a 库里 %d 封，第一页 %d 条、第二页 %d 条，两页不重叠"
   % (total, len(ids1), len(ids2)),
   total == len(ids1) + len(ids2) and not (set(ids1) & set(ids2)),
   "重叠＝OFFSET 没进到 SQL")
ids_no_off = [int(x) for x in psql(
    "SELECT m.id FROM %s ORDER BY m.id DESC LIMIT 100;" % where
).split("\n") if x.strip()]
ok("O3b 旧写法（不带 OFFSET）再问一次还是同一批＝缺陷本体（%d 条全重合）"
   % len(set(ids_no_off) & set(ids1)),
   ids_no_off == ids1 and ids1 != ids2,
   "带 OFFSET 的第二页必须与第一页不同，否则上面那条 A/B 是假的")
print("   [账面] root 站内信 %d 封；修复前客户端最多见到最新的 100 封"
      % total)

print("== O4 界面层（%s）==" % ("升硬" if LIVE else "默认只报读数"))
if not LIVE:
    print("   [WARN 不判红] 现网镜像若无本批修复，?offset=100 仍回第一页；"
          "站方重建 api 后用 --live 复跑（这一步会签发并撤销一枚探针 Token）")
else:
    name = "p444-live-%d" % int(time.time())

    def rq(method, path, body=None, hdrs=None):
        r = urllib.request.Request(BASE + path, method=method)
        r.add_header("Content-Type", "application/json")
        for k, v in (hdrs or {}).items():
            r.add_header(k, v)
        data = json.dumps(body).encode() if body is not None else None
        try:
            with urllib.request.urlopen(r, data, timeout=30) as f:
                return f.status, json.loads(f.read())
        except urllib.error.HTTPError as e:
            raw = e.read()
            try:
                return e.code, json.loads(raw)
            except Exception:
                return e.code, {"raw": raw[:80].decode("utf-8", "replace")}

    st, lg = rq("POST", "/auth/login",
                {"username": "root", "password": "password123"})
    jwt = (lg.get("data") or {}).get("token")
    H = {"Authorization": "Bearer " + (jwt or "")}
    st, it = rq("POST", "/me/tokens",
                {"password": "password123", "name": name,
                 "scopes": ["read"], "rate_per_min": 120}, H)
    secret = (it.get("data") or {}).get("token")
    tid = (it.get("data") or {}).get("id")
    try:
        TH = {"Authorization": "Token " + (secret or "")}
        st, a = rq("GET", "/open/me/messages?limit=100", hdrs=TH)
        st2, b = rq("GET", "/open/me/messages?limit=100&offset=100", hdrs=TH)
        da, db = a.get("data") or {}, b.get("data") or {}
        ia = [x["id"] for x in da.get("items") or []]
        ib = [x["id"] for x in db.get("items") or []]
        ok("O4a 第二页与第一页不重叠且带 total/has_more（total=%s）"
           % da.get("total"),
           da.get("total") == total and not (set(ia) & set(ib))
           and isinstance(da.get("has_more"), bool),
           "现网镜像没带上修复时这条会红＝部署账不是产品账")
        ok("O4b 越界 offset 给空页而不是回第一页",
           st2 == 200 and len(ib) == total - 100,
           "第二页 %d 条，库里共 %d 条" % (len(ib), total))
    finally:
        rq("POST", "/me/tokens/revoke", {"id": tid}, H)
        left = psql("SELECT count(*) FROM api_tokens WHERE name='%s'"
                    " AND revoked_at IS NULL;" % name)
        ok("O4c 探针 Token 已撤销（未撤行数 %s）" % left, left == "0")

summary()
