# -*- coding: utf-8 -*-
"""闸门 DR：装扮（头像框/动态头像）的写入原子性与「展示列必须有所有权支撑」。

批一百六十九。会员视角：商店买一个头像框 → 去「我的装扮」戴上 → 30 天后到期
→ 再看主页。两处问题：

  ① P-355 撕裂窗口（本批已修）：`/dressup/wear` 原先是「摘同槽位 → 置
     wearing=TRUE → 落 users.avatar_frame_id」等 4~8 条**各自提交**的语句；
     中间任何一条失败（DB 抖动、进程被杀）就留下「面板说佩戴中、页面上没效果」
     或反过来。改为同一事务，末尾一次 commit；P-307 那段「先手工回滚标记再
     拒单」的补写也变多余（return Err 即整笔回滚）。

  ② P-356 账实脱钩（**待裁，本批不代改**）：`users.avatar_frame_id` 只要非空
     就生效——`/me`(me.rs:65)、`/me/settings`(me_settings.rs:34)、
     对外主页(profile.rs:51) 三个读者都只看这一列，**没有任何一处回头验证
     用户是否仍佩戴且仍持有**那枚框。现网实证：user 4406 带着
     avatar_frame_id=1 却**一条 user_dressups 都没有**；库里也**没有**任何
     过期装扮回收 job（`apps/worker` 只在注释里提到 user_dressups）。
     于是「过期后仍永久挂着付费头像框」「P-307 误写的框没人清」两种情况都会
     对外可见。两条出路各有代价，需要站方拍板：
       (a) 读侧判定：三个读者的 JOIN 加「存在佩戴中且未过期的 avatar_frame
           装扮」——但 SKU→框 id 的映射是三级来源（config.frame_id →
           旧写法 config.avatar_url 填数字 → 无），现网 user 4167 正是走旧
           SKU 的佩戴者，读侧若解析不到就会**误伤正常佩戴者**（把框吞掉）；
       (b) 对账 job（本仓惯例：worker 60s tick 清账）：把无支撑的
           avatar_frame_id 清空——同时能顺手修掉 P-307 的历史误写，
           但要接受「到期瞬间到下一次 tick」的短暂残留。
     本闸门先把它钉成**只降不升的账实上界**（现网 1 人）＋正控可植入，
     任何一侧改了口径都必须同步这里。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dr_dressup.py
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
DRESS = os.path.join(SRC, "economy_http", "dressup.rs")
ORPHAN_MAX = 1  # 现网 user 4406 一例（P-356 待裁），只降不升


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


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


ORPHAN_SQL = ("SELECT count(*) FROM users u WHERE u.avatar_frame_id IS NOT "
              "NULL AND NOT EXISTS (SELECT 1 FROM user_dressups d JOIN "
              "shop_items si ON si.id = d.item_id WHERE d.user_id = u.id "
              "AND d.wearing AND si.kind = 'avatar_frame' AND (d.expires_at "
              "IS NULL OR d.expires_at > now()))")

body = cut(DRESS)

# ---------- DR1 原子性形状 ----------
i_begin = body.find(".begin()")
i_commit = body.find("tx.commit()")
after_begin = body[i_begin:i_commit] if 0 <= i_begin < i_commit else ""
leaks = after_begin.count(".execute(&state.repo.db)")
ok("DR1 佩戴流程在同一事务里（begin=%d commit=%d，事务外写 %d 处）"
   % (i_begin >= 0, i_commit > i_begin, leaks),
   i_begin >= 0 and i_commit > i_begin and leaks == 0,
   "事务里还漏着直接对连接池的写＝撕裂窗口没堵完")
n_unwear = body.count("wearing = FALSE")
ok("DR1b P-307 的手工回滚补写已删（摘除语句共 %d 条，应为 2）" % n_unwear,
   n_unwear == 2,
   "多于 2 条＝除「摘同槽/摘下佩戴」外又留了一处手工回滚（两条规则管同一件事）")

# ---------- DR2/DR3 所有权判据与摘下清列 ----------
ok("DR2 佩戴判据排除过期件",
   "ud.expires_at IS NULL OR ud.expires_at > now()" in body,
   "过期件仍可佩戴＝付费装扮永久有效")
trg = sval("SELECT count(*) FROM pg_trigger WHERE tgname = "
           "'dressup_one_per_slot_trg'")
ok("DR2b 单槽互斥触发器在位（%s 个）" % trg, trg == "1",
   "互斥从应用层挪走时，库层是最后一道")
ok("DR3 摘下佩戴会清对应展示列（框与动态头像两条都在）",
   "SET avatar_frame_id = NULL" in body and "SET avatar_url = NULL" in body,
   "只清标记不清列＝摘下后对外还挂着")

# ---------- DR4 账实上界 + DR5 正控 ----------
orph = sval(ORPHAN_SQL)
ok("DR4 带头像框却无佩戴支撑的会员数 %s ≤ 上界 %d" % (orph, ORPHAN_MAX),
   int(orph or 0) <= ORPHAN_MAX,
   "新增＝展示列与所有权又脱钩一次（P-356 未修前只许降）")
AB = r"""
SET lock_timeout = '5s';
BEGIN;
CREATE TEMP TABLE dr AS SELECT u.id AS uid FROM users u
  WHERE u.avatar_frame_id IS NULL AND u.class_id > 0 ORDER BY u.id LIMIT 1;
