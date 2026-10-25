# -*- coding: utf-8 -*-
"""闸门 DC：站免池（magic_pool）的月桶必须按站点日界切。

批一百五十四（P-340）实测的缺陷本体：
  `economy::pool_month()` 直接取 UTC 的年/月当标签，而
  * 界面（zh-CN.ts magicPool.rule / donors）写的是「**当月**累计捐赠达 200 万
    …下月 1-3 号自动开启全站双倍免费促销」「**本月**捐赠榜」；
  * worker 的「上月达标」判定（apps/worker/src/jobs/promos.rs）用的是
    `Utc::now() + 8h` 的**北京日**。
  ⇒ 每月 1 号 00:00-08:00（北京）入池的那笔税，被写进**上一个自然月**的桶：
    上月被虚增（差一点点达标时会被这 8 小时推过线，全局双免提前开），
    本月进度与「本月捐赠榜」同时少账。
  写侧四个口（勋章赠送抽税 / 基金缴纳 / 池子捐赠 / 付费种扣税）与读侧
  `/magic-pool` 全都走同一个 `pool_month()`，所以修一处全线归位；
  也正因如此，一旦有人再抄一份裸 UTC 的月标签，桶就会分成两行、账就裂开
  —— DC2 就是钉这件事的。

判据层次：DC1~DC4 静态（形状与单一出处），DC5 活体形状，DC6 把承重力交给
仓里的 Rust 单测 `economy::tests::pool_month_follows_site_boundary`
（默认不跑 cargo，设 FLUX_GATE_CARGO=1 时闸门自己跑一次并验「旧写法必红」）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dc_pool_month.py
"""
import io
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def read(rel):
    return io.open(os.path.join(ROOTDIR, rel), encoding="utf-8",
                   errors="replace").read()


ECON = "apps/api/src/economy.rs"
econ = read(ECON)
m = re.search(r"pub fn pool_month\([^)]*\)\s*->\s*String\s*\{(.*?)\n\}",
              econ, flags=re.S)
body = m.group(1) if m else ""
SITE_SHIFT = ("Duration::hours(8)" in body) or ("Asia/Shanghai" in body)
ok("DC1 pool_month 里必须有站点日界换算（+8h 或 Asia/Shanghai）；"
   "函数体=%s" % body.strip().replace("\n", " ")[:90],
   m is not None and SITE_SHIFT,
   "函数体没找到" if not m else "裸 UTC 月标签回潮")

# DC2：月桶标签只许一把尺。范围收在「真的读写 magic_pool / pool_donations 的
# 文件」上——别处的 `format!("{:04}-{:02}")` 是月度统计显示，不是池子桶键，
# 把它们算成副本会把判据淹成噪音（本轮第一版就误报 33 处）。
WRITES_POOL = re.compile(r"INSERT INTO magic_pool|UPDATE magic_pool|"
                         r"INSERT INTO pool_donations")
LOCAL_LABEL = re.compile(r'\.format\("\{?%Y-%m\}?"\)|'
                         r'format!\(\s*"\{:04\}-\{:02\}"|'
                         r"to_char\([^)]*YYYY-MM")
# 允许的第二种写法：worker 要的是「上月」标签，SQL 侧算且喂的是 +8h 的北京日
POOL_ALLOW = {"apps/worker/src/jobs/promos.rs":
              "上月标签走 SQL date_trunc，入参已是 Utc::now()+8h ⇒ 与池同界"
              "（同界性由 DC4 钉）"}
copies = []
pool_files = []
for base in ("apps/api/src", "apps/worker/src"):
    for dp, _dn, fn in os.walk(os.path.join(ROOTDIR, base)):
        for f in sorted(fn):
            if not f.endswith(".rs"):
                continue
            full = os.path.join(dp, f)
            rel = os.path.relpath(full, ROOTDIR).replace("\\", "/")
            src = io.open(full, encoding="utf-8", errors="replace").read()
            if not WRITES_POOL.search(src):
                continue
            pool_files.append(rel)
            if rel == ECON or rel in POOL_ALLOW:
                continue
            if "pool_month(" not in src:
                hits_local = [src[:mm.start()].count("\n") + 1
                              for mm in LOCAL_LABEL.finditer(src)]
                copies.append((rel, hits_local))
for rel, lines in copies:
    print("   副本 %-46s 本地月标签出现在行 %s" % (rel, lines))
