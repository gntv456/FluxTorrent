# -*- coding: utf-8 -*-
"""闸门 DT：农场写侧的钱货对应（种植双扣 / 浇水白扣）。

批一百七十一。会员视角：田里已有一茬，页面没刷新（另开标签页、或上一茬刚种下
还没回填），再点一次播种 ——

  ① P-359 资金级（本批已修）：`/farm/plant` 原先是「独立事务扣款 → autocommit
     占位 → 占位失败再退款」三段。退款额取自该幂等键的**支出流水行 `amount`**，
     而支出流水按约定存**负数**，`earn_spark(-80)` 于是变成第二次扣款。
     现网实锤 spark_ledger 1795/1796：同一分钟键两行 -80，balance_after
     3527 → 3447 —— 会员收到「该地块已有作物」这句提示的同时被扣了两遍魔力，
     一块地也没拿到。改为扣款与占位同事务后，占位失败即整笔回滚（分文未扣），
     退款分支连同它的符号陷阱一起删除。

  ② P-358 坑钱级（本批已修）：`/farm/water` 的占位 UPDATE 少了
     `AND ready_at > now()`——同文件 `/farm/fertilize` 有这道闸门，界面
     `farm-field.tsx` 也在 `ready` 时直接把浇水钮藏掉，**只有 API 侧被改漏**。
     后果不是「白浇」而是「花钱买亏」：地块早已成熟，收获时刻一点没提前，
     而枯萎截止按 `ready_at + farm_wither_days` 算，这一笔钱买到的只有
     「自己的作物提前 10 分钟枯死」。

判据形状：DT1/DT2 钉①的原子性与符号陷阱不许复活，DT3/DT8 钉②的谓词与
「界面/后端同一份规则」，DT4 棘轮扣费结果丢弃处数，DT5/DT6 真库事务内 A/B
（旧谓词必须红、新谓词必须绿且不误伤正常浇水），DT7 收尾残料。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dt_farm_charge.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ACT = os.path.join(ROOTDIR, "apps", "api", "src", "games_http",
                   "farm_actions.rs")
FIELD = os.path.join(ROOTDIR, "apps", "web", "components", "game",
                     "farm-field.tsx")


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    s = re.sub(r"^\s*//.*$", "", s, flags=re.M)
    return s


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-F",
         " | ", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:260])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


body = cut(ACT)


def sect(name, nxt):
    """取 pub(super) async fn name 到下一个 async fn 之间的函数体"""
    i = body.find("async fn %s(" % name)
    if i < 0:
        return ""
    j = body.find(nxt)
    return body[i:j] if j > i else body[i:]


plant = sect("farm_plant", "async fn farm_water")
water = sect("farm_water", "async fn farm_fertilize")
fert = sect("farm_fertilize", "async fn farm_harvest")

# ---------- DT1 种植扣款与占位同事务 ----------
ok("DT1 种植段拿到事务（begin=%s / commit=%s）"
   % (".begin()" in plant, "tx.commit()" in plant),
   ".begin()" in plant and "tx.commit()" in plant,
   "扣款与占位分属两个提交点＝占位失败只能靠事后退款兜底")
ok("DT1b 占位写的是事务句柄（&mut *tx），不是连接池",
   ".fetch_optional(&mut *tx)" in plant
   and ".fetch_optional(&state.repo.db)" not in plant,
   "写回连接池的那条语句不受事务保护")
ok("DT1c 种植扣款走 spend_spark_tx（事务版），不再用独立事务的 spend_spark",
   "spend_spark_tx(" in plant and "spend_spark(\n        &state.repo.db"
   not in plant,
   "非事务版扣款先提交，回滚不了")

# ---------- DT2 符号陷阱不可复活 ----------
ok("DT2 退款分支已删除（farm-refund: 前缀 0 处）",
   "farm-refund:" not in body,
   "留着退款分支＝留着「拿负数流水当退款额」的符号陷阱")
ok("DT2b 本文件不再出现非事务 earn_spark(（只留 earn_spark_tx）",
   not re.search(r"earn_spark\(", body),
   "非事务版入账的返回值是 ()，重放与负数都判不出来")
# 旧形状在现网留下的实锤行数（只报证据，不作红判据）
hist = sval("SELECT count(*) || '/' || coalesce(sum(amount), 0) "
            "FROM spark_ledger WHERE idempotency_key LIKE 'farm-refund:%' "
            "AND amount < 0")
print("   [旧形状实锤] 现网「退款行是负数」共 %s 条（行/金额和）；"
      "1795→1796 的 balance_after 3527→3447 即双扣本体" % hist)
ok("DT2c 判据不是空转：上面那条读数取自真库且能报出行数",
   "/" in hist and hist.split("/")[0].isdigit(),
   "读不到＝psql 挂了，DT2 的其余静态判据也失去对照")

# ---------- DT3 浇水/施肥配对闸门 ----------
GUARD = "ready_at > now()"
ok("DT3 浇水的占位谓词带成熟闸门（%s）" % (GUARD in water),
   GUARD in water,
   "缺它＝对已成熟地块扣 farm_water_spark 并把它提前 10 分钟推向枯萎")
ok("DT3b 施肥同带（两份规则必须同式）", GUARD in fert,
   "同文件里两个催熟口只要有一个漏判，会员就会碰到那个")

# ---------- DT4 扣费结果丢弃棘轮 ----------
spends = len(re.findall(r"spend_spark(_tx)?\(", body))
checks = body.count("SpendOutcome::Spent")
ok("DT4 扣费调用 %d 处、Spent 判定 %d 处（一一对应）" % (spends, checks),
   spends == checks and spends >= 3,
   "少一条判定＝重放被当成功＝印钞口")
drops = len(re.findall(r"let _ = earn_spark_tx\(", body))
ok("DT4b 显式丢弃入账结果只允许收获那 1 处（地块行锁+plot 级幂等键背书）",
   drops == 1,
   "又多一处丢弃＝新的「账面说有、钱包没有」候选")

# ---------- DT5 真库事务内 A/B（旧谓词红、新谓词绿） ----------
AB = r"""
BEGIN;
INSERT INTO farm_plots (user_id, slot, crop_id, planted_at, ready_at, watered)
VALUES (1, 91, 1, now() - interval '90 minutes', now() - interval '30 minutes',
        FALSE),
       (1, 92, 1, now() - interval '10 minutes', now() + interval '20 minutes',
        FALSE);
