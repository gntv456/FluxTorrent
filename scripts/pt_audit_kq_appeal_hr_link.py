# -*- coding: utf-8 -*-
"""KQ H&R 申诉受理的联动与回执（批七轮，2026-10-25 深测）。

缺陷本体：`POST /admin/appeals/handle` 对 kind='ban' 走 0209 P1-8/0285 的
「受理即联动解封」，而对 kind='hr' 只翻 appeals.status 并发一句「已受理」——
那条 H&R 仍是 violated、`snatches.hr_flag` 仍为 TRUE，会员照旧被挡在下载口外。
另一半在同一枚靶子上：「我的 H&R」卡片只把会员递到 /appeals，界面送出的
载荷**不带条目号**（`appeals/page.tsx` 只拼 kind/body），于是职员看到的每条
H&R 申诉都不知道指哪一条，想联动也没有目标。

修复两处（同一口径，不新增机制）：
  ① 建单侧：kind='hr' 且未给条目号时，**仅当该会员恰好只有一条 violated**
     才替他把 torrent_id 补上；多条留空——宁可让职员去问，也不替会员猜。
  ② 受理侧：accept + kind='hr' + 有靶子 ⇒ 走共用判据
     `services::hr_pardon::pardon_one`（与三个赦免口同一套护栏：只赦 violated、
     等级需严格高于目标、理由进审计与留痕），并且**早于**状态翻转；
     回执文案按真做了什么事分档，没撤销就不许说「已解除」。

  KQ0 pardon_one 早于 UPDATE appeals SET status（与解封同一前置）
  KQ1 SELECT 取回 a.kind 与 a.ref_id（联动得先看见靶子）
  KQ2 联动条件锁死在 accept + kind == "hr"（不顺手扩到 warn/other）
  KQ3 走共用判据 pardon_one，不在申诉里另写一份 UPDATE hr_snapshots
  KQ4 回执三档：撤销成功说「已一并撤销」；无靶子说「没有附带可撤销的记录」
  KQ5 响应带 hr_pardoned（界面与闸门都读得到，不许只靠文案猜）
  KQ6 建单侧只在 given.is_none() 时补号（界面将来带号也不被覆盖）
  KQ7 补号谓词只认 status = 'violated'（进行中的 open 义务不是违规）
  KQ8 补号判据是 rows.len() == 1（多条留空）
  KQ9 真数据正控：现网「恰好一条 violated」的会员在该 SELECT 下恰好回 1 行
  KQ10 真数据负控：现网「两条以上 violated」的会员回 ≥2 行 ⇒ len==1 若不成立，
       补号会指向 ORDER BY created_at DESC 的第一条，即替会员猜
  KQ11 两段联动并存：ban 的解封联动没被 hr 联动挤掉（同一函数里两把钥匙）

九轮把同一口径推到最后一种：kind='warn' 的受理此前也只翻状态——
`users.warned_until/warned_reason` 原样留着，后台「警告用户」面板
（`GET /admin/warned` 只列 `warned_until > now()`）与管理端用户详情的警告条
继续把当事人当背着警告的人。撤销走 `appeals/unwarn.rs`（与 appeal_unban
同形状：先 ensure_outranks，只清 warned_until IS NOT NULL 的行，动作进审计
appeal.unwarn），同样**早于**状态翻转；回执按真做的事分档，响应加 warn_lifted。

  KQ20 撤销警告早于申诉状态翻转      KQ21 联动锁死 accept + kind=="warn"
  KQ22 走 ensure_outranks 且带 IS NOT NULL   KQ23 审计名 appeal.unwarn
  KQ24 警告回执两档                 KQ25 响应带 warn_lifted
  KQ26 源码抠出撤销 UPDATE；真语句清「有警告」的行回 1 行
  KQ27 真语句对「本就没警告」的行回 0 行（不谎称撤过）
  KQ28 负控：去掉 IS NOT NULL 后同一空行仍算撤了一条（判据承重）
  KQ29 事务回滚后两枚夹具原样

跑法（默认读源码，不写库；KQ9~KQ11 与 KQ26~KQ29 只读现网夹具或在事务里
自造自滚，不留痕）：
    python scripts/pt_audit_kq_appeal_hr_link.py
"""
import io
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
APPEALS = os.path.join(ROOT, "apps", "api", "src", "gaps_http", "appeals.rs")
CREATE = os.path.join(ROOT, "apps", "api", "src", "gaps_http",
                      "appeals", "create.rs")

FAIL = []
PASS = 0


def ok(name, cond, detail=""):
    global PASS
    if cond:
        PASS += 1
        print("[PASS] %s%s" % (name, ("  " + detail) if detail else ""))
    else:
        FAIL.append(name)
        print("[FAIL] %s  %s" % (name, detail))


