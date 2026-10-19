# -*- coding: utf-8 -*-
"""闸门 DZ：候选投票（/offers/vote）三段必须同事务。

批一百七十。会员视角：给候选投一票要扣 1 火花。旧实现是四段各自提交
（`repo/offers.rs:83` 占位 → `economy_http` 扣款 → 计票），只在扣款失败时
调一次 best-effort 的 `drop_vote` 撤占位。两个可观察后果：

  ① 撤票自己失败（连接抖动/被 23505 之外的错误打断）＝「已投过票」的占位
     行留下、火花没扣、票数没加 —— 会员白丢一票，界面还说「投过了」；
  ② **幂等键残留而占位行不在**（候选清理任务删过 offer_votes、或①留下的
     反状态）＝ `place_vote` 返回 true，而 `spend_spark` 走自己的事务返回
     Ok(Replayed) 被旧 handler 当成成功放过 → 计票 +1，**一分钱不花的白票**
     （与 0291/P-347 同族：结果没被消费）。

修法照 P-359（农场播种）：占位/扣款/计票三段收进同一个 `tx`，扣款用
`spend_spark_tx` 并 `matches!(…, SpendOutcome::Spent)` fail-close；
`drop_vote` 连同它的符号陷阱一并删除（撤票改由整笔回滚负责）。

②不是假想路径：`admin_http/user_del.rs:126` 删 `offer_votes` 却从不删
`spark_ledger`（全仓 `DELETE FROM spark_ledger` 零命中），而账号是墓碑化
不物理删除（同文件 P-160 注释）——「清号 → 复活 → 再投同一个未转正候选」
就会落进这个状态。实测 A/B（同一支探针打两个基线）：现网旧码 :8080 三条红
——第三票回 code=0「ok」、offers.votes 1→2，而 spark_ledger 仍 1 行、
spark_balance 只扣过一次 ⇒ **白票坐实**；侧进程 :8099（本批新码）20 条全绿、
收尾残料 0、余额复原。

判据：
  DZ1 结构：offer_vote 里 begin → place_vote/spend_spark_tx/count_vote →
      commit 同段顺序钉死，且不许出现 autocommit 的 `spend_spark(`；
  DZ2 普查：全仓 `drop_vote` 引用数＝0（基数＝repo/offers.rs 的 fn 数）；
  DZ3 仓储层：place_vote/count_vote 收执行器泛型（E: sqlx::PgExecutor）；
  DZ4 承重自证：同一检测器喂「旧四段形状」必须红、喂「新形状」必须绿；
  DZ5 白票引信：Replayed 必须回落 1002，且该中文句在 validation_details.tsv
      里三语齐（缺译就在出口折成英文键，会员看到的是天书）；
  DZ6 棘轮：本文件所在的「资金原语结果被丢弃」普查（DK1）计数只降不升；
  DZ_LIVE=1（慢，真写现网库再自清）：活体 14 条——首票扣 1 计 1、重放
      1002 零变化、人为清掉占位行后再投必须 1002 且**不留占位/不加票/
      不二次扣款**，收尾残料归零。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dz_offer_vote.py
     活体：DZ_LIVE=1 FLUX_API_BASE=http://127.0.0.1:8099/api/v1 python -u \\
           scripts/pt_audit_dz_offer_vote.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
H = os.path.join(ROOTDIR, "apps", "api", "src", "content_http", "offers.rs")
R = os.path.join(ROOTDIR, "apps", "api", "src", "repo", "offers.rs")
TSV = os.path.join(ROOTDIR, "apps", "api", "i18n", "validation_details.tsv")
DK = os.path.join(ROOTDIR, "scripts", "pt_audit_dk_money_outcome.py")
NEW_SENTENCE = "这一票的火花已经扣过"


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    s = re.sub(r"^\s*//.*$", "", s, flags=re.M)
    return s


def fn_span(src, name):
    """取 async fn name…{ … } 整段（按花括号配平，容忍泛型参数）"""
    i = src.find("async fn %s" % name)
    if i < 0:
        return ""
    d = 0
    j = src.find("{", i)
    if j < 0:
        return ""
    k = j
    while k < len(src):
        if src[k] == "{":
            d += 1
        elif src[k] == "}":
            d -= 1
            if d == 0:
                return src[i:k + 1]
        k += 1
    return src[i:]


# ---- 检测器：一个 handler 段里「四段各自提交」的形状 ----
ATOMIC = re.compile(r"\.begin\(\)")
GEN_TX = re.compile(r"spend_spark_tx\(\s*&mut tx")
LOOSE = re.compile(r"\bspend_spark\(")
UNDO = re.compile(r"drop_vote\(")
OUTCOME = re.compile(r"SpendOutcome::Spent")


def loose_shape(seg):
    """红判据：段内出现独立提交的扣款或 best-effort 撤票，或缺整笔事务。"""
    return bool(LOOSE.search(seg) or UNDO.search(seg) or not ATOMIC.search(seg))


hb = cut(H)
rb = cut(R)
vote = fn_span(hb, "offer_vote")

# DZ1 结构与顺序
ok("DZ1a offer_vote 整段存在且带 begin/commit",
   bool(vote) and ".begin()" in vote and "tx.commit()" in vote,
   "段长=%d" % len(vote))
order = [vote.find("place_vote"), vote.find("spend_spark_tx"),
         vote.find("count_vote"), vote.find("tx.commit()")]
ok("DZ1b 占位→扣款→计票→提交顺序钉死",
   all(x >= 0 for x in order) and order == sorted(order), "偏移=%s" % order)
ok("DZ1c 不许有 autocommit 扣款或撤票",
   not LOOSE.search(vote) and not UNDO.search(vote))
ok("DZ1d 两处仓储写都递 &mut *tx",
   vote.count("&mut *tx") >= 2 and bool(GEN_TX.search(vote)),
   "&mut *tx=%d" % vote.count("&mut *tx"))

# DZ2 全仓 drop_vote 普查
hits, base = 0, 0
for root, _d, names in os.walk(os.path.join(ROOTDIR, "apps")):
    rl = root.replace("\\", "/")
    if "/target/" in rl or "/node_modules/" in rl:
        continue
    for n in names:
        if not n.endswith(".rs"):
            continue
        t = cut(os.path.join(root, n))
        base += len(re.findall(r"pub async fn ", t))
        hits += len(UNDO.findall(t))
ok("DZ2 全仓撤票原语引用为 0（基数=pub async fn %d 个）" % base,
   hits == 0, "命中=%d" % hits)
ok("DZ2b repo/offers.rs 里 drop_vote 已删（fn 基数照报）",
   "drop_vote" not in rb,
   "fn 数=%d" % len(re.findall(r"pub async fn ", rb)))

# DZ3 仓储层执行器泛型（tight 同时用于承重自证）
def tight(seg):
    return ("exe: E" in seg and "sqlx::PgExecutor" in seg
            and ".execute(exe)" in seg and "&repo.db" not in seg)


for fname in ("place_vote", "count_vote"):
    seg = fn_span(rb, fname)
    ok("DZ3 %s 收执行器泛型" % fname, tight(seg), "段长=%d" % len(seg))
OLD_REPO = """
pub async fn place_vote(
    repo: &Repo,
    offer_id: i64,
    user_id: i64,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query("INSERT INTO offer_votes …").execute(&repo.db).await?;
    Ok(r.rows_affected() > 0)
}
"""
ok("DZ3c 承重自证：旧仓储形状（autocommit + &repo.db）必须判红",
   not tight(fn_span(OLD_REPO, "place_vote")),
   "tight=%s" % tight(fn_span(OLD_REPO, "place_vote")))

# DZ4 承重自证：检测器必须抓得住旧形状、放得过新形状
OLD = """
pub(super) async fn offer_vote(...) -> DomainResult<HttpResponse> {
    let voted = crate::repo::offers::place_vote(&state.repo, 1, 2).await?;
    if !voted { return Err(DomainError::Validation("已投过票啦".into())); }
    spend_spark(&state.repo.db, 2, 1, "vote", "k", "offer", 1).await?;
    crate::repo::offers::drop_vote(&state.repo, 1, 2).await;
    crate::repo::offers::count_vote(&state.repo, 1).await?;
    Ok(ok(json!({ "voted": true })))
}
"""
ok("DZ4a 旧四段形状必须被检测器判红", loose_shape(OLD),
   "旧形状 loose=%s" % loose_shape(OLD))
ok("DZ4b 现网新形状必须判绿", not loose_shape(vote),
   "新形状 loose=%s" % loose_shape(vote))

# DZ5 白票引信 + 译文齐
ok("DZ5a Replayed 必须 fail-close 回 1002",
   bool(OUTCOME.search(vote)) and "已经扣过" in vote
   and "DomainError::Validation" in vote)
row = [x for x in rd(TSV).split("\n") if x.startswith(NEW_SENTENCE)]
ok("DZ5b 新句三语齐（%d 列）" % len(row),
   len(row) == 1 and len(row[0].split("\t")) == 3
   and all(c.strip() for c in row[0].split("\t")),
   "命中行=%d" % len(row))

# DZ6 同族闸门 DK 复跑：借来的判据要同时认退出码与 FAIL 行数（rc=0 而
# FAIL>0 = helper 自己崩在半路，不能当绿）
out = subprocess.run(
    [sys.executable, DK], capture_output=True, text=True,
    encoding="utf-8", cwd=ROOTDIR)
dk_fail = (out.stdout or "").count("[FAIL]")
ok("DZ6 同族 DK 仍绿（rc=0 且 FAIL=0）",
   out.returncode == 0 and dk_fail == 0,
   "rc=%d FAIL=%d" % (out.returncode, dk_fail))


# ---- 活体（DZ_LIVE=1）：真扣一次火花再自清 ----
if os.environ.get("DZ_LIVE") == "1":
    from pt_audit_lib import BASE, call, dig, login

    def psql(q):
        r = subprocess.run(
            ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
             "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f",
             "-"],
            input=q, capture_output=True, text=True, encoding="utf-8")
        if r.returncode != 0:
            print("   psql 失败：%s" % (r.stderr or "").strip()[:260])
        return (r.stdout or "").strip()

    tok = login()
    uid = int(psql("SELECT id FROM users WHERE username = 'root'"))
    tid = int(psql("SELECT id FROM torrents WHERE approval_status = 1 "
                   "ORDER BY id LIMIT 1"))
    bal0 = int(psql("SELECT spark_balance FROM users WHERE id = %d" % uid))
    oid = int(dig(call("POST", "/offers", {"torrent_id": tid}, tok)[1],
                  "data", "id"))
    idem = "offer-vote:%d:%d" % (uid, oid)

    def one(q):
        return int(psql(q))

    vq = ("SELECT count(*) FROM spark_ledger WHERE idempotency_key = '%s'"
          % idem)
    pq = ("SELECT count(*) FROM offer_votes WHERE offer_id = %d "
          "AND user_id = %d" % (oid, uid))
    try:
        st, pl = call("POST", "/offers/vote", {"offer_id": oid}, tok)
        ok("DZ7a 首票 200 且计票 1",
           st == 200 and one("SELECT votes FROM offers WHERE id = %d" % oid)
           == 1, "st=%s" % st)
        ok("DZ7b 首票扣 1 火花、流水 1 行",
           one("SELECT spark_balance FROM users WHERE id = %d" % uid)
           == bal0 - 1 and one(vq) == 1)
        st2, pl2 = call("POST", "/offers/vote", {"offer_id": oid}, tok)
        ok("DZ7c 重放 1002 零变化",
           (pl2 or {}).get("code") == 1002 and one(vq) == 1
           and one(pq) == 1,
           "code=%s" % (pl2 or {}).get("code"))
        psql("DELETE FROM offer_votes WHERE offer_id = %d AND user_id = %d;"
             % (oid, uid))
        st3, pl3 = call("POST", "/offers/vote", {"offer_id": oid}, tok)
        msg3 = (pl3 or {}).get("message", "")
        ok("DZ7d 键残留时拒白票（1002 含「已经扣过」）",
           (pl3 or {}).get("code") == 1002 and "已经扣过" in msg3,
           "msg=%s" % msg3)
        ok("DZ7e 拒绝后不白送：票 1／占位 0／流水 1／余额只扣 1",
           one("SELECT votes FROM offers WHERE id = %d" % oid) == 1
           and one(pq) == 0 and one(vq) == 1
           and one("SELECT spark_balance FROM users WHERE id = %d" % uid)
           == bal0 - 1)
    finally:
        psql("DELETE FROM offer_votes WHERE offer_id = %d;" % oid)
        psql("DELETE FROM spark_ledger WHERE idempotency_key = '%s';" % idem)
        psql("DELETE FROM offers WHERE id = %d;" % oid)
        psql("UPDATE users SET spark_balance = %d WHERE id = %d;"
             % (bal0, uid))
        rest = (one("SELECT count(*) FROM offers WHERE id = %d" % oid)
                + one("SELECT count(*) FROM offer_votes WHERE offer_id = %d"
                      % oid))
        ok("DZ7f 收尾残料 0 且余额复原",
           rest == 0 and one(
               "SELECT spark_balance FROM users WHERE id = %d" % uid) == bal0,
           "残料合计=%d" % rest)
else:
    print("   [skip] DZ7 活体未跑（DZ_LIVE=1 才开；会写现网库再自清）")

summary()
