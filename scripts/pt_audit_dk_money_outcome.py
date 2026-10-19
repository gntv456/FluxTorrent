# -*- coding: utf-8 -*-
"""闸门 DK：资金原语的返回结果必须被消费（SpendOutcome 不许静默丢弃）。

批一百六十三 P-347。`cargo build` 之前一直挂着 3 条
`unused spend::SpendOutcome that must be used`：
  games_http/casino.rs:79（拆文件后在 casino_bigsmall.rs）
  casino.rs 奖金入账结果丢掉 → 下面照写 arcade_pool_rounds
  games_http/casino.rs:272       大小竞猜道具加成入账结果丢掉 → 回显 payout
  payment/tiers.rs:121          法币捐赠档位魔力入账结果丢掉 → 回执拼「魔力 +N」
三条的现实路径都被上游的 spend/claim CAS 闸住（必然 Spent），所以不是现网
错账；但**没人断言**——一旦有人把上游那道闸挪走或改了幂等键，就会变成
「账面与回执都说发了、钱没进账」的静默缺（这正是本仓反复出现的 P-323 /
P-326 / P-345 家族形状）。现在四处（含 arcade_rewards.rs:173）一律
`if !matches!(…, SpendOutcome::Spent) { return Err(LedgerConflict) }`，
fail-close：宁可整笔回滚，也不发空头回执。

判据：
  DK1 全仓普查资金原语的**裸调用**（语句以 `xxx_spark(_tx)(…).await?;` 结尾
      且前面没有 matches!/let/绑定）＝违例，必须 0，并报出总调用数当基数
      （命中 0 与基数 0 是两回事）；
  DK2 承重自证：同一个检测器喂「旧形状」与「新形状」两段合成代码，
      必须只抓得住旧的、放得过新的；
  DK3 四处修形钉：casino×2 / tiers / arcade_rewards 里都要有
      `SpendOutcome::Spent` 判定 + LedgerConflict 回落；
  DK4（可选，慢）编译级复跑：DK_FULL=1 时 cargo build 并断言
      "must be used" 告警数＝0——静态普查只是编译器的近似，这里收口。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dk_money_outcome.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
APPS = os.path.join(ROOTDIR, "apps")
# 只有返回 SpendOutcome 的三只有「结果要不要消费」的问题：
# earn_spark（非 tx 版）返回 DomainResult<()>，没得消费（spend.rs:153）
CALL = re.compile(r"spend_spark(_tx)?\(|earn_spark_tx\(")
# 语句起点：到最近的分号/花括号/换行为止
HEAD = re.compile(r"[;{}][^;{}]*$")
ACK_BASELINE = 2
FIXED = [
    os.path.join(APPS, "api", "src", "games_http", "casino.rs"),
    os.path.join(APPS, "api", "src", "games_http", "casino_bigsmall.rs"),
    os.path.join(APPS, "api", "src", "payment", "tiers.rs"),
    os.path.join(APPS, "api", "src", "games_http", "arcade_rewards.rs"),
]


def rs_files():
    out = []
    for root, _d, names in os.walk(APPS):
        rl = root.replace("\\", "/")
        if "/target/" in rl or "/node_modules/" in rl:
            continue
        out += [os.path.join(root, n) for n in names if n.endswith(".rs")]
    return out


def strip_comments(s):
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


NL = chr(10)


def violations(text):
    """[(行号, 片段)]：资金原语的返回值被静默丢弃的调用点。

    形状判据（够覆盖本仓四种写法，宁保守不误报）：
      ① 往上找最近的非空代码行，若出现 matches!/let/=/? 绑定 ⇒ 结果被消费；
      ② 否则往下找语句收尾，若以 `.await?;` 或 `.await);` 收尾且中间没有
         任何使用（=> / 变量名）⇒ 返回值被丢掉。
    """
    lines = text.split(NL)
    hits, acks, total = [], [], 0
    consumed = re.compile(r"matches!|let\s|=[^=]|\?\s*$|->\s")
    for i, ln in enumerate(lines):
        if not CALL.search(ln) or re.search(
                r"pub (async )?fn (earn|spend)_spark", ln):
            continue
        stripped = ln.strip()
        if stripped.startswith(("///", "//", "use ")):
            continue
        total += 1
        # `let _ =` 是「显式承认丢弃」——不算违例，但要进台账棘轮
        if stripped.startswith("let _ ="):
            acks.append((i + 1, stripped[:70]))
            continue
        up = NL.join(x for x in lines[max(0, i - 6):i] if x.strip())
        if consumed.search(up) or consumed.search(stripped):
            continue
        down = lines[i:i + 14]
        for d in down:
            dd = d.strip()
            if not dd or dd.startswith("//"):
                continue
            if re.search(r"(\.await\??;|\.await\);)\s*$", dd):
                hits.append((i + 1, stripped[:70]))
                break
            if "=>" in dd or re.search(r"\w+\s*=\s*$", dd):
                break
    return total, hits, acks


# ---------- DK1 全仓普查 ----------
files = rs_files()
all_hits, all_acks, grand = [], [], 0
for f in files:
    t, h, a = violations(strip_comments(io.open(f, encoding="utf-8",
                                 errors="replace").read()))
    grand += t
    all_acks += [(os.path.relpath(f, ROOTDIR).replace("\\", "/"), x, y)
                 for x, y in a]
    all_hits += [(os.path.relpath(f, ROOTDIR).replace("\\", "/"), p, c)
                 for p, c in h]
ok("DK1 资金原语调用 %d 处里裸丢弃返回值 = %d（须 0）"
   % (grand, len(all_hits)),
   grand >= 20 and not all_hits,
   "普查覆盖 %d 个 .rs；违例：%s" % (
       len(files), "; ".join("%s:%s %s" % x for x in all_hits[:6]) or "-"))

# ---------- DK2 承重自证：检测器必须抓得住旧形状 ----------
OLD = '    if spark > 0 {\n        earn_spark_tx(tx, uid, spark, "game",' \
      ' win_idem).await?;\n    }\n'
NEW = '    if spark > 0 {\n        if !matches!(\n            ' \
      'earn_spark_tx(tx, uid, spark, "game", win_idem).await?,\n' \
      '            SpendOutcome::Spent\n        ) {\n            return ' \
      'Err(DomainError::LedgerConflict);\n        }\n    }\n'
t_old, h_old, a_old = violations(OLD)
t_new, h_new, a_new = violations(NEW)
ok("DK2 检测器承重：旧形状抓到 %d 处（须 1）、新形状放过 %d 处（须 0）"
   % (len(h_old), len(h_new)), len(h_old) == 1 and not h_new,
   "抓不住旧形状＝DK1 那条红永远不可能出现（空判据）")

# ---------- DK3 四处修形钉 ----------
weak = []
for f in FIXED:
    s = strip_comments(io.open(f, encoding="utf-8", errors="replace").read())
    if "SpendOutcome::Spent" not in s or "LedgerConflict" not in s:
        weak.append(os.path.basename(f))
ok("DK3 发放路径都改成「非 Spent 就整笔回滚」（退化 %d）" % len(weak),
   not weak, "、".join(weak) or "-")
ok("DK2b 显式丢弃（`let _ =`）的数量不涨：%d 处（基线 %d）"
   % (len(all_acks), ACK_BASELINE), len(all_acks) <= ACK_BASELINE,
   "新增静默丢弃＝发钱口少了一道「到底到账没」的追问")
print("   [台账] 显式 `let _ =` 丢弃 SpendOutcome：%d 处（%s）"
      % (len(all_acks), "; ".join("%s:%s" % (x[0].split("/")[-1], x[1])
                                  for x in all_acks[:8]) or "-"))

ok("DK3b 反向钉：自愿转账口仍要求收件方在册（tip/medal_gift 的 status<2 "
   "不许被抹平）",
   all(("status < 2" in strip_comments(io.open(f, encoding="utf-8",
             errors="replace").read()) or "st < 2" in strip_comments(
        io.open(f, encoding="utf-8", errors="replace").read()))
        for f in [os.path.join(APPS, "api", "src", "community_http",
                               "tip.rs"),
                  os.path.join(APPS, "api", "src", "community_http",
                               "medal_gift.rs")]), "-")

# ---------- DK4 可选：编译级收口 ----------
if os.environ.get("DK_FULL") == "1":
    r = subprocess.run(["cargo", "build", "--bins", "--message-format=short"],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", cwd=ROOTDIR)
    warns = (r.stdout or "").count("must be used")
    ok("DK4 编译级：`must be used` 告警 %d 条（须 0，rc=%s）" % (warns,
                                                                 r.returncode),
       r.returncode == 0 and warns == 0, "静态普查只是近似，编译器才是原判据")
else:
    print("   [说明] DK4 编译级复跑默认跳过（要跑：DK_FULL=1 python -u "
          "本脚本）；本轮实测 `cargo build --bin flux-api --bin flux-worker` "
          "rc=0 且该告警数已从 3 降到 0")

# ---------- DK7 发放结果被吞的台账（批一百七十四新增，只降不升） ----------
# 形状：`let _ = earn_spark…(…)` —— 发放失败被整条吞掉，会员少收魔力这件事
# 在站点里既无日志也无账面差（P-323/P-328 同族）。like.rs 那处已在本批改成
# 出声；剩下的登记在此，谁修掉一处就把清单缩一行，**新增一条就红**。
# 形状：`let _ = earn_spark…(…)` 且**语句结尾没有 `?`** —— 带 `?` 的那些
# （subtitles_cancel 的退款、farm_actions 的收获派彩）其实会把错误向上抛，
# 丢弃的只是 `SpendOutcome`/`()`，属 DK2b/DK4b 管的「丢弃结果」而不是「吞掉失败」。
# 批一百七十四初版把两种混在一起数，虚报了 6 处；本轮分清后现网真吞只有下面这些。
# 谁修掉一处就把清单缩一行，**新增一条就红**。
LEDGER = {
    # P-365 同族：负数走 earn_spark（回收 +1 惩罚），修法属资金口径，待站方裁
    "apps/api/src/community_http/post_edit.rs": 1,
    # P-365 同族两条：删主题扣 2 + 级联回收回帖各扣 1
    "apps/api/src/community_http/post_topic_delete.rs": 2,
}
found = {}
_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
for root, _, fs in os.walk(os.path.join(_root, "apps", "api", "src")):
    for f in fs:
        if not f.endswith(".rs"):
            continue
        p = os.path.join(root, f)
        t = io.open(p, encoding="utf-8", errors="replace").read()
        t = "\n".join(re.sub(r"//.*", "", ln) for ln in t.split("\n"))
        n = 0
        for m in re.finditer(
                r"let _ =\s*(?:earn_spark|earn_spark_tx|spend_spark"
                r"|spend_spark_tx)\(", t):
            # 只看「真吞」：语句到第一个 `;` 为止，结尾不是 `?` 才算没抛出去
            seg = t[m.start():m.start() + 400].split(";")[0]
            if not seg.rstrip().endswith("?"):
                n += 1
        if n:
            found[os.path.relpath(p, _root).replace("\\", "/")] = n
new_on = {k: v for k, v in found.items() if k not in LEDGER}
grew = {k: (v, LEDGER[k]) for k, v in found.items()
        if k in LEDGER and v > LEDGER[k]}
ok("DK7 发放结果被吞的站点 %d 处，必须等于在册 %d 处（新增 %s，变多 %s）"
   % (sum(found.values()), sum(LEDGER.values()),
      sorted(new_on) or "无", sorted(grew) or "无"),
   not new_on and not grew,
   "每吞一次发放，「会员该收到却没收到」就少一个可查证的出口；"
   "修掉请把台账与 `tracing::warn!` 一起改（对照 like.rs 的 P-363 形状）")
for k in sorted(LEDGER):
    print("      [在册] %s：%d 处（现读数 %d）" % (k, LEDGER[k],
                                                  found.get(k, 0)))

# ---------- DK8 余额地板：负余额只降不升（批一百七十五新增） ----------
# 现网 25 个会员 spark_balance < 0，而 `users` 上**没有 spark_balance 的 CHECK**；
# 成因之一是可确定的一条：扣罚走的是发放原语 `earn_spark(…, -2, "forum-topic-del", …)`
# （post_topic_delete.rs），`checked_balance` 只防 int64 溢出、不防负数，
# `UPDATE users SET spark_balance = spark_balance + $2` 也没有地板 ⇒ 绕过
# `spend_spark` 的余额充足校验，可以一路扣成负数。
# 修法是口径问题（夹到 0 / 拦住动作 / 允许欠账），要站方拍板 + 一次性处置存量 25 人，
# 所以这里钉成**只降不升的上界**：新增一个负余额会员就红，处置完应收敛到 0。
def _dk_one(sql):
    import subprocess
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-q", "-d", "fluxtorrent", "-tA", "-c", sql],
        capture_output=True, text=True, encoding="utf-8")
    return (r.stdout or "").strip().split("\n")[0].strip()


if "sval" not in globals():          # 本闸门原自带取数器名不同，这里补一个别名
    sval = _dk_one
NEG_CEIL = 25
neg = int(sval("SELECT count(*) FROM users WHERE spark_balance < 0") or 0)
ok("DK8 余额为负的会员 %d ≤ 上界 %d（只降不升）" % (neg, NEG_CEIL),
   neg <= NEG_CEIL,
   "上升＝又有一条扣费路径绕过 spend 的充足校验（对照 earn_spark 收负数那一族）")
chk = sval("SELECT count(*) FROM pg_constraint WHERE "
           "conrelid = 'users'::regclass "
           "AND pg_get_constraintdef(oid) LIKE '%spark_balance%'")
print("   [在册] users.spark_balance 的库层 CHECK：%s 条（0＝地板只写在应用层）"
      % chk)

# ---------- DK9 「只判 Err、不判 Replayed」的扣款口（台账棘轮，只降不升） ----------
# DK1 数的是「结果被整个丢弃」（`spend_spark(…).await?;` 没人接）。这里补它
# 的另一半盲区：`if let Err(e) = spend_spark(…)` **看着像消费了**，其实放过了
# `Ok(Replayed)`——即「幂等键还在、业务占位行已被删掉」那一族（P-379 实测的
# 白票形状）。现网 spark_ledger.idempotency_key 有全局唯一约束，而
# admin_http/user_del.rs 清号会删 thanks/offer_votes/fun_votes/task_claims 等
# 占位行、从不删流水 ⇒ 复活后对同一目标再操作一次就撞进来。
# 正解不是加判断就完事：键必须锚「这一行」（P-380），否则白票翻成永久死端。
# 所以这一条按**在册数**钉，修一处降一处，不许新增。
LEAKY = re.compile(r"if let Err\([^)]*\)\s*=\s*(?:crate::)?(?:economy_http::)?"
                   r"spend_spark\(")
leaky_files = []
for _p in sorted(rs_files()):
    _t = strip_comments(io.open(_p, encoding="utf-8", errors="replace").read())
    if LEAKY.search(_t):
        leaky_files.append(os.path.relpath(_p, ROOTDIR).replace("\\", "/"))
DK9_CEIL = 3
ok("DK9 「只判 Err 不判 Replayed」的扣款口 %d ≤ 上界 %d（只降不升）"
   % (len(leaky_files), DK9_CEIL),
   len(leaky_files) <= DK9_CEIL,
   "新增一处＝多一条可被清号复活踩中的白拿口：%s" % leaky_files)
print("   [在册] DK9 待修清单（每处照 P-379+P-380 一起改：同事务 + 键锚行）：")
for f in leaky_files:
    print("      · %s" % f)

summary()