def read(path):
    return io.open(path, encoding="utf-8", errors="replace").read()


def strip_comments(src):
    """剥掉行注释与 // 文档注释——否则我自己写的说明会替实现作证。"""
    return "\n".join(l for l in src.splitlines()
                     if not l.strip().startswith("//"))


def body_of(src, fn):
    """取函数体：`pub async fn`、`async fn`、`pub(crate) fn` 都得认。"""
    i = src.find("fn " + fn + "(")
    if i < 0:
        return ""
    j = src.find("\n}", i)
    return src[i:j] if j > i else src[i:]


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-v", "ON_ERROR_STOP=1", "-tA", "-F", "|"],
        input=sql, capture_output=True, text=True, encoding="utf-8",
        errors="replace")
    err = (r.stderr or "").strip()
    if r.returncode != 0 or "ERROR" in err:
        raise RuntimeError("psql 失败：%s" % err[:400])
    return (r.stdout or "").strip().splitlines()


def sole_select_sql():
    """从源码里抠出建单侧那条补号 SELECT（不抄一份，抄了就测不到搬家）。"""
    src = strip_comments(read(CREATE))
    m = re.search(r'"(SELECT torrent_id FROM hr_snapshots[\s\S]{0,300}?)"',
                  src)
    if not m:
        return ""
    return re.sub(r"\\\s*\n\s*", " ", m.group(1))


def finish():
    print("\n%d 条通过 / %d 条失败" % (PASS, len(FAIL)))
    if FAIL:
        print("失败：" + "、".join(FAIL))
        sys.exit(1)
    sys.exit(0)


