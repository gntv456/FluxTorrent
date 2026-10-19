# -*- coding: utf-8 -*-
"""闸门 DH：付费种子购买链的扣费前置、资金守恒与账面一一对应。

批一百六十（资深会员视角走一遍「看到付费种 → 下载 → 被扣魔力 → 种主入账
→ 站方抽税」这条钱路）。实测结论（现网 16 条已购行全量对账）：
  扣费前置：5 个扣费出口（网页下载 / 凭证下载 / nexusphp 兼容 / 批量 / RSS
            链接）都在 charge_for_download 之前调 ensure_downloadable；
            顺序若被重构打乱，会出现「被停下载权限的人先被扣 500 魔力」。
  资金守恒：买方 -price、种主 +net、magic_pool +tax 三份都来自同一个
            net=price*(100-tax)/100；站内只允许这一把尺子。
  幂等防线：spark_ledger 的 trg_spark_idem 是 RAISE EXCEPTION（不是静默跳过），
            同键重放会整段事务回滚——这才是「连点两次只扣一次」的真闸门；
            torrent_purchases 的主键必须是 (user_id, torrent_id)，否则代码里
            那句 ON CONFLICT DO NOTHING 永远不冲突，已购行能塞进第二行。
  账面：已购行 16 条，买方流水齐、金额等于成交价、无自购留行。
        唯一一条「流水有、已购行没有」的孤儿属于已删号 + 已删种（2026-10-03
        的历史残留，钱与人都已不存在，不构成可行动的洞），故 DH4 只数活体。

正控：DH6 在事务里插一条「有已购行、无买方流水」的假账，必须让 DH4 打红，
      再 ROLLBACK——不然这条判据可能永远不可能红。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dh_paid_chain.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOTDIR, "apps", "api", "src")


def cut(f):
    s = io.open(f, encoding="utf-8", errors="replace").read()
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def rs_files():
    out = []
    for root, _d, names in os.walk(SRC):
        if "target/" in root.replace("\\", "/"):
            continue
        out += [os.path.join(root, n) for n in names if n.endswith(".rs")]
    return out


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:220])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


# ---------- DH1 扣费前置：每个 charge 出口都必须先过 ensure_downloadable ----
exits, oops = [], []
for f in rs_files():
    if f.endswith("torrents/charge.rs"):
        continue
    s = cut(f)
    i_c = s.find("charge_for_download(")
    i_e = s.find("ensure_downloadable(")
    if i_c < 0:
        continue
    exits.append(os.path.relpath(f, ROOTDIR).replace("\\", "/"))
    if i_e < 0 or i_e > i_c:
        oops.append(os.path.relpath(f, ROOTDIR).replace("\\", "/"))
ok("DH1 凡扣魔力都先判可下载（出口 %d 个、违规 %d）" % (len(exits), len(oops)),
   len(exits) >= 4 and not oops, "缺前置：%s ／ 在册：%s"
   % ("、".join(oops) or "-", "、".join(exits)))

# ---------- DH2 抽税尺子只许一份 ----------
tax_files, rulers = [], []
for f in rs_files():
    s = cut(f)
    if "upload_price_tax" in s:
        tax_files.append(os.path.relpath(f, ROOTDIR).replace("\\", "/"))
    if re.search(r"price\s*\*\s*\(100\s*-", s):
        rulers.append(os.path.relpath(f, ROOTDIR).replace("\\", "/"))
ok("DH2 税率读数与净得公式各只有一份（读税 %s、算净 %s）"
   % (tax_files, rulers), len(tax_files) == 1 and len(rulers) == 1,
   "出现第二把尺子＝口径分家")

# ---------- DH3 库内防线：主键形态 + 幂等触发器行为 ----------
pk = psql("SELECT a.attname FROM pg_index i JOIN pg_attribute a "
          "ON a.attrelid=i.indrelid AND a.attnum=ANY(i.indkey) "
          "WHERE i.indexrelid='torrent_purchases_pkey'::regclass "
          "ORDER BY 1")
trg = sval("SELECT count(*) FROM pg_trigger t JOIN pg_class c "
           "ON c.oid=t.tgrelid WHERE t.tgname='trg_spark_idem' "
           "AND c.relname LIKE 'spark_ledger%'")
raises = "RAISE EXCEPTION" in psql("SELECT prosrc FROM pg_proc WHERE "
                                   "proname='spark_ledger_idem_guard'")
parts = sval("SELECT count(*) FROM pg_class WHERE relname LIKE "
             "'spark_ledger_%' AND relkind='r'")
ok("DH3 torrent_purchases 主键=(user_id,torrent_id) 且幂等触发器会 RAISE"
   "（分区触发器 %s/%s）" % (trg, parts),
   pk.split("\n") == ["torrent_id", "user_id"] and raises
   and int(trg or 0) >= int(parts or 0), "主键列=%s RAISE=%s" % (pk, raises))

# ---------- DH4 正常会员的账面一一对应 ----------
# status 语义以**鉴权层**为准：passkey_login.rs:148 `if status >= 2`
# 拦登录、appeal_unban.rs:28 解封写回 0、appeals.rs:76 申诉只认
# `status >= 2`、overview.rs:35 页脚在册数用 `status < 2` ⇒ **<2 正常、
# >=2 被处置（2 停用/3 墓碑/4 封禁）**。被处置会员的账也要对，只是
# 分开数：DH4 数正常会员，DH4b 数被处置会员，任一条非 0 都是洞。
LIVE = ("FROM torrent_purchases p JOIN users u ON u.id = p.user_id "
        "JOIN torrents t ON t.id = p.torrent_id "
        "WHERE u.status < 2 AND t.approval_status = 1")
n_live = sval("SELECT count(*) " + LIVE)
miss_buy = sval("SELECT count(*) " + LIVE + " AND NOT EXISTS (SELECT 1 "
                "FROM spark_ledger l WHERE l.kind='torrent_buy' AND "
                "l.idempotency_key='torrent-buy:'||p.user_id||':'||"
                "p.torrent_id)")
wrong_amt = sval("SELECT count(*) FROM torrent_purchases p "
                 "JOIN users u ON u.id = p.user_id "
                 "JOIN torrents t ON t.id = p.torrent_id "
                 "JOIN spark_ledger l ON l.kind = 'torrent_buy' "
                 "AND l.user_id = p.user_id "
                 "AND l.idempotency_key = "
                 "'torrent-buy:' || p.user_id || ':' || p.torrent_id "
                 "WHERE u.status < 2 AND t.approval_status = 1 "
                 "AND l.amount <> -p.price")
no_sell = sval("SELECT count(*) " + LIVE + " AND t.owner_id <> p.user_id "
               "AND NOT EXISTS (SELECT 1 FROM spark_ledger l WHERE "
               "l.kind='torrent_sell' AND l.user_id=t.owner_id AND "
               "l.idempotency_key='torrent-sell:'||p.user_id||':'||"
               "p.torrent_id)")
self_buy = sval("SELECT count(*) " + LIVE + " AND t.owner_id = p.user_id")
orph = sval("SELECT count(*) FROM spark_ledger l JOIN torrents t "
            "ON t.id=l.ref_id WHERE l.kind='torrent_buy' AND t.approval_"
            "status = 1 AND NOT EXISTS (SELECT 1 FROM torrent_purchases p "
            "WHERE p.user_id=l.user_id AND p.torrent_id=l.ref_id)")
hist = sval("SELECT count(*) FROM spark_ledger l WHERE l.kind="
            "'torrent_buy' AND NOT EXISTS (SELECT 1 FROM torrent_purchases "
            "p WHERE p.user_id=l.user_id AND p.torrent_id=l.ref_id)")
ok("DH4 活体账面守恒对齐（在册 %s：缺买方流水 %s、金额不符 %s、缺卖方入账 "
   "%s、自购留行 %s、花钱却下不了 %s）"
   % (n_live, miss_buy, wrong_amt, no_sell, self_buy, orph),
   all(x == "0" for x in (miss_buy, wrong_amt, no_sell, self_buy, orph))
   and int(n_live or 0) > 0,
   "任一条非 0＝有人付了钱却拿不到，或站方账实不符")
sus_ok = all(x == "0" for x in (
    (sval("SELECT count(*) FROM torrent_purchases p JOIN users u "
          "ON u.id = p.user_id JOIN torrents t ON t.id = p.torrent_id "
          "WHERE u.status >= 2 AND t.approval_status = 1 AND NOT EXISTS "
          "(SELECT 1 FROM spark_ledger l WHERE l.kind='torrent_buy' AND "
          "l.idempotency_key='torrent-buy:'||p.user_id||':'||p.torrent_id)"),
     sval("SELECT count(*) FROM torrent_purchases p JOIN users u "
          "ON u.id = p.user_id JOIN torrents t ON t.id = p.torrent_id "
          "JOIN spark_ledger l ON l.kind='torrent_buy' "
          "AND l.user_id=p.user_id AND l.idempotency_key="
          "'torrent-buy:'||p.user_id||':'||p.torrent_id "
          "WHERE u.status >= 2 AND t.approval_status = 1 "
          "AND l.amount <> -p.price"))))
n_sus = sval("SELECT count(*) FROM torrent_purchases p JOIN users u "
             "ON u.id = p.user_id JOIN torrents t ON t.id = p.torrent_id "
             "WHERE u.status >= 2 AND t.approval_status = 1")
ok("DH4b 被处置会员(>=2)的账同样对齐（在册 %s）" % n_sus, sus_ok,
   "停用/封禁账号的已购行也必须有买方流水且金额相符")

print("   [台账] 全部 torrent_buy 流水里的孤儿（含已删号/已删种）：%s 条"
      "——历史残留，钱与对象都已不存在" % hist)

# ---------- DH5 价格与税率读数 ----------
neg = sval("SELECT count(*) FROM torrents WHERE price < 0")
big = sval("SELECT count(*) FROM torrents WHERE price > 100000000")
tax = sval("SELECT COALESCE((SELECT value::int FROM site_settings WHERE "
           "name='upload_price_tax'),30)")
ok("DH5 价格无负值(=%s)无天文数(=%s)、税率读数 %s 落在 clamp(0,90) 内"
   % (neg, big, tax), neg == "0" and big == "0" and 0 <= int(tax) <= 90, "-")

# ---------- DH6 正控：造一条「已购无流水」的假账，DH4 必须打红 ----------
ctl = psql("BEGIN; SET LOCAL lock_timeout='5s'; "
           "CREATE TEMP TABLE ctl AS SELECT DISTINCT p.user_id AS uid, "
           "t.id AS tid FROM torrent_purchases p JOIN users pu "
           "ON pu.id = p.user_id AND pu.status < 2 JOIN torrents t "
           "ON t.price > 0 "
           "AND t.approval_status = 1 WHERE NOT EXISTS (SELECT 1 "
           "FROM torrent_purchases q WHERE q.user_id=p.user_id AND "
           "q.torrent_id=t.id) LIMIT 1; "
           "INSERT INTO torrent_purchases (user_id, torrent_id, price) "
           "SELECT uid, tid, 500 FROM ctl; "
           "SELECT 'planted='||count(*) FROM torrent_purchases p JOIN ctl "
           "c ON c.uid=p.user_id AND c.tid=p.torrent_id; ROLLBACK;")
planted = "planted=1" in ctl
ok("DH6 正控可植入：事务内确实插进一条假已购行（planted=%s）" % planted,
   planted, "插不进＝DH4 那条判据根本不可能红，属空判据")
chk_red = sval("BEGIN; SET LOCAL lock_timeout='5s'; CREATE TEMP TABLE ctl "
               "AS SELECT DISTINCT p.user_id AS uid, t.id AS tid FROM torrent_"
               "purchases p JOIN users pu ON pu.id = p.user_id "
               "AND pu.status < 2 JOIN torrents t ON t.price > 0 AND t."
               "approval_status = 1 WHERE NOT EXISTS (SELECT 1 FROM "
               "torrent_purchases q WHERE q.user_id=p.user_id AND "
               "q.torrent_id=t.id) LIMIT 1; INSERT INTO torrent_purchases "
               "(user_id, torrent_id, price) SELECT uid, tid, 500 FROM ctl;"
               "SELECT count(*) " + LIVE + " AND NOT EXISTS (SELECT 1 FROM "
               "spark_ledger l WHERE l.kind='torrent_buy' AND "
               "l.idempotency_key='torrent-buy:'||p.user_id||':'||"
               "p.torrent_id); ROLLBACK;")
ok("DH6b 同一条判据在植入假账后确实打红（读数 %s，应为 1）" % chk_red,
   chk_red == "1", "植入假账仍读 0＝判据不承重")

# ---------- DH7 残料：正控事务回滚后不许留行 ----------
left = sval("SELECT count(*) FROM torrent_purchases WHERE price = 500 "
            "AND created_at > now() - interval '5 minutes'")
ok("DH7 正控残料清零（近 5 分钟新插入的假已购行 %s）" % left, left == "0",
   "闸门不许往现网留数据")

# ---------- DH8 付费下载的余额错误码（批一百六十八 P-401） ----------
# 付费下载是经济操作，余额不足必须回 4001/409（`spend_spark` 同一把尺，
# errors.rs 的注释也写着「购买/抽卡/打赏…余额不足」）。旧实现回的是
# `Validation(format!("魔力不足：该种子为付费种子…"))` ⇒ HTTP 400 + code 1002
# 「参数校验失败」，等于告诉会员「你请求发错了」，而真原因是钱不够；且那句是
# 动态 format! 串（英文/繁体用户看简体），前端只对 4001 才给得出充值引导。
# 实测 A/B：旧镜像 400/1002 → 新二进制 409/4001（发布者豁免口两边都 200）。
BAD8 = re.compile(r'Validation\(\s*format!\(\s*"魔力不足')
CHARGE = os.path.join(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))), "apps", "api", "src", "torrents", "charge.rs")
ch8 = io.open(CHARGE, encoding="utf-8", errors="replace").read()
ok("DH8 付费下载余额不足走 4001/409，不再伪装成参数错误",
   "InsufficientSpark" in ch8 and BAD8.search(ch8) is None,
   "回 1002/400＝把「钱不够」报成「请求发错」，前端的充值引导也不触发")
OLD8 = ('if balance < price {\n    return Err(DomainError::Validation(\n'
        '        format!("魔力不足：该种子为付费种子（{price} 魔力）")));}')
ok("DH8b 承重自证：旧的 1002 写法必须判红", BAD8.search(OLD8) is not None,
   "把旧码喂进判据仍算绿＝这条不承重")

summary()
