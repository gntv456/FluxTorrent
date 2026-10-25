# -*- coding: utf-8 -*-
"""闸门 DJ：自愿转账两口的收件方口径必须同尺，劳动所得那两口不许被同尺。

批一百六十二。会员视角：看到好帖点「打赏」。账是全的（单事务、幂等、
自赏/匿名/额度都拦），但**收钱那一方是什么状态没人判**——而另一只自愿
转账口（勋章赠送 community_http/medal_gift.rs:38）一直要求 `status < 2`。
打赏打进出不了账的账号＝钱沉进死账户（现网被处置账号已持有 769,204 魔力）、
站内信永远没人看。修法：tip 与赠送同尺，收件方非正常会员直接拒。

反向也要钉：字幕酬劳（content_http/subtitles_close.rs / subtitles_fulfill.rs）
与求种悬赏**不该**加这条闸——那是劳动所得，作者中途被封也要照付，
加了闸就变成「干了活领不到钱」。本闸门把两种口径都钉住，谁被"顺手对齐"
就红。

判据四层：
  DJ1 静态同尺：两只自愿口都有 `status < 2` 判据；两只酬劳口都没有；
  DJ2 真库 A/B：事务内把一位作者置 status=4，旧形状（不读状态）仍放行、
      新形状必须拒；
  DJ3 台账 + 不变量：post_tips 里打给被处置账号的笔数必须为 0；
      被处置账号余额合计只报数（历史沉没资本，归站方处置）；
  DJ4 还原自证：A/B 前后 posts/users/post_tips 三表行数逐位相等。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dj_tip_recipient.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HTTP = os.path.join(ROOTDIR, "apps", "api", "src")
VOLUNTARY = [os.path.join(HTTP, "community_http", "tip.rs"),
             os.path.join(HTTP, "community_http", "medal_gift.rs")]
EARNED = [os.path.join(HTTP, "content_http", "subtitles_close.rs"),
          os.path.join(HTTP, "content_http", "subtitles_fulfill.rs")]


def cut(f):
    s = io.open(f, encoding="utf-8", errors="replace").read()
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("   psql 失败：%s" % (r.stderr or "").strip()[:220])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


# ---------- DJ1 同尺 / 反向钉 ----------
miss = []
for f in VOLUNTARY:
    s = cut(f)
    if not ("status < 2" in s or "st < 2" in s):
        miss.append(os.path.basename(f))
leaked = []
for f in EARNED:
    s = cut(f)
    if "status < 2" in s or "st < 2" in s:
        leaked.append(os.path.basename(f))
ok("DJ1a 自愿转账口都按 `status < 2` 判收件方（缺判据 %d）" % len(miss),
   not miss, "、".join(miss) or "-")
ok("DJ1b 酬劳/悬赏口**不许**被同尺顺手对齐（越界 %d）" % len(leaked),
   not leaked,
   "%s 被加了状态闸＝被封的人干了活领不到钱，口径要单独拍板" % "、".join(leaked))

# ---------- DJ2 真库 A/B：作者被置封禁后，旧形状放行、新形状拒 ----------
before = {t: sval("SELECT count(*) FROM %s" % t)
          for t in ("posts", "users", "post_tips")}
AB = r"""
SET lock_timeout = '5s';
BEGIN;
-- 取一个正常会员作者，把它置成封禁（status=4）
CREATE TEMP TABLE ab AS
  SELECT p.id AS pid, u.id AS uid FROM posts p
  JOIN users u ON u.id = p.user_id AND u.status < 2
  ORDER BY p.id LIMIT 1;
UPDATE users SET status = 4 WHERE id = (SELECT uid FROM ab);
-- 旧形状：只取作者，不看状态（tip.rs 修订前）
SELECT 'old_ok=' || (SELECT count(*) FROM posts p
  JOIN topics t ON t.id = p.topic_id
  JOIN ab ON ab.pid = p.id WHERE p.user_id IS NOT NULL)::text;
-- 新形状：带上 LEFT JOIN users 的 status 判据（matches!(.., Some(st) if st<2)）
SELECT 'new_ok=' || (SELECT count(*) FROM posts p
  JOIN topics t ON t.id = p.topic_id
  LEFT JOIN users u ON u.id = p.user_id
  JOIN ab ON ab.pid = p.id
  WHERE p.user_id IS NOT NULL AND u.status IS NOT NULL
    AND u.status < 2)::text;
SELECT 'planted_status=' || status FROM users
  WHERE id = (SELECT uid FROM ab);
ROLLBACK;
"""
rd = {}
for ln in psql(AB).split("\n"):
    ln = ln.strip()
    if "=" in ln:
        k, v = ln.split("=", 1)
        rd[k.strip()] = v.strip()
ok("DJ2 作者被置封禁后：旧形状仍放行(old=%s)、新形状必须拒(new=%s)、"
   "植入状态=%s" % (rd.get("old_ok"), rd.get("new_ok"),
                    rd.get("planted_status")),
   rd.get("old_ok") == "1" and rd.get("new_ok") == "0"
   and rd.get("planted_status") == "4",
   "读数不齐＝取样失败（现网没有正常会员的楼层）或判据没落地")

# ---------- DJ3 不变量 + 台账 ----------
dirty = sval("SELECT count(*) FROM post_tips t JOIN users u "
             "ON u.id = t.to_user WHERE u.status >= 2")
ok("DJ3a 历史上没有打赏打进被处置账号（实测 %s 笔，须 0）" % dirty,
   dirty == "0", "非 0＝已有钱沉进死账户，要按笔给出处置口径")
dead = sval("SELECT coalesce(sum(spark_balance),0) FROM users "
            "WHERE status >= 2")
print("   [台账] 被处置账号（status>=2）合计持有魔力 %s——历史沉没资本，"
      "本站无回收口，归站方处置（不是本闸门的洞）" % dead)
tips = sval("SELECT count(*) FROM post_tips")
print("   [账面] post_tips 累计 %s 笔；tip 口是自愿转账，与勋章赠送同尺"
      % tips)

# ---------- DJ4 还原自证 ----------
after = {t: sval("SELECT count(*) FROM %s" % t)
         for t in ("posts", "users", "post_tips")}
same = all(before[t] == after[t] for t in before)
ok("DJ4 A/B 前后三表行数逐位相等（posts %s→%s / users %s→%s / "
   "post_tips %s→%s）" % (before["posts"], after["posts"], before["users"],
                          after["users"], before["post_tips"],
                          after["post_tips"]),
   same, "回滚没吃干净＝闸门在往现网写数据")

summary()
