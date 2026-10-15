# -*- coding: utf-8 -*-
"""CI 用：M4 注册治理四道闸变更型端到端（0370/0371）。

覆盖：一次性邮箱（含子域后缀匹配）→ 同 IP 限额（摘要留痕、无明文 IP）→
注册窗口（已结束/未开始/名额满）→ maxusers 上限（事务内 advisory 锁）→
域名库 admin API（导入去重/导出/删除）→ 普通用户 403 → reg_cleanup cron。

副作用：临时改 5 个 settings 键、注册 1~5 个测试账号，结束全部还原/墓碑化
（audit_log 0266 哈希链禁改删 + FK 无 ON DELETE，硬删必炸——见 ci_reg_common）。
退出码非 0 = 失败。前提：api + worker 已起（worker 供 reg_cleanup 触发）。
"""
import hashlib
import sys
import time

from ci_reg_common import (
    check, clear_throttle, call, data_of, jwt, leftover, msg, nowmark,
    psql, register, report, restore_settings, set_setting, tombstone,
)

MARK = "m4gate"
NEW_DOMAIN = "m4smoke-test.example"
# 与 api auth_http/register_gates.rs::ip_hash 同算法（sha3-256 hex）
H_LOCAL = hashlib.sha3_256(b"127.0.0.1").hexdigest()
SETTINGS = ("registration_mode", "reg_ip_limit", "reg_ip_window_hours",
            "reg_window", "maxusers")


def run_cron_once(tok: str) -> bool:
    """触发 reg_cleanup 并轮询 job_triggers 至 done（约 1 分钟上限）。"""
    for _ in range(2):
        st, r = call("POST", "/api/v1/admin/jobs/run", tok,
                     {"job": "reg_cleanup"})
        tid = data_of(r).get("id") if st == 200 else None
        if not tid:
            time.sleep(3)
            continue
        for _ in range(15):
            time.sleep(4)
            s = psql(f"SELECT status FROM job_triggers WHERE id = {tid}")
            if s == "done":
                return True
            if s == "failed":
                return False
    return False


