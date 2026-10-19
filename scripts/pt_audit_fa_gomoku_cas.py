# -*- coding: utf-8 -*-
"""闸门 FA：五子棋对局的归属、轮次与「落子不被并发覆盖」。

批一百八十二。会员视角：开了两个标签页各下一子，回头发现盘上只有一颗——
而且自己还多了一次行棋机会。

  P-372（本批已修）：`POST /gomoku/games/{id}/move` 先 SELECT 出
  (black_id, white_id, board, turn, winner_id)，判完归属/轮次/占位后，
  **无条件** `UPDATE ... SET board=$2 WHERE id=$1`。两次并发落子（同一玩家的
  连点、双标签页、脚本重试）读到同一份 board，各自算出完整盘面，后提交者
  整盘覆盖前一笔 ⇒ 少一颗子 + 白多一次行棋；若两次都判成胜负，`winner_id`
  也会互相覆盖（谁赢取决于谁最后落地）。
  改法：落子带 CAS（`AND board=$5 AND turn=$6` 用我刚读到的值），0 行即冲突
  拒回 422 让客户端刷新——这把「静默覆盖」变成「看得见的一次冲突」。

判据形状：FA1/FA2 钉 CAS 条件与 0 行判定；FA3 钉归属/轮次/占位三道闸没被
换序时顺手删掉；FA4 用真库事务内两次同谓词 CAS 证明第二次必为 0 行（正控，
同时证明「无 CAS 时两次都算成功」这一旧形状）；FA5 钉 join 侧的同类 CAS；
FA6 报现网对局面貌（含一个待办：`gomoku_create` 无频控、无「每人进行中局数」上限）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_fa_gomoku_cas.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOTDIR, "apps", "api", "src", "http", "frames_gomoku.rs")


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
        print("   psql 失败：%s" % (r.stderr or "").strip()[:240])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


body = cut(SRC)
i_move = body.find("async fn gomoku_move")
i_get = body.find("async fn gomoku_get")
move = body[i_move:i_get] if 0 <= i_move else body[i_move:]
i_join = body.find("async fn gomoku_join")
join = body[i_join:body.find("struct MoveBody")]

# ---------- FA1 CAS 形状 ----------
ok("FA1 落子 UPDATE 带 board+turn 双条件（读到什么就按什么改）",
   "WHERE id=$1 AND board=$5 AND turn=$6" in move,
   "无条件 UPDATE 就是后写覆盖前写：少一颗子、白多一次行棋、winner 互相盖")
ok("FA1b 落子判 rows_affected，0 行回可见冲突",
   "rows_affected()" in move and "if done == 0" in move,
   "不判就是「接口回 200 而棋子被吞」")
ok("FA1c 冲突句是整句字面量（可查 validation_details.tsv）",
   '"棋盘已被改动，请刷新后重试"' in rd(SRC),
   "动态串无法按原句查表（validation_i18n_guard 那条棘轮）")

# ---------- FA2 三道归属闸仍在 ----------
ok("FA2 非对局者不能落子（Forbidden 分支在）",
   "DomainError::Forbidden" in move,
   "换序/改写时最容易把归属判定顺手删掉")
ok("FA2b 轮次判定与占位判定都在（turn、已有棋子）",
   "还没轮到你" in move and "该点已有棋子" in move,
   "少了任一判定＝可以连下两手或叠子")
ok("FA2c 已终局与无对手两道闸在",
   "对局已结束" in move and "等待对手加入" in move,
   "终局后还能落子＝胜者可被改写")

# ---------- FA4 真库正控：同一谓词的第二次 CAS 必须 0 行 ----------
AB = r"""
BEGIN;
-- 探针必须用库里真实存在的两个会员：gomoku_games 对 black_id/white_id 都有外键
INSERT INTO gomoku_games (id, black_id, white_id, board, turn)
SELECT 999997, min(u.id), (array_agg(u.id ORDER BY u.id))[2], '', 'b'
  FROM users u HAVING count(*) >= 2
ON CONFLICT (id) DO NOTHING;
-- 第一次：按读到的 board='' / turn='b' 改（应 1 行）
WITH x AS (UPDATE gomoku_games SET board='b' || repeat('.', 224), turn='w'
   WHERE id=999997 AND board='' AND turn='b' RETURNING 1)
SELECT 'cas1=' || count(*) FROM x;
-- 第二次：同一份旧值再 CAS（并发后到者，应 0 行）
WITH y AS (UPDATE gomoku_games SET board='w' || repeat('.', 224), turn='b'
   WHERE id=999997 AND board='' AND turn='b' RETURNING 1)
SELECT 'cas2=' || count(*) FROM y;
-- 无 CAS 的旧写法：同一行照样算成功（覆盖的来源）
WITH z AS (UPDATE gomoku_games SET board=repeat('.', 225), turn='b'
   WHERE id=999997 RETURNING 1)
SELECT 'naive2=' || count(*) FROM z;
ROLLBACK;
SELECT 'residue=' || count(*) FROM gomoku_games WHERE id=999997;
"""
pc = {}
for ln in psql(AB).split("\n"):
    ln = ln.strip()
    if "=" in ln:
        k, v = ln.split("=", 1)
        pc[k.strip()] = v.strip()
ok("FA4 正控（真表）：首次 CAS=%s（须 1）、同旧值二次 CAS=%s（须 0）、"
   "无 CAS 的写法=%s（须 1，即覆盖来源）、回滚残留 %s（须 0）"
   % (pc.get("cas1"), pc.get("cas2"), pc.get("naive2"), pc.get("residue")),
   pc.get("cas1") == "1" and pc.get("cas2") == "0"
   and pc.get("naive2") == "1" and pc.get("residue") == "0",
   "二次 CAS 若也 1，说明条件没写对；residue 非 0＝探针留下对局")

# ---------- FA5 join 侧同类 CAS ----------
ok("FA5 加入局用条件更新（white_id IS NULL AND black_id <> 自己）+ 0 行拒",
   "white_id IS NULL AND black_id <> $2" in join and "rows_affected() == 0"
   in join,
   "不判就能把已有对手的对局抢过来，或自己跟自己下")

# ---------- FA6 现网账 + 待办 ----------
row = sval("SELECT count(*) || ' 局 / 已终局 ' || "
           "count(*) FILTER (WHERE winner_id IS NOT NULL) FROM gomoku_games")
mine = sval("SELECT coalesce(max(c)::text, '0') FROM (SELECT count(*) c "
            "FROM gomoku_games GROUP BY black_id) x")
print("   [现网账] gomoku_games：%s；单人最多建房 %s 局" % (row, mine))
print("   [待办] gomoku_create 无频控、也无「每人进行中局数」上限："
      "任何会员可以无限建房（每 POST 一行）。要加限流或上限请一并"
      "把本行改成硬判据。")

# ---------- FA7 残料 ----------
res = sval("SELECT count(*) FROM gomoku_games WHERE id=999997")
ok("FA7 本闸门无残料（探针对局 %s 局）" % res, res == "0",
   "探针局留在现网会被别人加入")

summary()
