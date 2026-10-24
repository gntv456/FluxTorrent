# -*- coding: utf-8 -*-
"""LT 闸门：入账(earn)侧撞幂等重放，一律不许「报成功而不付、还把账抹平」。

病形（P-640，2026-10-24 从「未审面普查」的 290 条里选出来的一条）：
宠物领取 `POST /games/pet/claim` 走 `tick_tx(..., Some(idem))`，旧写法是
```rust
let out = earn_spark_tx(tx, uid, new_pending, "game", idem).await?;
if !matches!(out, SpendOutcome::Replayed) { earned = new_pending; }
new_pending = 0;              // ← 重放也清账
```
同一枚幂等键第二次到达时：不付钱、把刚攒出来的 pending 清零、还回 200。
两个后果叠在一起——
  ① `spend.rs:219` 成文口径点名的最贵形状：「接口成功而用户零到账」；
  ② 抹掉的是这枚键**之后新攒**的产出，不是已付过的那笔（静默吞资产）。
同模块的投喂腿（`food_coupon.rs::pay_feed_tx`）对同一件事是**报 400**
「这次投喂已受理，请勿重复提交」⇒ 两个出口一条事实两种表征。

修法：给领取补一个与投喂同口径的 `expect_fresh`，让整笔事务回滚
（攒的产出留在原地，下次带新键照常付），不另造机制。

判据三层：
  静态：领取分支必须走 `expect_fresh`；旧形状（重放仍清账）不许再出现；
        投喂/领取两条「已受理」同形；成文口径那段注释还在（删了要有人喊）；
        **全站 earn 调用点普查**——每个点归入四类处置之一（expect_spent /
        expect_fresh / matches! 判 / 丢弃），丢弃型必须逐条写清「为什么这枚键
        不可能重放」并与台账两头对账（候选数≠缺陷数：本批实测 22 点里
        17 点已判、5 点丢弃有论证、1 点就是刚修的宠物领取）。
  单测：`cargo test --bin flux-api food_coupon`（翻转 expect_fresh 即红）。
  活体：拿 root 的宠物行做确定性夹具（先整行备份，收尾 `IS DISTINCT FROM`
        复原），把 last_tick 拨回 6 小时、能量压到 3 ⇒ 每次恰好攒 2 火花：
        领(A 键)付 → 再攒 → 领(A 键)必须 **400 且不出账** → 领(B 键)必须照付。
        旧代码在这里是 200 + 那 2 火花蒸发 ⇒ 现网打这条必红，就是缺陷本体。
正控：LT6 跑在 CTRL_REV（修复前一提交）的 pets.rs 上必须报洞。
"""
import os
import re
import subprocess
import sys
import uuid

sys.path.insert(0, "scripts")
from pt_audit_lib import (call, dig, login, ok, psql_stdin,
                          FAILURES)  # noqa: E402

API = "apps/api/src"
PETS = API + "/games_http/pets.rs"
FCOUP = API + "/games_http/food_coupon.rs"
SPEND = API + "/economy_http/spend.rs"
CTRL_REV = "43c0de9b"  # 本批修复的前一个提交（绝对钉，别用 HEAD~1）
UID = 1               # root：宠物行的整行备份/复原见 finally