def main() -> int:
    print("== 0. 备份设置 + 清理残留 ==")
    saved = {n: psql(f"SELECT value FROM site_settings WHERE name='{n}'")
             for n in SETTINGS}
    tombstone("zzdel_" + MARK)
    psql(f"DELETE FROM disposable_email_domains WHERE domain = "
         f"'{NEW_DOMAIN}' OR domain LIKE '%.{NEW_DOMAIN}'; "
         f"DELETE FROM reg_ip_signups WHERE ip_hash = '{H_LOCAL}'")
    set_setting("registration_mode", "open")
    set_setting("reg_window", "")
    uid = int(psql("SELECT id FROM users WHERE class_id >= 90 "
                   "ORDER BY id LIMIT 1") or 0)
    t_root = jwt(uid) if uid else "jwt-缺失"
    check("解析 staff uid", uid > 0, f"uid={uid}")

    ts = nowmark()
    print("== 1. 一次性邮箱闸（含子域后缀匹配）==")
    psql(f"INSERT INTO disposable_email_domains (domain, source) VALUES "
         f"('{NEW_DOMAIN}', 'import') ON CONFLICT DO NOTHING")
    clear_throttle()
    st, r = register(f"{MARK}disp{ts}", f"abc@{NEW_DOMAIN}")
    check("一次性邮箱被拒", st == 400 and "一次性" in msg(r),
          f"HTTP {st} {msg(r)}")
    clear_throttle()
    st, r = register(f"{MARK}disp2{ts}", f"abc@mail.{NEW_DOMAIN}")
    check("子域命中拒绝", st == 400 and "一次性" in msg(r),
          f"HTTP {st} {msg(r)}")

    print("== 2. 同 IP 限额闸 ==")
    set_setting("reg_ip_limit", "1")
    clear_throttle()
    st, r = register(f"{MARK}one{ts}", f"one{ts}@example.com")
    uid1 = data_of(r).get("user_id") if st == 200 else None
    check("首个注册成功（限额 1）", st == 200, f"HTTP {st} {msg(r)}")
    if uid1:
        row = psql("SELECT length(ip_hash) || '|' || "
                   "(ip_hash ~ '^[0-9a-f]{64}$')::text "
                   f"FROM reg_ip_signups WHERE user_id = {uid1}")
        check("摘要留痕（64 位 hex，无明文 IP）", row == "64|true", row)
    clear_throttle()
    st, r = register(f"{MARK}two{ts}", f"two{ts}@example.com")
    check("同 IP 第 2 个被拒", st == 400 and "网络环境" in msg(r),
          f"HTTP {st} {msg(r)}")

    print("== 3. 注册窗口闸 ==")
    set_setting("reg_ip_limit", "0")
    set_setting("reg_window", '{"open":"2020-01-01T00:00:00Z",'
                              '"close":"2020-01-02T00:00:00Z","slots":0,'
                              '"label":"ci 旧窗口"}')
    clear_throttle()
    st, r = register(f"{MARK}w1{ts}", f"w1{ts}@example.com")
    check("窗口已结束被拒", st == 400 and "结束" in msg(r),
          f"HTTP {st} {msg(r)}")
    set_setting("reg_window", '{"open":"2099-01-01T00:00:00Z",'
                              '"close":"2099-01-02T00:00:00Z","slots":0,'
                              '"label":"ci 未来窗口"}')
    clear_throttle()
    st, r = register(f"{MARK}w2{ts}", f"w2{ts}@example.com")
    check("窗口未开始被拒", st == 400 and "尚未开始" in msg(r),
          f"HTTP {st} {msg(r)}")
    set_setting("reg_window", '{"open":"2020-01-01T00:00:00Z",'
                              '"close":"2099-01-01T00:00:00Z","slots":1,'
                              '"label":"ci 名额窗口"}')
    clear_throttle()
    st, r = register(f"{MARK}w3{ts}", f"w3{ts}@example.com")
    check("名额已满被拒", st == 400 and "名额已满" in msg(r),
          f"HTTP {st} {msg(r)}")

    print("== 4. 用户总量闸（maxusers）==")
    set_setting("reg_window", "")
    used = psql("SELECT count(*) FROM users WHERE status < 2")
    set_setting("maxusers", used)
    clear_throttle()
    st, r = register(f"{MARK}cap{ts}", f"cap{ts}@example.com")
    check("达上限被拒", st == 400 and "上限" in msg(r),
          f"HTTP {st} {msg(r)}（现役 {used}）")

    print("== 5. 域名库 admin API ==")
    st, r = call("GET", "/api/v1/admin/email-domains?q=mailinator", t_root)
    items = data_of(r).get("items") or []
    check("列表可查（含内置库）", st == 200 and len(items) > 0,
          f"HTTP {st} n={len(items)}")
    st, r = call("POST", "/api/v1/admin/email-domains/import", t_root,
                 {"domains": [NEW_DOMAIN, "不是域名", "x"],
                  "text": f"y.{NEW_DOMAIN}"})
    d = data_of(r)
    check("导入：合法入库 / 非法点名",
          st == 200 and d.get("invalid", 0) >= 2,
          f"added={d.get('added')} invalid={d.get('invalid')}")
    st, r = call("GET", f"/api/v1/admin/email-domains?q={NEW_DOMAIN}",
                 t_root)
    check("导入后可查（含子域行）",
          len(data_of(r).get("items") or []) >= 2)
    st, r = call("GET", "/api/v1/admin/email-domains/export", t_root)
    doms = data_of(r).get("domains") or []
    check("导出含内置+导入",
          NEW_DOMAIN in doms and "mailinator.com" in doms,
          f"count={len(doms)}")
    st, r = call("POST", "/api/v1/admin/email-domains/delete", t_root,
                 {"domains": [NEW_DOMAIN, f"y.{NEW_DOMAIN}"]})
    check("删除 2 行", st == 200 and data_of(r).get("deleted") == 2,
          f"HTTP {st}")

    print("== 6. 无权限拒绝 ==")
    if uid1:
        st, r = call("GET", "/api/v1/admin/email-domains", jwt(uid1, 1))
        check("普通用户 403", st == 403, f"HTTP {st}")
    else:
        check("普通用户 403", False, "第 2 步未拿到 uid1")

    print("== 7. 注册摘要清理 cron ==")
    psql("INSERT INTO reg_ip_signups (ip_hash, user_id, created_at) "
         f"VALUES ('{'0' * 64}', NULL, now() - interval '40 days')")
    done = run_cron_once(t_root) if uid else False
    left = psql("SELECT count(*) FROM reg_ip_signups WHERE ip_hash = "
                f"'{'0' * 64}'")
    check("40 天前摘要被清理", done and left == "0",
          f"trigger={done} 残留={left}")

    print("== 8. 还原设置 + 墓碑化测试账号 ==")
    restore_settings(saved)
    tombstone(MARK)
    psql(f"DELETE FROM disposable_email_domains WHERE domain = "
         f"'{NEW_DOMAIN}' OR domain LIKE '%.{NEW_DOMAIN}'; "
         f"DELETE FROM reg_ip_signups WHERE ip_hash = '{H_LOCAL}'")
    check("测试账号清理归零", leftover(MARK) == "0",
          f"残留 {leftover(MARK)}")
    back = psql("SELECT value FROM site_settings WHERE name = "
                "'registration_mode'")
    check("registration_mode 已还原", back == saved["registration_mode"],
          back)
    return report()


if __name__ == "__main__":
    sys.exit(main())