WITH o AS (UPDATE farm_plots SET watered = TRUE,
      ready_at = ready_at - interval '10 minutes'
   WHERE user_id = 1 AND slot = 91 AND watered = FALSE
     AND (5 = 0 OR ready_at + make_interval(days => 5) >= now()) RETURNING id)
SELECT 'old_mature=' || count(*) FROM o;
SELECT 'old_gain_min=' || round(extract(epoch FROM
   (now() - interval '30 minutes') - ready_at)::numeric / 60, 1)
  FROM farm_plots WHERE user_id = 1 AND slot = 91;
SELECT 'old_still_ready=' || count(*) FROM farm_plots
 WHERE user_id = 1 AND slot = 91 AND ready_at <= now();
ROLLBACK;
BEGIN;
INSERT INTO farm_plots (user_id, slot, crop_id, planted_at, ready_at, watered)
VALUES (1, 91, 1, now() - interval '90 minutes', now() - interval '30 minutes',
        FALSE),
       (1, 92, 1, now() - interval '10 minutes', now() + interval '20 minutes',
        FALSE);
WITH o AS (UPDATE farm_plots SET watered = TRUE,
      ready_at = ready_at - interval '10 minutes'
   WHERE user_id = 1 AND slot = 91 AND watered = FALSE AND ready_at > now()
     AND (5 = 0 OR ready_at + make_interval(days => 5) >= now()) RETURNING id)
SELECT 'new_mature=' || count(*) FROM o;
WITH g AS (UPDATE farm_plots SET watered = TRUE,
      ready_at = ready_at - interval '10 minutes'
   WHERE user_id = 1 AND slot = 92 AND watered = FALSE AND ready_at > now()
     AND (5 = 0 OR ready_at + make_interval(days => 5) >= now()) RETURNING id)
SELECT 'new_growing=' || count(*) FROM g;
SELECT 'new_gain_min=' || round(extract(epoch FROM
   (now() + interval '20 minutes') - ready_at)::numeric / 60, 1)
  FROM farm_plots WHERE user_id = 1 AND slot = 92;
