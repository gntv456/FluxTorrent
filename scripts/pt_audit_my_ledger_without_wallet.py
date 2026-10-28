#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""MY：往 `spark_ledger` 写行的每一处，必须同一文件动钱包，且不许静默吞。

触发这件闸门的是 P-462（2026-10-28，月度契约 job）：
`apps/worker/src/jobs/contract.rs` 的奖罚入账写成两条
`let _ = sqlx::query("INSERT INTO spark_ledger …")`——只落流水、
**不 UPDATE `users.spark_balance`**，而全库 spark_ledger（父表 + 各月分区）上
唯一的触发器是幂等护栏 `trg_spark_idem`（现网实查：父表与各月分区都只有它）。
钱不是「永久不到账」——jobs/seeding.rs:141-149 每小时把快照重算成
`balance_baseline + SUM(amount)`（只覆盖 2h 内有过流水的用户，排除
orphan_offset），下一拍会补上。真正的缺陷是那之前那条 `balance_after` 是个钱包
里还不存在的数（Merkle / 对账读的就是这一列；同族 preserve.rs 干脆不写这一列，
正是为了不造这个数），加上失败被 `let _ =` 吞成零日志、零账面，以及罚金门槛
读的是可能滞后一小时的快照。罚侧更隐蔽：
`WHERE u.spark_balance >= $2` 不满足时 INSERT 恒 0 行 = 罚金永久不收，而契约
状态早已翻成 breached——「状态位先于动账」的同一条病形，被 `let _ =` 演成
零日志、零账面。

为什么既有门没拦住：DW 的 kind 尺读的是「原语调用点 + 现网已有行」，
裸 SQL 造的新 kind 在第一次跑成之前库里没有行 ⇒ 它在册；K1 同样只扫原语。
所以这把尺量的不是「有没有行」，而是**代码里每一处写流水的语句形状对不对**。

口径说明：钱包判据按「同文件」粒度判，不按同事务粒度——后者要解析 Rust 函数体
边界，收益不值当把尺子做瞎。真要拆事务，先把这个文件的事务边界拿 AST 量出来。
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTS = ["apps/api/src", "apps/worker/src"]
LANGS = ["zh-CN", "zh-TW", "en", "ja"]
# 台账（多一个少一个都红）：违例文件 -> 为什么不判
WALLET_LEDGER = {
    # [UNADJUDICATED] same shape as P-462, NOT yet proven broken: the file
    # writes spark_ledger rows (and omits balance_after) with no wallet
    # UPDATE in the same file. Next round must first read the settlement
    # side (does anything else credit spark_balance for this kind?) before
    # turning either into a fix or a permanent exemption with a reason.
    "apps/worker/src/jobs/preserve.rs":
        "刻意只写流水：余额快照由 jobs/seeding.rs:141-149 按 "
        "baseline+SUM(amount) 每小时重算（2h 内有过流水的用户），"
        "且本文件不填 balance_after，不留与钱包脱节的数（P-462 定性）",
    "apps/worker/src/jobs/seeding_referral.rs":
        "同 preserve：邀请分润只落流水，余额走 seeding 重算那一拍；"
        "本文件同样不写 balance_after ⇒ 不会留下造假的余额链",
}
SWALLOW_LEDGER = {}  # 收紧成同语句判定后：全仓零违例（MY1 正控证明尺会响）

# 裸 SQL kind 里由别的语言文件负责登记的例外
KIND_EXEMPT = set()

INSERT = re.compile(r"INSERT\s+INTO\s+spark_ledger", re.I)
WALLET = re.compile(r"UPDATE\s+users[\s\S]{0,240}?spark_balance\s*=", re.I)
# `let _ =` 是否吞掉了写流水：**只看这一条语句自己**。
# 第一版用 1500 字符窗口，从 `let _ = sqlx::query("INSERT INTO messages
# …")` 一路够到隔壁事务里的 spark_ledger 插入，把 settle.rs 读成违例——
# 那条流水其实在 db.begin()…tx.commit() 里，被吞的是站内信不是钱。
LET_UNTIL_SEMI = re.compile(r"let\s+_\s*=\s*sqlx::query\(", re.I)
SQL_TAIL_LIMIT = 4000
STMT = re.compile(r"INSERT\s+INTO\s+spark_ledger[\s\S]{0,900}?[\"#]", re.I)
LIT = re.compile(r"'([a-z][a-z0-9_]{2,})'")


def strip_comments(text):
    """逐行剥 `//` 与 `--`：先跳过引号内内容，免得把 "https://…" 当注释吃掉半行。

    第一版只认 `--`（SQL 味），结果 Rust 注释里的 `let _ = sqlx::query("INSERT
    INTO spark_ledger …")` 这句**讲解旧代码的注释**被当成真语句，把两条尺同时
    点红——尺子读注释，红的是解释而不是缺陷。
    """
    out = []
    for line in text.splitlines():
        i, quote = 0, False
        while i < len(line) - 1:
            c = line[i]
            if c == '"':
                quote = not quote
            elif not quote and ((c == "/" and line[i + 1] == "/")
                                or (c == "-" and line[i + 1] == "-")):
                line = line[:i]
                break
            i += 1
        out.append(line)
    return "\n".join(out)