def main():
    ap = strip_comments(read(APPEALS))
    handle = body_of(ap, "appeal_handle")
    fb = re.sub(r"\\\s*\n\s*", " ", handle)
    create = strip_comments(read(CREATE))
    sole = body_of(create, "sole_violated_hr")

    par = handle.find("pardon_one(")
    upd = handle.find("UPDATE appeals SET status")
    ok("KQ0 pardon_one 早于申诉状态翻转", 0 <= par < upd,
       "pardon@%d update@%d" % (par, upd))
    ok("KQ1 SELECT 取回 kind 与 ref_id",
       "a.kind" in fb and "a.ref_id" in fb, "")
    ok("KQ2 联动条件锁死 accept + kind==\"hr\"",
       bool(re.search(r"if\s+body\.accept\s+&&\s+kind\s*==\s*\"hr\""
                      r"\s*\{", handle)))
    ok("KQ3 走共用判据而非另写 UPDATE",
       "crate::services::hr_pardon::pardon_one(" in handle
       and "UPDATE hr_snapshots" not in handle)
    ok("KQ4 回执按真做的事分档",
       "对应那条 H&R 违规已一并撤销" in handle
       and "本次受理没有附带可撤销的 H&R 记录" in handle)
    ok("KQ5 响应带 hr_pardoned",
       '"hr_pardoned": hr_pardoned' in handle)
    ok("KQ6 建单侧不覆盖界面带来的条目号",
       bool(re.search(r"if\s+kind\s*!=\s*\"hr\"\s*\|\|\s*given\.is_some"
                      r"\(\)\s*\{\s*return\s+Ok\(given\)", sole)))
    ok("KQ7 补号谓词只认 violated",
       "status = 'violated'" in sole and "status = 'open'" not in sole)
    ok("KQ8 补号判据是恰好一条",
       bool(re.search(r"rows\.len\(\)\s*==\s*1", sole)),
       "多条 ⇒ None，不替会员猜")

    sql = sole_select_sql()
    ok("KQ9a 能从源码抠出补号 SELECT", bool(sql), sql[:120])
    if not sql:
        return finish()
    one = sql.replace("$1", "(SELECT user_id FROM hr_snapshots WHERE "
                            "status = 'violated' GROUP BY user_id HAVING "
                            "count(*) = 1 LIMIT 1)")
    two = sql.replace("$1", "(SELECT user_id FROM hr_snapshots WHERE "
                            "status = 'violated' GROUP BY user_id HAVING "
                            "count(*) >= 2 ORDER BY count(*) DESC LIMIT 1)")
    r_one = psql("WITH q AS (%s) SELECT 'ONE|' || count(*) FROM q;" % one)
    r_two = psql("WITH q AS (%s) SELECT 'TWO|' || count(*) FROM q;" % two)
    n1 = [x for x in r_one if x.startswith("ONE|")]
    n2 = [x for x in r_two if x.startswith("TWO|")]
    got1 = n1[0].split("|")[1] if n1 else "-"
    got2 = n2[0].split("|")[1] if n2 else "-"
    ok("KQ9 真数据正控：单条违规者该 SELECT 恰回 1 行", got1 == "1",
       "ONE=%s" % got1)
    ok("KQ10 真数据负控：多条违规者回 >=2 行（len==1 若不成立就会猜条）",
       got2 != "-" and int(got2) >= 2, "TWO=%s" % got2)

    # ---- 警告联动（九轮：kind='warn' 受理即撤销在册警告）----
    UNWARN = os.path.join(ROOT, "apps", "api", "src", "gaps_http",
                          "appeals", "unwarn.rs")
    uw = strip_comments(read(UNWARN)) if os.path.exists(UNWARN) else ""
    ok("KQ20 撤销警告早于申诉状态翻转",
       0 <= handle.find("lift_if_outranked(") < upd,
       "lift@%d update@%d" % (handle.find("lift_if_outranked("), upd))
    ok("KQ21 联动条件锁死 accept + kind==\"warn\"",
       bool(re.search(r"if\s+body\.accept\s+&&\s+kind\s*==\s*\"warn\""
                      r"\s*\{", handle)))
    ok("KQ22 撤销走 ensure_outranks 且只清真背着警告的行",
       0 <= uw.find("ensure_outranks(") < uw.find("UPDATE users")
       and "warned_until IS NOT NULL" in uw, uw[:80])
    ok("KQ23 撤销动作进审计 appeal.unwarn", '"appeal.unwarn"' in uw)
    ok("KQ24 警告回执两档（撤了说撤了 / 没得撤不谎称）",
       "那条警告已撤销" in handle and "没有撤销任何警告" in handle)
    ok("KQ25 响应带 warn_lifted", '"warn_lifted": warn_lifted' in handle)

    m = re.search(r'"(UPDATE users SET warned_until = NULL[\s\S]{0,160}?)"',
                  uw)
    uw_sql = re.sub(r"\\\s*\n\s*", " ", m.group(1)) if m else ""
    ok("KQ26a 能从源码抠出撤销 UPDATE", bool(uw_sql), uw_sql[:100])
    if uw_sql:
        ids = psql("SELECT 'ID|' || id FROM users WHERE id <> 1"
                   " AND warned_until IS NULL ORDER BY id LIMIT 2;")
        pair = [x.split("|")[1] for x in ids if x.startswith("ID|")]
        ok("KQ26b 取到两枚无警告夹具", len(pair) == 2, str(ids))
        if len(pair) == 2:
            a, b = pair
            loose = uw_sql.replace(" AND warned_until IS NOT NULL", "")
            tx = ["BEGIN;",
                  "UPDATE users SET warned_until = now() + interval '7 days'"
                  " WHERE id = %s;" % a,
                  "WITH hit AS (%s RETURNING id) SELECT 'HIT_A|' || count(*)"
                  " FROM hit;" % uw_sql.replace("$1", a),
                  "WITH hit AS (%s RETURNING id) SELECT 'HIT_B|' || count(*)"
                  " FROM hit;" % uw_sql.replace("$1", b),
                  "WITH hit AS (%s RETURNING id) SELECT 'LOOSE_B|' || count(*)"
                  " FROM hit;" % loose.replace("$1", b),
                  "ROLLBACK;"]
            out = psql("\n".join(tx))

            def cnt(tag):
                rows = [x for x in out if x.startswith(tag + "|")]
                return rows[0].split("|")[1] if len(rows) == 1 else None

            ok("KQ26 真语句正控：有警告的行清真后回 1 行",
               cnt("HIT_A") == "1", "HIT_A=%s" % cnt("HIT_A"))
            ok("KQ27 真语句：本就没警告的行回 0 行（不谎称撤过）",
               cnt("HIT_B") == "0", "HIT_B=%s" % cnt("HIT_B"))
            ok("KQ28 负控：去掉 IS NOT NULL 后同一空行仍算「撤了一条」",
               cnt("LOOSE_B") == "1", "LOOSE_B=%s" % cnt("LOOSE_B"))
            back = psql("SELECT 'AFTER|' || id || '|' ||"
                        " COALESCE(warned_until::text,'NULL') FROM users"
                        " WHERE id IN (%s, %s) ORDER BY id;" % (a, b))
            left = [x for x in back if x.startswith("AFTER|")]
            ok("KQ29 事务回滚后夹具原样（两行都仍无警告）",
               len(left) == 2 and all(x.endswith("|NULL") for x in left),
               str(left))

    head = subprocess.run(
        ["git", "-C", ROOT, "show", "HEAD:apps/api/src/gaps_http/appeals.rs"],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    ok("KQ11 两段联动并存：解封联动没被这次改动挤掉",
       "restore_if_outranked(" in handle
       and "a.kind = 'ban'" in fb and bool(head.stdout),
       "HEAD 取到 %d 行" % len((head.stdout or "").splitlines()))
    finish()


main()