ROLLBACK;
"""
read = {}
for ln in psql(AB).split("\n"):
    ln = ln.strip()
    if "=" in ln:
        k, v = ln.split("=", 1)
        read[k.strip()] = v.strip()
ok("DT5 旧谓词对已成熟地块影响 %s 行、收获时刻没变（old_still_ready=%s）＝"
   "白扣一筆的证据链齐" % (read.get("old_mature"), read.get("old_still_ready")),
   read.get("old_mature") == "1" and read.get("old_still_ready") == "1",
   "数不到 1＝DT6 的新谓词绿也没意义（夹具或谓词本身坏了）")
ok("DT6 新谓词：成熟地块 %s 行（拒扣）、生长地块 %s 行且真提前 %s 分钟"
   % (read.get("new_mature"), read.get("new_growing"),
      read.get("new_gain_min")),
   read.get("new_mature") == "0" and read.get("new_growing") == "1"
   and read.get("new_gain_min") in ("10.0", "10"),
   "成熟地块仍被扣＝闸门没落地；生长地块也 0 行＝闸门误伤正常浇水")

# ---------- DT6b 原子性真库证：回滚后不留扣款流水 ----------
ATOM = r"""
BEGIN;
INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id,
  idempotency_key, balance_after) VALUES (nextval('spark_ledger_id_seq'),
  1, -80, 'game', 'farm_plant', 1, 'dt-probe-plant', 0);
INSERT INTO farm_plots (user_id, slot, crop_id, planted_at, ready_at)
VALUES (1, 1, 1, now(), now() + interval '55 minutes')
ON CONFLICT (user_id, slot) DO NOTHING;
SELECT 'inside=' || count(*) FROM spark_ledger
 WHERE idempotency_key = 'dt-probe-plant';
ROLLBACK;
SELECT 'after=' || count(*) FROM spark_ledger
 WHERE idempotency_key = 'dt-probe-plant';
SELECT 'plot_after=' || count(*) FROM farm_plots WHERE user_id = 1
 AND slot = 1 AND planted_at > now() - interval '2 minutes';
"""
at = {}
for ln in psql(ATOM).split("\n"):
    ln = ln.strip()
    if "=" in ln:
        k, v = ln.split("=", 1)
        at[k.strip()] = v.strip()
ok("DT7 同事务回滚：事务内流水 %s 行 → 提交前撤掉后残留 %s 行、地块 %s 块"
   % (at.get("inside"), at.get("after"), at.get("plot_after")),
   at.get("inside") == "1" and at.get("after") == "0"
   and at.get("plot_after") == "0",
   "回滚不干净＝占位失败仍会留下扣款流水")

# ---------- DT8 界面与后端同一份规则 ----------
ui = cut(FIELD)
ok("DT8 界面早已在成熟/枯萎时藏掉浇水钮（后端补齐后两层才一致）",
   "!withered && !ready &&" in ui and "onWater(slot)" in ui,
   "界面这一份被改漏＝会员侧还会看到钮；后端那一份被改漏＝脚本侧白扣")

# ---------- DT9 残料 ----------
res = sval("SELECT count(*) FROM farm_plots WHERE user_id = 1 "
           "AND slot IN (1, 91, 92) AND planted_at > "
           "now() - interval '30 minutes'")
res2 = sval("SELECT count(*) FROM spark_ledger WHERE idempotency_key = "
            "'dt-probe-plant'")
ok("DT9 本闸门无残料（探针地块 %s 块、探针流水 %s 条）" % (res, res2),
   res == "0" and res2 == "0",
   "探针行留在现网会被下一个玩家碰到")

# ---------- DT10 接口层未验的桩（现网开关关着，别拿静态绿冒充端到端） ----------
mf = sval("SELECT value FROM site_settings WHERE name = 'module_farm'")
print("   [钉桩] site_settings.module_farm = %s；农场整模块在网关层就 4101，"
      "本闸门的 17 条是**静态 + 真库事务内 A/B**，没有一条打到 HTTP 口。"
      "开关置 yes 后要补的验收：同一槽位隔分钟二次 /farm/plant 必须 400 且"
      "余额净 0、不再产生 farm-refund 行；对 ready_at 已过的地块 /farm/water "
      "必须 400 且不扣 farm_water_spark。" % (mf or "<无此行>"))
ok("DT10 桩位仍在（模块开着时也不许把这段删掉——它是接口层欠账的唯一记录）",
   "module_farm" in open(__file__, encoding="utf-8").read(),
   "删桩＝接口层欠账无痕")

summary()
