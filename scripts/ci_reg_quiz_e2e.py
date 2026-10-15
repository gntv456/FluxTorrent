# -*- coding: utf-8 -*-
"""CI 用：M5 答题注册 + PoW + 规则准确率变更型端到端（0372）。

覆盖：题库 admin CRUD（新增/非法拒/停启用/导入去重）→ 抽题不含答案 →
答对通过 / 答错拦截（留痕）→ 票据一次性（重放被拒）→ pass_required=no
软闸 → 缺票据被拒 → PoW 求解通过/伪解被拒 → 家族准确率与处置分布。

副作用：临时改 9 个 settings 键、建 1 道测试题 + 5 个测试账号 + 2 条假
cheat_events，结束全部还原/墓碑化。退出码非 0 = 失败。
前提：api 已起（本脚本不需要 worker）。
"""
import json
import sys

from ci_reg_common import (
    call, check, clear_throttle, data_of, jwt, leftover, msg, nowmark,
    psql, register, report, restore_settings, set_setting, solve_pow,
    staff_uid, tombstone,
)

MARK = "m5quiz"
CAT = "smoketest"
Q_TEXT = "CI 冒烟：本站对刷上传量是否违规？"
Q_OPTS = ["不违规，是技巧", "违规，会被封号", "视情况", "不清楚"]
Q_ANS = 1
SETTINGS = ("registration_mode", "reg_quiz_enabled", "reg_quiz_category",
            "reg_quiz_pass_required", "reg_quiz_ttl_minutes",
            "captcha_provider", "pow_difficulty_bits", "reg_ip_limit",
            "reg_window")


def quiz_ticket() -> str:
    _, r = call("GET", "/api/v1/auth/quiz-challenge", None)
    return str(data_of(r).get("ticket", ""))


def pow_checks(ts: str) -> None:
    set_setting("reg_quiz_enabled", "no")
    set_setting("captcha_provider", "pow")
    set_setting("pow_difficulty_bits", "12")
    st, r = call("GET", "/api/v1/auth/pow-challenge?scope=register", None)
    d = data_of(r)
    ch = str(d.get("pow_challenge", ""))
    bits = int(d.get("pow_difficulty", 0) or 0)
    check("注册 PoW 挑战（无需失败计数）",
          st == 200 and len(ch) == 64 and bits == 12, f"HTTP {st}")
    sol = solve_pow(ch, bits)
    clear_throttle()
    st, r = register(f"{MARK}pow{ts}", f"pow{ts}@example.com",
                     {"pow_token": d.get("pow_token", ""),
                      "pow_solution": sol})
    check("PoW 解得对即注册成功", st == 200, f"HTTP {st} {msg(r)}")
    clear_throttle()
    _, r2 = call("GET", "/api/v1/auth/pow-challenge?scope=register", None)
    d2 = data_of(r2)
    st, r = register(f"{MARK}powb{ts}", f"powb{ts}@example.com",
                     {"pow_token": d2.get("pow_token", ""),
                      "pow_solution": "0"})
    bad = st == 400 and "PoW" in msg(r)
    if not bad:
        # 运气好第一个解就是 "0"：改用缺失解再拒一次
        st, r = register(f"{MARK}powc{ts}", f"powc{ts}@example.com",
                         {"pow_token": d2.get("pow_token", "")})
        bad = st == 400
    check("PoW 伪解/缺失被拒", bad, f"HTTP {st} {msg(r)}")


def accuracy_checks(t_root: str, uid: int) -> None:
    """准确率现算自 cheat_events.false_positive：造一真一误两条假证据。"""
    anchor = uid or int(psql("SELECT id FROM users ORDER BY id LIMIT 1")
                        or 0)
    psql("INSERT INTO cheat_events (user_id, agent, reason, hits, "
         "resolved_at, false_positive) VALUES "
         f"({anchor}, 'm5fam:a', 'CI 冒烟证据', 1, now(), true), "
         f"({anchor}, 'm5fam:b', 'CI 冒烟证据', 1, now(), false)")
    st, r = call("GET", "/api/v1/admin/cheat-accuracy?days=7", t_root)
    d = data_of(r)
    fams = {f["family"]: f for f in (d.get("families") or [])}
    f = fams.get("m5fam")
    check("家族级误判率", st == 200 and f is not None
          and f["false_positive"] == 1 and f["accuracy"] == 0.5, f"{f}")
    check("处置结论分布返回", st == 200
          and isinstance(d.get("sanctions"), list),
          f"{d.get('sanctions')}")


