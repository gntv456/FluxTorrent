"""种子详情子资源的「主详情 404、子资源 200」劈叉闸门（TV，P-643）。

2026-10-25 版主实测：一颗**对待审队列外的会员不可见**的种（被拒/待审且不是
自己发的），主详情按 visibility.rs 的口径回 1004，但 `/torrents/{id}/related`
回 200 且带满 8 条推荐、`/torrents/{id}/group` 回 200——而随手编一个不存在的
号回 200 空数组。两码之差把「这个号存在、只是没过审」做成了可枚举的探针。

本闸门两半：
  * 运行时（承重）：把全部 id 级 GET 口对同一颗隐种各打一次，**任何一口都不许
    回成功信封**；同循环换 staff token 再打一遍做正控——若路由名写错或口已下线，
    正控的放行数会塌下去，而不是让前半段「全都非 200」假绿。
  * 静态（棘轮）：id 级 GET 口的**集合**钉住，新开的口不在册即红；被修的两只
    自身必须仍带可见性闸门（属性被摘就红，不等下一次人工深测）。

用法：默认打现网 :8080；验新码时 FLUX_API_BASE=http://127.0.0.1:8099/api/v1
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, ok, summary  # noqa: E402
from ci_reg_common import jwt, psql  # noqa: E402

SRC = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                   "apps", "api", "src")

# 2026-10-25 实测在册的 id 级 GET 口（新增/删除必须同步本清单）
LEDGER = {
    "/torrents/{id}",
    "/torrents/{id}/aggregate",
    "/torrents/{id}/detail",
    "/torrents/{id}/collections",
    "/torrents/{id}/comments",
    "/torrents/{id}/delete-preview",
    "/torrents/{id}/download",
    "/torrents/{id}/file-conflicts",
    "/torrents/{id}/files",
    "/torrents/{id}/group",
    "/torrents/{id}/logs/{ordinal}",
    "/torrents/{id}/magnet",
    "/torrents/{id}/nfo",
    "/torrents/{id}/peers",
    "/torrents/{id}/previews",
    "/torrents/{id}/related",
    "/torrents/{id}/snatches",
    "/torrents/{id}/tags",
    "/torrents/{id}/thanks",
    "/torrents/{id}/trumps",
}
# 只有职员档能进的口：隐种下回 2003 而不是 1004，同样是「不给看」
STAFF_ONLY = {"/torrents/{id}/delete-preview"}
# 需要真下载/真文件、成功与否都不该在本案里断言的口
NO_SUCCESS = LEDGER - {"/torrents/{id}/download", "/torrents/{id}/magnet"}


def scan_get_paths():
    """源码里全部 `#[get("/torrents/…")]` 且路径含 {id} 的口。"""
    found = set()
    for dp, _, fns in os.walk(SRC):
        for fn in fns:
            if not fn.endswith(".rs"):
                continue
            s = open(os.path.join(dp, fn), encoding="utf-8",
                     errors="replace").read()
            for m in re.finditer(r'#\[get\("(/torrents/[^"]*)"\)\]', s):
                if "{id}" in m.group(1):
                    found.add(m.group(1))
    return found


def body_of(rel, fn_name):
    """某个 handler 的函数体（剥 // 注释与 /// 文档注释后再扫）。"""
    s = open(os.path.join(SRC, rel.replace("/", os.sep)),
             encoding="utf-8", errors="replace").read()
    m = re.search(r"(?:pub(?:\(crate\))?\s+)?async fn %s\s*\(" % fn_name, s)
    if not m:
        return ""
    tail = s[m.end():]
    nxt = re.search(r"\n#\[", tail)
    seg = tail[:nxt.start()] if nxt else tail
    return "\n".join(l.split("//")[0] for l in seg.split("\n"))


def gated(rel, fn_name):
    b = body_of(rel, fn_name)
    return any(t in b for t in ("assert_visible", "visibility_sql"))


paths = scan_get_paths()
ok("TV1 id 级 GET 口集合与台账一致（多一口/少一口都红）",
   paths == LEDGER,
   "新增未登记=%s 台账已不存在=%s" % (sorted(paths - LEDGER),
                                      sorted(LEDGER - paths)))

print("== S2 被修的两只自身仍带闸门 ==")
ok("TV2 related.rs 仍走 assert_visible（属性被摘即红）",
   gated("torrent_http/related.rs", "torrent_related"))
ok("TV3 group.rs group_info 仍按可见性放行（P-583 公开≠放行隐种）",
   gated("publish_http/group.rs", "group_info"))

# ---------------- 运行时 ----------------
print("== S3 活体：同一颗隐种，逐口不许回成功 ==")
viewer = int(psql("SELECT id FROM users WHERE class_id < 90 AND status < 2 "
                  "AND id <> 1 ORDER BY id DESC LIMIT 1") or 0)
hidden = psql("SELECT t.id FROM torrents t JOIN users u ON u.id = t.owner_id "
              "WHERE t.approval_status IN (0, 2) AND u.class_id < 90 "
              "AND t.owner_id <> %d ORDER BY t.id DESC LIMIT 1" % viewer)