def bodies():
    for d in ROOTS:
        if not os.path.isdir(d):
            continue
        for dp, _dn, fns in os.walk(d):
            for fn in sorted(fns):
                if fn.endswith(".rs"):
                    # 路径统一成正斜杠：os.walk 在 Windows 给反斜杠，而台账键
                    # 是按仓库习惯写的前斜杠——不归一会让「实测=台账」永远不等
                    # （集合内容一致却判红，第一跑就是这么红的）。
                    p = os.path.join(dp, fn).replace(chr(92), "/")
                    yield p, strip_comments(
                        open(p, encoding="utf-8",
                             errors="replace").read())


files = dict(bodies())
no_wallet = sorted(p for p, b in files.items()
                   if INSERT.search(b) and not WALLET.search(b))
def statement_body(text, start):
    """从 `let _ = sqlx::query(` 之后扫到第一个「引号外分号」——一条语句。"""
    i, quote = start, False
    while i < len(text) and i - start < SQL_TAIL_LIMIT:
        c = text[i]
        if c == '"':
            quote = not quote
        elif c == ";" and not quote:
            return text[start:i]
        i += 1
    return text[start:i]


def swallows_statement(body):
    for m in LET_UNTIL_SEMI.finditer(body):
        if INSERT.search(statement_body(body, m.end())):
            return True
    return False


swallowed = sorted(p for p, b in files.items() if swallows_statement(b))
stmt_count = sum(len(INSERT.findall(b)) for b in files.values())

raw_kinds = set()
for b in files.values():
    for m in STMT.finditer(b):
        for lit in LIT.findall(m.group(0)):
            if lit not in ("users", "spark_ledger", "bigint", "text", "true",
                           "false", "public"):
                raw_kinds.add(lit)

labels = {}
for lang in LANGS:
    labels[lang] = open(os.path.join("apps", "web", "i18n", "%s.ts" % lang),
                        encoding="utf-8", errors="replace").read()


def labelled(k):
    return all(re.search(r"(?m)^\s*%s\s*:" % re.escape(k), v)
               for v in labels.values())


ghost = ('async fn ghost(db: &sqlx::PgPool) {\n'
         '    let _ = sqlx::query("INSERT INTO spark_ledger '
         '(user_id, amount, kind, balance_after) VALUES ($1, $2, '
         "'my_probe_ghost_kind', 0)\").execute(db).await;\n}\n")
paired = ghost.replace(
    ").execute(db).await;",
    ").execute(db).await;\n    sqlx::query(\"UPDATE users SET "
    "spark_balance = spark_balance + $2 WHERE id = $1\")"
    ".execute(db).await;")
ok("MY1 正控：合成件须同时被两条尺点名（无钱包 UPDATE + let _ = 吞）",
   INSERT.search(ghost) and not WALLET.search(ghost)
   and swallows_statement(ghost), "吞尺=%s" % bool(swallows_statement(ghost)))
ok("MY1b 正控（反向）：同一件补上钱包 UPDATE 后，钱包尺必须转绿",
   WALLET.search(paired) and INSERT.search(paired), "")
ok("MY1c 正控：探针 kind 若四语无译名，kind 尺必须抓到它（不是恒绿）",
   not labelled("my_probe_ghost_kind"), "")

ok("MY2 「写流水的文件必须同文件动钱包」违例 = 台账 %s"
   % sorted(WALLET_LEDGER), set(no_wallet) == set(WALLET_LEDGER),
   "实测=%s" % no_wallet)
ok("MY3 「let _ = 吞掉 spark_ledger 写入」违例 = 台账 %s"
   % sorted(SWALLOW_LEDGER), set(swallowed) == set(SWALLOW_LEDGER),
   "实测=%s" % swallowed)
miss = sorted(k for k in raw_kinds if not labelled(k))
# MY4 只钉「本轮新造的两个 kind 必须四语在册」这一条可证的回归。
# 裸 SQL 里其余字面量要变成断言，得先按列表位置解析 INSERT 的第 4 列
# （`kind`）——现在这把窗口尺会把 'torrent'（ref_type）、
# 'spark_ledger_id_seq'（nextval）一起捞进来，硬断言就是假红。
# 所以 miss 里除这两个之外的项**只打印不判**，留给下一轮做位置解析。
mine = {"contract_reward", "contract_penalty"}
ok("MY4 回归钉：P-462 新增的两个 kind 四语齐（删译名即红）",
   all(labelled(k) for k in mine), "缺=%s" % sorted(
       k for k in mine if not labelled(k)))
print("   [账面外] 裸 SQL 字面量里四语无译名者（含非 kind 噪声，"
      "待位置解析后才能判）：%s" % miss)
ok("MY5 反空转：全仓 INSERT INTO spark_ledger 语句数须 > 10",
   stmt_count > 10, "语句数=%d 文件数=%d" % (stmt_count, len(files)))

print("   [实况] 语句 %d 处；缺钱包尺违例=%s 吞失败=%s 裸 SQL kind=%s"
      % (stmt_count, no_wallet, swallowed, sorted(raw_kinds)))
summary()