def main() -> int:
    print("== 0. 备份设置 + 清理残留 ==")
    saved = {n: psql(f"SELECT value FROM site_settings WHERE name='{n}'")
             for n in SETTINGS}
    uid = staff_uid()
    t_root = jwt(uid) if uid else "jwt-缺失"
    check("解析 staff uid", uid > 0, f"uid={uid}")
    psql(f"DELETE FROM reg_quiz_questions WHERE category = '{CAT}'; "
         f"DELETE FROM cheat_events WHERE agent LIKE 'm5fam%'; "
         "DELETE FROM reg_quiz_attempts WHERE created_at > "
         "now() - interval '10 minutes'")
    tombstone("zzdel_" + MARK)
    set_setting("registration_mode", "open")
    set_setting("reg_ip_limit", "0")
    set_setting("reg_window", "")

    ts = nowmark()
    print("== 1. 题库 admin CRUD ==")
    st, r = call("GET", "/api/v1/admin/reg-quiz", t_root)
    d = data_of(r)
    check("题库列表 + 内置题", st == 200 and d.get("total", 0) >= 14,
          f"HTTP {st} total={d.get('total')}")
    st, r = call("POST", "/api/v1/admin/reg-quiz", t_root,
                 {"category": CAT, "question": Q_TEXT, "options": Q_OPTS,
                  "answer_index": Q_ANS, "explanation": "CI 冒烟用题"})
    qid = int(data_of(r).get("id") or 0)
    check("新增题目", st == 200 and qid > 0, f"HTTP {st} id={qid}")
    st, r = call("POST", "/api/v1/admin/reg-quiz", t_root,
                 {"category": CAT, "question": "坏题",
                  "options": ["只有一项"], "answer_index": 5})
    check("非法题目被拒", st == 400, f"HTTP {st} {msg(r)}")
    for on in (False, True):
        st, r = call("POST", "/api/v1/admin/reg-quiz/update", t_root,
                     {"id": qid, "enabled": on})
        check("停用/启用题目", st == 200, f"HTTP {st} enabled={on}")
    st, r = call("POST", "/api/v1/admin/reg-quiz/import", t_root,
                 {"questions": [
                     {"category": CAT, "question": Q_TEXT,
                      "options": Q_OPTS, "answer_index": Q_ANS},
                     {"category": CAT,
                      "question": "CI 冒烟第二题：H&R 会怎样？",
                      "options": ["无事", "被扣分甚至封禁"],
                      "answer_index": 1},
                     {"category": CAT, "question": "x",
                      "options": ["a", "b"], "answer_index": 0}]})
    d = data_of(r)
    check("导入：去重 + 非法点名",
          st == 200 and d.get("added") == 1 and d.get("invalid") == 1,
          f"added={d.get('added')} invalid={d.get('invalid')}")

    print("== 2. 答题注册：答对通过 ==")
    set_setting("reg_quiz_enabled", "yes")
    set_setting("reg_quiz_category", CAT)
    set_setting("reg_quiz_pass_required", "yes")
    st, r = call("GET", "/api/v1/auth/quiz-challenge", None)
    d = data_of(r)
    check("抽题返回选项但不含答案",
          st == 200 and len(d.get("options") or []) >= 2
          and "answer" not in json.dumps(d),
          f"HTTP {st} options={len(d.get('options') or [])}")
    clear_throttle()
    st, r = register(f"{MARK}ok{ts}", f"ok{ts}@example.com",
                     {"quiz_ticket": d.get("ticket", ""),
                      "quiz_answer": Q_ANS})
    check("答对即注册成功", st == 200, f"HTTP {st} {msg(r)}")

    print("== 3. 答错拦截 + 票据一次性 ==")
    clear_throttle()
    tick = quiz_ticket()
    st, r = register(f"{MARK}bad{ts}", f"bad{ts}@example.com",
                     {"quiz_ticket": tick, "quiz_answer": 0})
    check("答错被拒", st == 400 and "答错" in msg(r), f"HTTP {st} {msg(r)}")
    left = psql("SELECT count(*) FROM reg_quiz_attempts WHERE "
                "passed IS FALSE AND created_at > "
                "now() - interval '10 minutes'")
    check("答题留痕（失败题）", left != "0", f"{left} 条")
    clear_throttle()
    st, r = register(f"{MARK}re{ts}", f"re{ts}@example.com",
                     {"quiz_ticket": tick, "quiz_answer": Q_ANS})
    check("票据用后即焚（重放被拒）",
          st == 400 and "票据" in msg(r), f"HTTP {st} {msg(r)}")

    print("== 4. pass_required=no：只记录不拦截 ==")
    set_setting("reg_quiz_pass_required", "no")
    clear_throttle()
    tick3 = quiz_ticket()
    st, r = register(f"{MARK}soft{ts}", f"soft{ts}@example.com",
                     {"quiz_ticket": tick3, "quiz_answer": 0})
    check("答错仍可注册（软闸）", st == 200, f"HTTP {st} {msg(r)}")

    print("== 5. 未答题直接提交 ==")
    clear_throttle()
    st, r = register(f"{MARK}none{ts}", f"none{ts}@example.com")
    check("缺票据被拒", st == 400 and "答题" in msg(r),
          f"HTTP {st} {msg(r)}")

    print("== 6. PoW 注册验证码 ==")
    pow_checks(ts)
    print("== 7. 规则准确率统计 ==")
    accuracy_checks(t_root, uid)
    print("== 8. 还原设置 + 墓碑化 ==")
    restore_settings(saved)
    psql(f"DELETE FROM reg_quiz_questions WHERE category = '{CAT}'; "
         f"DELETE FROM cheat_events WHERE agent LIKE 'm5fam%'")
    tombstone(MARK)
    check("测试账号清理归零", leftover(MARK) == "0",
          f"残留 {leftover(MARK)}")
    check("测试题目清理归零",
          psql("SELECT count(*) FROM reg_quiz_questions WHERE "
               f"category = '{CAT}'") == "0")
    back = psql("SELECT value FROM site_settings WHERE name = "
                "'captcha_provider'")
    check("captcha_provider 已还原", back == saved["captcha_provider"],
          back)
    return report()


if __name__ == "__main__":
    sys.exit(main())