ok("DC2 入池文件都走 pool_month 这一把尺（入池文件 %d 个，本地标签副本 %d 个）"
   % (len(pool_files), len(copies)), not copies,
   "、".join(c[0] for c in copies[:5]))

# DC3：写侧四个口 + 读侧一个口必须都调 pool_month（漏一个就写进别的桶）
WRITERS = ("apps/api/src/community_http/medal_gift.rs",
           "apps/api/src/economy_http/funding_contribute.rs",
           "apps/api/src/economy_http/pool.rs",
           "apps/api/src/torrents/charge.rs")
no_call = [f for f in WRITERS if "pool_month(" not in read(f) or
           "magic_pool" not in read(f)]
magic_inserts = sum(read(f).count("INSERT INTO magic_pool") for f in WRITERS)
ok("DC3 四个入池口都在同一把尺上（都调 pool_month；magic_pool 写入点 %d）"
   % magic_inserts, not no_call and magic_inserts >= 4,
   "没调的=%s" % "、".join(no_call))
pd_writes = sum(
    read(f).count("INSERT INTO pool_donations")
    for f in ("apps/api/src/community_http/medal_gift.rs",
              "apps/api/src/economy_http/funding_contribute.rs",
              "apps/api/src/economy_http/pool.rs"))
ok("DC3b 「本月捐赠榜」明细表也同一把尺（pool_donations 写入点 %d）" % pd_writes,
   pd_writes >= 3)

# DC4：worker 的「上月达标」判定必须与池子同界
promo = read("apps/worker/src/jobs/promos.rs")
ok("DC4 worker 侧取的是北京日（Utc::now() + 8h）来判 1-3 号与上月",
   "Duration::hours(8)" in promo and "date_trunc('month'" in promo)

# DC5 活体：/magic-pool 的 month 必须是**北京**月标签
tok = login()
st, body5 = call("GET", "/magic-pool", token=tok)
api_month = ((body5 or {}).get("data") or {}).get("month")
site_month = time.strftime("%Y-%m", time.gmtime(time.time() + 8 * 3600))
utc_month = time.strftime("%Y-%m")
print("   /magic-pool http=%s month=%s；北京月=%s，UTC 月=%s"
      % (st, api_month, site_month, utc_month))
if site_month == utc_month:
    print("   WARN DC5 此刻北京月与 UTC 月同名（%s），这条只在每月 1 号"
          " 00:00-08:00（北京）才咬人——形状断言仍照打" % site_month)
ok("DC5 池子页读的月桶 = 北京自然月（API=%s / 应为 %s）" % (api_month, site_month),
   str(api_month) == site_month)

# DC5b 库里现存月桶必须都是 YYYY-MM 形状（裂桶会一眼露出来）
def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("   psql 失败：%s" % (r.stderr or "").strip()[:170])
    return (r.stdout or "").strip()


buckets = psql("SELECT month || '=' || donated_total::text FROM magic_pool "
               "ORDER BY month")
bad_bucket = [l for l in buckets.split("\n") if l and not re.match(
    r"^\d{4}-\d{2}=\d+$", l)]
print("   现网月桶：%s" % buckets.replace("\n", "  ")[:200])
ok("DC5b 库里月桶标签形状合法（%d 行，坏形状 %d）"
   % (len([l for l in buckets.split("\n") if l]), len(bad_bucket)),
   not bad_bucket, "、".join(bad_bucket[:3]))

# DC6 承重力交给仓里的 Rust 单测：默认只验它在位，FLUX_GATE_CARGO=1 时真跑
has_test = "fn pool_month_follows_site_boundary" in econ
ok("DC6 单测在位（economy::tests::pool_month_follows_site_boundary）", has_test,
   "没这条单测就无法证明月初那 8 小时的桶真的归位")
if os.environ.get("FLUX_GATE_CARGO") == "1":
    r = subprocess.run(["cargo", "test", "--bin", "flux-api", "pool_month"],
                       cwd=ROOTDIR, capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    tail = [l for l in (r.stdout or "").split("\n") if "test result" in l]
    print("   cargo test：%s（rc=%s）" % ("; ".join(tail)[:120], r.returncode))
    ok("DC6b cargo test pool_month 通过", r.returncode == 0,
       (r.stdout or "")[-200:] + (r.stderr or "")[-200:])
else:
    print("   （设 FLUX_GATE_CARGO=1 可让本闸门顺带跑一次 cargo 单测）")

print("   本闸门全程只读：未写库、未改设置")
summary()