hid_t = psql("SELECT owner_id FROM torrents WHERE id = %s" % hidden)
staff = int(psql("SELECT id FROM users WHERE class_id >= 90 AND status < 2 "
                 "ORDER BY id LIMIT 1") or 0)
vis_ok = psql("SELECT t.id FROM torrents t WHERE t.approval_status = 1 "
              "AND t.group_id IS NOT NULL ORDER BY t.id DESC LIMIT 1")
must = [("夹具：低档会员 uid", viewer), ("夹具：职员 uid", staff),
        ("夹具：过审且有组的种", vis_ok)]
for label, v in must:
    ok("TV4 %s 取到（取不到即红，不许静默跳过）" % label, bool(v), str(v))
ok("TV5 夹具：对本人不可见的种存在（待审/被拒且非本人发的）",
   bool(hidden), "hidden=%s" % hidden)
if not all([viewer, staff, hidden, vis_ok]):
    print("   缺夹具，活体段不跑（已红在上面）")
    summary()

vtok = jwt(viewer, cls=1)
stok = jwt(staff, cls=99)
otok = jwt(int(hid_t), cls=1)
hid = int(hidden)


def hit(tok, path):
    """(http, code, body)——第三个位置放**整份信封**，不是 msg。

    （先前一版把 msg 放在这里，TV13 于是把「取不到 data.group」读成
    「产品不放行匿名」＝闸门自造的假红。）
    """
    st, body = call("GET", path, token=tok)
    body = body if isinstance(body, dict) else {}
    return st, body.get("code"), body


# 正控先行：staff 同批口里放行成功的条数——路由名写错/口下线会塌在这里
staff_pass = []
for tpl in sorted(LEDGER):
    p = tpl.replace("{ordinal}", "1").replace("{id}", str(hid))
    st, code, _ = hit(stok, p)
    if st == 200 and code == 0:
        staff_pass.append(tpl)
print("   正控：职员对同一颗隐种回 200 的口 = %d/%d" % (
    len(staff_pass), len(LEDGER)))
ok("TV6 正控：职员侧仍能看到隐种的附属数据（>=8 口，否则是路由写错）",
   len(staff_pass) >= 8, "实得 %d：%s" % (len(staff_pass), staff_pass[:10]))

leaks = []
for tpl in sorted(NO_SUCCESS):
    p = tpl.replace("{ordinal}", "1").replace("{id}", str(hid))
    st, code, body = hit(vtok, p)
    if st == 200 and code == 0:
        leaks.append((tpl, p))
        print("   LEAK %-34s http=200 code=0" % p)
    else:
        print("   ok   %-34s http=%s code=%s %s" % (p, st, code,
                                                    str(body.get("msg"))[:40]))
ok("TV7 非本人且非职员的会员，对隐种任何 id 级 GET 口都不得回成功",
   not leaks, "漏口=%s" % [t for t, _ in leaks])

print("== S8 定向：隐种可见性口径四案 ==")
for path, label in [("/torrents/%d/related", "相关种子"),
                    ("/torrents/%d/group", "同组资源")]:
    st, code, _ = hit(vtok, path % hid)
    ok("TV9 %s：非本人低档会员看隐种 = 1004" % label, code == 1004,
       "http=%s code=%s" % (st, code))
    st, code, _ = hit(otok, path % hid)
    ok("TV10 %s：种主本人仍看得到（不误伤）" % label, code == 0,
       "http=%s code=%s" % (st, code))
    st, code, _ = hit(vtok, path % int(vis_ok))
    ok("TV11 %s：过审种照常回 200（闸门没打死可用面）" % label, code == 0,
       "http=%s code=%s" % (st, code))

st, code, _ = hit(None, "/torrents/%d/related" % hid)
ok("TV12 匿名走 related 仍要登录（2001）", code == 2001,
   "http=%s code=%s" % (st, code))
st, code, body = hit(None, "/torrents/%d/group" % int(vis_ok))
name_ok = bool(((body or {}).get("data") or {}).get("group"))
ok("TV13 匿名读**过审**种的同组资源照旧放行（P-583 详情页公开口径）",
   code == 0 and name_ok, "http=%s code=%s group=%s" % (st, code, name_ok))
st, code, _ = hit(None, "/torrents/%d/group" % hid)
ok("TV14 匿名读隐种的同组资源 = 1004（不再是 group:null 的 200）",
   code == 1004, "http=%s code=%s" % (st, code))

ghost = hid + 99999999
for path, label in [("/torrents/%d/related", "相关种子"),
                    ("/torrents/%d/group", "同组资源")]:
    a = hit(vtok, path % hid)[1]
    b = hit(vtok, path % ghost)[1]
    ok("TV15 %s：隐种与不存在的号同码（存在性探针失效）" % label, a == b,
       "隐种=%s 幻号=%s" % (a, b))

print("== S16 静态：related 的 SQL 仍只推过审种 ==")
rel_sql = open(os.path.join(SRC, "torrents", "related.rs"),
               encoding="utf-8", errors="replace").read()
ok("TV16 torrents/related.rs 候选段仍带 approval_status = 1（4 段都在）",
   rel_sql.count("t.approval_status = 1") >= 4,
   "出现 %d 次" % rel_sql.count("t.approval_status = 1"))

summary()