UPDATE users SET avatar_frame_id = (SELECT min(id) FROM avatar_frames)
 WHERE id = (SELECT uid FROM dr);
SELECT 'orphans=' || (SELECT count(*) FROM users u
  WHERE u.avatar_frame_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM
    user_dressups d JOIN shop_items si ON si.id = d.item_id
    WHERE d.user_id = u.id AND d.wearing AND si.kind = 'avatar_frame'
      AND (d.expires_at IS NULL OR d.expires_at > now())));
ROLLBACK;
SELECT 'after=' || count(*) FROM users u WHERE u.avatar_frame_id IS NOT
  NULL AND NOT EXISTS (SELECT 1 FROM user_dressups d JOIN shop_items si
  ON si.id = d.item_id WHERE d.user_id = u.id AND d.wearing
  AND si.kind = 'avatar_frame' AND (d.expires_at IS NULL
  OR d.expires_at > now()));
"""
rd5 = {}
for ln in psql(AB).split("\n"):
    ln = ln.strip()
    if "=" in ln:
        k, v = ln.split("=", 1)
        rd5[k.strip()] = v.strip()
ok("DR5 正控：造一个无支撑的头像框后判据数到 orphans=%s（应为 %d），"
   "回滚后回到 %s" % (rd5.get("orphans"), int(orph or 0) + 1,
                      rd5.get("after")),
   rd5.get("orphans") == str(int(orph or 0) + 1)
   and rd5.get("after") == orph,
   "数不到＝DR4 是空判据；读数缺失＝psql 报错或取样失败")

# ---------- DR6 待裁台账 ----------
who = psql("SELECT u.id || '/' || u.username FROM users u WHERE "
           "u.avatar_frame_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM "
           "user_dressups d JOIN shop_items si ON si.id = d.item_id "
           "WHERE d.user_id = u.id AND d.wearing AND si.kind = "
           "'avatar_frame' AND (d.expires_at IS NULL OR d.expires_at > "
           "now()))")
nojob = sval("SELECT count(*) FROM pg_proc WHERE proname LIKE "
             "'%dressup%expire%'")
print("   [待裁 P-356] 无支撑带框会员：%s；库里无过期装扮回收函数(%s)——"
      "读侧判定 vs 对账 job 两条出路见本文件头，改口径必须同步这里"
      % (who.replace("\n", "、") or "无", nojob))

summary()
