# -*- coding: utf-8 -*-
"""KP 申诉受理的解封护栏顺序（P-561，2026-10-22 资深用户深测）。

缺陷本体：appeal_handle 先把 appeals.status 翻成 accepted，再调
restore_if_outranked 做 0285 的等级护栏。护栏一旦拒绝（低档 staff 受理
高档目标——正是 0285 要挡的「放出被封的管理员/站长」场景）：
  - appeals 已消费（status='accepted'）
  - 账号仍封（restore 的 UPDATE 没跑）
  - 当事人收不到任何通知（notify 在错误返回之后）
  - staff 重试只得到「申诉不存在或已处理」
一条申诉被静默吃掉。

修复：翻转状态**之前**先取目标与解封意图（SELECT 只认 status='open'，
已处理申诉不再触发解封），解封动作整体前置；其内部仍二次校验等级。

  KP0 解封动作早于 UPDATE appeals SET status
  KP1 SELECT 的解封意图带 a.status = 'open'（已处理申诉不解封）
  KP2 restore_if_outranked 实参仍带 actor id/等级/目标
  KP3 解封动作内部 ensure_outranks 早于 UPDATE users
  KP4 解封动作自身的状态阈值仍在（status >= 2）
  KP5 申诉状态翻转仍要求 status='open'（不重复消费）
"""
import io
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
APPEALS = os.path.join(ROOT, "apps", "api", "src", "gaps_http", "appeals.rs")
UNBAN = os.path.join(ROOT, "apps", "api", "src", "gaps_http",
                     "appeal_unban.rs")

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


def handler_body(src):
    """只取 appeal_handle 的函数体，避免被同文件别的 handler 洗白。"""
    i = src.find("pub async fn appeal_handle")
    if i < 0:
        return ""
    j = src.find("\n}", i)
    return src[i:j] if j > i else src[i:]


def flat(s):
    """把 Rust 字符串续行（反斜杠+换行）压成一行再匹配。"""
    return re.sub(r"\\\s*\n\s*", " ", s)


def order_ok(body):
    i = body.find("restore_if_outranked(")
    j = body.find("UPDATE appeals SET status")
    return 0 <= i < j


def main():
    src = read(APPEALS)
    body = handler_body(src)
    fb = flat(body)
    ok("KP0 解封早于申诉状态翻转", order_ok(body),
       "restore@%d update@%d" % (body.find("restore_if_outranked("),
                                 body.find("UPDATE appeals SET status")))
    ok("KP1 SELECT 的解封意图带 open 守卫",
       bool(re.search(r"\(\s*\$2 AND a\.kind = 'ban' AND a\.status = 'open'"
                      r"\s+AND u\.status >= 2\s*\)", fb)))
    ok("KP2 解封调用实参完整",
       bool(re.search(r"restore_if_outranked\(\s*&state,\s*auth\.id,\s*"
                      r"auth\.class_id,\s*user_id,\s*\)", body)))
    u = read(UNBAN)
    ok("KP3 解封内部护栏早于写 users",
       0 <= u.find("ensure_outranks(") < u.find("UPDATE users"))
    ok("KP4 解封动作状态阈值仍在", "status >= 2" in u)
    ok("KP5 申诉翻转仍只消费 open",
       bool(re.search(r"UPDATE appeals SET status[\s\S]{0,200}?"
                      r"AND status = 'open'", body)))

    print("\n%d 条通过 / %d 条失败" % (PASS, len(FAIL)))
    if FAIL:
        print("失败：" + "、".join(FAIL))
        sys.exit(1)


main()