def read(rel, rev=None):
    if rev:
        out = subprocess.run(["git", "show", "%s:%s" % (rev, rel)],
                             capture_output=True, encoding="utf-8",
                             errors="replace")
        return out.stdout if out.returncode == 0 else None
    try:
        with open(rel, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def strip_comments(src):
    """剥掉 `//` 注释再判形状。

    不剥就会自伤：本批我在 `food_coupon.rs` 的注释里抄了一句发放名单的
    旧文案（为了说明为什么不复用 `expect_spent`），扫描把它读成「会员口
    借用了名单口径的文案」——静态 grep 型判据必须先剥注释，这是旧账重演。
    """
    return "\n".join(l.split("//")[0] for l in src.splitlines())


def window(src, lineno, before=7, after=16):
    lines = src.splitlines()
    return "\n".join(lines[max(0, lineno - before):lineno + after])


def classify(src, lineno):
    """按**绑定变量**判处置，不用「窗口里出现过 let _ =」当证据。

    粗窗口在本批两次骗过我：别的语句里的 `matches!` / `let _ =` 会被算成
    这一点的处置（farm_actions.rs:392 那句 `let _ = earn_spark_tx(...)` 就是
    被邻座的 matches! 洗成「已判」的）。先找这一点把结果绑成什么名字，
    再看那个名字怎么被处置。
    """
    lines = src.splitlines()
    at = lineno - 1
    before = "\n".join(lines[max(0, at - 3):at])
    # 处置写法离调用点可以很远（多行实参 + `.await?;` 再判），所以**找名字**
    # 用宽域、判「直接丢弃」用紧域：宽域只匹配带该变量名的形状，不会串到邻座。
    tight = "\n".join(lines[at:at + 3])
    body = "\n".join(lines[max(0, at - 3):at + 24])
    if re.search(r"let\s+_\s*=\s*\S*\s*earn_spark_tx", tight):
        return "丢弃"
    m = re.search(r"let\s+(\w+)\s*=\s*\S*\s*earn_spark_tx", body)
    if m is None:
        m = re.search(r"let\s+(\w+)\s*=\s*$", before.rstrip())
    if m:
        name = m.group(1)
        if re.search(r"expect_spent\(\s*%s" % re.escape(name), body):
            return "expect_spent"
        if re.search(r"expect_fresh\(\s*%s" % re.escape(name), body):
            return "expect_fresh"
        if re.search(r"matches!\(\s*%s" % re.escape(name), body):
            return "matches!"
        if re.search(r"let\s+_\s*=\s*%s\b" % re.escape(name), body):
            return "丢弃"
        return "无处置"
    if "expect_spent(" in before:
        return "expect_spent"
    if "expect_fresh(" in before:
        return "expect_fresh"
    if "matches!(" in before:
        return "matches!"
    return "无处置"


# ---------------------------------------------------------------- 静态层
pets = read(PETS) or ""
i = pets.find("if let Some(idem) = claim_idem")
claim_blk = pets[i:i + 700] if i >= 0 else ""
ok("LT1 领取分支必须走 expect_fresh（重放＝拒，不是清账）",
   "expect_fresh(out)?" in claim_blk, claim_blk[:150])
OLD = "if !matches!(out, SpendOutcome::Replayed)"
ok("LT2 旧形状不许再出现（重放不付但仍清零）", OLD not in pets,
   "pets.rs 里还能找到那句吞重放的 matches!")

fcoup_code = strip_comments(read(FCOUP) or "")
msgs = sorted(set(re.findall(r'"这次([^"]*?)已受理，请勿重复提交"',
                             fcoup_code)))
ok("LT3 投喂与领取两条「已受理」同形（同模块不许两种表征）",
   len(msgs) == 2 and all(m in ("投喂", "领取") for m in msgs),
   "抓到=%s" % msgs)
ef_body = fcoup_code[fcoup_code.find("fn expect_fresh"):][:400]
ok("LT4 expect_fresh 的文案不是批量发放那句（会员看不懂「重复用户」）",
   "重复用户" not in ef_body and "已受理" in ef_body,
   "函数体=%s" % ef_body[:150])
spend_doc = read(SPEND) or ""
ok("LT5 成文口径仍在（spend.rs 写明发放侧重放要当成失败回滚）",
   "当成失败" in spend_doc and "让所在事务一起回滚" in spend_doc,
   "那段口径注释被删了，本闸门判据的依据就没了")

ctrl_pets = read(PETS, CTRL_REV)
ci = ctrl_pets.find("if let Some(idem) = claim_idem") if ctrl_pets else -1
ctrl_blk = ctrl_pets[ci:ci + 700] if ci >= 0 else ""
ok("LT6 正控：%s 那版必须被 LT1 报洞（判据承重）" % CTRL_REV,
   ctrl_pets is not None and "expect_fresh(out)?" not in ctrl_blk,
   "旧版读不到／已含 expect_fresh＝正控失效")

# ---- 全站 earn 调用点普查：每个点必须有处置，丢弃型必须逐条论证 ----
DISCARD_LEDGER = {
    "community_http/tip.rs":
        "idem 每次带新 uuid（tip.rs:93），且 spend 腿已先闸成 Spent ⇒ "
        "锚定它的 {idem}:to 不可能撞上重放；代码注释写明该论证",
    "economy_http/checkin.rs":
        "idem=attendance:{uid}:{日期}，同事务内先由 ON CONFLICT DO NOTHING "
        "保证首签唯一 ⇒ 走到 earn 时必然是新签，重放不可达",
    "economy_http/loans_apply.rs":
        "idem=loan:{uid}:{新建贷款 id}，id 本次事务才生成 ⇒ 重放不可达",
    "economy_http/loans.rs":
        "扣减之前先在同事务里 EXISTS 预检该 idem，命中即 400「该笔支取已受理」"
        "⇒ 到 earn 时不可能重放；残留撕裂情形跳过入账正是期望行为",
    "games_http/farm_actions.rs":
        "收获的地块在本事务里已锁定且紧接着 DELETE，idem 绑 plot_id ⇒ "
        "同一地块不可能被第二次收获，重放不可达（注释写明显式丢弃）",
    "economy_http/bank_withdraw.rs":
        "idem=withdraw:{存单 id}，与 worker 到期兑付共用同一把键；销单 CAS "
        "与入账同事务 ⇒ 撞 Replayed 说明本息已由兑付腿发过，此处只补销单，"
        "跳过入账才是正确处置（注释写明「有意容忍的重放」）",
}
grep = subprocess.run(["git", "grep", "-n", "earn_spark_tx(", "--", API],
                      capture_output=True, encoding="utf-8",
                      errors="replace")
sites = []
for line in (grep.stdout or "").splitlines():
    m = re.match(r"^(.+?):(\d+):(.*)$", line)
    if not m:
        continue
    f, n, txt = m.group(1), int(m.group(2)), m.group(3)
    if "fn earn_spark_tx" in txt:  # 定义处不是调用点
        continue
    kind = classify(strip_comments(read(f) or ""), n)
    sites.append((f.replace(API + "/", ""), n, kind))
by = {}
for f, n, k in sites:
    by.setdefault(k, []).append("%s:%d" % (f, n))
disc = sorted({f.split(":")[0] for f in by.get("丢弃", [])})
unreg = sorted(set(disc) - set(DISCARD_LEDGER))
stale = sorted(set(DISCARD_LEDGER) - set(disc))
print("earn 调用点=%d  处置分布=%s" % (
    len(sites), {k: len(v) for k, v in by.items()}))
ok("LT7 基数可信（earn 调用点 ≥ 20 个，别测在空集合上）",
   len(sites) >= 20, "基数=%d" % len(sites))
ok("LT8 没有任何调用点对重放「无处置」（本批病形的普查面）",
   not by.get("无处置"), by.get("无处置"))
ok("LT9 丢弃型（let _ =）逐条登记了为什么这枚键不可能重放", not unreg,
   "未登记：%s" % unreg)
ok("LT10 台账没过期条目（丢弃集合与台账两头对得上）", not stale,
   "已不在丢弃集：%s" % stale)
ok("LT11 每条登记理由都写出锚定关系（≥ 24 字，不许空转放行）",
   all(len(v) >= 24 for v in DISCARD_LEDGER.values()),
   "过短：%s" % [k for k, v in DISCARD_LEDGER.items() if len(v) < 24])
ok("LT12 宠物领取是「expect_fresh」而不是丢弃（修法没被改回去）",
   any("pets.rs" in s for s in by.get("expect_fresh", [])),
   "pets.rs 的处置分类=%s" % [k for k, v in by.items()
                              if any("pets.rs" in s for s in v)])

print()
if os.environ.get("LT_SKIP_LIVE"):
    print("（LT_SKIP_LIVE=1：跳过活体层）")
    print("红账：%s" % (FAILURES or "无"))
    sys.exit(1 if FAILURES else 0)
print("=" * 68)
print("二、活体：重放吞产出（打的是 %s）"
      % os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1"))
print("=" * 68)
tok = login()
RUN = "lt" + uuid.uuid4().hex[:10]
KEY_A, KEY_B = RUN + "-A", RUN + "-B"
SQL_ACCRUE = (
    "INSERT INTO arcade_pets (user_id) VALUES (%d) "
    "ON CONFLICT (user_id) DO NOTHING;\n"
    "UPDATE arcade_pets SET energy = 3, pending = 0, "
    "last_tick = now() - interval '6 hours' WHERE user_id = %d;" % (UID, UID))
bal0 = int(psql_stdin("SELECT spark_balance FROM users WHERE id=%d" % UID))
had_row = psql_stdin("SELECT count(*) FROM arcade_pets WHERE user_id=%d"
                     % UID) == "1"
psql_stdin("DROP TABLE IF EXISTS _bk_arcade_pets_lt;\n"
           "CREATE TABLE _bk_arcade_pets_lt AS "
           "SELECT * FROM arcade_pets WHERE user_id=%d;" % UID)
earned_a = earned_b = None
try:
    psql_stdin(SQL_ACCRUE)
    s1, r1 = call("POST", "/games/pet/claim",
                  {"idempotency_key": KEY_A}, tok)
    earned_a = dig(r1, "data", "earned")
    ok("LT13 夹具成立：同一枚键的第一次领取必须真付（earned>0）",
       s1 == 200 and earned_a and earned_a > 0,
       "http=%s earned=%r msg=%s" % (s1, earned_a, dig(r1, "message")))
    pend = psql_stdin("SELECT pending::text FROM arcade_pets "
                      "WHERE user_id=%d" % UID)
    ok("LT13b 付完之后账确实清了（pending=0，不是留着下次一起付）",
       pend == "0", "pending=%s" % pend)

    psql_stdin(SQL_ACCRUE)          # 再攒一笔（同夹具 ⇒ 同额度）
    s2, r2 = call("POST", "/games/pet/claim",
                  {"idempotency_key": KEY_A}, tok)
    msg2 = dig(r2, "message") or ""
    ok("LT14 同一枚键的第二次到达必须被拒（400「已受理」），不许报成功",
       s2 == 400 and "已受理" in msg2,
       "http=%s code=%s msg=%s" % (s2, dig(r2, "code"), msg2[:120]))
    bal_mid = int(psql_stdin("SELECT spark_balance FROM users WHERE id=%d"
                             % UID))
    ok("LT15 被拒那一次没有出账（整笔回滚，不是部分入账后再报错）",
       s2 == 400 and bal_mid == bal0 + earned_a,
       "http=%s 余额=%d 应为=%d" % (
           s2, bal_mid, bal0 + (earned_a or 0)))

    s3, r3 = call("POST", "/games/pet/claim",
                  {"idempotency_key": KEY_B}, tok)
    earned_b = dig(r3, "data", "earned")
    ok("LT16 换新键照常付（证明那笔产出留在原地，没被重放吃掉）",
       s3 == 200 and earned_b and earned_b > 0 and earned_b == earned_a,
       "http=%s earned=%r 第一次=%r" % (s3, earned_b, earned_a))
    led = psql_stdin(
        "SELECT count(*) FROM spark_ledger WHERE idempotency_key IN "
        "('game-pet-claim:%d:%s', 'game-pet-claim:%d:%s')"
        % (UID, KEY_A, UID, KEY_B))
    ok("LT17 本轮专属键只落了 2 笔流水（不多发、不漏发）", led == "2",
       "行数=%s" % led)
finally:
    bal1 = int(psql_stdin("SELECT spark_balance FROM users WHERE id=%d"
                          % UID))
    psql_stdin(
        "UPDATE arcade_pets a SET species = b.species, name = b.name, "
        "level = b.level, exp = b.exp, hunger = b.hunger, energy = b.energy, "
        "pending = b.pending, last_tick = b.last_tick "
        "FROM _bk_arcade_pets_lt b WHERE a.user_id = b.user_id;"
        if had_row else
        "DELETE FROM arcade_pets WHERE user_id=%d;" % UID)
    diff = psql_stdin(
        "SELECT count(*) FROM arcade_pets a JOIN _bk_arcade_pets_lt b "
        "ON a.user_id = b.user_id WHERE a IS DISTINCT FROM b;"
        if had_row else
        "SELECT count(*) FROM arcade_pets WHERE user_id=%d;" % UID)
    psql_stdin("DROP TABLE IF EXISTS _bk_arcade_pets_lt;")
    ok("LT18 复原：root 的宠物行与备份逐列相同（IS DISTINCT FROM 零差异）",
       diff == "0", "差异行数=%s" % diff)
    paid = (earned_a or 0) + (earned_b or 0)
    ok("LT19 余额变动 = 两次真付之和（账实相符，被拒那次没进也没出）",
       bal1 - bal0 == paid, "前=%d 后=%d 断言=%d" % (bal0, bal1, paid))
    print("   本轮探针标识=%s（game 类目里留 2 行流水，按前缀可查）" % RUN)

print()
print("红账：%s" % (FAILURES or "无"))
sys.exit(1 if FAILURES else 0)
